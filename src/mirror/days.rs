//! @agents-index Reconciles the mirror days table with the published rows of loupe.db: writes only a difference, in one transaction.

use rusqlite::{Connection, params};
use std::collections::HashMap;

/// The count of days written and deleted by one reconcile.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub written: u64,
    pub deleted: u64,
}

type Key = (Option<String>, Option<String>);

fn keys(conn: &Connection, sql: &str) -> rusqlite::Result<HashMap<String, Key>> {
    let mut stmt = conn.prepare(sql)?;
    stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect()
}

/// Compares the day, content hash, and built_at of each published row; a row in another state is
/// never read, so a pending_review or deleted day never reaches the mirror.
pub fn reconcile(src: &Connection, mirror: &mut Connection) -> rusqlite::Result<Changes> {
    let want = keys(
        src,
        "SELECT day, content_hash, built_at FROM days WHERE state = 'published'",
    )?;
    let have = keys(mirror, "SELECT day, content_hash, built_at FROM days")?;
    let changed: Vec<&String> = want
        .iter()
        .filter(|(d, k)| have.get(*d) != Some(k))
        .map(|(d, _)| d)
        .collect();
    let gone: Vec<&String> = have.keys().filter(|d| !want.contains_key(*d)).collect();
    let mut c = Changes::default();
    if changed.is_empty() && gone.is_empty() {
        return Ok(c);
    }
    let tx = mirror.transaction()?;
    for d in changed {
        // The record is read in the same pass as its key, so the row is never mixed.
        let row = src.query_row(
            "SELECT record, content_hash, built_at FROM days WHERE day = ?1 AND state = 'published'",
            [d],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        );
        match row {
            Ok((record, hash, built)) => {
                tx.execute(
                    "INSERT OR REPLACE INTO days (day, record, content_hash, built_at, state) VALUES (?1, ?2, ?3, ?4, 'published')",
                    params![d, record, hash, built],
                )?;
                c.written += 1;
            }
            // The day left the state published between the two reads; the next pass deletes it.
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e),
        }
    }
    for d in gone {
        tx.execute("DELETE FROM days WHERE day = ?1", [d])?;
        c.deleted += 1;
    }
    tx.commit()?;
    Ok(c)
}
