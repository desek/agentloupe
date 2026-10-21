//! @agents-index The login form, POST /private/login with the failed-login limit, POST /private/logout, and the Origin check.

use super::{PrivateState, cookie};
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::net::IpAddr;

/// The form with one password field. `failed` adds the neutral message, which never says how the
/// token was wrong.
pub fn form(status: StatusCode, failed: bool) -> Response {
    let msg = if failed {
        "<p class=\"login-msg\" role=\"alert\">The token is not correct.</p>"
    } else {
        ""
    };
    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Private page login</title>\n<link rel=\"stylesheet\" href=\"/assets/css/tokens.css\">\n<link rel=\"stylesheet\" href=\"/private/assets/css/private.css\">\n</head>\n<body>\n<main class=\"login\">\n<h1 class=\"title\">Private page</h1>\n<form method=\"post\" action=\"/private/login\">\n<label for=\"token\">Admin token</label>\n<input id=\"token\" name=\"token\" type=\"password\" autocomplete=\"current-password\" required>\n<button type=\"submit\" class=\"pill\">Log in</button>\n</form>\n{msg}\n</main>\n</body>\n</html>\n"
    );
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

/// A 303 to `to`, with an optional Set-Cookie.
pub fn see_other(to: &'static str, set_cookie: Option<String>) -> Response {
    let mut res = (StatusCode::SEE_OTHER, [(header::LOCATION, to)]).into_response();
    if let Some(c) = set_cookie.and_then(|c| c.parse().ok()) {
        res.headers_mut().insert(header::SET_COOKIE, c);
    }
    res
}

/// False when an `Origin` header is present and its host is not the `Host` of the request.
/// A missing Origin passes, because SameSite=Strict already keeps the cookie off cross-site posts.
pub fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let o = origin.to_str().ok().and_then(|o| o.split_once("://"));
    matches!((o, host), (Some((_, oh)), Some(h)) if oh.eq_ignore_ascii_case(h))
}

/// Decodes one form value: `+` is a space and `%XX` a byte.
pub fn decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(x) => {
                        out.push(x);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The `token` field of a form body.
fn token_field(body: &[u8]) -> String {
    String::from_utf8_lossy(body)
        .split('&')
        .find_map(|p| p.strip_prefix("token=").map(decode))
        .unwrap_or_default()
}

/// Handles POST /private/login. A blocked address gets 429 before the token is read; a wrong
/// token is logged with the remote address only.
pub async fn post(state: &PrivateState, req: Request<Body>, addr: IpAddr, https: bool) -> Response {
    let now = std::time::Instant::now();
    if state.limit.blocked(addr, now) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, "600")],
            "too many failed logins from this address; try again in 10 minutes\n",
        )
            .into_response();
    }
    let secure = cookie::secure(req.headers(), https);
    let body = axum::body::to_bytes(req.into_body(), 8192)
        .await
        .unwrap_or_default();
    if !super::auth::token_matches(state, &token_field(&body)) {
        state.limit.fail(addr, now);
        tracing::warn!(remote = %addr, "private login failed");
        return form(StatusCode::UNAUTHORIZED, true);
    }
    let v = cookie::value(&state.cookie_key, chrono::Utc::now().timestamp());
    see_other("/private/", Some(cookie::set(&v, cookie::MAX_AGE, secure)))
}

/// Handles POST /private/logout: clears the cookie.
pub fn logout(headers: &HeaderMap, https: bool) -> Response {
    let secure = cookie::secure(headers, https);
    see_other("/private/login", Some(cookie::set("", 0, secure)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_form_value() {
        assert_eq!(token_field(b"a=1&token=x%2By+z"), "x+y z");
        assert_eq!(token_field(b"token=%"), "%");
    }
}
