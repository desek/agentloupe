//! @agents-index The shared session step of the parser and the backfill: assigns a call to its segment and queues its days.

use super::segment::{self, Assigned};
use super::store::Scope;
use super::time;
use rusqlite::Connection;

/// Puts one call at `at_ms` in its segment with the gap rule, then queues the day of the call and,
/// when the call closed a segment on a gap, each day of that segment, in the open transaction.
/// Both origins of a call go through this one step, so a log call and a proxied call get the same segments.
pub fn append(
    conn: &Connection,
    scope: &Scope,
    key_source: &str,
    at_ms: i64,
    idle_secs: u64,
    account_id: Option<i64>,
    tz: chrono_tz::Tz,
) -> rusqlite::Result<Assigned> {
    let seg = segment::assign(conn, scope, key_source, at_ms, idle_secs, account_id)?;
    let now = time::text(chrono::Utc::now().timestamp_millis());
    let day = crate::day::boundary::day_of(at_ms, tz).to_string();
    crate::day::queue::put(conn, &day, crate::day::queue::CALL, &now)?;
    if let Some(closed) = seg.closed_id {
        crate::day::queue::put_session_days(
            conn,
            closed,
            tz,
            crate::day::queue::SESSION_CLOSED,
            &now,
        )?;
    }
    Ok(seg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::segment::tests::db;

    #[test]
    fn log_and_proxy_calls_share_gap_rule() {
        let c = db();
        let tz = chrono_tz::UTC;
        for (label, source) in [("pi", "opening"), ("pi-log", "log")] {
            let s = Scope {
                agent_label: label.into(),
                proxy_token_id: None,
                key_hash: "k".into(),
            };
            let a = append(&c, &s, source, 0, 60, None, tz).unwrap();
            let b = append(&c, &s, source, 30_000, 60, None, tz).unwrap();
            let d = append(&c, &s, source, 500_000, 60, None, tz).unwrap();
            assert_eq!(a.session_id, b.session_id);
            assert_ne!(a.session_id, d.session_id);
            assert_eq!((d.opened, d.closed), (1, 1));
        }
        let n: i64 = c
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 4);
    }
}
