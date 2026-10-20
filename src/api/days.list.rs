//! @agents-index GET /v1/days: the envelope with from, to, updated_at, the subscription cost sum, and the stored published records.

use super::error::ApiError;
use super::{ApiState, cors, query};
use axum::extract::{RawQuery, State};
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;

/// The stored record texts of the published days in [from, to], and the latest build time of
/// all published days.
fn read(s: &ApiState, from: &str, to: &str) -> rusqlite::Result<(Vec<String>, Option<String>)> {
    let conn = s.conn.lock().unwrap_or_else(|p| p.into_inner());
    let records = conn
        .prepare_cached(
            "SELECT record FROM days WHERE state = 'published' AND day >= ?1 AND day <= ?2 ORDER BY day",
        )?
        .query_map([from, to], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    let updated: Option<String> = conn.query_row(
        "SELECT max(built_at) FROM days WHERE state = 'published'",
        [],
        |r| r.get(0),
    )?;
    Ok((records, updated))
}

/// The sum of the subscription cost of each day of the range, with the current configuration.
fn subscription_sum(s: &ApiState, from: chrono::NaiveDate, to: chrono::NaiveDate) -> String {
    let mut sum = Decimal::ZERO;
    let mut d = from;
    while d <= to {
        let text = crate::subscription::prorate::subscription_cost_for_day(d, &s.subscriptions);
        sum += Decimal::from_str(&text).unwrap_or_default();
        d = d.succ_opt().unwrap_or(to + chrono::Duration::days(1));
    }
    let v = serde_json::to_value(Wrap(crate::day::sum::round4(sum))).unwrap_or_default();
    v.to_string()
}

#[derive(serde::Serialize)]
struct Wrap(#[serde(serialize_with = "crate::day::record::number")] Decimal);

pub async fn handle(
    State(s): State<Arc<ApiState>>,
    method: Method,
    RawQuery(raw): RawQuery,
) -> Response {
    if let Some(r) = cors::method(&method) {
        return r;
    }
    let (from, to) = match query::range(raw.as_deref(), super::today(s.tz)) {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let (records, updated) = match read(&s, &from.to_string(), &to.to_string()) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "the days list could not read the days table. Fix: check the database file. Verify: curl /v1/days returns 200");
            return ApiError::internal().into_response();
        }
    };
    let updated = updated.map_or("null".to_string(), |u| serde_json::json!(u).to_string());
    // The records are the stored text; the API never builds a record.
    let body = format!(
        r#"{{"from":"{from}","to":"{to}","updated_at":{updated},"subscription_cost_usd":{},"days":[{}]}}"#,
        subscription_sum(&s, from, to),
        records.join(",")
    );
    cors::ok(body)
}
