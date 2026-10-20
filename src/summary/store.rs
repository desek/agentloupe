//! @agents-index SQL of session_summaries and day_summaries: due segments, the upsert of a status, retry, and the day row.

use rusqlite::{Connection, OptionalExtension, params};

/// The most segments that one batch reads.
pub const BATCH: i64 = 20;

/// Closed segments with no summary row, or with a pending row that is due; oldest close first.
/// `limit` -1 reads every due segment, for a dry run that writes nothing.
pub fn due(conn: &Connection, now: &str, limit: i64) -> rusqlite::Result<Vec<i64>> {
    conn.prepare(
        "SELECT s.id FROM sessions s LEFT JOIN session_summaries ss ON ss.session_id = s.id \
         WHERE s.state = 'closed' AND (ss.id IS NULL OR (ss.status = 'pending' AND (ss.next_attempt_at IS NULL OR ss.next_attempt_at <= ?1))) \
         ORDER BY s.closed_at, s.id LIMIT ?2",
    )?
    .query_map(params![now, limit], |r| r.get(0))?
    .collect()
}

/// The attempt count of the row of a segment; 0 when it has no row.
pub fn attempts(conn: &Connection, session_id: i64) -> rusqlite::Result<u32> {
    Ok(conn
        .query_row(
            "SELECT attempts FROM session_summaries WHERE session_id = ?1",
            [session_id],
            |r| r.get::<_, u32>(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// The new state of the row of one segment.
pub struct Row<'a> {
    pub status: &'a str,
    pub line: Option<&'a str>,
    pub model: Option<&'a str>,
    pub attempts: u32,
    pub last_error_kind: Option<&'a str>,
    pub next_attempt_at: Option<String>,
}

/// Inserts or replaces the state of the row of a segment; `created_at` stays from the first write.
pub fn put(conn: &Connection, session_id: i64, r: &Row, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO session_summaries (session_id, status, line, model, attempts, last_error_kind, next_attempt_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8) \
         ON CONFLICT (session_id) DO UPDATE SET status = ?2, line = ?3, model = ?4, attempts = ?5, last_error_kind = ?6, next_attempt_at = ?7, updated_at = ?8",
        params![session_id, r.status, r.line, r.model, r.attempts, r.last_error_kind, r.next_attempt_at, now],
    )
    .map(|_| ())
}

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

/// The segments that started in [from, to), most calls first: state, summary status, and line.
pub struct DaySeg {
    pub state: String,
    pub status: Option<String>,
    pub line: Option<String>,
}

pub fn day_segments(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<Vec<DaySeg>> {
    conn.prepare(
        "SELECT s.state, ss.status, ss.line FROM sessions s LEFT JOIN session_summaries ss ON ss.session_id = s.id \
         WHERE s.started_at >= ?1 AND s.started_at < ?2 ORDER BY s.call_count DESC, s.id",
    )?
    .query_map(params![from, to], |r| {
        Ok(DaySeg {
            state: r.get(0)?,
            status: r.get(1)?,
            line: r.get(2)?,
        })
    })?
    .collect()
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
