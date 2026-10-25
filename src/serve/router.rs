//! @agents-index The HTTP router: health, the public API, the dashboard files, the private page, and the live route first; other reserved paths 404; the rest goes to the proxy.

use crate::proxy::ProxyState;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use std::sync::Arc;

/// The reserved first path segments: the names that the public API, the static handler, and the live route serve.
/// The only list, so the route parser rejects these names too.
pub const RESERVED: &[&str] = &["healthz", "v1", "assets", "favicon.ico", "private", "live"];

/// Builds the app: `/healthz` and the public API, then a fallback that serves the dashboard files,
/// sends `/private/...` to the private page when `private` is set, answers other reserved paths
/// with 404, and proxies the rest. The CORS layer covers every route, and adds its header only to
/// `/healthz` and the paths under `/v1/`.
pub fn router(
    state: Arc<ProxyState>,
    api: Arc<crate::api::ApiState>,
    site: Arc<crate::web::Site>,
    private: Option<Arc<crate::private::PrivateState>>,
    live: Option<Arc<crate::live::route::LiveState>>,
) -> Router {
    super::health::router()
        .merge(crate::api::router(api))
        .fallback(move |req: Request<Body>| {
            let state = state.clone();
            let site = site.clone();
            let private = private.clone();
            let live = live.clone();
            async move { dispatch(state, site, private, live, req).await }
        })
        .layer(axum::middleware::from_fn(crate::api::cors::layer))
}

async fn dispatch(
    state: Arc<ProxyState>,
    site: Arc<crate::web::Site>,
    private: Option<Arc<crate::private::PrivateState>>,
    live: Option<Arc<crate::live::route::LiveState>>,
    req: Request<Body>,
) -> Response {
    if crate::web::path::is_static(req.uri().path()) {
        return crate::web::serve::handle(&site, &req);
    }
    let first = req
        .uri()
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    // Without the admin token the private page does not exist: its paths fall to the 404 below.
    if let (Some(p), "private") = (&private, first) {
        return crate::private::routes::handle(p, &site, req).await;
    }
    // Only the exact path /live upgrades; with the live feed disabled it falls to the 404 below.
    if let (Some(l), "/live") = (&live, req.uri().path()) {
        return crate::live::route::handle(l.clone(), req).await;
    }
    if first.is_empty() || RESERVED.contains(&first) {
        // A reserved name that no route serves, for example /v1/other, is not found.
        return super::not_found::response(req.uri().path());
    }
    crate::proxy::handle(state, req).await
}
