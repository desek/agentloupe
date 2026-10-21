//! @agents-index The injected monotonic clock of the token gate and the live client: the system clock in serve, a set clock in tests.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

/// A future that ends at an instant of a clock.
pub type Sleep = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A source of monotonic time that a component receives instead of reading the system clock,
/// so a test can assert a timer with no wait.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    /// Ends when `now()` reaches `at`.
    fn sleep_until(&self, at: Instant) -> Sleep;
}

/// The system clock; the binary uses only this one, so each timer keeps its value.
pub struct System;

impl Clock for System {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, at: Instant) -> Sleep {
        Box::pin(tokio::time::sleep_until(at))
    }
}

/// The system clock as a shared clock, the default of each component.
pub fn system() -> Arc<dyn Clock> {
    Arc::new(System)
}

/// A clock that a test sets: time moves only when the test calls `advance`.
pub struct Set {
    base: Instant,
    offset: watch::Sender<Duration>,
}

impl Set {
    pub fn new() -> Arc<Set> {
        Arc::new(Set {
            base: Instant::now(),
            offset: watch::Sender::new(Duration::ZERO),
        })
    }

    /// Moves the clock forward and wakes every sleep that ends on or before the new time.
    pub fn advance(&self, by: Duration) {
        self.offset.send_modify(|o| *o += by);
    }
}

impl Clock for Set {
    fn now(&self) -> Instant {
        self.base + *self.offset.borrow()
    }

    fn sleep_until(&self, at: Instant) -> Sleep {
        let (base, mut rx) = (self.base, self.offset.subscribe());
        Box::pin(async move {
            while base + *rx.borrow_and_update() < at {
                if rx.changed().await.is_err() {
                    // The clock is gone, so the time never comes.
                    std::future::pending::<()>().await;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_clock_sleep_ends_on_advance() {
        let c = Set::new();
        let at = c.now() + Duration::from_secs(15);
        let mut s = c.sleep_until(at);
        assert!(futures_util::poll!(&mut s).is_pending());
        c.advance(Duration::from_secs(14));
        assert!(futures_util::poll!(&mut s).is_pending());
        c.advance(Duration::from_secs(1));
        assert!(futures_util::poll!(&mut s).is_ready());
    }
}
