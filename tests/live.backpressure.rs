//! @agents-index Integration test of the back-pressure rule: a client that falls behind the hub is closed with 1008, and the sender never waits.

#[path = "common/live.rs"]
mod live;

use agent_loupe::live::hub::Hub;
use live::{config, frame, next, open, serve};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

// One thread: the burst of sends below runs with no await, so the client loop cannot read in between.
#[tokio::test]
async fn slow_client_dropped() {
    let hub = Hub::new(64);
    let (port, _d) = serve(hub.clone(), config(10, 64)).await;
    let (mut ws, _) = open(port, "/live", &[]).await.unwrap();
    frame(&mut ws, 5).await;
    let t0 = Instant::now();
    for _ in 0..10_000 {
        hub.exchange_start();
        hub.exchange_end(200);
    }
    let took = t0.elapsed();
    assert!(took < Duration::from_secs(1), "the sends waited {took:?}");
    loop {
        match next(&mut ws, 5).await {
            Some(Message::Close(Some(c))) => {
                assert_eq!(c.code, CloseCode::Policy);
                break;
            }
            Some(_) => continue,
            None => panic!("no close 1008"),
        }
    }
    println!("20000 sends took {took:?}");
}
