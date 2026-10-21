//! @agents-index Integration test: a backfilled segment gets a scrubbed session line from the log transcript, and a re-run retries it.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

fn env(url: &str) -> Vec<(&'static str, String)> {
    vec![
        ("LOUPE_SUMMARY_API_KEY", "sk-test-Bf7Kq2".into()),
        ("LOUPE_SUMMARY_BASE_URL", url.into()),
    ]
}

fn run(data: &std::path::Path, root: &std::path::Path, url: &str) -> std::process::Output {
    let e = env(url);
    let pairs: Vec<(&str, &str)> = e.iter().map(|(k, v)| (*k, v.as_str())).collect();
    bf::ok(
        data,
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &pairs,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn backfilled_segment_gets_scrubbed_line() {
    let fake = fake::spawn().await;
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = bf::claude_tree(logs.path(), false);
    let o = run(
        data.path(),
        &root,
        &format!("http://127.0.0.1:{fake}/summary/line"),
    );
    assert_eq!(loupe::count(&o, "summaries_done"), 1);
    let line: String = bf::db(data.path())
        .query_row(
            "SELECT line FROM session_summaries WHERE status = 'done'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(line, "fixed the parser in per for in places");
    let bytes: Vec<u8> = ["loupe.db", "loupe.db-wal"]
        .iter()
        .flat_map(|f| std::fs::read(data.path().join(f)).unwrap_or_default())
        .collect();
    for marker in ["MARKERPROMPT", "MARKERTOOLRESULT", "MARKERANSWER", "user:"] {
        assert!(
            !loupe::contains(&bytes, marker.as_bytes()),
            "{marker} is in the database"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn rerun_retries_pending_summary() {
    let fake = fake::spawn().await;
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = bf::claude_tree(logs.path(), false);
    let a = run(
        data.path(),
        &root,
        &format!("http://127.0.0.1:{fake}/summary/500"),
    );
    assert_eq!(loupe::count(&a, "summaries_retry_later"), 1);
    let status = |d: &std::path::Path| -> String {
        bf::db(d)
            .query_row("SELECT status FROM session_summaries", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(status(data.path()), "pending");
    // The time of the next attempt passes.
    bf::db(data.path())
        .execute_batch("UPDATE session_summaries SET next_attempt_at = NULL")
        .unwrap();
    let b = run(
        data.path(),
        &root,
        &format!("http://127.0.0.1:{fake}/summary/line"),
    );
    assert_eq!(loupe::count(&b, "calls_added"), 0);
    assert_eq!(loupe::count(&b, "summaries_done"), 1);
    assert_eq!(status(data.path()), "done");
}
