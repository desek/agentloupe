//! @agents-index The live_state row of the mirror: the live snapshot totals plus the hub values, and its write.

use crate::live::hub::Hub;
use crate::live::snapshot;
use rusqlite::{Connection, params};

/// The values of the row, without `updated_at`, so two rows compare by content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub snapshot: crate::live::event::Snapshot,
    pub ended: i64,
}

/// The totals with the day rule of the live snapshot, and the in-flight count and the ended
/// counter of the hub (0 when the hub is off).
pub fn read(src: &Connection, tz: chrono_tz::Tz, now_ms: i64, hub: &Hub) -> rusqlite::Result<Row> {
    Ok(Row {
        snapshot: snapshot::read(src, tz, now_ms, hub.in_flight())?,
        ended: hub.ended(),
    })
}

/// Replaces the one row in one transaction.
pub fn write(mirror: &mut Connection, row: &Row, now_ms: i64) -> rusqlite::Result<()> {
    let s = &row.snapshot;
    let tx = mirror.transaction()?;
    tx.execute("DELETE FROM live_state", [])?;
    tx.execute(
        "INSERT INTO live_state (date, calls, input, output, cache_read, cache_write, reasoning, cost_usd, segments_open, ended, in_flight, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            s.date,
            s.calls,
            s.tokens.input,
            s.tokens.output,
            s.tokens.cache_read,
            s.tokens.cache_write,
            s.tokens.reasoning,
            s.cost_usd.to_string(),
            s.segments_open,
            row.ended,
            s.in_flight,
            now_ms
        ],
    )?;
    tx.commit()
}

/// Sets `in_flight` 0 at a clean shutdown, so a reader does not show exchanges that will not end.
pub fn clear_in_flight(mirror: &Connection, now_ms: i64) -> rusqlite::Result<()> {
    mirror.execute(
        "UPDATE live_state SET in_flight = 0, updated_at = ?1",
        [now_ms],
    )?;
    Ok(())
}
