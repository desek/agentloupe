//! @agents-index The HTTP router: health and the public API first, reserved paths 404, the rest falls back to the proxy; CORS on public paths.

use crate::proxy::ProxyState;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use std::sync::Arc;

/// The reserved first path segments; the only list, so the route parser rejects these names too.
pub const RESERVED: &[&str] = &["healthz", "v1", "assets", "favicon.ico"];

/// Builds the app: `/healthz` and the public API, then a fallback that answers reserved paths with
/// 404 and proxies the rest. The CORS layer covers every route, and adds its header only to
/// `/healthz` and the paths under `/v1/`.
pub fn router(state: Arc<ProxyState>, api: Arc<crate::api::ApiState>) -> Router {
    super::health::router()
        .merge(crate::api::router(api))
        .fallback(move |req: Request<Body>| {
            let state = state.clone();
            async move { dispatch(state, req).await }
        })
        .layer(axum::middleware::from_fn(crate::api::cors::layer))
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
