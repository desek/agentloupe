//! @agents-index The retry of each wake: fetches again each published day of the last 7 days that has a label with no ok fetch.

use super::{Settings, fetch, store};
use crate::day::{boundary, state, store as days};
use chrono::Duration;
use chrono_tz::Tz;
use rusqlite::Connection;

/// The window of the retry, in days before today.
pub const WINDOW_DAYS: i64 = 7;

/// Runs the retry; a fetch that changes a row puts the day in the queue with the reason github.
/// Returns the number of fetches.
pub fn run(
    conn: &Connection,
    s: &Settings,
    tz: Tz,
    now_ms: i64,
    now: &str,
) -> rusqlite::Result<u64> {
    let today = boundary::day_of(now_ms, tz);
    let mut n = 0;
    for back in (0..=WINDOW_DAYS).rev() {
        let d = today - Duration::days(back);
        let key = d.to_string();
        match days::read(conn, &key)? {
            Some(row) if row.state != state::DELETED => {}
            _ => continue,
        }
        for (label, token) in &s.tokens {
            if store::status(conn, &key, label)?.as_deref() != Some("ok") {
                fetch::fetch_label(conn, s, label, token, d, tz, now)?;
                n += 1;
            }
        }
    }
    Ok(n)
}
