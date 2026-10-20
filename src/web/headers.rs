//! @agents-index The headers of the static responses: ETag and no-cache on each, and the security headers of the page.

use axum::http::{HeaderMap, HeaderValue, header};

/// The policy of the page: only its own origin, no inline script, no frames.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// The headers of every static response; `page` adds the security headers of `GET /`.
pub fn of(etag: &str, content_type: &'static str, page: bool) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if let Ok(v) = HeaderValue::from_str(etag) {
        h.insert(header::ETAG, v);
    }
    if page {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        );
        h.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        h.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
    }
    h
}
