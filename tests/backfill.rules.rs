//! @agents-index Integration test: loupe backfill idle-gap segments, queued days in LOUPE_TZ, and the mix of a captured and a new session.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use bf::{CLAUDE_SESSION, claude_line, claude_tree, ok, scalar, write};

#[test]
fn idle_gap_splits_segments() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = format!(
        "{}\n{}\n",
        claude_line("s", "m1", "2026-03-02T10:00:00Z", "claude-haiku-5-5"),
        claude_line("s", "m2", "2026-03-02T10:02:00Z", "claude-haiku-5-5")
    );
    write(&logs.path().join("p/s.jsonl"), &text);
    let p = logs.path().to_str().unwrap();
    ok(
        data.path(),
        &["--source", "claude-code", "--path", p],
        &[("LOUPE_IDLE_SECS", "60")],
    );
    let q = "SELECT count(*) FROM sessions WHERE state = 'closed' AND call_count = 1";
    assert_eq!(scalar(data.path(), q), 2);
}

#[test]
fn two_days_are_queued() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = format!(
        "{}\n{}\n",
        claude_line("s", "m1", "2026-03-02T10:00:00Z", "claude-haiku-5-5"),
        claude_line("s", "m2", "2026-03-03T10:00:00Z", "claude-haiku-5-5")
    );
    write(&logs.path().join("p/s.jsonl"), &text);
    let p = logs.path().to_str().unwrap();
    let o = ok(
        data.path(),
        &["--source", "claude-code", "--path", p],
        &[("LOUPE_TZ", "UTC")],
    );
    assert_eq!(loupe::count(&o, "days_queued"), 2);
    let q = "SELECT count(*) FROM agg_queue WHERE day IN ('2026-03-02', '2026-03-03')";
    assert_eq!(scalar(data.path(), q), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn only_new_session_is_added() {
    let l = loupe::Loupe::start(
        fake::spawn().await,
        &[],
        &[
            ("LOUPE_PARSE_INTERVAL_SECS", "86400"),
            ("LOUPE_SUMMARY_INTERVAL_SECS", "86400"),
        ],
    );
    let before = l.rows();
    let resp = reqwest::Client::new()
        .post(l.url("/claude/fake/v1/messages"))
        .header("x-claude-code-session-id", CLAUDE_SESSION)
        .json(&serde_json::json!({"model":"claude-haiku-5-5","messages":[{"role":"user","content":"hi"}]}))
        .send()
        .await
        .unwrap();
    resp.bytes().await.unwrap();
    l.wait_rows(before + 1);
    assert!(loupe::parse(l.data.path(), &["--once"]).status.success());
    let logs = tempfile::tempdir().unwrap();
    let root = claude_tree(logs.path(), false);
    let r = claude_line(
        "r-session",
        "mr",
        "2026-03-05T10:00:00Z",
        "claude-haiku-5-5",
    );
    write(&root.join("other/r-session.jsonl"), &format!("{r}\n"));
    let o = ok(
        l.data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[],
    );
    assert_eq!(loupe::count(&o, "sessions_already_captured"), 1);
    assert_eq!(loupe::count(&o, "calls_added"), 1);
    let q = "SELECT count(*) FROM calls WHERE origin = 'log'";
    assert_eq!(scalar(l.data.path(), q), 1);
}
