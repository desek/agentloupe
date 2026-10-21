//! @agents-index The GET /private/api/providers handler: the cached provider status, with no credential and no base URL.

use super::PrivateState;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Reads the cache only, so the answer never waits for a ping.
pub fn get(state: &PrivateState) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        state.providers.json().to_string(),
    )
        .into_response()
}
