//! @agents-index The static handler: method rule, path check, lookup in the built-in files, 304 on a matching ETag.

use super::Site;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Answers a static path. It reads no cookie, no authorization header, and no database table,
/// and it sets no cookie.
pub fn handle(site: &Site, req: &Request<Body>) -> Response {
    let head = req.method() == Method::HEAD;
    if req.method() != Method::GET && !head {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD")],
        )
            .into_response();
    }
    let Some(file) = super::path::resolve(req.uri().path()).and_then(|p| site.get(p)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let page = file.content_type.starts_with("text/html");
    let headers = super::headers::of(&file.etag, file.content_type, page);
    let matched = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|t| t.trim() == file.etag || t.trim() == "*")
        });
    if matched {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }
    let body = if head {
        Body::empty()
    } else {
        Body::from(file.body.clone().into_owned())
    };
    (StatusCode::OK, headers, body).into_response()
}
