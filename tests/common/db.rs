//! @agents-index Test helper: a migrated database in a temp dir, with segments, log calls, and session text rows made by SQL.
#![allow(dead_code)]

use rusqlite::{Connection, params};

/// A migrated database; keep the temp dir alive while the connection is used.
pub fn open() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = agent_loupe::store::open::open(dir.path()).unwrap();
    agent_loupe::store::migrate::migrate(&mut conn).unwrap();
    (dir, conn)
}

/// One backfilled segment of session `key` in `state` with `n` calls at `at`; returns its id.
pub fn segment(conn: &Connection, key: &str, state: &str, at: &str, n: i64) -> i64 {
    let closed = (state == "closed").then_some(at);
    conn.execute(
        "INSERT INTO sessions (agent_label, key_hash, key_source, segment, started_at, last_call_at, closed_at, state, call_count) \
         VALUES ('claude-code', ?1, 'log', 1, ?2, ?2, ?3, ?4, ?5)",
        params![key, at, closed, state, n],
    )
    .unwrap();
    let id = conn.last_insert_rowid();
    for i in 0..n {
        conn.execute(
            "INSERT INTO calls (call_index, started_at, agent_label, upstream, session_id, status, input, cache_read, cache_write_5m, cache_write_1h, output, origin, log_key) \
             VALUES (?1, ?2, 'claude-code', 'anthropic', ?3, 200, 1, 0, 0, 0, 1, 'log', ?4)",
            params![i, at, id, format!("{key}-{id}-{i}")],
        )
        .unwrap();
    }
    id
}

/// One row of a session text table.
pub fn text_row(
    conn: &Connection,
    table: &str,
    key: &str,
    day: &str,
    status: &str,
    text: Option<&str>,
) {
    conn.execute(
        &format!(
            "INSERT INTO {table} (session_key, day, status, text, attempts, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 1, '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')"
        ),
        params![key, day, status, text],
    )
    .unwrap();
}

/// A summary client with a test API key against `url`.
pub fn client(url: &str) -> agent_loupe::summary::client::Client {
    agent_loupe::summary::client::Client::new(
        url,
        "claude-haiku-5-5",
        agent_loupe::summary::credential::Credential::ApiKey(
            agent_loupe::config::secret::Secret::new("sk-test-Dl9".into()),
        ),
    )
}
