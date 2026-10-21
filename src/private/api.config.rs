//! @agents-index The GET /private/api/config handler: the masked entries of loupe config show as the process loaded them at start.

use super::PrivateState;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

/// One object per entry (name, value, source); the values come masked from the shared render.
pub fn get(state: &PrivateState) -> Response {
    let entries: Vec<_> = state
        .config
        .iter()
        .map(|l| serde_json::json!({"name": l.name, "value": l.value, "source": l.source}))
        .collect();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::json!({ "entries": entries }).to_string(),
    )
        .into_response()
}
