//! @agents-index The summary worker: one pass over the due session texts of the proxied sessions, then the sleep.

use super::client::Client;
use super::store_session_day;
use crate::session::time;
use rusqlite::Connection;

/// What a pass needs. No client means no credential, so every row gets `disabled`.
pub struct Settings {
    pub data_key: [u8; 32],
    pub client: Option<Client>,
    pub public: Vec<String>,
    pub max_attempts: u32,
    /// The time zone that cuts the days of the aggregation queue.
    pub tz: chrono_tz::Tz,
    /// Below this count of conversation characters, the transcript also holds the clipped tool parts.
    pub min_conversation: usize,
}

/// The counts that `loupe summarize --once` prints: `segments_read` is the count of the pairs read.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub segments_read: u64,
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
    pub disabled: u64,
    pub retry_later: u64,
}

/// The runtime that sends the summary requests of one pass.
pub fn runtime() -> rusqlite::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

/// One pass over the session texts: discovery, then each due pair of a proxied session. The worker
/// sends no request for the line of a segment. The counts are of the session day summary rows.
/// A dry run counts the due pairs and writes nothing.
pub fn pass(conn: &Connection, s: &Settings, dry_run: bool) -> rusqlite::Result<Counts> {
    if dry_run {
        let now = time::text(chrono::Utc::now().timestamp_millis());
        let n = store_session_day::due(conn, &now)?
            .into_iter()
            .filter(|p| p.key_source != "log")
            .count();
        return Ok(Counts {
            segments_read: n as u64,
            ..Counts::default()
        });
    }
    let rt = runtime()?;
    let src = super::session_day::Proxied {
        conn,
        data_key: s.data_key,
    };
    let t = super::session_day::pass(conn, s, &rt, &src, None)?;
    Ok(Counts {
        segments_read: t.pairs_read,
        done: t.done,
        skipped: t.skipped,
        failed: t.failed,
        disabled: t.disabled,
        retry_later: t.retry_later,
    })
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

#[cfg(test)]
mod tests {
    use crate::config::loader::resolve;
    use std::collections::BTreeMap;

    fn settings(pairs: &[(&str, &str)]) -> super::Settings {
        let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
        let mut env: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        env.insert("LOUPE_DATA_KEY".into(), key.into());
        env.insert("LOUPE_ACCOUNT_KEY".into(), key.into());
        let cfg = resolve(&[], &env, &BTreeMap::new()).validate().unwrap();
        super::super::settings(&cfg)
    }

    #[test]
    fn summary_opt_in_rule() {
        assert!(settings(&[]).client.is_none(), "no provider, no client");
        let a = settings(&[("CLAUDE_CODE_OAUTH_TOKEN", "t")])
            .client
            .unwrap();
        assert_eq!(a.url(), "https://api.anthropic.com/v1/messages");
        let o = settings(&[
            ("OPENAI_API_KEY", "o"),
            ("LOUPE_SUMMARY_MODEL", "gpt-5-mini"),
        ])
        .client
        .unwrap();
        assert_eq!(o.url(), "https://api.openai.com/v1/chat/completions");
    }
}
