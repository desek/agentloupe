//! @agents-index SQL of the sessions table: find the open segment of a scope, insert, join, and close a segment.

use super::time;
use rusqlite::{Connection, OptionalExtension, params};

/// The scope of a session: one agent label, one proxy token (or none), one session key hash.
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    pub agent_label: String,
    pub proxy_token_id: Option<i64>,
    pub key_hash: String,
}

/// The open segment of a scope, with its last call time in Unix milliseconds.
pub struct Open {
    pub id: i64,
    pub last_call_ms: i64,
}

pub fn open_segment(conn: &Connection, s: &Scope) -> rusqlite::Result<Option<Open>> {
    conn.query_row(
        "SELECT id, last_call_at FROM sessions WHERE agent_label = ?1 AND proxy_token_id IS ?2 AND key_hash = ?3 AND state = 'open'",
        params![s.agent_label, s.proxy_token_id, s.key_hash],
        |r| Ok(Open { id: r.get(0)?, last_call_ms: time::ms(&r.get::<_, String>(1)?) }),
    )
    .optional()
}

/// Inserts segment number max + 1 of the scope, open, with one call.
pub fn insert(
    conn: &Connection,
    s: &Scope,
    key_source: &str,
    at_ms: i64,
    account_id: Option<i64>,
) -> rusqlite::Result<i64> {
    let at = time::text(at_ms);
    conn.query_row(
        "INSERT INTO sessions (agent_label, proxy_token_id, key_hash, key_source, segment, started_at, last_call_at, state, call_count, account_id) \
         VALUES (?1, ?2, ?3, ?4, (SELECT coalesce(max(segment), 0) + 1 FROM sessions WHERE agent_label = ?1 AND proxy_token_id IS ?2 AND key_hash = ?3), ?5, ?5, 'open', 1, ?6) RETURNING id",
        params![s.agent_label, s.proxy_token_id, s.key_hash, key_source, at, account_id],
        |r| r.get(0),
    )
}

/// Adds one call; the last call time only moves forward, so a late import cannot move it back.
pub fn join(conn: &Connection, id: i64, at_ms: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET call_count = call_count + 1, last_call_at = max(last_call_at, ?2) WHERE id = ?1",
        params![id, time::text(at_ms)],
    )
    .map(|_| ())
}

pub fn close(conn: &Connection, id: i64, closed_ms: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET state = 'closed', closed_at = ?2 WHERE id = ?1",
        params![id, time::text(closed_ms)],
    )
    .map(|_| ())
}
