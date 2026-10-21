//! @agents-index Integration test: the conversation threshold check of serve and backfill, and loupe summarize --rebuild-days.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/session_days.rs"]
mod sd;

const VAR: &str = "LOUPE_SUMMARY_MIN_CONVERSATION_CHARS";

fn assert_threshold_error(out: &std::process::Output) {
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(VAR) && err.contains("0 or more"), "{err}");
}

#[test]
fn bad_conversation_threshold_fails() {
    let data = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("serve")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_LISTEN", "127.0.0.1:0")
        .env(VAR, "abc")
        .output()
        .unwrap();
    assert_threshold_error(&out);
}

#[test]
fn negative_conversation_threshold_fails_backfill() {
    let data = tempfile::tempdir().unwrap();
    let out = bf::run(data.path(), &["--source", "claude-code"], &[(VAR, "-1")]);
    assert_threshold_error(&out);
}

/// Every row of the two session text tables and of session_summaries, as text.
fn dump(data: &std::path::Path) -> Vec<String> {
    let c = bf::db(data);
    let mut out = vec![];
    for sql in [
        "SELECT 'f' || day || status || coalesce(text, '') || attempts FROM session_full_summaries ORDER BY day",
        "SELECT 'd' || day || status || coalesce(text, '') || attempts FROM session_day_summaries ORDER BY day",
        "SELECT 's' || session_id || status || coalesce(line, '') || updated_at FROM session_summaries ORDER BY session_id",
    ] {
        let rows: Vec<String> = c
            .prepare(sql)
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        out.extend(rows);
    }
    out
}

fn rebuild(data: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["--rebuild-days", "2026-01-01..2026-01-02"];
    args.extend(extra);
    loupe::summarize(data, &args, &[("LOUPE_TZ", "UTC")])
}

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_days_resets_rows_in_range() {
    let fake = fake::spawn().await;
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = sd::tree(logs.path(), "rb", &sd::days("rb", 3));
    sd::backfill(data.path(), &root, fake, Some("rec_rebuild"));
    let before = dump(data.path());
    let out = rebuild(data.path(), &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(loupe::count(&out, "summaries_reset"), 4);
    for table in ["session_full_summaries", "session_day_summaries"] {
        let rows = sd::statuses(data.path(), table);
        assert_eq!(rows[0].1, "pending");
        assert_eq!(rows[1].1, "pending");
        assert_eq!(rows[2].1, "done");
    }
    let after = dump(data.path());
    let keep = |v: &[String]| -> Vec<String> {
        v.iter()
            .filter(|r| r.starts_with('s') || r[1..].starts_with("2026-01-03"))
            .cloned()
            .collect()
    };
    assert_eq!(keep(&after), keep(&before));
    // The next backfill run builds the reset rows of the backfilled session again.
    sd::backfill(data.path(), &root, fake, Some("rec_rebuild"));
    for table in ["session_full_summaries", "session_day_summaries"] {
        assert!(
            sd::statuses(data.path(), table)
                .iter()
                .all(|(_, s)| s == "done")
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_days_dry_run_changes_nothing() {
    let fake = fake::spawn().await;
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = sd::tree(logs.path(), "rd", &sd::days("rd", 3));
    sd::backfill(data.path(), &root, fake, Some("rec_rebuild_dry"));
    let before = dump(data.path());
    let out = rebuild(data.path(), &["--dry-run"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(loupe::count(&out, "summaries_reset"), 4);
    assert_eq!(dump(data.path()), before);
}

#[test]
fn rebuild_days_bad_range_names_form() {
    let data = tempfile::tempdir().unwrap();
    let out = loupe::summarize(data.path(), &["--rebuild-days", "yesterday"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("YYYY-MM-DD..YYYY-MM-DD"));
}
