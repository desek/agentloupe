//! @agents-index build_day_line: combines the done session day summaries of one day into a scrubbed paragraph, or a fallback, in day_summaries.

use super::client::{self, Client, Outcome};
use super::{retry, rules, scrub, store, store_session_day};
use crate::session::time;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;
use rusqlite::Connection;

/// The most session lines in a fallback paragraph.
const MAX_FALLBACK: usize = 8;
/// The fewest sentences of a model paragraph that is kept.
const MIN_SENTENCES: usize = 3;

/// What a day build needs.
pub struct Settings<'a> {
    pub client: Option<&'a Client>,
    pub public: &'a [String],
    pub max_attempts: u32,
    pub tz: Tz,
}

/// The result of one build; `line` is the stored text.
#[derive(Debug)]
pub struct DayResult {
    pub status: &'static str,
    pub session_lines: usize,
    pub line: Option<String>,
}

/// The UTC bounds of a calendar day in `tz`, as stored time text.
fn bounds(day: NaiveDate, tz: Tz) -> (String, String) {
    let at = |d: NaiveDate| {
        let local = d.and_hms_opt(0, 0, 0).expect("midnight exists");
        tz.from_local_datetime(&local)
            .earliest()
            .map_or(0, |t| t.timestamp_millis())
    };
    let next = day.succ_opt().unwrap_or(day);
    (time::text(at(day)), time::text(at(next)))
}

/// The session day summaries as sentences, most calls first, at most 8.
fn fallback(lines: &[String]) -> String {
    lines
        .iter()
        .take(MAX_FALLBACK)
        .map(|l| {
            let mut c = l.chars();
            let head = c
                .next()
                .map(|h| h.to_uppercase().collect::<String>())
                .unwrap_or_default();
            format!("{head}{}.", c.as_str().trim_end_matches('.'))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The combine request with the retries in line: a retryable failure waits the backoff and tries again.
fn combine(c: &Client, s: &Settings, lines: &[String]) -> Option<String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    let system = rules::system(rules::DAY_RULES, s.public);
    let user = rules::user(&lines.join("\n"));
    for attempt in 1..=s.max_attempts {
        let (kind, retry_after) = match rt.block_on(c.send(&system, &user, client::DAY_TOKENS)) {
            Outcome::Text(t) if !client::is_skip(&t) => return Some(t),
            Outcome::Text(_) => ("skip", None),
            Outcome::Fatal { kind } => {
                tracing::info!(
                    attempts = attempt,
                    error_kind = kind,
                    "day paragraph request failed"
                );
                return None;
            }
            Outcome::Retry { kind, retry_after } => (kind, retry_after),
        };
        tracing::info!(
            attempts = attempt,
            error_kind = kind,
            "day paragraph request will retry"
        );
        if attempt < s.max_attempts {
            std::thread::sleep(std::time::Duration::from_secs(retry::delay_secs(
                attempt,
                retry_after,
            )));
        }
    }
    None
}

/// Builds the line of `day` from its done session day summaries. Returns `not_ready` and writes nothing
/// while a segment with a call on the day is open or a session day summary of the day is pending.
/// A dry run sends no request and writes nothing; its status is `ready` for a buildable day.
pub fn build_day_line(
    conn: &Connection,
    s: &Settings,
    day: NaiveDate,
    dry_run: bool,
) -> rusqlite::Result<DayResult> {
    let (from, to) = bounds(day, s.tz);
    let rows = store_session_day::day_texts(conn, &day.to_string(), &from, &to)?;
    let blocked = store_session_day::any_open(conn, &from, &to)?
        || rows.iter().any(|r| r.status == "pending");
    let lines: Vec<String> = rows
        .iter()
        .filter(|r| r.status == "done")
        .filter_map(|r| r.text.clone())
        .collect();
    let n = lines.len();
    let result = |status, line| DayResult {
        status,
        session_lines: n,
        line,
    };
    if blocked {
        return Ok(result("not_ready", None));
    }
    if dry_run {
        return Ok(result(if n == 0 { "empty" } else { "ready" }, None));
    }
    let r = if n == 0 {
        result("empty", None)
    } else {
        let para = s
            .client
            .and_then(|c| combine(c, s, &lines))
            .map(|t| scrub::paragraph(&t, s.public))
            .filter(|p| scrub::sentence_count(p) >= MIN_SENTENCES);
        match para {
            Some(p) => result("done", Some(p)),
            None => result(
                "fallback",
                Some(scrub::paragraph(&fallback(&lines), s.public)),
            ),
        }
    };
    store::put_day(
        conn,
        &day.to_string(),
        r.status,
        r.line.as_deref(),
        n,
        &time::text(chrono::Utc::now().timestamp_millis()),
    )?;
    tracing::info!(day = %day, status = r.status, session_lines = n, "day line built");
    Ok(r)
}
