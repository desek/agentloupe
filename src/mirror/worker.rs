//! @agents-index The mirror task: reconciles days every 5 s and writes live_state on change (1 s minimum, 5 s maximum) through an injected clock.

use super::{days, live};
use crate::live::hub::Hub;
use crate::publish::clock::Clock;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The interval of the days reconcile, and the longest gap between two live_state writes.
pub const RECONCILE_MS: i64 = 5_000;
/// The shortest gap between two live_state writes.
pub const LIVE_MIN_MS: i64 = 1_000;

/// What one pass did; a failed write counts in `failures` and logged one warning.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub days: days::Changes,
    pub live_written: bool,
    pub failures: u32,
}

/// The state of the task between passes. The binary drives it from a thread; a test calls `pass`
/// with a set clock.
pub struct Task {
    src: Connection,
    mirror: Connection,
    tz: chrono_tz::Tz,
    hub: Hub,
    clock: Box<dyn Clock>,
    last_reconcile: Option<i64>,
    last_live: Option<(i64, live::Row)>,
}

fn warn(what: &str, e: &rusqlite::Error) {
    tracing::warn!(error = %e, "the public mirror {what} write failed; the next pass tries again. Fix 1: check the disk space and the permissions of <data dir>/public. Fix 2: stop each other process that writes public.db. See docs/specification/days.md, section Public mirror. Verify: the next pass logs no warning and GET /v1/days of the role public shows the published days");
}

impl Task {
    pub fn new(
        src: Connection,
        mirror: Connection,
        tz: chrono_tz::Tz,
        hub: Hub,
        clock: Box<dyn Clock>,
    ) -> Task {
        Task {
            src,
            mirror,
            tz,
            hub,
            clock,
            last_reconcile: None,
            last_live: None,
        }
    }

    /// One pass: the days reconcile when due, then the live row when due. A failure logs one
    /// warning and leaves the due time, so the next pass tries again.
    pub fn pass(&mut self) -> Pass {
        let now = self.clock.now_ms();
        let mut p = Pass::default();
        if self.last_reconcile.is_none_or(|t| now - t >= RECONCILE_MS) {
            match days::reconcile(&self.src, &mut self.mirror) {
                Ok(c) => {
                    p.days = c;
                    self.last_reconcile = Some(now);
                }
                Err(e) => {
                    warn("days", &e);
                    p.failures += 1;
                }
            }
        }
        // Inside the shortest gap no write can be due, so the totals are not read.
        if self
            .last_live
            .as_ref()
            .is_some_and(|(t, _)| now - t < LIVE_MIN_MS)
        {
            return p;
        }
        match live::read(&self.src, self.tz, now, &self.hub) {
            Ok(row) => {
                let due = match &self.last_live {
                    None => true,
                    Some((t, last)) => {
                        now - t >= RECONCILE_MS || (now - t >= LIVE_MIN_MS && *last != row)
                    }
                };
                if due {
                    match live::write(&mut self.mirror, &row, now) {
                        Ok(()) => {
                            self.last_live = Some((now, row));
                            p.live_written = true;
                        }
                        Err(e) => {
                            warn("live_state", &e);
                            p.failures += 1;
                        }
                    }
                }
            }
            Err(e) => {
                warn("live_state", &e);
                p.failures += 1;
            }
        }
        p
    }

    /// The clean shutdown write: `in_flight` 0.
    pub fn shutdown(&self) {
        if let Err(e) = live::clear_in_flight(&self.mirror, self.clock.now_ms()) {
            warn("live_state", &e);
        }
    }
}

/// The running task of `loupe serve`. `stop` ends the thread and writes `in_flight` 0.
pub struct Handle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Starts the thread: a pass at start, then one each 100 ms of sleep; the clock decides what is due.
/// It is never in the request path, so a slow write never delays the proxy.
pub fn start(mut task: Task) -> Handle {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            task.pass();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        task.shutdown();
    });
    Handle {
        stop,
        thread: Some(thread),
    }
}

impl Handle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
