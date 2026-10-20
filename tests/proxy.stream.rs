//! @agents-index Integration test: server-sent events reach the client chunk by chunk, before the upstream stream ends.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;

use futures_util::StreamExt;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn sse_chunks_arrive_before_stream_end() {
    let fake = fake::spawn().await;
    let l = Loupe::start(fake, &[], &[]);
    let t0 = Instant::now();
    let r = reqwest::get(l.url("/pi/fake/sse?chunks=5&delay_ms=200"))
        .await
        .unwrap();
    assert_eq!(r.headers()["content-type"], "text/event-stream");
    let mut s = r.bytes_stream();
    let first = s.next().await.unwrap().unwrap();
    let at = t0.elapsed();
    assert!(String::from_utf8_lossy(&first).starts_with("data: "));
    assert!(at < Duration::from_millis(300), "first chunk after {at:?}");
    let mut n = first.len();
    while let Some(c) = s.next().await {
        n += c.unwrap().len();
    }
    assert!(t0.elapsed() >= Duration::from_millis(800));
    l.wait_rows(1);
    let size: i64 = l
        .db()
        .query_row("SELECT response_size FROM exchanges", [], |r| r.get(0))
        .unwrap();
    assert_eq!(size as usize, n);
}
