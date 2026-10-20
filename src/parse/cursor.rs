//! @agents-index The cursor of a background worker in worker_cursors: the last exchange rowid it processed.

use rusqlite::{Connection, OptionalExtension, params};

/// The name of the parser cursor.
pub const PARSER: &str = "parser";

/// The last processed exchange rowid, or 0 before the first.
pub fn get(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    Ok(conn
        .query_row(
            "SELECT last_id FROM worker_cursors WHERE name = ?1",
            [name],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

pub fn set(conn: &Connection, name: &str, last_id: i64, at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO worker_cursors (name, last_id, updated_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (name) DO UPDATE SET last_id = excluded.last_id, updated_at = excluded.updated_at",
        params![name, last_id, at],
    )
    .map(|_| ())
}
