//! @agents-index The headers of every private response: no-store, X-Frame-Options DENY, a strict CSP, and no CORS header.

use axum::http::{HeaderValue, header};
use axum::response::Response;

/// Only the own origin; the login form posts to the own origin only.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Adds the private headers and removes any CORS header.
pub fn apply(mut res: Response) -> Response {
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // same-origin, not no-referrer: under no-referrer a browser sends `Origin: null` on the form
    // posts, and the Origin check then refuses the own login form.
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    h.remove(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    res
}
