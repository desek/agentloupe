//! @agents-index SQL of day_overrides: the edited summary and the deletion mark of the operator, which survive each rebuild.

use rusqlite::{Connection, OptionalExtension, params};

/// The override of one day.
pub struct Override {
    pub summary: Option<String>,
    pub deleted_at: Option<String>,
}

pub fn read(conn: &Connection, day: &str) -> rusqlite::Result<Option<Override>> {
    conn.query_row(
        "SELECT summary, deleted_at FROM day_overrides WHERE day = ?1",
        [day],
        |r| {
            Ok(Override {
                summary: r.get(0)?,
                deleted_at: r.get(1)?,
            })
        },
    )
    .optional()
}

pub fn set_summary(conn: &Connection, day: &str, text: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO day_overrides (day, summary, deleted_at, updated_at) VALUES (?1, ?2, NULL, ?3) \
         ON CONFLICT (day) DO UPDATE SET summary = ?2, updated_at = ?3",
        params![day, text, now],
    )
    .map(|_| ())
}

/// Sets the deletion time; a second delete keeps the first time.
pub fn set_deleted(conn: &Connection, day: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO day_overrides (day, summary, deleted_at, updated_at) VALUES (?1, NULL, ?2, ?2) \
         ON CONFLICT (day) DO UPDATE SET deleted_at = coalesce(deleted_at, ?2), updated_at = ?2",
        params![day, now],
    )
    .map(|_| ())
}

pub fn clear_deleted(conn: &Connection, day: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE day_overrides SET deleted_at = NULL, updated_at = ?2 WHERE day = ?1 AND deleted_at IS NOT NULL",
        params![day, now],
    )
    .map(|_| ())
}
