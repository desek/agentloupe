//! @agents-index Integration test: the retention keeps the bodies of a session until its session day summaries are final, and logs bodies_held.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/summary.rs"]
mod summary;

use summary::{QUIET, scalar, send_session};

/// A loupe with one closed segment of one call, summarized with no credential, and its session day
/// summary set back to pending with a next attempt far in the future, so no pass takes it.
async fn held(fake: u16) -> loupe::Loupe {
    let l = loupe::Loupe::start(fake, &[], QUIET);
    send_session(&l, "r1", 1).await;
    loupe::parse_and_close(l.data.path());
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[("LOUPE_RAW_RETENTION_HOURS", "100000")],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    l.db()
        .execute_batch("UPDATE session_day_summaries SET status = 'pending', next_attempt_at = '2999-01-01T00:00:00.000Z'")
        .unwrap();
    l
}

fn bodies(l: &loupe::Loupe) -> i64 {
    scalar(
        l,
        "SELECT count(*) FROM exchanges WHERE request_body IS NOT NULL",
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn bodies_wait_for_session_day_summaries() {
    let fake = fake::spawn().await;
    let l = held(fake).await;
    let now = chrono::Utc::now().timestamp_millis() + 1000;
    let c = agent_loupe::retention::worker::pass(&l.db(), 0, now, false).unwrap();
    assert_eq!((c.bodies_deleted, c.bodies_held), (0, 1));
    assert_eq!(bodies(&l), 1);
    l.db()
        .execute_batch("UPDATE session_day_summaries SET status = 'done', text = 'Worked on it.'")
        .unwrap();
    let c = agent_loupe::retention::worker::pass(&l.db(), 0, now, false).unwrap();
    assert_eq!((c.bodies_deleted, c.bodies_held), (1, 0));
    assert_eq!(bodies(&l), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn bodies_held_counter_logged() {
    let fake = fake::spawn().await;
    let l = held(fake).await;
    let out = loupe::summarize(
        l.data.path(),
        &["--once"],
        &[("LOUPE_RAW_RETENTION_HOURS", "0")],
    );
    assert!(out.status.success());
    let line = l
        .log()
        .lines()
        .rev()
        .find(|x| x.contains("retention pass done"))
        .map(str::to_string)
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["fields"]["bodies_held"], 1, "{line}");
    assert_eq!(bodies(&l), 1);
}
