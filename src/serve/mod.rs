//! @agents-index Runs the HTTP server: opens and migrates the store, starts the capture writer, binds LOUPE_LISTEN, serves until a signal.

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
    let spill_dir = cfg.data_dir.join("spill");
    let (capture, rx) = Capture::new(cfg.capture_queue, spill_dir.clone(), cfg.data_key);
    let writer = writer::start(
        crate::store::open::open(&cfg.data_dir)?,
        rx,
        spill_dir,
        cfg.data_key,
    );
    let state = Arc::new(ProxyState {
        upstreams,
        upstreams_file: cfg.upstreams_file.display().to_string(),
        client: proxy::client(cfg.connect_timeout)?,
        capture,
        account_key: cfg.account_key,
        capture_max_bytes: cfg.capture_max_bytes,
        headers_timeout: cfg.headers_timeout,
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
