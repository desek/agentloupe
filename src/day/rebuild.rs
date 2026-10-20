//! @agents-index Builds one day and writes it: asks for the day line, then builds, upserts, and updates the queue in one transaction.

use super::record::{Record, content_hash};
use super::state::Review;
use super::{build, queue, store};
use crate::subscription::config::Subscription;
use crate::summary::day as line;
use chrono::NaiveDate;
use chrono_tz::Tz;
use rusqlite::Connection;

/// What a build of a day needs. `summary` gives the day line its client; no client gives a
/// fallback line and sends nothing.
pub struct Ctx {
    pub tz: Tz,
    pub subscriptions: Vec<Subscription>,
    pub review: Review,
    pub summary: crate::summary::worker::Settings,
    /// The GitHub token labels; empty when no token is set.
    pub github_labels: Vec<String>,
}

impl Ctx {
    /// The context of `loupe serve`, `loupe aggregate`, and `loupe day` from the configuration.
    pub fn from_config(cfg: &crate::config::Config) -> Ctx {
        Ctx {
            tz: cfg.summary.tz,
            subscriptions: cfg.days.subscriptions.clone(),
            review: cfg.days.review,
            summary: crate::summary::settings(cfg),
            github_labels: cfg.github.tokens.iter().map(|(l, _)| l.clone()).collect(),
        }
    }
}

/// The result of one build.
pub struct Built {
    pub record: Record,
    pub revision: i64,
    pub written: bool,
    pub state: Option<String>,
    pub not_ready: bool,
}

/// Asks the summary area to build the day line; true when it is not ready.
fn day_line(conn: &Connection, ctx: &Ctx, day: NaiveDate) -> rusqlite::Result<bool> {
    let s = line::Settings {
        client: ctx.summary.client.as_ref(),
        public: &ctx.summary.public,
        max_attempts: ctx.summary.max_attempts,
        tz: ctx.tz,
    };
    Ok(line::build_day_line(conn, &s, day, false)?.status == "not_ready")
}

fn atomic<T>(conn: &Connection, f: impl FnOnce() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f() {
        Ok(v) => conn.execute_batch("COMMIT").map(|_| v),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Builds `day` and writes it with the upsert rule. `consume` deletes the queue row in the same
/// transaction, unless the day line is not ready, which puts the day back. A dry run reads the
/// day line as it is and writes nothing.
pub fn rebuild(
    conn: &Connection,
    ctx: &Ctx,
    day: NaiveDate,
    now: &str,
    consume: bool,
    dry_run: bool,
) -> rusqlite::Result<Built> {
    let key = day.to_string();
    if dry_run {
        let record = build::build_day(conn, day, ctx.tz, &ctx.subscriptions, &ctx.github_labels)?;
        let row = store::read(conn, &key)?;
        return Ok(Built {
            record,
            revision: row.as_ref().map_or(0, |r| r.revision),
            written: false,
            state: row.map(|r| r.state),
            not_ready: false,
        });
    }
    let not_ready = day_line(conn, ctx, day)?;
    atomic(conn, || {
        let record = build::build_day(conn, day, ctx.tz, &ctx.subscriptions, &ctx.github_labels)?;
        let json = record.json();
        let up = store::upsert(
            conn,
            &key,
            &json,
            &content_hash(&json),
            now,
            ctx.review.first_state(),
        )?;
        if not_ready {
            queue::put(conn, &key, queue::SUMMARY_NOT_READY, now)?;
        } else if consume {
            queue::delete(conn, &key)?;
        }
        Ok(Built {
            record,
            revision: up.revision,
            written: up.written,
            state: Some(up.state),
            not_ready,
        })
    })
}
