//! @agents-index Summarizes the due backfilled sessions through the shared summary path, with transcripts built again from the log.

use super::run::{Counts, Outcome};
use super::{Source, transcript};
use crate::session::time;
use crate::summary::session_day::{self, Window};
use crate::summary::store_session_day::Pair;
use crate::summary::worker;
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

/// The session texts of the due backfilled pairs of `source` whose log files the run found; no
/// request for the line of a segment. The summary counters are of the session day summary rows.
pub fn pass(
    conn: &Connection,
    ws: &worker::Settings,
    source: Source,
    k: &[u8; 32],
    o: &Outcome,
    c: &mut Counts,
) -> rusqlite::Result<()> {
    let rt = worker::runtime()?;
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
    c.summaries_done += t.done;
    c.summaries_skipped += t.skipped;
    c.summaries_retry_later += t.retry_later;
    c.summaries_failed += t.failed;
    c.summaries_disabled += t.disabled;
    Ok(())
}
