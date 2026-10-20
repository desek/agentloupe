//! @agents-index SQL of the days table: read a row, the upsert by content hash with the revision rule, state changes, and tombstones.

use super::state;
use rusqlite::{Connection, OptionalExtension, params};

/// One days row; record and content_hash are None in a tombstone.
#[derive(Debug, Clone)]
pub struct Row {
    pub record: Option<String>,
    pub content_hash: Option<String>,
    pub revision: i64,
    pub state: String,
    pub published_at: Option<String>,
}

pub fn read(conn: &Connection, day: &str) -> rusqlite::Result<Option<Row>> {
    conn.query_row(
        "SELECT record, content_hash, revision, state, published_at FROM days WHERE day = ?1",
        [day],
        |r| {
            Ok(Row {
                record: r.get(0)?,
                content_hash: r.get(1)?,
                revision: r.get(2)?,
                state: r.get(3)?,
                published_at: r.get(4)?,
            })
        },
    )
    .optional()
}

/// The result of an upsert: the revision after it, and whether it wrote a new content hash.
pub struct Upserted {
    pub revision: i64,
    pub written: bool,
    pub state: String,
}

/// Writes `record` when its hash is new. A new row gets revision 1 and `first_state`; an
/// existing row keeps its state, and an equal hash changes nothing.
pub fn upsert(
    conn: &Connection,
    day: &str,
    record: &str,
    hash: &str,
    now: &str,
    first_state: &str,
) -> rusqlite::Result<Upserted> {
    match read(conn, day)? {
        Some(row) if row.content_hash.as_deref() == Some(hash) => Ok(Upserted {
            revision: row.revision,
            written: false,
            state: row.state,
        }),
        Some(row) => {
            conn.execute(
                "UPDATE days SET record = ?2, content_hash = ?3, revision = revision + 1, built_at = ?4 WHERE day = ?1",
                params![day, record, hash, now],
            )?;
            Ok(Upserted {
                revision: row.revision + 1,
                written: true,
                state: row.state,
            })
        }
        None => {
            let published = (first_state == state::PUBLISHED).then_some(now);
            conn.execute(
                "INSERT INTO days (day, record, content_hash, revision, built_at, state, published_at) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
                params![day, record, hash, now, first_state, published],
            )?;
            Ok(Upserted {
                revision: 1,
                written: true,
                state: first_state.to_string(),
            })
        }
    }
}

/// Sets the state; `published_at` gets `now` only the first time the state becomes published.
pub fn set_state(conn: &Connection, day: &str, to: &str, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE days SET state = ?2, published_at = CASE WHEN ?2 = 'published' THEN coalesce(published_at, ?3) ELSE published_at END WHERE day = ?1",
        params![day, to, now],
    )
    .map(|_| ())
}

/// Makes the day a tombstone: state deleted, null record and hash. A day with no row gets a
/// row with revision 0.
pub fn tombstone(conn: &Connection, day: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO days (day, record, content_hash, revision, built_at, state, published_at) VALUES (?1, NULL, NULL, 0, NULL, 'deleted', NULL) \
         ON CONFLICT (day) DO UPDATE SET record = NULL, content_hash = NULL, state = 'deleted'",
        [day],
    )
    .map(|_| ())
}

/// The days that have a row, any state.
pub fn days_with_row(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<String>> {
    conn.prepare("SELECT day FROM days")?
        .query_map([], |r| r.get(0))?
        .collect()
}
