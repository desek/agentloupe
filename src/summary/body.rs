//! @agents-index Loads the transcript of a segment: picks the request body with the longest history, one decrypted body at a time.

use super::transcript;
use crate::capture::seal::open;
use rusqlite::{Connection, OptionalExtension, params};

/// The exchanges of the segment that made a call and still have a request body, in id order.
fn exchanges(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<(i64, String)>> {
    conn.prepare(
        "SELECT DISTINCT e.rowid, e.id FROM calls c JOIN exchanges e ON e.id = c.exchange_id \
         WHERE c.session_id = ?1 AND e.parse_outcome = 'call' AND e.request_body IS NOT NULL ORDER BY e.rowid",
    )?
    .query_map([session_id], |r| Ok((r.get(0)?, r.get(1)?)))?
    .collect()
}

/// The parsed request body of one exchange; None when it cannot be read, decrypted, or parsed.
fn body(
    conn: &Connection,
    key: &[u8; 32],
    rowid: i64,
    id: &str,
) -> rusqlite::Result<Option<serde_json::Value>> {
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
        .and_then(|b| open(key, id, "request", &b).ok())
        .and_then(|plain| serde_json::from_slice(&plain).ok()))
}

/// The transcript of the segment; empty when no body has a turn with text.
/// The first pass keeps only the history length of each body, so at most one body is decrypted at a time.
pub fn load(
    conn: &Connection,
    key: &[u8; 32],
    session_id: i64,
    min_conversation: usize,
) -> rusqlite::Result<String> {
    let list = exchanges(conn, session_id)?;
    let mut lens = Vec::with_capacity(list.len());
    for (rowid, id) in &list {
        lens.push(
            body(conn, key, *rowid, id)?
                .as_ref()
                .map_or(0, transcript::history_len),
        );
    }
    let Some(i) = transcript::pick_longest(&lens) else {
        return Ok(String::new());
    };
    let (rowid, id) = &list[i];
    Ok(body(conn, key, *rowid, id)?
        .map(|b| transcript::build(&b, min_conversation))
        .unwrap_or_default())
}
