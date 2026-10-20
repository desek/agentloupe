//! @agents-index The due rule: a day D is due at or after LOUPE_PUBLISH_AT in LOUPE_TZ on D plus one, and the next due time.

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

/// The local time now in `tz`.
fn local(now_ms: i64, tz: Tz) -> chrono::NaiveDateTime {
    tz.timestamp_millis_opt(now_ms)
        .single()
        .map(|t| t.naive_local())
        .unwrap_or_default()
}

pub fn is_due(day: NaiveDate, now_ms: i64, tz: Tz, publish_at: NaiveTime) -> bool {
    day.succ_opt()
        .is_some_and(|next| local(now_ms, tz) >= next.and_time(publish_at))
}

/// The next instant at which a day becomes due: `publish_at` today in `tz` if it is still
/// ahead, else tomorrow.
pub fn next_due(now_ms: i64, tz: Tz, publish_at: NaiveTime) -> DateTime<Tz> {
    let now = local(now_ms, tz);
    let mut date = now.date();
    if now.time() >= publish_at {
        date = date.succ_opt().unwrap_or(date);
    }
    let at = date.and_time(publish_at);
    tz.from_local_datetime(&at)
        .earliest()
        .unwrap_or_else(|| tz.from_utc_datetime(&at))
}

/// The same instant in UTC.
pub fn utc(t: DateTime<Tz>) -> DateTime<Utc> {
    t.with_timezone(&Utc)
}
