//! @agents-index The day boundaries in LOUPE_TZ: the day of an instant, the UTC bounds of a day, and its length in hours.

use crate::session::time;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;

/// Unix milliseconds of the first instant of `day` in `tz`. A local midnight that does not exist
/// (a gap of a time change) moves to the first local minute after it that exists.
pub fn start_ms(day: NaiveDate, tz: Tz) -> i64 {
    let mut local = day.and_hms_opt(0, 0, 0).expect("midnight exists");
    for _ in 0..240 {
        if let Some(t) = tz.from_local_datetime(&local).earliest() {
            return t.timestamp_millis();
        }
        local += chrono::Duration::minutes(1);
    }
    0
}

/// The UTC bounds [from, to) of `day` in `tz`, as stored time text, so a text compare on
/// `started_at` selects the calls of the day.
pub fn bounds(day: NaiveDate, tz: Tz) -> (String, String) {
    let next = day.succ_opt().unwrap_or(day);
    (
        time::text(start_ms(day, tz)),
        time::text(start_ms(next, tz)),
    )
}

/// The calendar day in `tz` of the instant `ms`.
pub fn day_of(ms: i64, tz: Tz) -> NaiveDate {
    tz.timestamp_millis_opt(ms)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default()
}

/// The day in `tz` of a stored time text.
pub fn day_of_text(text: &str, tz: Tz) -> NaiveDate {
    day_of(time::ms(text), tz)
}

/// The length of `day` in `tz` in whole hours: 23 or 25 on a day with a time change.
pub fn span_hours(day: NaiveDate, tz: Tz) -> i64 {
    let next = day.succ_opt().unwrap_or(day);
    (start_ms(next, tz) - start_ms(day, tz)) / 3_600_000
}
