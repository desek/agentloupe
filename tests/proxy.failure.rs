//! @agents-index Integration test: header timeouts, an unreachable upstream gives 502, a capture failure never breaks forwarding.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

fn outcome(l: &Loupe) -> (i64, String) {
    l.wait_rows(1);
    l.db()
        .query_row("SELECT status, outcome FROM exchanges", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_headers_under_timeout_pass() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS", "2")]);
    let r = reqwest::get(l.url("/pi/fake/slow?headers_ms=1000"))
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.bytes().await.unwrap();
    assert_eq!(outcome(&l), (200, "complete".into()));
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_headers_over_timeout_is_502() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS", "1")]);
    let r = reqwest::get(l.url("/pi/fake/slow?headers_ms=3000"))
        .await
        .unwrap();
    assert_eq!(r.status(), 502);
    let body: serde_json::Value = r.json().await.unwrap();
    assert!(
        body["cause"].as_str().unwrap().contains("timeout"),
        "{body}"
    );
    assert_eq!(body["upstream"], "fake");
    assert_eq!(outcome(&l), (502, "upstream_unreachable".into()));
}

#[tokio::test(flavor = "multi_thread")]
async fn unreachable_upstream_is_502() {
    let fake = fake::spawn().await;
    let closed = loupe::free_port();
    let l = Loupe::start(fake, &[("dead", format!("http://127.0.0.1:{closed}"))], &[]);
    let r = reqwest::get(l.url("/pi/dead/v1/x")).await.unwrap();
    assert_eq!(r.status(), 502);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["upstream"], "dead");
    assert!(!body["cause"].as_str().unwrap().is_empty());
    assert_eq!(outcome(&l), (502, "upstream_unreachable".into()));
    let err: Option<String> = l
        .db()
        .query_row("SELECT error FROM exchanges", [], |r| r.get(0))
        .unwrap();
    assert!(err.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn capture_failure_does_not_break_forwarding() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    // Every insert into the table fails, as on a database that is read-only.
    l.db()
        .execute_batch("CREATE TRIGGER deny BEFORE INSERT ON exchanges BEGIN SELECT RAISE(ABORT, 'read-only'); END;")
        .unwrap();
    for _ in 0..2 {
        let r = reqwest::get(l.url("/pi/fake/bytes?n=200000"))
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(r.bytes().await.unwrap().len(), 200000);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !l
        .log()
        .lines()
        .any(|x| x.contains("ERROR") && x.contains("Fix:"))
    {
        assert!(
            std::time::Instant::now() < deadline,
            "no error line: {}",
            l.log()
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
