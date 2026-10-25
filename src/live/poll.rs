//! @agents-index The live poller of the role public: each second it reads live_state of the mirror and sends the differences to the local hub.

use super::event::{CallsBatch, Event, ExchangesBatch};
use super::hub::Hub;
use super::snapshot_mirror::{self, Row};
use crate::clock::Clock;
use rusqlite::Connection;
use std::sync::Arc;
use std::time::Duration;

/// The fixed interval of the poll; it has no variable.
pub const INTERVAL: Duration = Duration::from_secs(1);

/// The check of each pass on the table set and the version; the binary passes one that stops
/// the process, a test passes one that records the call.
pub type Check = Box<dyn Fn(&Connection) + Send>;

/// The state between two passes: the mirror connection, the local hub, and the last row.
pub struct Poller {
    conn: Connection,
    hub: Hub,
    check: Check,
    last: Option<Row>,
}

/// The events from the row of the last pass to the row of this pass, in send order.
pub fn diff(last: &Row, new: &Row, ts: i64) -> Vec<Event> {
    let (a, b) = (&last.snapshot, &new.snapshot);
    if a.date != b.date {
        return vec![Event::Snapshot(b.clone())];
    }
    let mut out = Vec::new();
    // A counter that goes down means the role full started again: no negative difference.
    let calls_down = b.calls < a.calls
        || b.cost_usd < a.cost_usd
        || b.tokens.input < a.tokens.input
        || b.tokens.output < a.tokens.output
        || b.tokens.cache_read < a.tokens.cache_read
        || b.tokens.cache_write < a.tokens.cache_write
        || b.tokens.reasoning < a.tokens.reasoning;
    if !calls_down && b.calls > a.calls {
        out.push(Event::CallsBatch {
            ts,
            batch: CallsBatch {
                calls: b.calls - a.calls,
                tokens: super::event::Tokens {
                    input: b.tokens.input - a.tokens.input,
                    output: b.tokens.output - a.tokens.output,
                    cache_read: b.tokens.cache_read - a.tokens.cache_read,
                    cache_write: b.tokens.cache_write - a.tokens.cache_write,
                    reasoning: b.tokens.reasoning - a.tokens.reasoning,
                },
                cost_usd: Some(b.cost_usd - a.cost_usd),
            },
        });
    }
    if b.segments_open != a.segments_open {
        out.push(Event::SegmentsOpen {
            ts,
            count: b.segments_open,
        });
    }
    let ended = (new.ended - last.ended).max(0);
    if ended > 0 || b.in_flight != a.in_flight {
        out.push(Event::ExchangesBatch {
            ts,
            batch: ExchangesBatch {
                started: (ended + b.in_flight - a.in_flight).max(0),
                ended,
                in_flight: b.in_flight,
            },
        });
    }
    out
}

impl Poller {
    pub fn new(conn: Connection, hub: Hub, check: Check) -> Poller {
        Poller {
            conn,
            hub,
            check,
            last: None,
        }
    }

    /// One pass at the wall time `now_ms`: the check, one read transaction, the in-flight count
    /// of the hub, then the events. A read failure logs one warning and keeps the base.
    pub fn pass(&mut self, now_ms: i64) -> Vec<Event> {
        (self.check)(&self.conn);
        let read = self.conn.unchecked_transaction().and_then(|tx| {
            let r = snapshot_mirror::row(&tx, now_ms)?;
            tx.commit()?;
            Ok(r)
        });
        let row = match read {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "the live poll read of the public mirror failed; the next pass tries again. Fix 1: check the mount of the volume loupe-public. Fix 2: check that the role full runs and writes the mirror. See docs/specification/live.md, section Public role. Verify: the next pass logs no warning and /live sends frames");
                return Vec::new();
            }
        };
        let Some(row) = row else {
            self.hub.set_in_flight(0);
            self.last = None;
            return Vec::new();
        };
        self.hub.set_in_flight(row.snapshot.in_flight);
        let events = match &self.last {
            Some(last) => diff(last, &row, now_ms),
            // The first pass is the base; each client got its snapshot from the route.
            None => Vec::new(),
        };
        for e in &events {
            self.hub.send(e.clone());
        }
        self.last = Some(row);
        events
    }
}

/// Runs the poller until the task ends: a pass at start, then one each `INTERVAL` of `clock`.
pub async fn run(
    mut poller: Poller,
    clock: Arc<dyn Clock>,
    wall: Arc<dyn crate::publish::clock::Clock>,
) {
    let mut next = clock.now();
    loop {
        clock.sleep_until(next).await;
        next += INTERVAL;
        let now = wall.now_ms();
        poller = match tokio::task::spawn_blocking(move || {
            poller.pass(now);
            poller
        })
        .await
        {
            Ok(p) => p,
            Err(_) => return,
        };
    }
}
