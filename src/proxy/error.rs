//! @agents-index Builds the JSON error responses of the proxy: unknown upstream, bad route, upstream unreachable.

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::Response;

/// One JSON error response; the body never holds a header value or a request body.
pub fn json(status: StatusCode, body: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("static parts are valid")
}
