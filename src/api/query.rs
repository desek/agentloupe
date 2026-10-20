//! @agents-index Parses and checks the query of GET /v1/days: from and to as YYYY-MM-DD, the defaults, and the range limits.

use super::error::ApiError;
use chrono::NaiveDate;

/// The most days in one range, both ends included.
pub const MAX_DAYS: i64 = 400;
/// The default length back from `to` when `from` is absent.
pub const DEFAULT_BACK: i64 = 365;

/// A strict YYYY-MM-DD date.
pub fn date(text: &str) -> Option<NaiveDate> {
    let b = text.as_bytes();
    let shape = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    shape
        .then(|| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
        .flatten()
}

fn invalid(name: &str, value: &str) -> ApiError {
    ApiError::bad(
        "invalid_date",
        format!("{name}={value} is not a valid YYYY-MM-DD date"),
    )
}

/// The checked range of a raw query, with `to` defaulting to `today`.
pub fn range(raw: Option<&str>, today: NaiveDate) -> Result<(NaiveDate, NaiveDate), ApiError> {
    let (mut from, mut to) = (None, None);
    for pair in raw.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "from" => from = Some(date(v).ok_or_else(|| invalid("from", v))?),
            "to" => to = Some(date(v).ok_or_else(|| invalid("to", v))?),
            other => {
                return Err(ApiError::bad(
                    "unknown_parameter",
                    format!("the query parameter {other} is not known; only from and to are"),
                ));
            }
        }
    }
    let to = to.unwrap_or(today);
    let from = from.unwrap_or(to - chrono::Duration::days(DEFAULT_BACK));
    if from > to {
        return Err(ApiError::bad(
            "inverted_range",
            format!("from {from} is after to {to}"),
        ));
    }
    if (to - from).num_days() + 1 > MAX_DAYS {
        return Err(ApiError::bad(
            "range_too_long",
            format!("the range {from} to {to} has more than {MAX_DAYS} days"),
        ));
    }
    Ok((from, to))
}
