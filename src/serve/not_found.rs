//! @agents-index The 404 answer for an unknown path, shared by both roles; the echoed path never holds a leading lpt_ token segment.

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::Response;

/// The path without a leading token segment, so no body or log echoes a raw token.
pub fn strip_token(path: &str) -> &str {
    let rest = path.trim_start_matches('/');
    match rest.split_once('/') {
        Some((first, tail)) if first.starts_with("lpt_") => &path[path.len() - tail.len() - 1..],
        None if rest.starts_with("lpt_") => "/",
        _ => path,
    }
}

/// `404 {"error":"not_found","path":...}` with the token segment removed from the path.
pub fn response(path: &str) -> Response {
    let body = serde_json::json!({"error": "not_found", "path": strip_token(path)});
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("static parts are valid")
}

#[cfg(test)]
mod tests {
    use super::strip_token;

    #[test]
    fn removes_only_a_leading_token_segment() {
        assert_eq!(strip_token("/lpt_abc/v1/days"), "/v1/days");
        assert_eq!(strip_token("/lpt_abc"), "/");
        assert_eq!(strip_token("/v1/lpt_abc"), "/v1/lpt_abc");
        assert_eq!(strip_token("/unknown"), "/unknown");
    }
}
