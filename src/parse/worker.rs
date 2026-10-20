//! @agents-index The parser worker: a pass over the exchanges after the cursor, one transaction per exchange, then the idle step.

use super::cursor::{self, PARSER};
use super::exchange::{self, content_type};
use super::format::{self, Exchange};
use super::outcome::Outcome;
use crate::capture::seal::open;
use crate::session::{idle, time};
use rusqlite::{Connection, params};

/// The most exchanges that one batch reads.
const BATCH: i64 = 100;

/// The keys and the idle time that a pass needs.
pub struct Settings {
    pub data_key: [u8; 32],
    pub account_key: [u8; 32],
    pub idle_secs: u64,
    /// Upstream name to its litellm_provider, from the allowlist file; selects the prices of a call.
    pub providers: std::collections::BTreeMap<String, String>,
}

/// The counts that `loupe parse` prints.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub exchanges_read: u64,
    pub calls_added: u64,
    pub sessions_opened: u64,
    pub sessions_closed: u64,
}

/// Starts a transaction, or a savepoint inside the outer transaction of a dry run.
/// IMMEDIATE takes the write lock first, so two parsers never both read the same cursor.
fn begin(conn: &Connection, nested: bool) -> rusqlite::Result<()> {
    conn.execute_batch(if nested {
        "SAVEPOINT exchange"
    } else {
        "BEGIN IMMEDIATE"
    })
}

fn end(conn: &Connection, nested: bool, ok: bool) -> rusqlite::Result<()> {
    conn.execute_batch(match (nested, ok) {
        (true, true) => "RELEASE exchange",
        (true, false) => "ROLLBACK TO exchange; RELEASE exchange",
        (false, true) => "COMMIT",
        (false, false) => "ROLLBACK",
    })
}

/// Runs `f` inside one transaction that commits on success and rolls back on error.
fn atomic<T>(
    conn: &Connection,
    nested: bool,
    f: impl FnOnce() -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    begin(conn, nested)?;
    match f() {
        Ok(v) => end(conn, nested, true).map(|_| v),
        Err(e) => {
            let _ = end(conn, nested, false);
            Err(e)
        }
    }
}

/// Parses the exchange at `rowid` and advances the cursor past it, in one transaction.
fn one(
    conn: &Connection,
    s: &Settings,
    rowid: i64,
    nested: bool,
    c: &mut Counts,
) -> rusqlite::Result<()> {
    atomic(conn, nested, || {
        if cursor::get(conn, PARSER)? >= rowid {
            return Ok(());
        }
        let row = exchange::read(conn, rowid)?;
        let bodies = open(&s.data_key, &row.id, "request", &row.request_body).and_then(|q| {
            Ok((
                q,
                open(&s.data_key, &row.id, "response", &row.response_body)?,
            ))
        });
        let (fmt, outcome, added) = match bodies {
            Err(_) => {
                tracing::error!(
                    exchange_id = %row.id,
                    outcome = "decrypt_failed",
                    "the parser cannot decrypt the bodies of this exchange. Fix: check that LOUPE_DATA_KEY is the key that sealed the stored rows. Verify: run loupe config show and compare the source of LOUPE_DATA_KEY"
                );
                ("unknown", Outcome::DecryptFailed, Default::default())
            }
            Ok((request, response)) => {
                let ct = content_type(&row.response_headers);
                let parsed = format::parse(&Exchange {
                    path: &row.path,
                    content_type: &ct,
                    response_truncated: row.response_truncated,
                    request: &request,
                    response: &response,
                });
                match parsed.result {
                    Ok(u) => (
                        parsed.format,
                        Outcome::Call,
                        super::call::store(conn, s, &row, &request, &u)?,
                    ),
                    Err(o) => (parsed.format, o, Default::default()),
                }
                // The plain bodies drop here, at the end of this arm.
            }
        };
        conn.execute(
            "UPDATE exchanges SET parse_outcome = ?2 WHERE rowid = ?1",
            params![rowid, outcome.as_str()],
        )?;
        cursor::set(
            conn,
            PARSER,
            rowid,
            &time::text(chrono::Utc::now().timestamp_millis()),
        )?;
        tracing::debug!(exchange_id = %row.id, format = fmt, outcome = outcome.as_str(), calls = added.calls, "exchange parsed");
        c.exchanges_read += 1;
        c.calls_added += added.calls;
        c.sessions_opened += added.opened;
        c.sessions_closed += added.closed;
        Ok(())
    })
}

fn batch(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let from = cursor::get(conn, PARSER)?;
    conn.prepare("SELECT rowid FROM exchanges WHERE rowid > ?1 ORDER BY rowid LIMIT ?2")?
        .query_map(params![from, BATCH], |r| r.get(0))?
        .collect()
}

fn run(conn: &Connection, s: &Settings, now_ms: i64, nested: bool) -> rusqlite::Result<Counts> {
    let mut c = Counts::default();
    loop {
        let ids = batch(conn)?;
        if ids.is_empty() {
            break;
        }
        for rowid in ids {
            one(conn, s, rowid, nested, &mut c)?;
        }
    }
    c.sessions_closed += atomic(conn, nested, || {
        let w = idle::watermark(conn, cursor::get(conn, PARSER)?, now_ms)?;
        idle::close_idle(conn, s.idle_secs, w)
    })?;
    Ok(c)
}

/// One parse pass and one idle step. A dry run does the same work inside one transaction and rolls it back.
pub fn pass(
    conn: &Connection,
    s: &Settings,
    now_ms: i64,
    dry_run: bool,
) -> rusqlite::Result<Counts> {
    if !dry_run {
        return run(conn, s, now_ms, false);
    }
    conn.execute_batch("BEGIN")?;
    let r = run(conn, s, now_ms, true);
    conn.execute_batch("ROLLBACK")?;
    r
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
            match pass(&conn, &s, chrono::Utc::now().timestamp_millis(), false) {
                Ok(c) if c.exchanges_read > 0 || c.sessions_closed > 0 => tracing::info!(
                    exchanges_read = c.exchanges_read,
                    calls_added = c.calls_added,
                    sessions_opened = c.sessions_opened,
                    sessions_closed = c.sessions_closed,
                    "parse pass done"
                ),
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "the parse pass failed; the next pass retries. Fix: check the database file and the disk space. Verify: loupe parse --once exits 0")
                }
            }
            std::thread::sleep(interval);
        }
    })
}
