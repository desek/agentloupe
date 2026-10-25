//! @agents-index Integration tests of the read-only open of the mirror: no write, no create, the table and version checks, and fresh reads.

#[path = "common/public.rs"]
mod public;

use agent_loupe::mirror::open::{Error, open};

#[test]
fn insert_fails_read_only() {
    let d = tempfile::tempdir().unwrap();
    public::seed_mirror(d.path());
    let c = open(d.path()).unwrap();
    let e = c
        .execute(
            "INSERT INTO days (day, state) VALUES ('2026-02-01', 'published')",
            [],
        )
        .unwrap_err();
    assert!(
        e.to_string().contains("readonly") || e.to_string().contains("read-only"),
        "{e}"
    );
}

#[test]
fn missing_file_stops_and_makes_no_file() {
    let d = tempfile::tempdir().unwrap();
    let e = open(d.path()).unwrap_err();
    assert!(matches!(e, Error::Absent(_)));
    assert!(e.to_string().contains("start the role full first"));
    assert!(!agent_loupe::mirror::path(d.path()).exists());
    let out = public::command(d.path(), &[]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("does not exist"));
    assert!(!d.path().join("public").exists());
}

#[test]
fn extra_table_stops() {
    let d = tempfile::tempdir().unwrap();
    public::seed_mirror(d.path());
    let w = rusqlite::Connection::open(agent_loupe::mirror::path(d.path())).unwrap();
    w.execute_batch("CREATE TABLE exchanges (id INTEGER)")
        .unwrap();
    let e = open(d.path()).unwrap_err();
    assert!(e.to_string().contains("exchanges") && e.to_string().contains("not a public mirror"));
}

#[test]
fn other_version_stops() {
    let d = tempfile::tempdir().unwrap();
    public::seed_mirror(d.path());
    let w = rusqlite::Connection::open(agent_loupe::mirror::path(d.path())).unwrap();
    w.execute_batch("PRAGMA user_version = 99").unwrap();
    let e = open(d.path()).unwrap_err();
    assert!(matches!(e, Error::Version { found: 99, .. }), "{e}");
    assert!(e.to_string().contains("docker compose up -d loupe"));
}

#[test]
fn writer_commit_is_in_the_next_read() {
    let d = tempfile::tempdir().unwrap();
    public::seed_mirror(d.path());
    let r = open(d.path()).unwrap();
    let count = |c: &rusqlite::Connection| -> i64 {
        c.query_row("SELECT count(*) FROM days", [], |x| x.get(0))
            .unwrap()
    };
    assert_eq!(count(&r), 1);
    let w = agent_loupe::mirror::schema::open(d.path()).unwrap();
    w.execute(
        "INSERT INTO days (day, record, content_hash, built_at, state) VALUES ('2026-01-05', '{}', 'h', 'b', 'published')",
        [],
    )
    .unwrap();
    assert_eq!(count(&r), 2);
}
