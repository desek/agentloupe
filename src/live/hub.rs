//! @agents-index The live hub: one bounded broadcast channel; each send never waits, and a send with no client does nothing.

use super::event::{Event, Tokens};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::broadcast;

struct Inner {
    tx: broadcast::Sender<Event>,
    /// Exchanges that started and did not end.
    in_flight: AtomicI64,
    /// Exchanges that ended since the process started; the public mirror carries it.
    ended: AtomicI64,
    /// The open segment count of the last parse pass; -1 before the first pass.
    segments: AtomicI64,
}

/// The handle that the capture, the proxy, and the parser hold. A hub that is off (the live feed is
/// disabled, or a subcommand other than serve) drops every event.
#[derive(Clone)]
pub struct Hub {
    inner: Option<Arc<Inner>>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Hub {
    /// A hub that drops every event and has no receiver.
    pub fn off() -> Hub {
        Hub { inner: None }
    }

    /// A hub with a capacity of `capacity` events.
    pub fn new(capacity: usize) -> Hub {
        let (tx, _) = broadcast::channel(capacity);
        Hub {
            inner: Some(Arc::new(Inner {
                tx,
                in_flight: AtomicI64::new(0),
                ended: AtomicI64::new(0),
                segments: AtomicI64::new(-1),
            })),
        }
    }

    /// Puts the event on the channel. A broadcast send never waits; with no receiver it returns an
    /// error that is ignored, so the sender never sees a client.
    pub fn send(&self, e: Event) {
        if let Some(i) = &self.inner {
            let _ = i.tx.send(e);
        }
    }

    pub fn exchange_start(&self) {
        if let Some(i) = &self.inner {
            i.in_flight.fetch_add(1, Ordering::Relaxed);
        }
        self.send(Event::ExchangeStart { ts: now_ms() });
    }

    pub fn exchange_end(&self, status: u16) {
        if let Some(i) = &self.inner {
            i.in_flight.fetch_sub(1, Ordering::Relaxed);
            i.ended.fetch_add(1, Ordering::Relaxed);
        }
        self.send(Event::ExchangeEnd {
            ts: now_ms(),
            status,
        });
    }

    pub fn call_stored(&self, tokens: Tokens, cost_usd: Option<Decimal>) {
        self.send(Event::CallStored {
            ts: now_ms(),
            tokens,
            cost_usd,
        });
    }

    /// Sends `segments.open` only when the count differs from the last pass.
    pub fn segments_open(&self, count: i64) {
        if let Some(i) = &self.inner
            && i.segments.swap(count, Ordering::Relaxed) != count
        {
            self.send(Event::SegmentsOpen {
                ts: now_ms(),
                count,
            });
        }
    }

    pub fn in_flight(&self) -> i64 {
        self.inner
            .as_ref()
            .map_or(0, |i| i.in_flight.load(Ordering::Relaxed).max(0))
    }

    /// The count of exchanges that ended since start; 0 when the hub is off.
    pub fn ended(&self) -> i64 {
        self.inner
            .as_ref()
            .map_or(0, |i| i.ended.load(Ordering::Relaxed))
    }

    /// A receiver for one client, or None when the hub is off.
    pub fn subscribe(&self) -> Option<broadcast::Receiver<Event>> {
        self.inner.as_ref().map(|i| i.tx.subscribe())
    }

    /// True if the parser should count the open segments after a pass.
    pub fn is_on(&self) -> bool {
        self.inner.is_some()
    }
}
