//! @agents-index The idle step: closes each open segment whose last call plus the idle time is before the parse watermark.

use super::time;
use rusqlite::{Connection, params};

/// The parse watermark: the start of the oldest exchange after the cursor, or `now_ms` with no backlog.
/// Closing only before it makes a late parse of a backlog give the same segments as a live parse.
pub fn watermark(conn: &Connection, cursor: i64, now_ms: i64) -> rusqlite::Result<i64> {
    let oldest: Option<String> = conn.query_row(
        "SELECT min(started_at) FROM exchanges WHERE rowid > ?1",
        [cursor],
        |r| r.get(0),
    )?;
    Ok(oldest.map_or(now_ms, |t| time::ms(&t)))
}

/// Closes with `closed_at` = last call + idle time; returns the count of closed segments.
pub fn close_idle(conn: &Connection, idle_secs: u64, watermark_ms: i64) -> rusqlite::Result<u64> {
    let idle_ms = idle_secs as i64 * 1000;
    let open: Vec<(i64, String)> = conn
        .prepare("SELECT id, last_call_at FROM sessions WHERE state = 'open'")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut closed = 0;
    for (id, last) in open {
        let end = time::ms(&last) + idle_ms;
        if end < watermark_ms {
            conn.execute(
                "UPDATE sessions SET state = 'closed', closed_at = ?2 WHERE id = ?1",
                params![id, time::text(end)],
            )?;
            closed += 1;
        }
    }
    Ok(closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::segment::assign;
    use crate::session::segment::tests::{db, scope};

    const HOUR: i64 = 3_600_000;

    fn exchange(c: &Connection, id: &str, at_ms: i64) {
        c.execute(
            "INSERT INTO exchanges (id, started_at, ended_at, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_body, response_body, request_truncated, response_truncated, outcome) VALUES (?1, ?2, ?2, 'pi', 'x', 'POST', '/v1/messages', '', 200, 0, 0, '[]', '[]', x'', x'', 0, 0, 'complete')",
            params![id, time::text(at_ms)],
        )
        .unwrap();
    }

    #[test]
    fn idle_uses_last_call_time() {
        let c = db();
        let now = 10 * HOUR;
        let a = assign(&c, &scope(None), "header", now - 2 * HOUR, 60, None).unwrap();
        assert_eq!(
            close_idle(&c, 60, watermark(&c, 0, now).unwrap()).unwrap(),
            1
        );
        let closed_at: String = c
            .query_row(
                "SELECT closed_at FROM sessions WHERE id = ?1",
                [a.session_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(time::ms(&closed_at), now - 2 * HOUR + 60_000);
    }

    #[test]
    fn backlog_gap_within_idle_keeps_one_segment() {
        let c = db();
        let t0 = HOUR;
        exchange(&c, "e1", t0);
        exchange(&c, "e2", t0 + 30_000);
        let a = assign(&c, &scope(None), "header", t0, 60, None).unwrap();
        // The cursor stands after e1; e2 is not parsed and the clock is 2 hours later.
        let w = watermark(&c, 1, t0 + 2 * HOUR).unwrap();
        assert_eq!(close_idle(&c, 60, w).unwrap(), 0);
        let b = assign(&c, &scope(None), "header", t0 + 30_000, 60, None).unwrap();
        assert_eq!(b.session_id, a.session_id);
        let n: i64 = c
            .query_row(
                "SELECT call_count FROM sessions WHERE id = ?1",
                [a.session_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
    }
}
