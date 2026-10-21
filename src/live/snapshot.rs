//! @agents-index The snapshot of the current day: calls, token sums, and cost of the calls inside the day bounds in LOUPE_TZ, and the open segments.

use super::event::{Snapshot, Tokens};
use crate::day::boundary;
use rusqlite::Connection;
use rust_decimal::Decimal;

/// Reads the totals of the day that holds `now_ms` in `tz`, with the day rule of the day records:
/// a call is in the day if its start time is inside the bounds of the day.
pub fn read(
    conn: &Connection,
    tz: chrono_tz::Tz,
    now_ms: i64,
    in_flight: i64,
) -> rusqlite::Result<Snapshot> {
    let day = boundary::day_of(now_ms, tz);
    let (from, to) = boundary::bounds(day, tz);
    let mut s = Snapshot {
        date: day.format("%Y-%m-%d").to_string(),
        in_flight,
        ..Default::default()
    };
    let mut stmt = conn.prepare(
        "SELECT input, output, cache_read, cache_write_5m + cache_write_1h, cost_usd FROM calls \
         WHERE started_at >= ?1 AND started_at < ?2",
    )?;
    let mut rows = stmt.query([&from, &to])?;
    while let Some(r) = rows.next()? {
        s.calls += 1;
        s.tokens.add(&Tokens {
            input: r.get(0)?,
            output: r.get(1)?,
            cache_read: r.get(2)?,
            cache_write: r.get(3)?,
            reasoning: 0,
        });
        let c: Option<String> = r.get(4)?;
        s.cost_usd += c
            .and_then(|c| c.parse::<Decimal>().ok())
            .unwrap_or_default();
    }
    s.cost_usd = crate::day::sum::round4(s.cost_usd);
    s.segments_open = open_segments(conn)?;
    Ok(s)
}

/// The count of segments with the state open.
pub fn open_segments(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT count(*) FROM sessions WHERE state = 'open'",
        [],
        |r| r.get(0),
    )
}
