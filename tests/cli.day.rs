//! @agents-index Integration test: manual review hides a day until approve, a tombstone blocks the catch-up, restore publishes again.

#[path = "common/days.rs"]
mod days;
#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use std::time::{Duration, Instant};

const D: &str = "2026-10-10";

/// A loupe serve over a store with one call on D, with the publisher every second.
async fn start(review: &str) -> loupe::Loupe {
    let (dir, c) = days::db();
    days::simple(&c, "2026-10-10T10:00:00.000Z", "pi");
    drop(c);
    let fake = fake::spawn().await;
    loupe::Loupe::start_in(
        dir,
        fake,
        &[],
        &[
            ("LOUPE_TZ", "UTC"),
            ("LOUPE_DAY_REVIEW", review),
            ("LOUPE_PUBLISH_INTERVAL_SECS", "1"),
        ],
    )
}

fn wait_row(l: &loupe::Loupe) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while days::scalar(&l.db(), "SELECT count(*) FROM days") == 0 {
        assert!(Instant::now() < deadline, "no days row");
        std::thread::sleep(Duration::from_millis(100));
    }
}

async fn status(l: &loupe::Loupe) -> u16 {
    reqwest::get(l.url(&format!("/v1/days/{D}")))
        .await
        .unwrap()
        .status()
        .as_u16()
}

fn day(l: &loupe::Loupe, cmd: &str, env: &[(&str, &str)]) -> std::process::Output {
    let out = days::loupe(l.data.path(), &["day", cmd, "--date", D], env);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_review_hides_until_approve() {
    let l = start("manual").await;
    wait_row(&l);
    assert_eq!(
        days::text(&l.db(), "SELECT state FROM days").unwrap(),
        "pending_review"
    );
    assert_eq!(status(&l).await, 404);
    let out = day(&l, "approve", &[]);
    assert_eq!(days::field(&out, "state"), "published");
    assert_eq!(days::field(&out, "written"), "true");
    assert_eq!(status(&l).await, 200);
    let again = day(&l, "approve", &[]);
    assert_eq!(days::field(&again, "written"), "false");
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_tombstone_blocks_catch_up() {
    let l = start("auto").await;
    wait_row(&l);
    assert_eq!(status(&l).await, 200);
    let out = day(&l, "delete", &[]);
    assert_eq!(days::field(&out, "state"), "deleted");
    std::thread::sleep(Duration::from_millis(2500));
    let db = l.db();
    assert_eq!(
        days::text(&db, "SELECT state FROM days").unwrap(),
        "deleted"
    );
    assert!(days::text(&db, "SELECT record FROM days").is_none());
    assert_eq!(status(&l).await, 404);
    let approve = days::loupe(l.data.path(), &["day", "approve", "--date", D], &[]);
    assert_eq!(approve.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&approve.stderr).contains("loupe day restore --date"));
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_publishes_again() {
    let l = start("auto").await;
    wait_row(&l);
    day(&l, "delete", &[]);
    assert_eq!(status(&l).await, 404);
    let out = day(&l, "restore", &[]);
    assert_eq!(days::field(&out, "state"), "published");
    assert_eq!(status(&l).await, 200);
    let edit = days::loupe(
        l.data.path(),
        &["day", "edit", "--date", D, "--summary", "Edited day."],
        &[],
    );
    assert_eq!(days::field(&edit, "summary"), "present");
    assert!(!String::from_utf8_lossy(&edit.stdout).contains("Edited day."));
    let body = reqwest::get(l.url(&format!("/v1/days/{D}")))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("Edited day."));
    let empty = days::loupe(
        l.data.path(),
        &["day", "edit", "--date", D, "--summary", ""],
        &[],
    );
    assert_eq!(empty.status.code(), Some(2));
}
