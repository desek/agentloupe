//! @agents-index The GET /healthz route; it exists only after the store is ready, so a 200 means ready.

use axum::Router;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;

async fn healthz() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"status":"ok"}"#,
    )
}

pub fn router() -> Router {
    Router::new().route("/healthz", get(healthz))
}
