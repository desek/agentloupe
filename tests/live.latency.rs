//! @agents-index Measurement test: the 95th percentile time from the end of an exchange to its end frame on a live client, over 1000 exchanges.

#[path = "common/fake_upstream.rs"]
mod fake;
#[path = "common/live.rs"]
mod live;
#[path = "common/loupe.rs"]
mod loupe;

use loupe::Loupe;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const N: usize = 1000;
const THRESHOLD_MS: f64 = 250.0;

#[tokio::test(flavor = "multi_thread")]
async fn exchange_end_latency_p95() {
    let l = Loupe::start(
        fake::spawn().await,
        &[],
        &[("LOUPE_PARSE_INTERVAL_SECS", "86400")],
    );
    let (mut ws, _) = live::open(l.port, "/live", &[]).await.unwrap();
    live::frame(&mut ws, 5).await;
    // Each entry: the receive time and the count of ended exchanges so far.
    let seen: Arc<Mutex<Vec<(Instant, usize)>>> = Arc::default();
    let s2 = seen.clone();
    let reader = tokio::spawn(async move {
        let mut ended = 0;
        while ended < N {
            let Some(Message::Text(t)) = live::next(&mut ws, 10).await else {
                continue;
            };
            let f: serde_json::Value = serde_json::from_str(&t).unwrap();
            ended += match f["kind"].as_str().unwrap() {
                "exchange.end" => 1,
                "exchanges.batch" => f["data"]["ended"].as_u64().unwrap() as usize,
                _ => 0,
            };
            s2.lock().unwrap().push((Instant::now(), ended));
        }
    });
    let c = reqwest::Client::new();
    let mut ends = Vec::with_capacity(N);
    for _ in 0..N {
        let r = c.get(l.url("/pi/fake/json")).send().await.unwrap();
        r.bytes().await.unwrap();
        ends.push(Instant::now());
    }
    tokio::time::timeout(Duration::from_secs(30), reader)
        .await
        .unwrap()
        .unwrap();
    let seen = seen.lock().unwrap();
    let mut lat: Vec<f64> = ends
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let (at, _) = seen.iter().find(|(_, n)| *n > i).unwrap();
            at.saturating_duration_since(*e).as_secs_f64() * 1000.0
        })
        .collect();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = lat[(N * 95).div_ceil(100) - 1];
    println!("exchanges: {N} p95_ms: {p95:.1} threshold_ms: {THRESHOLD_MS}");
    assert!(p95 <= THRESHOLD_MS);
}
