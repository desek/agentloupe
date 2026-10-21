//! @agents-index Graceful shutdown: on SIGTERM or SIGINT stop accepting, give open requests 10 seconds, then return.

use axum::Router;
use std::time::Duration;
use tokio::net::TcpListener;

/// How long open requests may run after the signal.
const DRAIN: Duration = Duration::from_secs(10);

async fn signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutdown signal received, draining open requests");
}

/// Serves until a signal, then waits at most DRAIN for open requests. Each request carries its
/// remote address, which the failed-login limit of the private page reads.
pub async fn serve_until_signal(listener: TcpListener, app: Router) -> std::io::Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        signal().await;
        let _ = tx.send(());
    });
    let mut server = std::pin::pin!(server.into_future());
    tokio::select! {
        r = &mut server => return r,
        _ = rx => {}
    }
    if tokio::time::timeout(DRAIN, server).await.is_err() {
        tracing::info!("open requests did not finish within the drain time; closing them");
    }
    Ok(())
}
