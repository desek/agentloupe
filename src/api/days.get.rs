//! @agents-index GET /v1/days/{day}: the stored record of one published day, 404 not_found, or 400 invalid_date.

use super::error::ApiError;
use super::{ApiState, cors, query};
use axum::extract::{Path, State};
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use rusqlite::OptionalExtension;
use std::sync::Arc;

fn read(s: &ApiState, day: &str) -> rusqlite::Result<Option<String>> {
    let conn = s.conn.lock().unwrap_or_else(|p| p.into_inner());
    conn.query_row(
        "SELECT record FROM days WHERE day = ?1 AND state = 'published'",
        [day],
        |r| r.get(0),
    )
    .optional()
}

pub async fn handle(
    State(s): State<Arc<ApiState>>,
    method: Method,
    Path(day): Path<String>,
) -> Response {
    if let Some(r) = cors::method(&method) {
        return r;
    }
    if query::date(&day).is_none() {
        return ApiError::bad(
            "invalid_date",
            format!("{day} is not a valid YYYY-MM-DD date"),
        )
        .into_response();
    }
    match read(&s, &day) {
        Ok(Some(record)) => cors::ok(record),
        Ok(None) => ApiError::not_found(&day).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "the day route could not read the days table. Fix: check the database file. Verify: curl /v1/days returns 200");
            ApiError::internal().into_response()
        }
    }
}
