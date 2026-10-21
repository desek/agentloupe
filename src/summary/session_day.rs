//! @agents-index The full session summary and the session day summary of each session and day: requests, status chain, rows.

use super::client::{self, Client, Outcome};
use super::store_session_day as store;
use super::transcript_proxied as proxied;
use super::{retry, rules, scrub, worker::Settings};
use crate::day::boundary;
use crate::session::time;
use rusqlite::Connection;

/// The most sentences of a full session summary.
const FULL_SENTENCES: usize = 8;
/// The most sentences of a session day summary.
const DAY_SENTENCES: usize = 4;

/// The UTC bounds of the day of a pair, as stored time text.
pub struct Window {
    pub start: String,
    pub end: String,
}

/// The transcript source of a pair. Each source gives messages; the one builder applies the rules.
pub trait Source {
    /// True when this source builds the transcripts of the pair.
    fn wants(&self, p: &store::Pair) -> bool;
    /// The messages of the whole session up to the end of the day.
    fn full(&self, p: &store::Pair, w: &Window) -> rusqlite::Result<Vec<serde_json::Value>>;
    /// The messages of the day only.
    fn day(&self, p: &store::Pair, w: &Window) -> rusqlite::Result<Vec<serde_json::Value>>;
}

/// The proxied source: the stored request bodies of every segment of the session.
pub struct Proxied<'a> {
    pub conn: &'a Connection,
    pub data_key: [u8; 32],
}

impl Source for Proxied<'_> {
    fn wants(&self, p: &store::Pair) -> bool {
        p.key_source != "log"
    }
    fn full(&self, p: &store::Pair, w: &Window) -> rusqlite::Result<Vec<serde_json::Value>> {
        proxied::full(self.conn, &self.data_key, &p.key, &p.day, &w.end)
    }
    fn day(&self, p: &store::Pair, w: &Window) -> rusqlite::Result<Vec<serde_json::Value>> {
        proxied::day(self.conn, &self.data_key, &p.key, &p.day, &w.start, &w.end)
    }
}

/// The counts of one pass, by the status of the session day summary rows it wrote.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Counts {
    pub pairs_read: u64,
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
    pub disabled: u64,
    pub retry_later: u64,
}

/// One written row: status, scrubbed text, error kind, next attempt, attempt count.
type Next = (
    String,
    Option<String>,
    Option<&'static str>,
    Option<String>,
    u32,
);

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The new row after one model request; a text under 2 words after the scrub is `skipped`.
fn after_request(out: Outcome, s: &Settings, attempts: u32, max: usize) -> Next {
    let attempts = attempts + 1;
    let (kind, retry_after) = match out {
        Outcome::Text(t) if client::is_skip(&t) => ("skip", None),
        Outcome::Text(t) => {
            let p = scrub::paragraph_max(&t, &s.public, max);
            return if p.split_whitespace().count() >= 2 {
                ("done".into(), Some(p), None, None, attempts)
            } else {
                ("skipped".into(), None, None, None, attempts)
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

fn due(st: &store::State, now: &str) -> bool {
    st.status == "pending" && st.next_attempt_at.as_deref().is_none_or(|t| t <= now)
}

/// Sends one request when the transcript has text; the transcript drops before the row is built.
fn request(
    c: &Client,
    s: &Settings,
    rt: &tokio::runtime::Runtime,
    rules_text: &str,
    user: String,
    attempts: u32,
    max: usize,
) -> Next {
    let system = rules::system(rules_text, &s.public);
    let out = rt.block_on(c.send(&system, &user, client::DAY_TOKENS));
    drop(user);
    after_request(out, s, attempts, max)
}

/// Writes one row, queues the day on a final status, and logs the allowed fields only.
fn write(
    conn: &Connection,
    s: &Settings,
    table: &str,
    p: &store::Pair,
    n: &Next,
    t0: std::time::Instant,
) -> rusqlite::Result<()> {
    let now = time::text(now_ms());
    let model = s.client.as_ref().map(|c| c.model.clone());
    let row = store::Row {
        status: &n.0,
        text: n.1.as_deref(),
        model: model.as_deref(),
        attempts: n.4,
        last_error_kind: n.2,
        next_attempt_at: n.3.clone(),
    };
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let r = store::put(conn, table, &p.key, &p.day, &row, &now).and_then(|_| {
        if table == store::DAY && n.0 != "pending" {
            crate::day::queue::put(conn, &p.day, crate::day::queue::SUMMARY_FINAL, &now)
        } else {
            Ok(())
        }
    });
    conn.execute_batch(if r.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
    r?;
    tracing::info!(session_key = %p.key, day = %p.day, table, status = %n.0, attempts = n.4, error_kind = n.2.unwrap_or(""), duration_ms = t0.elapsed().as_millis() as u64, "session text");
    Ok(())
}

/// Summarizes one pair: the full session summary first, then the session day summary with it as
/// context. Returns the status of the session day summary row, or None when the pair waits.
pub fn one(
    conn: &Connection,
    s: &Settings,
    rt: &tokio::runtime::Runtime,
    src: &dyn Source,
    p: &store::Pair,
) -> rusqlite::Result<Option<String>> {
    let Ok(date) = chrono::NaiveDate::parse_from_str(&p.day, "%Y-%m-%d") else {
        return Ok(None);
    };
    let (start, end) = boundary::bounds(date, s.tz);
    // The full session summary waits until every segment with a call on the day is closed.
    if store::open_on(conn, &p.key, &start, &end)? {
        return Ok(None);
    }
    let w = Window { start, end };
    let now = time::text(now_ms());
    let full = store::get(conn, store::FULL, &p.key, &p.day)?;
    let mut full = full.unwrap_or(store::State {
        status: "pending".into(),
        text: None,
        attempts: 0,
        next_attempt_at: None,
    });
    if due(&full, &now) {
        let t0 = std::time::Instant::now();
        let n: Next = match &s.client {
            None => ("disabled".into(), None, None, None, full.attempts),
            Some(c) => {
                let t = super::transcript::build_messages(&src.full(p, &w)?, s.min_conversation);
                if t.is_empty() {
                    ("skipped".into(), None, None, None, full.attempts)
                } else {
                    request(
                        c,
                        s,
                        rt,
                        rules::FULL_RULES,
                        rules::user(&t),
                        full.attempts,
                        FULL_SENTENCES,
                    )
                }
            }
        };
        write(conn, s, store::FULL, p, &n, t0)?;
        full.status = n.0.clone();
        full.text = n.1.clone();
    }
    if full.status == "pending" {
        return Ok(Some("pending".into()));
    }
    let Some(day) = store::get(conn, store::DAY, &p.key, &p.day)? else {
        return Ok(None);
    };
    if !due(&day, &now) {
        return Ok(Some(day.status));
    }
    let t0 = std::time::Instant::now();
    let n: Next = match (&s.client, full.status.as_str(), &full.text) {
        // A final full summary that is not done gives its status to the day row, with no request.
        (_, st, _) if st != "done" => (st.to_string(), None, None, None, day.attempts),
        (None, _, _) => ("disabled".into(), None, None, None, day.attempts),
        (Some(c), _, text) => {
            let t = super::transcript::build_messages(&src.day(p, &w)?, s.min_conversation);
            if t.is_empty() {
                ("skipped".into(), None, None, None, day.attempts)
            } else {
                let user = rules::user_with_context(text.as_deref().unwrap_or(""), &t);
                request(
                    c,
                    s,
                    rt,
                    rules::SESSION_DAY_RULES,
                    user,
                    day.attempts,
                    DAY_SENTENCES,
                )
            }
        }
    };
    write(conn, s, store::DAY, p, &n, t0)?;
    Ok(Some(n.0))
}

/// One pass over the due pairs that `src` wants, after the discovery of new pairs.
/// `label` limits the discovery to one agent label.
pub fn pass(
    conn: &Connection,
    s: &Settings,
    rt: &tokio::runtime::Runtime,
    src: &dyn Source,
    label: Option<&str>,
) -> rusqlite::Result<Counts> {
    let now = time::text(now_ms());
    store::discover(conn, s.tz, label, &now)?;
    let mut c = Counts::default();
    for p in store::due(conn, &now)?.into_iter().filter(|p| src.wants(p)) {
        if label.is_some_and(|l| l != p.agent_label) {
            continue;
        }
        c.pairs_read += 1;
        match one(conn, s, rt, src, &p)?.as_deref() {
            Some("done") => c.done += 1,
            Some("skipped") => c.skipped += 1,
            Some("failed") => c.failed += 1,
            Some("disabled") => c.disabled += 1,
            Some("pending") => c.retry_later += 1,
            _ => {}
        }
    }
    Ok(c)
}
