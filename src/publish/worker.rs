//! @agents-index The publisher worker: on each wake, writes each due day with activity and no row, then builds again each queued due day.

use super::clock::Clock;
use super::due;
use crate::day::activity::{self, Source};
use crate::day::rebuild::{self, Ctx};
use crate::day::{queue, state, store};
use crate::session::time;
use chrono::{NaiveDate, NaiveTime};
use rusqlite::Connection;

/// What the worker needs.
pub struct Settings {
    pub ctx: Ctx,
    pub publish_at: NaiveTime,
    pub sources: Vec<Box<dyn Source>>,
}

/// The counts of one wake.
#[derive(Debug, Default, PartialEq)]
pub struct Counts {
    pub published: u64,
    pub updated: u64,
}

fn log(b: &rebuild::Built, reason: &str, what: &str) {
    tracing::info!(
        day = %b.record.day,
        revision = b.revision,
        reason,
        calls = b.record.calls,
        sessions = b.record.sessions,
        unpriced_calls = b.record.unpriced_calls,
        "{what}"
    );
}

/// One wake. Days are written oldest first; a day with a row is never created again, so a
/// restart repeats no work and a tombstone stays.
pub fn wake(conn: &Connection, s: &Settings, clock: &dyn Clock) -> rusqlite::Result<Counts> {
    let now_ms = clock.now_ms();
    let now = time::text(now_ms);
    let tz = s.ctx.tz;
    let mut c = Counts::default();
    let rows = store::days_with_row(conn)?;
    let mut day = activity::first(&s.sources, conn, tz)?;
    while let Some(d) = day {
        if !due::is_due(d, now_ms, tz, s.publish_at) {
            break;
        }
        if !rows.contains(&d.to_string()) && activity::any(&s.sources, conn, d, tz)? {
            let b = rebuild::rebuild(conn, &s.ctx, d, &now, true, false)?;
            log(&b, "due", "day published");
            c.published += 1;
        }
        day = d.succ_opt();
    }
    for (key, reason) in queue::list(conn)? {
        let Ok(d) = NaiveDate::parse_from_str(&key, "%Y-%m-%d") else {
            queue::delete(conn, &key)?;
            continue;
        };
        if !due::is_due(d, now_ms, tz, s.publish_at) {
            continue;
        }
        match store::read(conn, &key)? {
            Some(row) if row.state != state::DELETED => {
                let b = rebuild::rebuild(conn, &s.ctx, d, &now, true, false)?;
                if b.written {
                    log(&b, &reason, "day updated");
                    c.updated += 1;
                }
            }
            // A tombstone, or a due day with no activity: nothing to build.
            _ => queue::delete(conn, &key)?,
        }
    }
    Ok(c)
}

/// Starts the worker thread of `loupe serve`: a wake at start, then one per `interval`.
/// A failed wake logs one line and the next wake retries, so the worker never stops.
pub fn start(
    conn: Connection,
    s: Settings,
    clock: Box<dyn Clock>,
    interval: std::time::Duration,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            if let Err(e) = wake(&conn, &s, clock.as_ref()) {
                tracing::warn!(error = %e, "the publisher wake failed; the next wake retries. Fix: check the database file and the disk space. Verify: loupe aggregate --date YYYY-MM-DD exits 0")
            }
            std::thread::sleep(interval);
        }
    })
}
