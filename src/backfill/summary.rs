//! @agents-index Summarizes the due backfilled segments through the shared summary path, with the transcript built again from the log.

use super::run::{Counts, Outcome};
use super::{Source, transcript};
use crate::session::time;
use crate::summary::{store, worker};
use rusqlite::Connection;

/// One summary request for each due backfilled segment of `source` whose log files the run found.
/// The transcript exists only in memory for the request; only the scrubbed line is stored.
pub fn pass(
    conn: &Connection,
    ws: &worker::Settings,
    source: Source,
    k: &[u8; 32],
    o: &Outcome,
    c: &mut Counts,
) -> rusqlite::Result<()> {
    let rt = worker::runtime()?;
    let now = time::text(chrono::Utc::now().timestamp_millis());
    for d in store::due_log(conn, &now, source.name())? {
        let Some(idx) = o.index.get(&d.key_hash) else {
            continue;
        };
        let files: Vec<_> = idx.iter().map(|i| &o.files[*i]).collect();
        let after = d.after.as_deref().map(time::ms);
        let until = time::ms(&d.last_call_at);
        let status = worker::summarize(conn, ws, &rt, d.id, false, || {
            Ok(transcript::segment(
                source,
                k,
                &files,
                &d.key_hash,
                after,
                until,
            ))
        })?;
        match status.as_str() {
            "done" => c.summaries_done += 1,
            "skipped" => c.summaries_skipped += 1,
            "pending" => c.summaries_retry_later += 1,
            "failed" => c.summaries_failed += 1,
            "disabled" => c.summaries_disabled += 1,
            _ => {}
        }
    }
    Ok(())
}
