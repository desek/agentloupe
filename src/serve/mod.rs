//! @agents-index Runs the HTTP server: migrates the store, bootstrap token, starts capture, parser, summary, ping, publisher, retention workers, binds, serves.

mod health;
pub mod router;
mod shutdown;

use crate::capture::writer::{self, Capture};
use crate::config::Config;
use crate::proxy::{self, ProxyState, upstreams::Upstreams};
use std::error::Error;
use std::sync::Arc;

/// Blocks until a shutdown signal; the database is open and migrated, and the spill files are
/// imported, before the port opens.
pub fn run(cfg: &Config, upstreams: Upstreams) -> Result<(), Box<dyn Error>> {
    let mut conn = crate::store::open::open(&cfg.data_dir)?;
    let applied = crate::store::migrate::migrate(&mut conn)?;
    tracing::info!(applied, db = %cfg.data_dir.join("loupe.db").display(), "database open and migrated");
    let raw = cfg.bootstrap_token.as_ref().map(|s| s.expose());
    let outcome = crate::token::bootstrap::run(&conn, raw).map_err(|e| {
        format!("the bootstrap token step failed: {e}. Fix 1: set LOUPE_BOOTSTRAP_TOKEN to a value from loupe token generate (see .env.example). Fix 2: unset LOUPE_BOOTSTRAP_TOKEN. Verify: loupe config show, then loupe token list")
    })?;
    tracing::info!(outcome = ?outcome, "bootstrap token step done");
    let gate = crate::token::gate::Gate::new(
        crate::store::open::open(&cfg.data_dir)?,
        crate::store::open::open(&cfg.data_dir)?,
    );
    if let Some(w) = gate.state()?.warning() {
        tracing::warn!("{w}");
    }
    let t0 = std::time::Instant::now();
    let index = crate::price::index::Index::embedded();
    tracing::info!(
        entries = index.len(),
        price_commit = crate::price::embed::PRICE_COMMIT,
        load_ms = t0.elapsed().as_millis() as u64,
        "price index loaded"
    );
    // The hub starts only when the live feed is enabled; an off hub drops every event.
    let hub = if cfg.live.enabled {
        crate::live::hub::Hub::new(cfg.live.buffer)
    } else {
        crate::live::hub::Hub::off()
    };
    let spill_dir = cfg.data_dir.join("spill");
    let (capture, rx) = Capture::new(cfg.capture_queue, spill_dir.clone(), cfg.data_key);
    let writer = writer::start(
        crate::store::open::open(&cfg.data_dir)?,
        rx,
        spill_dir,
        cfg.data_key,
    );
    // The worker thread ends with the process; each exchange is one transaction, so a stop loses nothing.
    let _parser = crate::parse::worker::start(
        crate::store::open::open(&cfg.data_dir)?,
        crate::parse::worker::Settings {
            data_key: cfg.data_key,
            account_key: cfg.account_key,
            idle_secs: cfg.idle_secs,
            providers: proxy::upstreams::providers(&upstreams),
            tz: cfg.summary.tz,
            hub: hub.clone(),
        },
        cfg.parse_interval,
    );
    // One line with the detected providers and the summary provider; it holds no credential.
    let detected: Vec<&str> = crate::provider::detect::names(&cfg.providers)
        .iter()
        .map(|p| p.name())
        .collect();
    tracing::info!(
        detected = %detected.join(","),
        summary_provider = cfg.summary.provider.map_or("none", |p| p.name()),
        "providers detected"
    );
    let provider_status = crate::provider::status::Cache::new(&cfg.providers);
    // The summary client sends to the base URL of the summary provider, never through the proxy routes.
    let _summary = crate::summary::worker::start(
        crate::store::open::open(&cfg.data_dir)?,
        crate::summary::settings(cfg),
        cfg.summary.interval,
    );
    let github = crate::github::Settings::from_config(cfg);
    match &github {
        None => tracing::info!("github: disabled, no token"),
        Some(g) => tracing::info!(labels = g.tokens.len(), "github: enabled"),
    }
    // The publisher builds each due day and each queued day; it reads time only through its clock.
    // The GitHub fetches run in its thread, so the proxy latency does not change.
    let _publisher = crate::publish::worker::start(
        crate::store::open::open(&cfg.data_dir)?,
        crate::publish::worker::Settings {
            ctx: crate::day::rebuild::Ctx::from_config(cfg),
            publish_at: cfg.days.publish_at,
            sources: crate::day::activity::registered(),
            github,
        },
        Box::new(crate::publish::clock::System),
        cfg.days.publish_interval,
    );
    let _retention = crate::retention::worker::start(
        crate::store::open::open(&cfg.data_dir)?,
        cfg.summary.raw_retention_hours,
    );
    let state = Arc::new(ProxyState {
        upstreams,
        upstreams_file: cfg.upstreams_file.display().to_string(),
        client: proxy::client(cfg.connect_timeout)?,
        capture,
        account_key: cfg.account_key,
        capture_max_bytes: cfg.capture_max_bytes,
        headers_timeout: cfg.headers_timeout,
        gate,
        hub: hub.clone(),
    });
    let live = match cfg.live.enabled {
        true => Some(crate::live::route::LiveState::new(
            hub,
            cfg.live.clone(),
            cfg.summary.tz,
            crate::store::open::open(&cfg.data_dir)?,
        )),
        false => None,
    };
    tracing::info!(live_enabled = live.is_some(), "live route state");
    // The API connection reads only the days table.
    let api = Arc::new(crate::api::ApiState {
        conn: std::sync::Mutex::new(crate::store::open::open(&cfg.data_dir)?),
        tz: cfg.summary.tz,
        subscriptions: cfg.days.subscriptions.clone(),
    });
    // The title goes into index.html once here, so each request serves the same bytes and ETag.
    let site = Arc::new(crate::web::Site::new(&cfg.site_title));
    // The private state exists only with an admin token; it keeps a digest and a key, never the token.
    let private = match &cfg.admin_token {
        Some(t) => Some(Arc::new(crate::private::PrivateState::new(
            crate::store::open::open(&cfg.data_dir)?,
            cfg.summary.tz,
            t.expose(),
            provider_status.clone(),
            cfg.shown.clone(),
        ))),
        None => None,
    };
    tracing::info!(private_page = private.is_some(), "private page state");
    let rt = tokio::runtime::Runtime::new()?;
    // The ping runs in its own task, so a provider that hangs never delays the listener.
    let _ping = crate::provider::worker::spawn(
        &rt,
        cfg.providers.clone(),
        provider_status,
        cfg.provider_ping,
    );
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
        // The bound address, not the configured one: a configured port 0 gets a free port from the kernel.
        tracing::info!(listen = %listener.local_addr()?, "listening");
        shutdown::serve_until_signal(listener, router::router(state, api, site, private, live))
            .await
    })?;
    // The runtime drop ends every open stream, which drops the last capture handle; the writer
    // then drains the queue and stops.
    drop(rt);
    let _ = writer.join();
    conn.close().map_err(|(_, e)| e)?;
    tracing::info!("database closed, exit");
    Ok(())
}
