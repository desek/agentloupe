//! @agents-index Integration test: a full capture queue spills to disk, every client still gets its full response, no record is lost.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

#[tokio::test(flavor = "multi_thread")]
async fn full_queue_keeps_responses() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[("LOUPE_CAPTURE_QUEUE", "1")]);
    // An exclusive transaction in this process blocks the writer of loupe on the database lock.
    let lock = l.db();
    lock.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let client = reqwest::Client::new();
    let calls = (0..10).map(|_| {
        let c = client.clone();
        let url = l.url("/pi/fake/bytes?n=1000");
        async move {
            let r = c.get(url).send().await.unwrap();
            (r.status().as_u16(), r.bytes().await.unwrap().len())
        }
    });
    for (status, len) in futures_util::future::join_all(calls).await {
        assert_eq!((status, len), (200, 1000));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !l.log().contains("spilled_total") {
        assert!(std::time::Instant::now() < deadline, "no spill warning");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        l.log()
            .lines()
            .any(|x| x.contains("\"WARN\"") || x.contains("warn"))
    );
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    l.wait_rows(10);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while l.spill_files() > 0 {
        assert!(std::time::Instant::now() < deadline, "spill not empty");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(l.rows(), 10);
}
