//! @agents-index The retention worker: nulls the bodies of parsed, not held exchanges with final session texts after LOUPE_RAW_RETENTION_HOURS; logs bodies_held.

use crate::session::time;
use rusqlite::{Connection, params};

/// The time between two retention passes.
pub const INTERVAL: std::time::Duration = std::time::Duration::from_secs(600);

/// The counts that `loupe summarize --once` prints for the retention pass.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub bodies_deleted: u64,
    pub bodies_kept_decrypt_failed: u64,
    /// The exchanges whose bodies stay because a session text of their session is not final.
    pub bodies_held: u64,
}

/// The exchanges with a call whose session still needs the whole history: a segment of the session is
/// open, or a session day summary of the session is pending. The proxied transcript reads these bodies.
const HELD: &str = "EXISTS (SELECT 1 FROM calls c JOIN sessions s ON s.id = c.session_id WHERE c.exchange_id = e.id AND ( \
        EXISTS (SELECT 1 FROM sessions o WHERE o.key_hash = s.key_hash AND o.state = 'open') \
        OR EXISTS (SELECT 1 FROM session_day_summaries r WHERE r.session_key = s.key_hash AND r.status = 'pending')))";

/// The exchanges whose bodies can still go: parsed, not decrypt_failed, bodies present.
const LIVE: &str = "FROM exchanges e WHERE e.bodies_deleted_at IS NULL AND e.parse_outcome IS NOT NULL AND e.parse_outcome != 'decrypt_failed'";

/// The exchanges whose bodies can go: live, not held, and the later of the parse time and the summary
/// time at or before the cutoff. The summary time is the latest update of the session day summaries
/// of the session of each call. A call with no session day summary row holds the bodies: its pairs
/// are not discovered yet.
const ELIGIBLE: &str = "FROM exchanges e WHERE e.bodies_deleted_at IS NULL AND e.parse_outcome IS NOT NULL AND e.parse_outcome != 'decrypt_failed' \
    AND NOT {HELD} AND NOT EXISTS (SELECT 1 FROM calls c JOIN sessions s ON s.id = c.session_id WHERE c.exchange_id = e.id \
        AND NOT EXISTS (SELECT 1 FROM session_day_summaries r WHERE r.session_key = s.key_hash)) \
    AND max(coalesce(e.parsed_at, ''), coalesce((SELECT max(r.updated_at) FROM calls c JOIN sessions s ON s.id = c.session_id \
        JOIN session_day_summaries r ON r.session_key = s.key_hash WHERE c.exchange_id = e.id), '')) <= ?1";

/// One retention pass at `now_ms`. Keeps every other column; calls and sessions are never touched.
pub fn pass(conn: &Connection, hours: u64, now_ms: i64, dry_run: bool) -> rusqlite::Result<Counts> {
    let eligible = ELIGIBLE.replace("{HELD}", HELD);
    let cutoff = time::text(now_ms - (hours as i64) * 3_600_000);
    let now = time::text(now_ms);
    let deleted = if dry_run {
        conn.query_row(
            &format!("SELECT count(*) {eligible}"),
            params![cutoff],
            |r| r.get::<_, i64>(0),
        )? as u64
    } else {
        conn.execute(
            &format!("UPDATE exchanges SET request_body = NULL, response_body = NULL, bodies_deleted_at = ?2 WHERE rowid IN (SELECT e.rowid {eligible})"),
            params![cutoff, now],
        )? as u64
    };
    let kept = conn.query_row(
        "SELECT count(*) FROM exchanges WHERE parse_outcome = 'decrypt_failed' AND bodies_deleted_at IS NULL",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    let held = conn.query_row(&format!("SELECT count(*) {LIVE} AND {HELD}"), [], |r| {
        r.get::<_, i64>(0)
    })? as u64;
    tracing::info!(
        bodies_deleted = deleted,
        bodies_kept_decrypt_failed = kept,
        bodies_held = held,
        dry_run,
        "retention pass done"
    );
    Ok(Counts {
        bodies_deleted: deleted,
        bodies_kept_decrypt_failed: kept,
        bodies_held: held,
    })
}

/// Starts the worker thread of `loupe serve`: a pass every 10 minutes until the process ends.
pub fn start(conn: Connection, hours: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            // The pass logs its own counters, bodies_held included, on each run.
            match pass(&conn, hours, chrono::Utc::now().timestamp_millis(), false) {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "the retention pass failed; the next pass retries. Fix: check the database file and the disk space. Verify: loupe summarize --once prints bodies_deleted")
                }
            }
            std::thread::sleep(INTERVAL);
        }
    })
}
