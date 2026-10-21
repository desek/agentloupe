//! @agents-index The limit of failed logins per remote address: 10 in 10 minutes blocks the address for 10 minutes, in memory only.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MAX_FAILS: usize = 10;
pub const WINDOW: Duration = Duration::from_secs(600);

#[derive(Default)]
struct Entry {
    fails: Vec<Instant>,
    blocked_until: Option<Instant>,
}

/// The failed logins per address. A restart clears it, so it never reaches the database.
#[derive(Default)]
pub struct Limiter {
    by_addr: Mutex<HashMap<IpAddr, Entry>>,
}

impl Limiter {
    /// True while `addr` is blocked.
    pub fn blocked(&self, addr: IpAddr, now: Instant) -> bool {
        let map = self.by_addr.lock().expect("limiter lock");
        map.get(&addr)
            .and_then(|e| e.blocked_until)
            .is_some_and(|t| now < t)
    }

    /// Records one failed login; the tenth in the window blocks the address for the window.
    pub fn fail(&self, addr: IpAddr, now: Instant) {
        let mut map = self.by_addr.lock().expect("limiter lock");
        // Drop the entries that are wholly in the past, so the map does not grow without limit.
        map.retain(|_, e| {
            e.blocked_until.is_some_and(|t| now < t)
                || e.fails.iter().any(|f| now.duration_since(*f) < WINDOW)
        });
        let e = map.entry(addr).or_default();
        e.fails.retain(|f| now.duration_since(*f) < WINDOW);
        e.fails.push(now);
        if e.fails.len() >= MAX_FAILS {
            e.blocked_until = Some(now + WINDOW);
            e.fails.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenth_failure_blocks_for_the_window() {
        let l = Limiter::default();
        let a: IpAddr = "127.0.0.1".parse().unwrap();
        let t = Instant::now();
        for _ in 0..9 {
            l.fail(a, t);
        }
        assert!(!l.blocked(a, t));
        l.fail(a, t);
        assert!(l.blocked(a, t));
        assert!(!l.blocked(a, t + WINDOW));
    }
}
