//! @agents-index The loop of one live client: the rate limit with coalescing, the heartbeat, the client frames, the lag and the write timeout.

use super::batch::Pending;
use super::event::{Event, heartbeat};
use super::hub::Hub;
use super::route::Slot;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::broadcast::{Receiver, error::RecvError};
use tokio::time::{Instant, sleep_until, timeout};

/// The idle time after which the route sends a heartbeat.
pub const HEARTBEAT: Duration = Duration::from_secs(15);
/// The longest time that one write may take before the client is closed.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// The largest client data frame that the route discards without a close.
const MAX_CLIENT_FRAME: usize = 1024;
/// Close codes: policy violation for a slow client, message too big for a large client frame.
const POLICY: u16 = 1008;
const TOO_BIG: u16 = 1009;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Why the loop ends.
enum End {
    Gone,
    /// The client sent a close; the socket layer queued the reply, which a flush sends.
    Reply,
    Close(u16, &'static str),
}

/// Runs until the client goes or the route closes it. `first` is the snapshot frame.
pub async fn run(
    socket: WebSocket,
    mut rx: Receiver<Event>,
    first: String,
    hub: Hub,
    rate: u32,
    _slot: Slot,
) {
    let (mut sink, mut stream) = socket.split();
    let gap = Duration::from_secs(1) / rate.max(1);
    let send = async |sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
                      text: String| {
        matches!(
            timeout(WRITE_TIMEOUT, sink.send(Message::Text(text.into()))).await,
            Ok(Ok(()))
        )
    };
    let mut last = Instant::now();
    let mut next_slot = last + gap;
    let mut pending = Pending::default();
    let end = if !send(&mut sink, first).await {
        End::Close(POLICY, "write timeout")
    } else {
        loop {
            let flush_at = if pending.is_empty() {
                last + HEARTBEAT * 2
            } else {
                next_slot
            };
            let frame = tokio::select! {
                r = rx.recv() => match r {
                    Ok(e) => {
                        let now = Instant::now();
                        if pending.is_empty() && now >= next_slot {
                            Some(e.frame())
                        } else {
                            pending.add(&e);
                            None
                        }
                    }
                    Err(RecvError::Lagged(_)) => break End::Close(POLICY, "the client is too slow"),
                    Err(RecvError::Closed) => break End::Gone,
                },
                m = stream.next() => match m {
                    None | Some(Err(_)) => break End::Gone,
                    Some(Ok(Message::Close(_))) => break End::Reply,
                    Some(Ok(Message::Text(t))) if t.len() > MAX_CLIENT_FRAME => break End::Close(TOO_BIG, "frame too large"),
                    Some(Ok(Message::Binary(b))) if b.len() > MAX_CLIENT_FRAME => break End::Close(TOO_BIG, "frame too large"),
                    // Pings get their pong from the socket layer; small data frames are discarded.
                    Some(Ok(_)) => None,
                },
                _ = sleep_until(flush_at), if !pending.is_empty() => pending.take(now_ms(), hub.in_flight()),
                _ = sleep_until(last + HEARTBEAT) => Some(heartbeat(now_ms())),
            };
            if let Some(f) = frame {
                if !send(&mut sink, f).await {
                    break End::Close(POLICY, "write timeout");
                }
                last = Instant::now();
                next_slot = last + gap;
            }
        }
    };
    match end {
        End::Close(code, reason) => {
            let close = Message::Close(Some(CloseFrame {
                code,
                reason: reason.into(),
            }));
            let _ = timeout(Duration::from_secs(1), sink.send(close)).await;
        }
        End::Reply => {
            let _ = timeout(Duration::from_secs(1), sink.close()).await;
        }
        End::Gone => {}
    }
}
