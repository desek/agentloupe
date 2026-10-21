//! @agents-index Integration test of the rate limit: events above the rate coalesce into batches, and an idle socket gets a heartbeat.

#[path = "common/live.rs"]
mod live;

use agent_loupe::live::event::Tokens;
use agent_loupe::live::hub::Hub;
use live::{config, frame, open, serve};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test(flavor = "multi_thread")]
async fn coalesces_above_rate() {
    let hub = Hub::new(4096);
    let (port, _d) = serve(hub.clone(), config(10, 4096)).await;
    let (mut ws, _) = open(port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    let t = Tokens {
        input: 1,
        output: 2,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
    };
    let sender = tokio::spawn(async move {
        for _ in 0..50 {
            for _ in 0..10 {
                hub.call_stored(t, None);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    let mut at = Vec::new();
    let (mut calls, mut input) = (0, 0);
    let end = Instant::now() + Duration::from_millis(3000);
    while Instant::now() < end {
        if let Some(Message::Text(x)) = live::next(&mut ws, 1).await {
            let f: serde_json::Value = serde_json::from_str(&x).unwrap();
            at.push(Instant::now());
            match f["kind"].as_str().unwrap() {
                "call.stored" => calls += 1,
                "calls.batch" => calls += f["data"]["calls"].as_i64().unwrap(),
                k => panic!("unexpected {k}"),
            }
            input += f["data"]["input"].as_i64().unwrap();
        }
    }
    sender.await.unwrap();
    assert_eq!((calls, input), (500, 500));
    for (i, a) in at.iter().enumerate() {
        let in_second = at[i..]
            .iter()
            .filter(|b| b.duration_since(*a) < Duration::from_millis(950))
            .count();
        assert!(in_second <= 10, "{in_second} frames in one second");
    }
    println!("frames: {}", at.len());
}

#[tokio::test(start_paused = true)]
async fn heartbeat_when_idle() {
    let (port, _d) = serve(Hub::new(64), config(10, 64)).await;
    let (mut ws, _) = open(port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    let t0 = tokio::time::Instant::now();
    let f = frame(&mut ws, 16).await;
    assert_eq!(f["kind"], "heartbeat");
    assert_eq!(live::keys(&f), ["ts"]);
    assert!(t0.elapsed() >= Duration::from_secs(15));
}
