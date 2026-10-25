//! @agents-index The start sequence of the role public: open the mirror read-only, build the public router, listen, then the start telemetry.
//!
//! This module and its children never import the capture, proxy, token, private, account,
//! session, parse, or store modules; a test reads their source to keep it so.

pub mod guard;
pub mod router;

use crate::config::Config;
use std::error::Error;
use std::sync::Arc;

/// Blocks until a shutdown signal. The guards ran before the log opened; this opens only the
/// mirror, read-only, and starts no worker.
pub fn run(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let mirror = || crate::mirror::open::open(&cfg.data_dir);
    let api_conn = mirror()?;
    tracing::info!(path = %crate::mirror::path(&cfg.data_dir).display(), "public mirror open read-only");
    let api = Arc::new(crate::api::ApiState {
        conn: std::sync::Mutex::new(api_conn),
        tz: cfg.summary.tz,
        subscriptions: cfg.days.subscriptions.clone(),
    });
    // The live poller is the only sender of the local hub; with the live feed off, nothing starts.
    let hub = match cfg.live.enabled {
        true => crate::live::hub::Hub::new(cfg.live.buffer),
        false => crate::live::hub::Hub::off(),
    };
    let poller = match cfg.live.enabled {
        true => Some(crate::live::poll::Poller::new(
            mirror()?,
            hub.clone(),
            Box::new(check_or_stop),
        )),
        false => None,
    };
    let live = match cfg.live.enabled {
        true => Some(crate::live::route::LiveState::with_source(
            hub,
            cfg.live.clone(),
            cfg.summary.tz,
            mirror()?,
            crate::clock::system(),
            crate::live::route::Source::Mirror,
        )),
        false => None,
    };
    tracing::info!(live_enabled = live.is_some(), "live route state");
    let site = Arc::new(crate::web::Site::new(&cfg.site_title));
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
        tracing::info!(listen = %listener.local_addr()?, "listening");
        crate::telemetry::start();
        if let Some(p) = poller {
            tokio::spawn(crate::live::poll::run(
                p,
                crate::clock::system(),
                Arc::new(crate::publish::clock::System),
            ));
        }
        super::shutdown::serve_until_signal(listener, router::router(api, site, live)).await
    })?;
    tracing::info!("exit");
    Ok(())
}

/// The check of each poll pass: on another table set or another version, it logs the error of
/// the start check and ends the process, so the restart policy starts it again.
pub fn check_or_stop(conn: &rusqlite::Connection) {
    if let Err(e) = crate::mirror::open::check(conn) {
        tracing::error!(error = %e, "the public mirror changed under the role public; stopping");
        std::process::exit(1);
    }
}
