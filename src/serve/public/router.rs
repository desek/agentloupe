//! @agents-index The router of the role public: health, the public API, the dashboard files, and /live; every other path and method answers 404.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use std::sync::Arc;

/// Builds the app of the role public. The fallback never forwards: it has no proxy, no token
/// gate, no capture, and no private routes to call.
pub fn router(
    api: Arc<crate::api::ApiState>,
    site: Arc<crate::web::Site>,
    live: Option<Arc<crate::live::route::LiveState>>,
) -> Router {
    super::super::health::router()
        .merge(crate::api::router(api))
        .fallback(move |req: Request<Body>| {
            let site = site.clone();
            let live = live.clone();
            async move { dispatch(site, live, req).await }
        })
        .layer(axum::middleware::from_fn(crate::api::cors::layer))
}

async fn dispatch(
    site: Arc<crate::web::Site>,
    live: Option<Arc<crate::live::route::LiveState>>,
    req: Request<Body>,
) -> Response {
    if crate::web::path::is_static(req.uri().path()) {
        return crate::web::serve::handle(&site, &req);
    }
    if let (Some(l), "/live") = (&live, req.uri().path()) {
        return crate::live::route::handle(l.clone(), req).await;
    }
    super::super::not_found::response(req.uri().path())
}
