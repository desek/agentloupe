//! @agents-index The one injected clock of the publisher: the system clock in serve, a set clock in tests.

use std::sync::atomic::{AtomicI64, Ordering};

pub trait Clock: Send + Sync {
    /// Unix milliseconds now.
    fn now_ms(&self) -> i64;
}

pub struct System;

impl Clock for System {
    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

/// A clock that a test sets.
pub struct Fixed(pub AtomicI64);

impl Fixed {
    pub fn new(ms: i64) -> Fixed {
        Fixed(AtomicI64::new(ms))
    }

    pub fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}

impl Clock for Fixed {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
