//! @agents-index The coalescing state of one client above the rate: a calls.batch sum, the last segments.open, and an exchanges.batch.

use super::event::{CallsBatch, Event, ExchangesBatch};

/// The events that wait for the next free slot of the rate limit.
#[derive(Default)]
pub struct Pending {
    calls: Option<CallsBatch>,
    segments: Option<String>,
    exchanges: Option<ExchangesBatch>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.calls.is_none() && self.segments.is_none() && self.exchanges.is_none()
    }

    /// Adds an event; only the last `segments.open` stays.
    pub fn add(&mut self, e: &Event) {
        match e {
            Event::ExchangeStart { .. } => self.exchanges.get_or_insert_default().started += 1,
            Event::ExchangeEnd { .. } => self.exchanges.get_or_insert_default().ended += 1,
            Event::CallStored {
                tokens, cost_usd, ..
            } => self.calls.get_or_insert_default().add(tokens, *cost_usd),
            Event::SegmentsOpen { .. } => self.segments = Some(e.frame()),
        }
    }

    /// The next frame to send, in the order exchanges, calls, segments; `in_flight` is the hub value now.
    pub fn take(&mut self, ts: i64, in_flight: i64) -> Option<String> {
        if let Some(mut x) = self.exchanges.take() {
            x.in_flight = in_flight;
            return Some(x.frame(ts));
        }
        if let Some(c) = self.calls.take() {
            return Some(c.frame(ts));
        }
        self.segments.take()
    }
}
