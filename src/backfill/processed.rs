//! @agents-index The processed-session rules: a Claude Code session key that the proxy holds, and the Pi first-exchange boundary.

use crate::session::time;
use rusqlite::Connection;

/// True when a session that the proxy capture made has this key hash. A backfilled session (key source
/// log) does not count: the log key already stops a second insert.
pub fn captured(conn: &Connection, key_hash: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sessions WHERE key_hash = ?1 AND key_source != 'log')",
        [key_hash],
        |r| r.get(0),
    )
}

/// The Pi boundary: the start of the oldest exchange of `label`, and the other agent labels with exchanges.
pub struct Boundary {
    pub at_ms: Option<i64>,
    pub others: Vec<String>,
}

pub fn pi_boundary(conn: &Connection, label: &str) -> rusqlite::Result<Boundary> {
    let at: Option<String> = conn.query_row(
        "SELECT min(started_at) FROM exchanges WHERE agent = ?1",
        [label],
        |r| r.get(0),
    )?;
    let others = conn
        .prepare("SELECT DISTINCT agent FROM exchanges WHERE agent != ?1 ORDER BY agent")?
        .query_map([label], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Boundary {
        at_ms: at.map(|t| time::ms(&t)),
        others,
    })
}

/// A Pi log session is captured when any entry is at or after the boundary, so each session stays whole in one origin.
pub fn pi_captured(last_ms: Option<i64>, b: &Boundary) -> bool {
    matches!((last_ms, b.at_ms), (Some(l), Some(at)) if l >= at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::segment::tests::db;

    fn exchange(c: &Connection, id: &str, agent: &str, at: &str) {
        c.execute(
            "INSERT INTO exchanges (id, started_at, ended_at, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_truncated, response_truncated, outcome) \
             VALUES (?1, ?2, ?2, ?3, 'u', 'POST', '/', '', 200, 0, 0, '{}', '{}', 0, 0, 'ok')",
            [id, at, agent],
        )
        .unwrap();
    }

    fn session(c: &Connection, key: &str, source: &str) {
        c.execute(
            "INSERT INTO sessions (agent_label, key_hash, key_source, segment, started_at, last_call_at, state, call_count) VALUES ('claude-code', ?1, ?2, 1, 'x', 'x', 'open', 1)",
            [key, source],
        )
        .unwrap();
    }

    #[test]
    fn captured_session_key_is_processed() {
        let c = db();
        session(&c, "k-proxy", "header");
        session(&c, "k-log", "log");
        assert!(captured(&c, "k-proxy").unwrap());
        assert!(!captured(&c, "k-log").unwrap());
        assert!(!captured(&c, "k-none").unwrap());
    }

    #[test]
    fn pi_session_before_first_exchange_only() {
        let c = db();
        let b = pi_boundary(&c, "pi").unwrap();
        assert!(b.at_ms.is_none() && b.others.is_empty());
        assert!(!pi_captured(Some(i64::MAX), &b));
        exchange(&c, "a", "pi", "2026-01-02T00:00:00.000Z");
        exchange(&c, "b", "pi", "2026-01-03T00:00:00.000Z");
        exchange(&c, "c", "other", "2026-01-01T00:00:00.000Z");
        let b = pi_boundary(&c, "pi").unwrap();
        assert_eq!(b.others, vec!["other".to_string()]);
        let at = time::ms("2026-01-02T00:00:00.000Z");
        assert!(!pi_captured(Some(at - 1), &b));
        assert!(pi_captured(Some(at), &b));
        assert!(pi_captured(Some(at + 86_400_000), &b));
        let none = pi_boundary(&c, "pi-dev").unwrap();
        assert!(!pi_captured(Some(at), &none));
        assert_eq!(none.others.len(), 2);
    }
}
