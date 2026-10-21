//! @agents-index SQL of session_summaries and day_summaries: retry of the kept rows, and the day row.

use rusqlite::{Connection, params};

/// Sets failed and disabled rows back to pending with 0 attempts; a dry run only counts them.
pub fn retry(conn: &Connection, now: &str, dry_run: bool) -> rusqlite::Result<usize> {
    if dry_run {
        return conn.query_row(
            "SELECT count(*) FROM session_summaries WHERE status IN ('failed', 'disabled')",
            [],
            |r| r.get::<_, i64>(0).map(|n| n as usize),
        );
    }
    conn.execute(
        "UPDATE session_summaries SET status = 'pending', attempts = 0, next_attempt_at = NULL, last_error_kind = NULL, updated_at = ?1 WHERE status IN ('failed', 'disabled')",
        [now],
    )
}

/// Replaces the row of a day, so a new run keeps the newest result.
pub fn put_day(
    conn: &Connection,
    day: &str,
    status: &str,
    line: Option<&str>,
    lines: usize,
    now: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO day_summaries (day, status, line, session_lines, built_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![day, status, line, lines as i64, now],
    )
    .map(|_| ())
}
