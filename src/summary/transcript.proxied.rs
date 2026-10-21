//! @agents-index The proxied transcript source: the longest history of every segment up to a day, and the messages that one day adds.

use crate::capture::seal::open;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

/// The exchanges with a call of the proxied session `key` that started in [from, to), in id order,
/// with a flag for a body that the retention already deleted.
fn exchanges(
    conn: &Connection,
    key: &str,
    from: &str,
    to: &str,
) -> rusqlite::Result<Vec<(i64, String, bool)>> {
    conn.prepare(
        "SELECT DISTINCT e.rowid, e.id, e.request_body IS NULL FROM calls c JOIN sessions s ON s.id = c.session_id \
         JOIN exchanges e ON e.id = c.exchange_id WHERE s.key_hash = ?1 AND s.key_source != 'log' \
         AND e.parse_outcome = 'call' AND c.started_at >= ?2 AND c.started_at < ?3 ORDER BY e.rowid",
    )?
    .query_map(params![key, from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
    .collect()
}

/// The parsed request body of one exchange; None when it cannot be read, decrypted, or parsed.
fn body(
    conn: &Connection,
    data_key: &[u8; 32],
    rowid: i64,
    id: &str,
) -> rusqlite::Result<Option<Value>> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT request_body FROM exchanges WHERE rowid = ?1",
            params![rowid],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    // The plain bytes drop at the end of this function; only the parsed value leaves it.
    Ok(blob
        .and_then(|b| open(data_key, id, "request", &b).ok())
        .and_then(|plain| serde_json::from_slice(&plain).ok()))
}

/// The longest message history of the calls in [from, to), and the count of deleted bodies.
/// The first pass keeps only the length of each history, so at most one body is decrypted at a time.
fn longest(
    conn: &Connection,
    data_key: &[u8; 32],
    key: &str,
    from: &str,
    to: &str,
) -> rusqlite::Result<(Vec<Value>, usize)> {
    let list = exchanges(conn, key, from, to)?;
    let missing = list.iter().filter(|e| e.2).count();
    let present: Vec<_> = list.into_iter().filter(|e| !e.2).collect();
    let mut lens = Vec::with_capacity(present.len());
    for (rowid, id, _) in &present {
        lens.push(
            body(conn, data_key, *rowid, id)?
                .as_ref()
                .map_or(0, super::transcript::history_len),
        );
    }
    let Some(i) = super::transcript::pick_longest(&lens) else {
        return Ok((vec![], missing));
    };
    let (rowid, id, _) = &present[i];
    let history = body(conn, data_key, *rowid, id)?
        .map(|b| super::transcript::history(&b).to_vec())
        .unwrap_or_default();
    Ok((history, missing))
}

fn log_missing(key: &str, day: &str, missing: usize) {
    if missing > 0 {
        tracing::info!(
            session_key = key,
            day,
            error_kind = "bodies_missing",
            "session text bodies missing"
        );
    }
}

/// The messages of the whole session up to `end`: the longest history of every segment.
/// A deleted body is left out, and the build goes on with the bodies that remain.
pub fn full(
    conn: &Connection,
    data_key: &[u8; 32],
    key: &str,
    day: &str,
    end: &str,
) -> rusqlite::Result<Vec<Value>> {
    let (history, missing) = longest(conn, data_key, key, "", end)?;
    log_missing(key, day, missing);
    Ok(history)
}

/// The messages that the calls of [start, end) add beyond the longest history of the calls before
/// `start`. A request body has no time for each message, so the history before the day is the cut.
pub fn day(
    conn: &Connection,
    data_key: &[u8; 32],
    key: &str,
    day: &str,
    start: &str,
    end: &str,
) -> rusqlite::Result<Vec<Value>> {
    let (before, m1) = longest(conn, data_key, key, "", start)?;
    let (today, m2) = longest(conn, data_key, key, start, end)?;
    log_missing(key, day, m1 + m2);
    let cut = before.len().min(today.len());
    Ok(today[cut..].to_vec())
}
