//! @agents-index Summarizes the due backfilled sessions and segments through the shared summary path, with transcripts built again from the log.

use super::run::{Counts, Outcome};
use super::{Source, transcript};
use crate::session::time;
use crate::summary::session_day::{self, Window};
use crate::summary::store_session_day::Pair;
use crate::summary::{store, worker};
use rusqlite::Connection;
use serde_json::Value;

/// The log source of the session texts: the files of the run that hold the session key.
struct Log<'a> {
    source: Source,
    k: &'a [u8; 32],
    o: &'a Outcome,
}

impl Log<'_> {
    fn read(&self, p: &Pair, after: Option<i64>, until: i64) -> Vec<Value> {
        let Some(idx) = self.o.index.get(&p.key) else {
            return vec![];
        };
        let files: Vec<_> = idx.iter().map(|i| &self.o.files[*i]).collect();
        transcript::messages(self.source, self.k, &files, &p.key, after, until)
    }
}

impl session_day::Source for Log<'_> {
    fn wants(&self, p: &Pair) -> bool {
        p.key_source == "log" && self.o.index.contains_key(&p.key)
    }
    fn full(&self, p: &Pair, w: &Window) -> rusqlite::Result<Vec<Value>> {
        Ok(self.read(p, None, time::ms(&w.end) - 1))
    }
    fn day(&self, p: &Pair, w: &Window) -> rusqlite::Result<Vec<Value>> {
        Ok(self.read(p, Some(time::ms(&w.start) - 1), time::ms(&w.end) - 1))
    }
}

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
    // The session texts come first: their discovery reads the segments that have no final line yet.
    let src = Log { source, k, o };
    let t = session_day::pass(conn, ws, &rt, &src, Some(source.name()))?;
    tracing::info!(
        pairs_read = t.pairs_read,
        done = t.done,
        skipped = t.skipped,
        failed = t.failed,
        disabled = t.disabled,
        retry_later = t.retry_later,
        "session text pass done"
    );
    let now = time::text(chrono::Utc::now().timestamp_millis());
    for d in store::due_log(conn, &now, source.name())? {
        let Some(idx) = o.index.get(&d.key_hash) else {
            continue;
        };
        let files: Vec<_> = idx.iter().map(|i| &o.files[*i]).collect();
        let after = d.after.as_deref().map(time::ms);
        let until = time::ms(&d.last_call_at);
        let status = worker::summarize(conn, ws, &rt, d.id, false, || {
            let m = transcript::messages(source, k, &files, &d.key_hash, after, until);
            Ok(crate::summary::transcript::build_messages(
                &m,
                ws.min_conversation,
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
