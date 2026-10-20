//! @agents-index Applies the versioned migrations from migrations/ in order, once each, recorded in schema_version.

use rusqlite::{Connection, params};

/// Every migration, in order. The version is the number prefix of the file name.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_schema_version.sql")),
    (2, include_str!("../../migrations/0002_exchanges.sql")),
];

/// Applies each migration not yet recorded; returns how many it applied.
pub fn migrate(conn: &mut Connection) -> rusqlite::Result<usize> {
    // The first migration makes the version table, so its absence means nothing is applied.
    let has_table: bool = conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |r| r.get(0),
    )?;
    let mut applied = 0;
    for (version, sql) in MIGRATIONS {
        let done: bool = has_table
            && conn.query_row(
                "SELECT count(*) > 0 FROM schema_version WHERE version = ?1",
                [version],
                |r| r.get(0),
            )?;
        if done {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![version],
        )?;
        tx.commit()?;
        applied += 1;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrations_apply_once() {
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..2 {
            let mut conn = crate::store::open::open(dir.path()).unwrap();
            super::migrate(&mut conn).unwrap();
        }
        let conn = crate::store::open::open(dir.path()).unwrap();
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows as usize, super::MIGRATIONS.len());
        assert_eq!(rows, 2);
        let exchanges: i64 = conn
            .query_row("SELECT count(*) FROM exchanges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(exchanges, 0);
    }
}
