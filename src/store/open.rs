//! @agents-index Opens <data dir>/loupe.db in WAL journal mode, so readers do not block the writer; the directory must exist.

use rusqlite::Connection;
use std::path::Path;

pub fn open(data_dir: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(data_dir.join("loupe.db"))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    debug_assert_eq!(mode, "wal");
    Ok(conn)
}

#[cfg(test)]
mod tests {
    #[test]
    fn opens_in_wal_mode() {
        let dir = tempfile::tempdir().unwrap();
        let conn = super::open(dir.path()).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}
