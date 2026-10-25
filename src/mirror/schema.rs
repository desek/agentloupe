//! @agents-index The writer open of the mirror: makes the directory and file, sets journal mode DELETE, and makes the tables again on another version.

use super::{TABLES, VERSION, path};
use rusqlite::Connection;
use std::path::Path;

const CREATE: &str = "\
CREATE TABLE days (\
 day TEXT PRIMARY KEY,\
 record TEXT,\
 content_hash TEXT,\
 built_at TEXT,\
 state TEXT NOT NULL CHECK (state = 'published'));\
CREATE TABLE live_state (\
 date TEXT NOT NULL,\
 calls INTEGER NOT NULL,\
 input INTEGER NOT NULL,\
 output INTEGER NOT NULL,\
 cache_read INTEGER NOT NULL,\
 cache_write INTEGER NOT NULL,\
 reasoning INTEGER NOT NULL,\
 cost_usd TEXT NOT NULL,\
 segments_open INTEGER NOT NULL,\
 ended INTEGER NOT NULL,\
 in_flight INTEGER NOT NULL,\
 updated_at INTEGER NOT NULL);";

/// The names of the tables in `sqlite_schema`, without the internal `sqlite_` tables, sorted.
pub fn tables(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

pub fn version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

/// True when the file holds exactly the two tables at the version of the binary.
fn current(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(version(conn)? == VERSION && tables(conn)? == TABLES)
}

/// Drops every table and makes the two tables again in one transaction. The mirror is derived
/// data, so the next reconcile fills it from loupe.db and nothing is lost.
pub fn rebuild(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    for t in tables(&tx)? {
        tx.execute_batch(&format!("DROP TABLE \"{}\"", t.replace('"', "\"\"")))?;
    }
    tx.execute_batch(CREATE)?;
    tx.execute_batch(&format!("PRAGMA user_version = {VERSION}"))?;
    tx.commit()
}

/// Opens or makes `<data dir>/public/public.db` for the role full. The journal mode DELETE lets a
/// reader on a read-only mount read it with no shared-memory file.
pub fn open(data_dir: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    let p = path(data_dir);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            format!("the public mirror directory {} could not be made: {e}. Fix 1: give the process write access to LOUPE_DATA_DIR. Fix 2: mount the volume loupe-public at <data dir>/public. Verify: loupe serve starts and the file public/public.db exists", dir.display())
        })?;
    }
    let mut conn = Connection::open(&p)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = DELETE", [], |r| r.get(0))?;
    if mode != "delete" {
        return Err(format!("the public mirror {} did not take the journal mode DELETE (it has {mode}). Fix: stop each other process that holds the file, then start loupe serve again. Verify: sqlite3 {} 'PRAGMA journal_mode' prints delete", p.display(), p.display()).into());
    }
    if !current(&conn)? {
        rebuild(&mut conn)?;
        tracing::info!(path = %p.display(), version = VERSION, "public mirror tables made");
    }
    Ok(conn)
}
