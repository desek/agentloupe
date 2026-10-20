//! @agents-index SQL of agg_queue: put a day with a reason, put each day of a segment, list, and delete.

use super::boundary;
use chrono_tz::Tz;
use rusqlite::{Connection, params};
use std::collections::BTreeSet;

pub const CALL: &str = "call";
pub const SESSION_CLOSED: &str = "session_closed";
pub const SUMMARY_FINAL: &str = "summary_final";
pub const SUMMARY_NOT_READY: &str = "summary_not_ready";

/// A second put of a day keeps one row with the newest reason and time.
pub fn put(conn: &Connection, day: &str, reason: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO agg_queue (day, reason, queued_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (day) DO UPDATE SET reason = ?2, queued_at = ?3",
        params![day, reason, now],
    )
    .map(|_| ())
}

/// Puts each day in `tz` that has a call of the segment `session_id`.
pub fn put_session_days(
    conn: &Connection,
    session_id: i64,
    tz: Tz,
    reason: &str,
    now: &str,
) -> rusqlite::Result<()> {
    let days: BTreeSet<String> = conn
        .prepare_cached("SELECT started_at FROM calls WHERE session_id = ?1")?
        .query_map([session_id], |r| r.get::<_, String>(0))?
        .map(|t| t.map(|t| boundary::day_of_text(&t, tz).to_string()))
        .collect::<rusqlite::Result<_>>()?;
    for d in days {
        put(conn, &d, reason, now)?;
    }
    Ok(())
}

/// The queued days with their reasons, oldest day first.
pub fn list(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    conn.prepare("SELECT day, reason FROM agg_queue ORDER BY day")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

pub fn delete(conn: &Connection, day: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM agg_queue WHERE day = ?1", [day])
        .map(|_| ())
}

/// The reason of a queued day, or None.
pub fn reason(conn: &Connection, day: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT reason FROM agg_queue WHERE day = ?1", [day], |r| {
        r.get(0)
    })
    .optional()
}
