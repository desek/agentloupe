//! @agents-index The retention worker: sets the bodies of parsed and summarized exchanges to null after LOUPE_RAW_RETENTION_HOURS.

use crate::session::time;
use rusqlite::{Connection, params};

/// The time between two retention passes.
pub const INTERVAL: std::time::Duration = std::time::Duration::from_secs(600);

/// The counts that `loupe summarize --once` prints for the retention pass.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub bodies_deleted: u64,
    pub bodies_kept_decrypt_failed: u64,
}

/// The exchanges whose bodies can go: parsed, not decrypt_failed, every call in a segment with a final
/// summary, and the later of the parse time and the summary time at or before the cutoff.
const ELIGIBLE: &str = "FROM exchanges e WHERE e.bodies_deleted_at IS NULL AND e.parse_outcome IS NOT NULL AND e.parse_outcome != 'decrypt_failed' \
    AND NOT EXISTS (SELECT 1 FROM calls c LEFT JOIN session_summaries ss ON ss.session_id = c.session_id \
        WHERE c.exchange_id = e.id AND (ss.status IS NULL OR ss.status = 'pending')) \
    AND max(coalesce(e.parsed_at, ''), coalesce((SELECT max(ss.updated_at) FROM calls c JOIN session_summaries ss ON ss.session_id = c.session_id WHERE c.exchange_id = e.id), '')) <= ?1";

/// One retention pass at `now_ms`. Keeps every other column; calls and sessions are never touched.
pub fn pass(conn: &Connection, hours: u64, now_ms: i64, dry_run: bool) -> rusqlite::Result<Counts> {
    let cutoff = time::text(now_ms - (hours as i64) * 3_600_000);
    let now = time::text(now_ms);
    let deleted = if dry_run {
        conn.query_row(
            &format!("SELECT count(*) {ELIGIBLE}"),
            params![cutoff],
            |r| r.get::<_, i64>(0),
        )? as u64
    } else {
        conn.execute(
            &format!("UPDATE exchanges SET request_body = NULL, response_body = NULL, bodies_deleted_at = ?2 WHERE rowid IN (SELECT e.rowid {ELIGIBLE})"),
            params![cutoff, now],
        )? as u64
    };
    let kept = conn.query_row(
        "SELECT count(*) FROM exchanges WHERE parse_outcome = 'decrypt_failed' AND bodies_deleted_at IS NULL",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    Ok(Counts {
        bodies_deleted: deleted,
        bodies_kept_decrypt_failed: kept,
    })
}

/// Starts the worker thread of `loupe serve`: a pass every 10 minutes until the process ends.
pub fn start(conn: Connection, hours: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            match pass(&conn, hours, chrono::Utc::now().timestamp_millis(), false) {
                Ok(c) if c.bodies_deleted > 0 => tracing::info!(
                    bodies_deleted = c.bodies_deleted,
                    bodies_kept_decrypt_failed = c.bodies_kept_decrypt_failed,
                    "retention pass done"
                ),
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "the retention pass failed; the next pass retries. Fix: check the database file and the disk space. Verify: loupe summarize --once prints bodies_deleted")
                }
            }
            std::thread::sleep(INTERVAL);
        }
    })
}
