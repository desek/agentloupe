//! @agents-index Integration test: the summary worker writes the session texts and no segment line, handles SKIP and failures, disables without a credential.

#[path = "common/backfill.rs"]
mod bf;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/summary.rs"]
mod summary;

use loupe::Loupe;
use summary::{QUIET, base, scalar, send_session, text};

const KEY_VALUE: &str = "sk-test-Zq8Wv3Lm";

/// A loupe with one closed segment of `n` calls.
async fn one_closed(fake: u16, n: usize) -> Loupe {
    let l = Loupe::start(fake, &[], QUIET);
    send_session(&l, "s1", n).await;
    loupe::parse_and_close(l.data.path());
    assert_eq!(
        scalar(&l, "SELECT count(*) FROM sessions WHERE state = 'closed'"),
        1
    );
    l
}

fn run(l: &Loupe, url: &str) -> std::process::Output {
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[
            ("LOUPE_SUMMARY_API_KEY", KEY_VALUE),
            ("LOUPE_SUMMARY_BASE_URL", url),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn closed_segment_gets_session_texts_and_no_line() {
    let fake = fake::spawn().await;
    let l = one_closed(fake, 3).await;
    let out = run(&l, &base(fake, "line"));
    assert_eq!(loupe::count(&out, "segments_read"), 1);
    assert_eq!(loupe::count(&out, "done"), 1);
    assert_eq!(scalar(&l, "SELECT count(*) FROM session_summaries"), 0);
    let line = text(
        &l,
        "SELECT text FROM session_day_summaries WHERE status = 'done'",
    )
    .unwrap();
    for bad in ["/", "@", "http", "42", ".rs"] {
        assert!(!line.contains(bad), "{line}");
    }
    let again = run(&l, &base(fake, "line"));
    assert_eq!(loupe::count(&again, "segments_read"), 0);
    let log = l.log() + &String::from_utf8_lossy(&out.stderr);
    for bad in ["fixed the parser", "please fix", KEY_VALUE] {
        assert!(!log.contains(bad), "the log holds {bad}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn skip_and_failures() {
    let fake = fake::spawn().await;
    let l = one_closed(fake, 1).await;
    let out = run(&l, &base(fake, "skip"));
    assert_eq!(loupe::count(&out, "retry_later"), 1);
    let row =
        "SELECT status || ':' || last_error_kind || ':' || attempts FROM session_full_summaries";
    assert_eq!(text(&l, row).unwrap(), "pending:skip:1");

    let l = one_closed(fake, 1).await;
    let out = run(&l, &base(fake, "401"));
    assert_eq!(loupe::count(&out, "failed"), 1);
    assert_eq!(text(&l, row).unwrap(), "failed:unauthorized:1");
    let log = l.log();
    assert!(log.contains("LOUPE_SUMMARY_API_KEY") && log.contains("loupe summarize --once"));
    assert!(!log.contains(KEY_VALUE));

    let l = one_closed(fake, 1).await;
    for i in 1..=5 {
        run(&l, &base(fake, "429"));
        if i < 5 {
            assert_eq!(text(&l, row).unwrap(), format!("pending:rate_limited:{i}"));
            l.db()
                .execute(
                    "UPDATE session_full_summaries SET next_attempt_at = NULL",
                    [],
                )
                .unwrap();
        }
    }
    assert_eq!(text(&l, row).unwrap(), "failed:rate_limited:5");
    let out = loupe::summarize(l.data.path(), &["--retry"], &[]);
    // The full session summary and the session day summary that took its status.
    assert_eq!(loupe::count(&out, "rows"), 2);
    let after = "SELECT status || ':' || attempts FROM session_full_summaries";
    assert_eq!(text(&l, after).unwrap(), "pending:0");
}

#[tokio::test(flavor = "multi_thread")]
async fn no_credential_is_disabled() {
    let fake = fake::spawn().await;
    let l = one_closed(fake, 2).await;
    // The close of the segment queued its day; the final status queues it again.
    assert_eq!(
        text(&l, "SELECT reason FROM agg_queue").unwrap(),
        "session_closed"
    );
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[("LOUPE_SUMMARY_BASE_URL", &base(fake, "line_nocred"))],
    );
    assert_eq!(
        text(&l, "SELECT reason FROM agg_queue").unwrap(),
        "summary_final"
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(loupe::count(&out, "disabled"), 1);
    assert_eq!(
        text(&l, "SELECT status FROM session_day_summaries").unwrap(),
        "disabled"
    );
    assert_eq!(fake::summary_hits("line_nocred"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn summary_call_not_counted() {
    let fake = fake::spawn().await;
    let l = one_closed(fake, 2).await;
    let counts = |l: &Loupe| {
        (
            scalar(l, "SELECT count(*) FROM exchanges"),
            scalar(l, "SELECT count(*) FROM calls"),
            scalar(l, "SELECT sum(call_count) FROM sessions"),
        )
    };
    let before = counts(&l);
    run(&l, &base(fake, "line_counted"));
    // One full session summary and one session day summary; no segment line.
    assert_eq!(fake::summary_hits("line_counted"), 2);
    loupe::parse_and_close(l.data.path());
    assert_eq!(counts(&l), before);
}

#[test]
fn worker_skips_log_segment_transcript() {
    let (logs, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let root = bf::claude_tree(logs.path(), false);
    bf::ok(
        data.path(),
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &[],
    );
    let pending = "SELECT count(*) FROM session_day_summaries WHERE status = 'pending'";
    let retry = loupe::summarize(data.path(), &["--retry"], &[]);
    assert!(retry.status.success());
    assert_eq!(bf::scalar(data.path(), pending), 1);
    let out = loupe::summarize(
        data.path(),
        &["--once"],
        &[
            ("LOUPE_SUMMARY_API_KEY", KEY_VALUE),
            ("LOUPE_SUMMARY_BASE_URL", "http://127.0.0.1:9/summary/line"),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(loupe::count(&out, "segments_read"), 0);
    assert_eq!(bf::scalar(data.path(), pending), 1);
}
