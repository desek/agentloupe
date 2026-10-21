//! @agents-index The summary worker: one pass over the due closed segments, a scrubbed line or a status for each, then the sleep.

use super::client::{self, Client, Outcome};
use super::{body, retry, rules, scrub, store};
use crate::session::time;
use rusqlite::Connection;

/// What a pass needs. No client means no credential, so every segment gets `disabled`.
pub struct Settings {
    pub data_key: [u8; 32],
    pub client: Option<Client>,
    pub public: Vec<String>,
    pub max_attempts: u32,
    /// The time zone that cuts the days of the aggregation queue.
    pub tz: chrono_tz::Tz,
}

/// The counts that `loupe summarize --once` prints.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub segments_read: u64,
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
    pub disabled: u64,
    pub retry_later: u64,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The new row of a segment after one model request.
fn after_request(
    out: Outcome,
    s: &Settings,
    attempts: u32,
) -> (
    String,
    Option<String>,
    Option<&'static str>,
    Option<String>,
    u32,
) {
    let attempts = attempts + 1;
    let (kind, retry_after) = match out {
        Outcome::Text(t) if client::is_skip(&t) => ("skip", None),
        Outcome::Text(t) => {
            return match scrub::line(&t, &s.public) {
                Some(l) => ("done".into(), Some(l), None, None, attempts),
                None => ("skipped".into(), None, None, None, attempts),
            };
        }
        Outcome::Fatal { kind } => return ("failed".into(), None, Some(kind), None, attempts),
        Outcome::Retry { kind, retry_after } => (kind, retry_after),
    };
    if attempts >= s.max_attempts {
        return ("failed".into(), None, Some(kind), None, attempts);
    }
    let next = now_ms() + 1000 * retry::delay_secs(attempts, retry_after) as i64;
    (
        "pending".into(),
        None,
        Some(kind),
        Some(time::text(next)),
        attempts,
    )
}

/// Summarizes one proxied segment from its request bodies.
fn one(
    conn: &Connection,
    s: &Settings,
    rt: &tokio::runtime::Runtime,
    id: i64,
    dry_run: bool,
) -> rusqlite::Result<String> {
    summarize(conn, s, rt, id, dry_run, || {
        body::load(conn, &s.data_key, id)
    })
}

/// The runtime that sends the summary requests of one pass.
pub fn runtime() -> rusqlite::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

/// Summarizes one segment and writes its row; returns the new status. A dry run sends nothing.
/// `load` gives the transcript; it is the only part that differs between a proxied and a backfilled segment.
pub fn summarize(
    conn: &Connection,
    s: &Settings,
    rt: &tokio::runtime::Runtime,
    id: i64,
    dry_run: bool,
    load: impl FnOnce() -> rusqlite::Result<String>,
) -> rusqlite::Result<String> {
    let t0 = std::time::Instant::now();
    let attempts = store::attempts(conn, id)?;
    let model = s.client.as_ref().map(|c| c.model.clone());
    let (status, line, kind, next, attempts) = match &s.client {
        None => ("disabled".to_string(), None, None, None, attempts),
        Some(c) => {
            let transcript = load()?;
            if transcript.is_empty() {
                ("skipped".to_string(), None, None, None, attempts)
            } else if dry_run {
                return Ok("would_request".into());
            } else {
                let system = rules::system(rules::SESSION_RULES, &s.public);
                let out =
                    rt.block_on(c.send(&system, &rules::user(&transcript), client::LINE_TOKENS));
                // The transcript drops here; only the scrubbed line is kept.
                drop(transcript);
                after_request(out, s, attempts)
            }
        }
    };
    if !dry_run {
        let row = store::Row {
            status: &status,
            line: line.as_deref(),
            model: model.as_deref(),
            attempts,
            last_error_kind: kind,
            next_attempt_at: next,
        };
        let now = time::text(now_ms());
        // A final status and the queue write of the days of the segment share one transaction.
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let r = store::put(conn, id, &row, &now).and_then(|_| {
            if status == "pending" {
                return Ok(());
            }
            crate::day::queue::put_session_days(
                conn,
                id,
                s.tz,
                crate::day::queue::SUMMARY_FINAL,
                &now,
            )
        });
        conn.execute_batch(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        r?;
    }
    tracing::info!(session_id = id, status = %status, attempts, error_kind = kind.unwrap_or(""), duration_ms = t0.elapsed().as_millis() as u64, "session summary");
    Ok(status)
}

/// One pass over every due segment, in batches of 20. A dry run reads all due segments once and writes nothing.
pub fn pass(conn: &Connection, s: &Settings, dry_run: bool) -> rusqlite::Result<Counts> {
    let rt = runtime()?;
    let mut c = Counts::default();
    let mut seen = std::collections::HashSet::new();
    loop {
        let limit = if dry_run { -1 } else { store::BATCH };
        let ids: Vec<i64> = store::due(conn, &time::text(now_ms()), limit)?
            .into_iter()
            .filter(|id| seen.insert(*id))
            .collect();
        if ids.is_empty() {
            break;
        }
        for id in ids {
            c.segments_read += 1;
            match one(conn, s, &rt, id, dry_run)?.as_str() {
                "done" => c.done += 1,
                "skipped" => c.skipped += 1,
                "failed" => c.failed += 1,
                "disabled" => c.disabled += 1,
                "pending" => c.retry_later += 1,
                _ => {}
            }
        }
        if dry_run {
            break;
        }
    }
    Ok(c)
}

/// Starts the worker thread of `loupe serve`: a pass, then a sleep of `interval`, until the process ends.
/// A failed pass logs one line and the next pass retries, so the worker never stops.
pub fn start(
    conn: Connection,
    s: Settings,
    interval: std::time::Duration,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            match pass(&conn, &s, false) {
                Ok(c) if c.segments_read > 0 => tracing::info!(
                    segments_read = c.segments_read,
                    done = c.done,
                    skipped = c.skipped,
                    failed = c.failed,
                    disabled = c.disabled,
                    retry_later = c.retry_later,
                    "summary pass done"
                ),
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "the summary pass failed; the next pass retries. Fix: check the database file and the disk space. Verify: loupe summarize --once exits 0")
                }
            }
            std::thread::sleep(interval);
        }
    })
}
