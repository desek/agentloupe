//! @agents-index Parses the query of the private API: from, to, group, tag_key, token, cursor; the error body of the private API.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;

/// The group of the report rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Group {
    Token,
    /// The tag key that makes the groups.
    Tag(String),
}

/// The checked query of a private API route.
#[derive(Debug)]
pub struct Params {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub group: Option<Group>,
    pub token: Option<String>,
    pub cursor: Option<i64>,
}

/// The error body of the private API, in the shape of the public API error, with no-store.
pub fn error(status: StatusCode, code: &'static str, message: String, fix: &str) -> Response {
    let body = serde_json::json!({"error": {"code": code, "message": message, "fix": fix}});
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.to_string(),
    )
        .into_response()
}

const FIX: &str = "send a query such as GET /private/api/v1/report?group=token&from=2026-07-01, or group=tag&tag_key=owner; for sessions token=<name>; from and to as YYYY-MM-DD, at most 400 days";

fn bad(code: &'static str, message: String) -> Box<Response> {
    Box::new(error(StatusCode::BAD_REQUEST, code, message, FIX))
}

/// Parses `raw`; `to` defaults to `today` and `from` to 365 days before `to`, as for /v1/days.
pub fn parse(raw: Option<&str>, today: NaiveDate) -> Result<Params, Box<Response>> {
    use crate::api::query::{DEFAULT_BACK, MAX_DAYS, date};
    let (mut from, mut to, mut group, mut tag_key, mut token, mut cursor) =
        (None, None, None, None, None, None);
    for pair in raw.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = super::login::decode(v);
        let d = |name: &str| {
            date(&v).ok_or_else(|| {
                bad(
                    "invalid_date",
                    format!("{name}={v} is not a valid YYYY-MM-DD date"),
                )
            })
        };
        match k {
            "from" => from = Some(d("from")?),
            "to" => to = Some(d("to")?),
            "group" => group = Some(v),
            "tag_key" => tag_key = Some(v),
            "token" => token = Some(v),
            "cursor" => {
                cursor = Some(v.parse::<i64>().map_err(|_| {
                    bad(
                        "invalid_cursor",
                        format!("cursor={v} is not a cursor of this route"),
                    )
                })?)
            }
            other => {
                return Err(bad(
                    "unknown_parameter",
                    format!("the query parameter {other} is not known"),
                ));
            }
        }
    }
    let to = to.unwrap_or(today);
    let from = from.unwrap_or(to - chrono::Duration::days(DEFAULT_BACK));
    if from > to {
        return Err(bad(
            "inverted_range",
            format!("from {from} is after to {to}"),
        ));
    }
    if (to - from).num_days() + 1 > MAX_DAYS {
        return Err(bad(
            "range_too_long",
            format!("the range {from} to {to} has more than {MAX_DAYS} days"),
        ));
    }
    let group = match (group.as_deref(), tag_key) {
        (None, _) => None,
        (Some("token"), _) => Some(Group::Token),
        (Some("tag"), Some(k)) if !k.is_empty() => Some(Group::Tag(k)),
        (Some("tag"), _) => return Err(bad("missing_tag_key", "group=tag needs tag_key".into())),
        (Some(g), _) => {
            return Err(bad(
                "invalid_group",
                format!("group={g} is not token or tag"),
            ));
        }
    };
    Ok(Params {
        from,
        to,
        group,
        token,
        cursor,
    })
}
