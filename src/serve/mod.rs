//! @agents-index Runs the HTTP server: migrates the store, bootstrap token step, starts capture and the parser worker, binds, serves.

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
        },
        cfg.parse_interval,
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
    });
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
        tracing::info!(listen = %cfg.listen, "listening");
        shutdown::serve_until_signal(listener, router::router(state)).await
    })?;
    // The runtime drop ends every open stream, which drops the last capture handle; the writer
    // then drains the queue and stops.
    drop(rt);
    let _ = writer.join();
    conn.close().map_err(|(_, e)| e)?;
    tracing::info!("database closed, exit");
    Ok(())
}
