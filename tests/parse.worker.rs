//! @agents-index Integration test: the parser cursor neither loses nor repeats a call after a kill, and a second parse adds nothing.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

/// Sends `n` Anthropic Messages requests through the proxy, half JSON and half SSE.
async fn send(l: &Loupe, n: usize) {
    let c = reqwest::Client::new();
    for i in 0..n {
        let body = serde_json::json!({"model":"claude-haiku-5-5","stream": i % 2 == 1,
            "messages":[{"role":"user","content":format!("q{}", i % 5)}]});
        let r = c
            .post(l.url("/pi/fake/v1/messages"))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        r.bytes().await.unwrap();
    }
    l.wait_rows(n as i64);
}

fn scalar(l: &Loupe, sql: &str) -> i64 {
    l.db().query_row(sql, [], |r| r.get(0)).unwrap()
}

const SLOW_WORKER: (&str, &str) = ("LOUPE_PARSE_INTERVAL_SECS", "86400");

#[tokio::test(flavor = "multi_thread")]
async fn parse_twice_adds_nothing() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[SLOW_WORKER]);
    send(&l, 6).await;
    let first = loupe::parse(l.data.path(), &["--once"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(scalar(&l, "SELECT count(*) FROM calls"), 6);
    assert_eq!(
        scalar(
            &l,
            "SELECT count(*) FROM calls WHERE origin = 'proxy' AND log_key IS NULL"
        ),
        6
    );
    // Each new call puts its day in the aggregation queue in the same transaction.
    assert!(scalar(&l, "SELECT count(*) FROM agg_queue WHERE reason = 'call'") >= 1);
    let parsed_at = |l: &Loupe| -> String {
        l.db()
            .query_row("SELECT group_concat(parsed_at) FROM exchanges", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(
        scalar(&l, "SELECT count(*) FROM exchanges WHERE parsed_at IS NULL"),
        0
    );
    let first_times = parsed_at(&l);
    let second = loupe::parse(l.data.path(), &["--once"]);
    assert!(second.status.success());
    assert_eq!(parsed_at(&l), first_times);
    assert_eq!(loupe::count(&second, "calls_added"), 0);
    assert_eq!(loupe::count(&second, "exchanges_read"), 0);
    assert_eq!(scalar(&l, "SELECT count(*) FROM calls"), 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_does_not_lose_or_repeat_calls() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[SLOW_WORKER]);
    send(&l, 300).await;
    let mut child = loupe::parse_command(l.data.path(), &["--once"])
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(40));
    let _ = child.kill();
    let _ = child.wait();
    let partial = scalar(&l, "SELECT count(*) FROM calls");
    let again = loupe::parse(l.data.path(), &["--once"]);
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(loupe::count(&again, "calls_added"), 300 - partial);
    assert_eq!(scalar(&l, "SELECT count(*) FROM calls"), 300);
    assert_eq!(
        scalar(&l, "SELECT count(DISTINCT exchange_id) FROM calls"),
        300
    );
    assert_eq!(scalar(&l, "SELECT sum(call_count) FROM sessions"), 300);
    assert_eq!(
        scalar(
            &l,
            "SELECT count(*) FROM exchanges WHERE parse_outcome = 'call'"
        ),
        300
    );
}
