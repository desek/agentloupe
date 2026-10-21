//! @agents-index Integration test: the retention deletes bodies only after parse and a final summary, keeps calls, sessions, and decrypt_failed.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/summary.rs"]
mod summary;

use loupe::Loupe;
use summary::{QUIET, base, scalar, send_session, text};

fn run(l: &Loupe, url: &str, hours: &str) -> std::process::Output {
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", url),
            ("LOUPE_RAW_RETENTION_HOURS", hours),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Every column of calls and sessions as one text, to prove the deletion leaves them unchanged.
fn kept_rows(l: &Loupe) -> String {
    text(l, "SELECT group_concat(x, '|') FROM (SELECT id || exchange_id || session_id || input || output || coalesce(cost_usd, '') AS x FROM calls UNION ALL SELECT id || state || call_count || started_at || coalesce(closed_at, '') FROM sessions)").unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn deletes_after_parse_and_summary() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    send_session(&l, "done1", 2).await;
    loupe::parse_and_close(l.data.path());
    let out = run(&l, &base(fake, "line"), "168");
    assert_eq!(loupe::count(&out, "done"), 1);
    assert_eq!(loupe::count(&out, "bodies_deleted"), 0);
    send_session(&l, "pending1", 2).await;
    loupe::parse_and_close(l.data.path());
    run(&l, &base(fake, "skip"), "168");
    let before = kept_rows(&l);
    let out = run(&l, &base(fake, "skip"), "0");
    assert_eq!(loupe::count(&out, "bodies_deleted"), 2);
    let deleted = "SELECT count(*) FROM exchanges WHERE request_body IS NULL AND response_body IS NULL AND bodies_deleted_at IS NOT NULL";
    assert_eq!(scalar(&l, deleted), 2);
    assert_eq!(
        scalar(
            &l,
            "SELECT count(*) FROM exchanges WHERE bodies_deleted_at IS NULL AND request_body IS NOT NULL"
        ),
        2
    );
    assert_eq!(kept_rows(&l), before);
    assert_eq!(
        scalar(&l, "SELECT count(*) FROM exchanges WHERE parsed_at IS NULL"),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_decrypt_failed() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], QUIET);
    send_session(&l, "bad1", 1).await;
    let other = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";
    let out = loupe::parse_command(l.data.path(), &["--once"])
        .env("LOUPE_DATA_KEY", other)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        scalar(
            &l,
            "SELECT count(*) FROM exchanges WHERE parse_outcome = 'decrypt_failed'"
        ),
        1
    );
    let out = run(&l, &base(fake, "line"), "0");
    assert_eq!(loupe::count(&out, "bodies_kept_decrypt_failed"), 1);
    assert_eq!(loupe::count(&out, "bodies_deleted"), 0);
    assert_eq!(
        scalar(
            &l,
            "SELECT count(*) FROM exchanges WHERE request_body IS NOT NULL"
        ),
        1
    );
}
