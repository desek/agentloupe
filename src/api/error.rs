//! @agents-index The error body of the public API: code, message, and a fix that names a valid example query; no-store.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

/// A valid example query for the fix text.
pub const EXAMPLE: &str = "GET /v1/days?from=2026-10-01&to=2026-10-31";

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub fix: String,
}

impl ApiError {
    pub fn bad(code: &'static str, message: String) -> ApiError {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            code,
            message,
            fix: format!("send a query such as {EXAMPLE}, with from and to as YYYY-MM-DD, from not after to, and at most 400 days"),
        }
    }

    pub fn not_found(day: &str) -> ApiError {
        ApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("no published record for the day {day}"),
            fix: format!("list the published days with {EXAMPLE}"),
        }
    }

    pub fn internal() -> ApiError {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "the days table could not be read".into(),
            fix: "try again later".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({"error": {"code": self.code, "message": self.message, "fix": self.fix}});
        (
            self.status,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            body.to_string(),
        )
            .into_response()
    }
}
