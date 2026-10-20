//! @agents-index Builds the JSON error responses of the proxy: unknown upstream, bad route, unreachable upstream, token refusals.

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

/// The 401 of the token gate; the body never echoes the presented token.
pub fn token(code: &str) -> Response {
    let fix = "put a valid active token first in the base URL, as /<token>/<agent>/<upstream>; make one with loupe token create --name <name>";
    let (status, detail, fix) = match code {
        "token_missing" => (
            StatusCode::UNAUTHORIZED,
            "proxy tokens exist, and the request has no token segment",
            fix,
        ),
        "token_unknown" => (
            StatusCode::UNAUTHORIZED,
            "the presented token matches no proxy token",
            fix,
        ),
        "token_revoked" => (
            StatusCode::UNAUTHORIZED,
            "the presented token is revoked",
            fix,
        ),
        "token_mode_off" => (
            StatusCode::UNAUTHORIZED,
            "no proxy token has ever existed, so the server does no token check, and the request has a token segment",
            "remove the token segment from the base URL, or make a token with loupe token create --name <name> so the server checks it",
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            "the token state cannot be read from the database",
            "check that <LOUPE_DATA_DIR>/loupe.db is readable, then retry",
        ),
    };
    json(
        status,
        serde_json::json!({
            "error": code,
            "detail": detail,
            "fix": fix,
            "check": "loupe token list",
            "reference": "docs/specification/proxy.tokens.md",
        }),
    )
}
