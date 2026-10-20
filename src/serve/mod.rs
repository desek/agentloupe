//! @agents-index Runs the HTTP server: opens and migrates the store, binds LOUPE_LISTEN, and serves until a shutdown signal.

mod health;
mod shutdown;

use crate::config::Config;
use std::error::Error;

/// Blocks until a shutdown signal; the database is open and migrated before the port opens.
pub fn run(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let mut conn = crate::store::open::open(&cfg.data_dir)?;
    let applied = crate::store::migrate::migrate(&mut conn)?;
    tracing::info!(applied, db = %cfg.data_dir.join("loupe.db").display(), "database open and migrated");
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
        tracing::info!(listen = %cfg.listen, "listening");
        shutdown::serve_until_signal(listener, health::router()).await
    })?;
    conn.close().map_err(|(_, e)| e)?;
    tracing::info!("database closed, exit");
    Ok(())
}
