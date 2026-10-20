//! @agents-index The HTTP router: reserved routes take precedence, every other path falls back to the proxy.

use crate::proxy::ProxyState;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use std::sync::Arc;

/// The reserved first path segments; the only list, so the route parser rejects these names too.
pub const RESERVED: &[&str] = &["healthz", "v1", "assets", "favicon.ico"];

/// Builds the app: `/healthz`, then a fallback that answers reserved paths with 404 and proxies the rest.
pub fn router(state: Arc<ProxyState>) -> Router {
    super::health::router().fallback(move |req: Request<Body>| {
        let state = state.clone();
        async move { dispatch(state, req).await }
    })
}

async fn dispatch(state: Arc<ProxyState>, req: Request<Body>) -> Response {
    let first = req
        .uri()
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    if first.is_empty() || RESERVED.contains(&first) {
        // The dashboard and the API own these paths; until they exist, they are not found.
        return crate::proxy::error::json(
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "not_found", "path": req.uri().path()}),
        );
    }
    crate::proxy::handle(state, req).await
}
