//! @agents-index Integration test: loupe backfill counters, idempotence, processed sessions, subagents, errors, range, and the Pi rule.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use bf::{CLAUDE_SESSION, claude_line, claude_tree, ok, pi_tree, run, scalar, write};
use loupe::Loupe;

fn stderr(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Sends one proxied call under `agent`, with the Claude Code session header when `session` is set, then parses it.
async fn proxied(l: &Loupe, agent: &str, session: Option<&str>) {
    let before = l.rows();
    let mut r = reqwest::Client::new()
        .post(l.url(&format!("/{agent}/fake/v1/messages")))
        .json(&serde_json::json!({"model":"claude-haiku-5-5","messages":[{"role":"user","content":"hi"}]}));
    if let Some(s) = session {
        r = r.header("x-claude-code-session-id", s);
    }
    let resp = r.send().await.unwrap();
    assert_eq!(resp.status(), 200);
    resp.bytes().await.unwrap();
    l.wait_rows(before + 1);
    let p = loupe::parse(l.data.path(), &["--once"]);
    assert!(p.status.success(), "{}", stderr(&p));
}

const QUIET: &[(&str, &str)] = &[
    ("LOUPE_PARSE_INTERVAL_SECS", "86400"),
    ("LOUPE_SUMMARY_INTERVAL_SECS", "86400"),
];

#[test]
fn backfill_twice_adds_nothing() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = claude_tree(logs.path(), true);
    let p = root.to_str().unwrap();
    let a = ok(data.path(), &["--source", "claude-code", "--path", p], &[]);
    assert_eq!(loupe::count(&a, "files_read"), 2);
    assert_eq!(loupe::count(&a, "calls_added"), 3);
    assert_eq!(loupe::count(&a, "days_queued"), 1);
    let b = ok(data.path(), &["--source", "claude-code", "--path", p], &[]);
    assert_eq!(loupe::count(&b, "calls_added"), 0);
    assert_eq!(loupe::count(&b, "calls_existing"), 3);
    assert_eq!(scalar(data.path(), "SELECT count(*) FROM calls"), 3);
    let a = scalar(
        data.path(),
        "SELECT count(*) FROM calls WHERE origin = 'log' AND exchange_id IS NULL AND agent_label = 'claude-code' AND status = 200 AND proxy_token_id IS NULL AND cost_usd IS NOT NULL",
    );
    assert_eq!(a, 3);
    let (w5, w1): (i64, i64) = bf::db(data.path())
        .query_row(
            "SELECT cache_write_5m, cache_write_1h FROM calls ORDER BY started_at LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((w5, w1), (10, 20));
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM sessions WHERE key_source = 'log' AND state = 'closed'"
        ),
        2
    );
}

#[test]
fn dry_run_writes_nothing() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = claude_tree(logs.path(), true);
    let o = ok(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            root.to_str().unwrap(),
            "--dry-run",
        ],
        &[],
    );
    assert_eq!(loupe::count(&o, "calls_added"), 3);
    assert_eq!(loupe::count(&o, "sessions_opened"), 2);
    assert_eq!(loupe::count(&o, "summaries_disabled"), 0);
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM sqlite_master WHERE name = 'calls'"
        ),
        0
    );
}

#[test]
fn subagent_file_is_own_session() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = claude_tree(logs.path(), true);
    ok(
        data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[],
    );
    assert_eq!(
        scalar(data.path(), "SELECT count(DISTINCT key_hash) FROM sessions"),
        2
    );
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM sessions WHERE call_count = 1"
        ),
        1
    );
}

#[test]
fn log_call_has_null_account() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = pi_tree(logs.path());
    let o = ok(
        data.path(),
        &["--source", "pi", "--path", root.to_str().unwrap()],
        &[],
    );
    assert_eq!(loupe::count(&o, "calls_added"), 3);
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM calls WHERE origin = 'log' AND account_id IS NULL"
        ),
        3
    );
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM calls WHERE cost_usd IS NULL"
        ),
        0
    );
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM calls WHERE cache_write_5m = 25 AND cache_write_1h = 15"
        ),
        1
    );
    let doc = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/specification/accounts.md"),
    )
    .unwrap();
    assert!(doc.contains("A call with origin `log` has the account `none`"));
}

#[test]
fn missing_source_flag_names_the_flag() {
    let data = tempfile::tempdir().unwrap();
    let o = run(data.path(), &[], &[]);
    assert_eq!(o.status.code(), Some(2));
    let e = stderr(&o);
    assert!(
        e.contains("--source") && e.contains("claude-code") && e.contains("Fix"),
        "{e}"
    );
    let o = run(
        data.path(),
        &["--source", "pi", "--path", "/nonexistent/backfill-root"],
        &[],
    );
    assert_eq!(o.status.code(), Some(3));
    assert!(stderr(&o).contains("Fix 1: pass --path"));
}

#[test]
fn unknown_source_value_fails() {
    let data = tempfile::tempdir().unwrap();
    let o = run(data.path(), &["--source", "codex"], &[]);
    assert_eq!(o.status.code(), Some(2));
    let e = stderr(&o);
    assert!(
        e.contains("--source claude-code") && e.contains("--source pi"),
        "{e}"
    );
}

#[test]
fn missing_path_exits_3() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let missing = logs.path().join("not-here");
    let o = run(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            missing.to_str().unwrap(),
        ],
        &[],
    );
    assert_eq!(o.status.code(), Some(3));
    let e = stderr(&o);
    assert!(
        e.contains(missing.to_str().unwrap()) && e.contains("ls "),
        "{e}"
    );
}

#[test]
fn missing_account_key_fails() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = claude_tree(logs.path(), false);
    let o = bf::command(
        data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
    )
    .env_remove("LOUPE_ACCOUNT_KEY")
    .output()
    .unwrap();
    assert!(!o.status.success());
    let e = stderr(&o);
    assert!(e.contains("LOUPE_ACCOUNT_KEY") && e.contains("fix 1"), "{e}");
}

#[test]
fn malformed_line_is_counted() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = logs.path().join("p/s.jsonl");
    let text = format!(
        "{}\n{{\"broken MARKERBROKEN\n{}\n",
        claude_line("s", "m1", "2026-03-02T10:00:00Z", "claude-haiku-5-5"),
        claude_line("s", "m2", "2026-03-02T10:01:00Z", "claude-haiku-5-5")
    );
    write(&f, &text);
    let o = ok(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            logs.path().to_str().unwrap(),
        ],
        &[],
    );
    assert_eq!(loupe::count(&o, "entries_skipped"), 1);
    assert_eq!(loupe::count(&o, "entries_read"), 3);
    assert_eq!(loupe::count(&o, "calls_added"), 2);
    let log = std::fs::read_to_string(data.path().join("logs/loupe.log")).unwrap();
    let line = log
        .lines()
        .find(|l| l.contains("malformed log line"))
        .unwrap();
    assert!(
        line.contains("\"file_index\":0") && line.contains("\"line\":2"),
        "{line}"
    );
    assert!(!log.contains("MARKERBROKEN") && !log.contains("s.jsonl"));
}

#[test]
fn closed_segment_not_reopened() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = format!(
        "{}\n{}\n",
        claude_line("s", "m1", "2026-03-02T10:00:00Z", "claude-haiku-5-5"),
        claude_line("s", "m2", "2026-03-04T10:00:00Z", "claude-haiku-5-5")
    );
    write(&logs.path().join("p/s.jsonl"), &text);
    let p = logs.path().to_str().unwrap();
    let a = ok(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            p,
            "--from",
            "2026-03-04",
        ],
        &[],
    );
    assert_eq!(loupe::count(&a, "calls_out_of_range"), 1);
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM sessions WHERE state = 'closed'"
        ),
        1
    );
    ok(data.path(), &["--source", "claude-code", "--path", p], &[]);
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM sessions WHERE state = 'closed' AND call_count = 1"
        ),
        2
    );
    assert_eq!(scalar(data.path(), "SELECT max(segment) FROM sessions"), 2);
}

#[test]
fn unpriced_model_is_unpriced_call() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    write(
        &logs.path().join("p/s.jsonl"),
        &claude_line("s", "m1", "2026-03-02T10:00:00Z", "made-up-model-zz"),
    );
    ok(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            logs.path().to_str().unwrap(),
        ],
        &[],
    );
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM calls WHERE cost_usd IS NULL AND unpriced_reason IS NOT NULL"
        ),
        1
    );
}

#[test]
fn interrupted_run_keeps_whole_files() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    write(
        &logs.path().join("p/a.jsonl"),
        &claude_line("s1", "m1", "2026-03-02T10:00:00Z", "claude-haiku-5-5"),
    );
    let b = format!(
        "{}\n{}\n",
        claude_line("s2", "m2", "2026-03-05T10:00:00Z", "claude-haiku-5-5"),
        claude_line("s2", "m3", "2026-03-05T10:01:00Z", "boom-model")
    );
    write(&logs.path().join("p/b.jsonl"), &b);
    let p = logs.path().to_str().unwrap();
    // A first run over an empty root makes the schema, so the test can add a trigger.
    let empty = tempfile::tempdir().unwrap();
    ok(
        data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            empty.path().to_str().unwrap(),
        ],
        &[],
    );
    // A trigger stands in for a failure in the middle of the second file.
    bf::db(data.path())
        .execute_batch("CREATE TRIGGER boom BEFORE INSERT ON calls WHEN NEW.model = 'boom-model' BEGIN SELECT RAISE(ABORT, 'injected'); END;")
        .unwrap();
    let o = run(data.path(), &["--source", "claude-code", "--path", p], &[]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(scalar(data.path(), "SELECT count(*) FROM calls"), 1);
    assert_eq!(
        scalar(
            data.path(),
            "SELECT count(*) FROM calls WHERE started_at LIKE '2026-03-05%'"
        ),
        0
    );
    bf::db(data.path())
        .execute_batch("DROP TRIGGER boom")
        .unwrap();
    let o = ok(data.path(), &["--source", "claude-code", "--path", p], &[]);
    assert_eq!(
        (
            loupe::count(&o, "calls_added"),
            loupe::count(&o, "calls_existing")
        ),
        (2, 1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn captured_session_is_skipped() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    proxied(&l, "claude", Some(CLAUDE_SESSION)).await;
    let logs = tempfile::tempdir().unwrap();
    let root = claude_tree(logs.path(), false);
    let o = ok(
        l.data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[],
    );
    assert_eq!(loupe::count(&o, "sessions_already_captured"), 1);
    assert_eq!(loupe::count(&o, "calls_already_captured"), 2);
    assert_eq!(loupe::count(&o, "calls_added"), 0);
    assert_eq!(
        scalar(
            l.data.path(),
            "SELECT count(*) FROM calls WHERE origin = 'log'"
        ),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn subagent_follows_main_session() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    proxied(&l, "claude", Some(CLAUDE_SESSION)).await;
    let logs = tempfile::tempdir().unwrap();
    let root = claude_tree(logs.path(), true);
    let o = ok(
        l.data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[],
    );
    assert_eq!(loupe::count(&o, "calls_already_captured"), 3);
    assert_eq!(
        scalar(
            l.data.path(),
            "SELECT count(*) FROM calls WHERE origin = 'log'"
        ),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn range_does_not_bypass_processed_rule() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    proxied(&l, "claude", Some(CLAUDE_SESSION)).await;
    let logs = tempfile::tempdir().unwrap();
    let root = claude_tree(logs.path(), false);
    let o = ok(
        l.data.path(),
        &[
            "--source",
            "claude-code",
            "--path",
            root.to_str().unwrap(),
            "--from",
            "2026-03-01",
            "--to",
            "2026-03-03",
        ],
        &[],
    );
    assert_eq!(loupe::count(&o, "calls_already_captured"), 2);
    assert_eq!(loupe::count(&o, "calls_out_of_range"), 0);
    assert_eq!(
        scalar(
            l.data.path(),
            "SELECT count(*) FROM calls WHERE origin = 'log'"
        ),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pi_label_unmatched_warns() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    proxied(&l, "pi-dev", None).await;
    let logs = tempfile::tempdir().unwrap();
    let root = pi_tree(logs.path());
    let later = serde_json::json!({"type":"session","id":"pi-later","timestamp":"2099-01-01T00:00:00Z"}).to_string() + "\n"
        + &serde_json::json!({"type":"message","id":"x1","timestamp":"2099-01-01T00:00:01Z","message":{"role":"assistant","provider":"anthropic","model":"claude-haiku-5-5","usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0}}}).to_string();
    write(&root.join("later/2099_x.jsonl"), &later);
    let p = root.to_str().unwrap();
    let o = ok(
        l.data.path(),
        &["--source", "pi", "--path", p, "--dry-run"],
        &[],
    );
    let e = stderr(&o);
    assert!(e.contains("--pi-label") && e.contains("pi-dev"), "{e}");
    assert_eq!(loupe::count(&o, "sessions_already_captured"), 0);
    assert_eq!(loupe::count(&o, "calls_added"), 4);
    let o = ok(
        l.data.path(),
        &[
            "--source",
            "pi",
            "--path",
            p,
            "--pi-label",
            "pi-dev",
            "--dry-run",
        ],
        &[],
    );
    assert!(!stderr(&o).contains("warning"));
    assert_eq!(loupe::count(&o, "sessions_already_captured"), 1);
    assert_eq!(loupe::count(&o, "calls_added"), 3);
}
