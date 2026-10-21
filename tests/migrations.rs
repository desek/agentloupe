//! @agents-index Integration test: the migration of the session text tables keeps every row of session_summaries.

#[path = "common/db.rs"]
mod db;

#[test]
fn migration_keeps_session_summaries() {
    let (dir, conn) = db::open();
    let sid = db::segment(&conn, "k", "closed", "2026-01-02T10:00:00.000Z", 1);
    conn.execute(
        "INSERT INTO session_summaries (session_id, status, line, model, attempts, created_at, updated_at) VALUES (?1, 'done', 'fixed the parser', 'm', 1, 'a', 'b')",
        [sid],
    )
    .unwrap();
    let dump = |c: &rusqlite::Connection| -> Vec<String> {
        c.prepare("SELECT session_id || status || line || model || attempts || created_at || updated_at FROM session_summaries")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let before = dump(&conn);
    // Back to the state before the last migration, then run it again.
    conn.execute_batch(
        "DROP TABLE session_full_summaries; DROP TABLE session_day_summaries; DROP INDEX sessions_key_hash; \
         DELETE FROM schema_version WHERE version = 19;",
    )
    .unwrap();
    drop(conn);
    let mut conn = agent_loupe::store::open::open(dir.path()).unwrap();
    assert_eq!(agent_loupe::store::migrate::migrate(&mut conn).unwrap(), 1);
    assert_eq!(dump(&conn), before);
    assert_eq!(before.len(), 1);
}
