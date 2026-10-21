//! @agents-index Integration test: the readers of calls (day build, private report, private sessions) keep the calls of origin log.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/loupe.rs"]
mod loupe;

#[test]
fn readers_keep_log_calls() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = bf::claude_tree(logs.path(), true);
    bf::ok(
        data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[("LOUPE_TZ", "UTC")],
    );
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"));
    let agg = c
        .args(["aggregate", "--date", "2026-03-02", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_TZ", "UTC")
        .output()
        .unwrap();
    assert!(
        agg.status.success(),
        "{}",
        String::from_utf8_lossy(&agg.stderr)
    );
    assert!(
        String::from_utf8_lossy(&agg.stdout).contains("calls: 3"),
        "{}",
        String::from_utf8_lossy(&agg.stdout)
    );
    let conn = bf::db(data.path());
    let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
    let tz = chrono_tz::UTC;
    let report = agent_loupe::private::report::build(
        &conn,
        tz,
        day,
        day,
        &agent_loupe::private::query::Group::Token,
    )
    .unwrap();
    assert!(report.to_string().contains("\"calls\":3"), "{report}");
    let sessions = agent_loupe::private::sessions::list(
        &conn,
        tz,
        day,
        day,
        agent_loupe::private::report::NO_TOKEN,
        None,
    )
    .unwrap();
    assert_eq!(
        sessions["rows"].as_array().map(Vec::len),
        Some(2),
        "{sessions}"
    );
}
