//! @agents-index Puts a call in the open segment of its scope, or closes the segment on a gap and starts the next one.

use super::store::{self, Scope};
use rusqlite::Connection;

/// The segment that holds the call, and the count of segments that the call opened and closed.
#[derive(Debug, PartialEq)]
pub struct Assigned {
    pub session_id: i64,
    pub opened: u64,
    pub closed: u64,
    /// The segment that this call closed on a gap, so the caller can queue its days.
    pub closed_id: Option<i64>,
}

/// A closed segment is never reopened: a call after a close always starts the next segment number.
pub fn assign(
    conn: &Connection,
    scope: &Scope,
    key_source: &str,
    at_ms: i64,
    idle_secs: u64,
    account_id: Option<i64>,
) -> rusqlite::Result<Assigned> {
    let idle_ms = idle_secs as i64 * 1000;
    let mut closed = 0;
    let mut closed_id = None;
    if let Some(open) = store::open_segment(conn, scope)? {
        if at_ms - open.last_call_ms <= idle_ms {
            store::join(conn, open.id, at_ms)?;
            return Ok(Assigned {
                session_id: open.id,
                opened: 0,
                closed: 0,
                closed_id: None,
            });
        }
        store::close(conn, open.id, open.last_call_ms + idle_ms)?;
        closed = 1;
        closed_id = Some(open.id);
    }
    let id = store::insert(conn, scope, key_source, at_ms, account_id)?;
    Ok(Assigned {
        session_id: id,
        opened: 1,
        closed,
        closed_id,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::store::migrate::migrate(&mut c).unwrap();
        c.execute_batch("INSERT INTO proxy_tokens (id, name, token_hash, tags, created_at) VALUES (1, 'a', 'h1', '[]', 'x'), (2, 'b', 'h2', '[]', 'x')")
            .unwrap();
        c
    }

    pub fn scope(token: Option<i64>) -> Scope {
        Scope {
            agent_label: "pi".into(),
            proxy_token_id: token,
            key_hash: "k".into(),
        }
    }

    fn row(c: &Connection, id: i64) -> (i64, String, i64) {
        c.query_row(
            "SELECT segment, state, call_count FROM sessions WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    #[test]
    fn resume_after_close_starts_new_segment() {
        let c = db();
        let s = scope(None);
        let a = assign(&c, &s, "header", 0, 60, None).unwrap();
        assert_eq!(
            assign(&c, &s, "header", 30_000, 60, None)
                .unwrap()
                .session_id,
            a.session_id
        );
        let b = assign(&c, &s, "header", 200_000, 60, None).unwrap();
        assert_eq!((b.opened, b.closed), (1, 1));
        assert_eq!(row(&c, a.session_id), (1, "closed".into(), 2));
        assert_eq!(row(&c, b.session_id), (2, "open".into(), 1));
        let closed_at: String = c
            .query_row(
                "SELECT closed_at FROM sessions WHERE id = ?1",
                [a.session_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(super::super::time::ms(&closed_at), 90_000);
    }

    #[test]
    fn same_key_other_token_is_other_session() {
        let c = db();
        let a = assign(&c, &scope(Some(1)), "header", 0, 60, None).unwrap();
        let b = assign(&c, &scope(Some(2)), "header", 1_000, 60, None).unwrap();
        assert_ne!(a.session_id, b.session_id);
        assert_eq!(row(&c, b.session_id).0, 1);
        let n: i64 = c
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
    }
}
