//! @agents-index Integration test: loupe aggregate --date today writes a record, --dry-run writes nothing, a bad --date exits 2.

#[path = "common/days.rs"]
mod days;

fn seeded_today() -> (tempfile::TempDir, String) {
    let (dir, c) = days::db();
    let now = chrono::Utc::now().timestamp_millis();
    days::simple(&c, &agent_loupe::session::time::text(now), "claude-code");
    let today = agent_loupe::day::boundary::day_of(now, chrono_tz::UTC).to_string();
    (dir, today)
}

#[test]
fn aggregate_today_writes_record() {
    let (dir, today) = seeded_today();
    let out = days::loupe(dir.path(), &["aggregate", "--date", "today"], &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(days::field(&out, "day"), today);
    assert_eq!(days::field(&out, "calls"), "1");
    assert_eq!(days::field(&out, "written"), "true");
    assert_eq!(days::field(&out, "revision"), "1");
    assert_eq!(days::field(&out, "summary"), "absent");
    let c = rusqlite::Connection::open(dir.path().join("loupe.db")).unwrap();
    let r = days::record(&c, &today).unwrap();
    assert_eq!(r["detail"]["by_agent"]["claude-code"]["calls"], 1);
    let again = days::loupe(dir.path(), &["aggregate", "--date", "today"], &[]);
    assert_eq!(days::field(&again, "written"), "false");
    assert_eq!(days::field(&again, "revision"), "1");
}

#[test]
fn aggregate_dry_run_writes_nothing() {
    let (dir, today) = seeded_today();
    let out = days::loupe(
        dir.path(),
        &["aggregate", "--date", &today, "--dry-run"],
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(days::field(&out, "calls"), "1");
    assert_eq!(days::field(&out, "written"), "false");
    let c = rusqlite::Connection::open(dir.path().join("loupe.db")).unwrap();
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 0);
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM day_summaries"), 0);
    // A day with no activity prints calls 0 and writes nothing.
    let empty = days::loupe(dir.path(), &["aggregate", "--date", "2001-01-01"], &[]);
    assert!(empty.status.success());
    assert_eq!(days::field(&empty, "calls"), "0");
    assert_eq!(days::field(&empty, "written"), "false");
    assert_eq!(days::scalar(&c, "SELECT count(*) FROM days"), 0);
}

#[test]
fn aggregate_bad_date_exits_2() {
    let (dir, _) = seeded_today();
    let out = days::loupe(dir.path(), &["aggregate", "--date", "2026-13-01"], &[]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--date"), "{err}");
    assert!(err.contains("2026-10-19"), "{err}");
    let help = days::loupe(dir.path(), &["aggregate", "--help"], &[]);
    let help = String::from_utf8_lossy(&help.stdout);
    for needle in [
        "--date",
        "--dry-run",
        "0 success",
        "2 bad input",
        "1 internal error",
    ] {
        assert!(help.contains(needle), "{needle} in {help}");
    }
}
