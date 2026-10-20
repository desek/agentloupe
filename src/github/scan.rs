//! @agents-index The catch-up scan of each wake: fetches each due day with no record and no ok fetch, per label, oldest first, at most 90 days back.

use super::{Settings, fetch, store};
use crate::day::activity::{Calls, Source};
use crate::day::{boundary, store as days};
use crate::publish::due;
use chrono::{Duration, NaiveTime};
use chrono_tz::Tz;
use rusqlite::Connection;

/// The longest lookback of the scan, in days.
pub const LOOKBACK_DAYS: i64 = 90;

/// Runs the scan; returns the number of fetches. It does not depend on the due days of the calls
/// source: it starts at the later of the first call day and today minus 90 days.
pub fn run(
    conn: &Connection,
    s: &Settings,
    tz: Tz,
    publish_at: NaiveTime,
    now_ms: i64,
    now: &str,
) -> rusqlite::Result<u64> {
    let floor = boundary::day_of(now_ms, tz) - Duration::days(LOOKBACK_DAYS);
    let start = Calls
        .first_activity_day(conn, tz)?
        .map_or(floor, |d| d.max(floor));
    let rows = days::days_with_row(conn)?;
    let mut n = 0;
    let mut day = Some(start);
    while let Some(d) = day {
        if !due::is_due(d, now_ms, tz, publish_at) {
            break;
        }
        let key = d.to_string();
        if !rows.contains(&key) {
            for (label, token) in &s.tokens {
                if store::status(conn, &key, label)?.as_deref() != Some("ok") {
                    fetch::fetch_label(conn, s, label, token, d, tz, now)?;
                    n += 1;
                }
            }
        }
        day = d.succ_opt();
    }
    Ok(n)
}
