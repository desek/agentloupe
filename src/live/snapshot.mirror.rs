//! @agents-index The snapshot of the role public from the live_state row of the mirror, with the zero row and the 15 s stale rule.

use super::event::{Snapshot, Tokens};
use rusqlite::{Connection, OptionalExtension};
use rust_decimal::Decimal;

/// A row older than this, by its `updated_at`, shows `in_flight` 0: the role full stopped
/// without a clean shutdown, so its open exchanges will not end.
pub const STALE_MS: i64 = 15_000;

/// The row of the mirror with the stale rule applied, and the ended counter of the role full.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub snapshot: Snapshot,
    pub ended: i64,
}

/// Reads the one row, or None when the role full did not write it yet.
pub fn row(conn: &Connection, now_ms: i64) -> rusqlite::Result<Option<Row>> {
    conn.query_row(
        "SELECT date, calls, input, output, cache_read, cache_write, reasoning, cost_usd, \
         segments_open, ended, in_flight, updated_at FROM live_state LIMIT 1",
        [],
        |r| {
            let cost: String = r.get(7)?;
            let updated: i64 = r.get(11)?;
            let in_flight: i64 = r.get(10)?;
            Ok(Row {
                snapshot: Snapshot {
                    date: r.get(0)?,
                    calls: r.get(1)?,
                    tokens: Tokens {
                        input: r.get(2)?,
                        output: r.get(3)?,
                        cache_read: r.get(4)?,
                        cache_write: r.get(5)?,
                        reasoning: r.get(6)?,
                    },
                    cost_usd: cost.parse::<Decimal>().unwrap_or_default(),
                    segments_open: r.get(8)?,
                    in_flight: if now_ms - updated > STALE_MS {
                        0
                    } else {
                        in_flight.max(0)
                    },
                },
                ended: r.get(9)?,
            })
        },
    )
    .optional()
}

/// The first frame of a socket in the role public; zero counts on the current day in `tz` when
/// the row is absent.
pub fn read(conn: &Connection, tz: chrono_tz::Tz, now_ms: i64) -> rusqlite::Result<Snapshot> {
    Ok(match row(conn, now_ms)? {
        Some(r) => r.snapshot,
        None => Snapshot {
            date: crate::day::boundary::day_of(now_ms, tz)
                .format("%Y-%m-%d")
                .to_string(),
            ..Default::default()
        },
    })
}
