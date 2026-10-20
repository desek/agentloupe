//! @agents-index The open CORS and method rules: allow-origin * on /v1/ and /healthz, the OPTIONS answer, and 405 for other methods.

use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// True for a path of the public API or of the health route.
fn public(path: &str) -> bool {
    path == "/healthz" || path == "/v1" || path.starts_with("/v1/")
}

/// Middleware: adds `access-control-allow-origin: *` to each public response.
pub async fn layer(req: Request<Body>, next: Next) -> Response {
    let open = public(req.uri().path());
    let mut res = next.run(req).await;
    if open {
        res.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    res
}

/// The answer to a method other than GET: 204 for OPTIONS, else 405. None for GET.
pub fn method(m: &Method) -> Option<Response> {
    if m == Method::GET {
        return None;
    }
    if m == Method::OPTIONS {
        return Some(
            (
                StatusCode::NO_CONTENT,
                [
                    (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS"),
                    (header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type"),
                ],
            )
                .into_response(),
        );
    }
    Some(
        (
            StatusCode::METHOD_NOT_ALLOWED,
            [
                (header::ALLOW, "GET, OPTIONS"),
                (header::CACHE_CONTROL, "no-store"),
            ],
        )
            .into_response(),
    )
}

/// A 200 JSON response with the public cache header.
pub fn ok(body: String) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=60"),
        ],
        body,
    )
        .into_response()
}
