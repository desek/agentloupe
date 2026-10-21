//! @agents-index SQL of session_full_summaries and session_day_summaries: discover pairs, due pairs, row writes, reset, and day reads.

use crate::day::boundary;
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

/// The table of full session summaries.
pub const FULL: &str = "session_full_summaries";
/// The table of session day summaries.
pub const DAY: &str = "session_day_summaries";

/// The pairs of session key and day (`YYYY-MM-DD` in `tz`) of the calls that `filter` selects.
/// `filter` is a condition on the segment `s`; it reads the parameter ?1 when it has one.
fn pairs(
    conn: &Connection,
    tz: Tz,
    filter: &str,
    arg: &str,
) -> rusqlite::Result<BTreeSet<(String, String)>> {
    // The minute groups keep the scan small; the day of each group comes from the time zone.
    let sql = format!(
        "SELECT DISTINCT s.key_hash, substr(c.started_at, 1, 16) FROM calls c JOIN sessions s ON s.id = c.session_id WHERE {filter}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = if filter.contains("?1") {
        stmt.query_map([arg], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(rows
        .into_iter()
        .map(|(k, t)| {
            let day = boundary::day_of_text(&format!("{t}:00.000Z"), tz).to_string();
            (k, day)
        })
        .collect())
}

fn insert_pending(
    conn: &Connection,
    table: &str,
    key: &str,
    day: &str,
    now: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO {table} (session_key, day, status, attempts, created_at, updated_at) VALUES (?1, ?2, 'pending', 0, ?3, ?3)"
        ),
        params![key, day, now],
    )
}

/// Makes a pending row in both tables for each day of each session with new work: an open segment,
/// or a closed segment with no final session line. Only those sessions are read, so the days of
/// sessions that ended before the tables existed get rows only through a rebuild.
/// `label` limits the sessions to one agent label; None reads every label.
pub fn discover(
    conn: &Connection,
    tz: Tz,
    label: Option<&str>,
    now: &str,
) -> rusqlite::Result<usize> {
    let open_work = "s.key_hash IN (SELECT n.key_hash FROM sessions n LEFT JOIN session_summaries ss ON ss.session_id = n.id \
        WHERE n.state = 'open' OR ss.id IS NULL OR ss.status = 'pending')";
    let filter = match label {
        Some(_) => format!("s.agent_label = ?1 AND {open_work}"),
        None => open_work.to_string(),
    };
    let mut made = 0;
    for (k, d) in pairs(conn, tz, &filter, label.unwrap_or(""))? {
        made += insert_pending(conn, FULL, &k, &d, now)?;
        made += insert_pending(conn, DAY, &k, &d, now)?;
    }
    Ok(made)
}

/// One pending pair: the session key, the day, and the source of its segments.
#[derive(Debug, Clone)]
pub struct Pair {
    pub key: String,
    pub day: String,
    /// The key source of the segments: `log` for a backfilled session.
    pub key_source: String,
    pub agent_label: String,
}

/// The pairs with a due pending row in either table, oldest day first.
pub fn due(conn: &Connection, now: &str) -> rusqlite::Result<Vec<Pair>> {
    let due = "status = 'pending' AND (next_attempt_at IS NULL OR next_attempt_at <= ?1)";
    conn.prepare(&format!(
        "SELECT p.session_key, p.day, (SELECT s.key_source FROM sessions s WHERE s.key_hash = p.session_key LIMIT 1), \
         (SELECT s.agent_label FROM sessions s WHERE s.key_hash = p.session_key LIMIT 1) FROM \
         (SELECT session_key, day FROM {FULL} WHERE {due} UNION SELECT session_key, day FROM {DAY} WHERE {due}) p \
         ORDER BY p.day, p.session_key"
    ))?
    .query_map([now], |r| {
        Ok(Pair {
            key: r.get(0)?,
            day: r.get(1)?,
            key_source: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            agent_label: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        })
    })?
    .collect()
}

/// True while a segment of the session with a call in [from, to) is open.
pub fn open_on(conn: &Connection, key: &str, from: &str, to: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sessions s JOIN calls c ON c.session_id = s.id \
         WHERE s.key_hash = ?1 AND s.state = 'open' AND c.started_at >= ?2 AND c.started_at < ?3)",
        params![key, from, to],
        |r| r.get(0),
    )
}

/// The state of one row: status, text, and attempt count.
#[derive(Debug, Clone)]
pub struct State {
    pub status: String,
    pub text: Option<String>,
    pub attempts: u32,
    pub next_attempt_at: Option<String>,
}

pub fn get(
    conn: &Connection,
    table: &str,
    key: &str,
    day: &str,
) -> rusqlite::Result<Option<State>> {
    conn.query_row(
        &format!("SELECT status, text, attempts, next_attempt_at FROM {table} WHERE session_key = ?1 AND day = ?2"),
        params![key, day],
        |r| {
            Ok(State {
                status: r.get(0)?,
                text: r.get(1)?,
                attempts: r.get(2)?,
                next_attempt_at: r.get(3)?,
            })
        },
    )
    .optional()
}

/// The new state of one row.
pub struct Row<'a> {
    pub status: &'a str,
    pub text: Option<&'a str>,
    pub model: Option<&'a str>,
    pub attempts: u32,
    pub last_error_kind: Option<&'a str>,
    pub next_attempt_at: Option<String>,
}

/// Inserts or replaces the state of the row of a pair; `created_at` stays from the first write.
pub fn put(
    conn: &Connection,
    table: &str,
    key: &str,
    day: &str,
    r: &Row,
    now: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO {table} (session_key, day, status, text, model, attempts, last_error_kind, next_attempt_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9) \
             ON CONFLICT (session_key, day) DO UPDATE SET status = ?3, text = ?4, model = ?5, attempts = ?6, last_error_kind = ?7, next_attempt_at = ?8, updated_at = ?9"
        ),
        params![key, day, r.status, r.text, r.model, r.attempts, r.last_error_kind, r.next_attempt_at, now],
    )
    .map(|_| ())
}

/// Makes or resets the rows of both tables for each pair with a call on a day in [from, to] to
/// pending; returns the count of rows. A dry run only counts them.
pub fn reset_days(
    conn: &Connection,
    tz: Tz,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
    now: &str,
    dry_run: bool,
) -> rusqlite::Result<usize> {
    let (lo, _) = boundary::bounds(from, tz);
    let (_, hi) = boundary::bounds(to, tz);
    let filter = format!("c.started_at >= '{lo}' AND c.started_at < '{hi}'");
    let list = pairs(conn, tz, &filter, "")?;
    if dry_run {
        return Ok(2 * list.len());
    }
    for (k, d) in &list {
        for table in [FULL, DAY] {
            conn.execute(
                &format!(
                    "INSERT INTO {table} (session_key, day, status, attempts, created_at, updated_at) VALUES (?1, ?2, 'pending', 0, ?3, ?3) \
                     ON CONFLICT (session_key, day) DO UPDATE SET status = 'pending', text = NULL, attempts = 0, last_error_kind = NULL, next_attempt_at = NULL, updated_at = ?3"
                ),
                params![k, d, now],
            )?;
        }
    }
    Ok(2 * list.len())
}

/// Sets failed and disabled rows of both tables back to pending with 0 attempts; a dry run only counts them.
pub fn retry(conn: &Connection, now: &str, dry_run: bool) -> rusqlite::Result<usize> {
    let mut n = 0;
    for table in [FULL, DAY] {
        n += if dry_run {
            conn.query_row(
                &format!("SELECT count(*) FROM {table} WHERE status IN ('failed', 'disabled')"),
                [],
                |r| r.get::<_, i64>(0).map(|n| n as usize),
            )?
        } else {
            conn.execute(
                &format!("UPDATE {table} SET status = 'pending', attempts = 0, next_attempt_at = NULL, last_error_kind = NULL, updated_at = ?1 WHERE status IN ('failed', 'disabled')"),
                [now],
            )?
        };
    }
    Ok(n)
}

/// The session day summaries of one day, the session with the most calls in [from, to) first.
pub struct DayText {
    pub status: String,
    pub text: Option<String>,
}

pub fn day_texts(
    conn: &Connection,
    day: &str,
    from: &str,
    to: &str,
) -> rusqlite::Result<Vec<DayText>> {
    conn.prepare(&format!(
        "SELECT r.status, r.text, (SELECT count(*) FROM calls c JOIN sessions s ON s.id = c.session_id \
         WHERE s.key_hash = r.session_key AND c.started_at >= ?2 AND c.started_at < ?3) AS n \
         FROM {DAY} r WHERE r.day = ?1 ORDER BY n DESC, r.session_key"
    ))?
    .query_map(params![day, from, to], |r| {
        Ok(DayText {
            status: r.get(0)?,
            text: r.get(1)?,
        })
    })?
    .collect()
}

/// True while a segment with a call in [from, to) is open.
pub fn any_open(conn: &Connection, from: &str, to: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sessions s JOIN calls c ON c.session_id = s.id \
         WHERE s.state = 'open' AND c.started_at >= ?1 AND c.started_at < ?2)",
        params![from, to],
        |r| r.get(0),
    )
}
