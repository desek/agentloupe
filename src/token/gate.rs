//! @agents-index The token gate: reads the token state with a cache of at most 1 second, accepts or refuses a request, writes last use.

use super::{format, store};
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

/// The longest time that a cached token state is used; the limit in the specification is 5 seconds.
const TTL: Duration = Duration::from_secs(1);
/// At most one write of `last_used_at` per token in this time.
const TOUCH_EVERY: Duration = Duration::from_secs(60);

/// The three states of the gate, from the rows of `proxy_tokens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The table never held a row.
    NoToken,
    /// At least one row is active.
    Required,
    /// Rows exist and every row is revoked.
    Closed,
}

impl State {
    /// The start warning for this state, if any; it names the fix.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            State::NoToken => Some(
                "no proxy token exists, so the proxy is open to every client that can reach the port. Fix: run loupe token create --name <name>, or set LOUPE_BOOTSTRAP_TOKEN to a value from loupe token generate and restart. Reference: docs/specification/proxy.tokens.md, section Token gate. Verify: loupe token list shows an active token",
            ),
            State::Closed => Some(
                "every proxy token is revoked, so the proxy refuses all proxied requests. Fix: run loupe token create --name <name>, or set LOUPE_BOOTSTRAP_TOKEN to a value from loupe token generate and restart. Reference: docs/specification/proxy.tokens.md, section Token gate. Verify: loupe token list shows an active token",
            ),
            State::Required => None,
        }
    }
}

/// The token that passed the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub id: i64,
    pub name: String,
}

/// The result of the gate for one request.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Forward; `None` in no-token mode.
    Accept(Option<Accepted>),
    /// Refuse with this error code.
    Refuse(&'static str),
}

struct Snapshot {
    at: Instant,
    state: State,
    by_hash: HashMap<String, store::GateRow>,
}

/// Shared by every request; built once in serve.
pub struct Gate {
    reads: Mutex<Connection>,
    cache: Mutex<Option<Snapshot>>,
    last_touch: Mutex<HashMap<i64, Instant>>,
    touch_tx: mpsc::Sender<i64>,
    /// How many writes of `last_used_at` were sent, for tests.
    pub touches: AtomicU64,
}

impl Gate {
    /// `writes` moves to a thread that writes `last_used_at`, so a request never waits for that write.
    pub fn new(reads: Connection, writes: Connection) -> Gate {
        let (touch_tx, rx) = mpsc::channel::<i64>();
        std::thread::spawn(move || {
            for id in rx {
                if let Err(e) = store::touch(&writes, id) {
                    tracing::warn!(error = %e, "last_used_at of a proxy token was not written; the request was not affected");
                }
            }
        });
        Gate {
            reads: Mutex::new(reads),
            cache: Mutex::new(None),
            last_touch: Mutex::new(HashMap::new()),
            touch_tx,
            touches: AtomicU64::new(0),
        }
    }

    /// The current state, read from the database if the cache is older than the limit.
    pub fn state(&self) -> Result<State, String> {
        self.with_snapshot(Instant::now(), |s| s.state)
    }

    fn with_snapshot<T>(&self, now: Instant, f: impl FnOnce(&Snapshot) -> T) -> Result<T, String> {
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        if cache
            .as_ref()
            .is_none_or(|s| now.duration_since(s.at) >= TTL)
        {
            let rows = {
                let conn = self.reads.lock().unwrap_or_else(|p| p.into_inner());
                store::gate_rows(&conn).map_err(|e| e.to_string())?
            };
            let state = if rows.is_empty() {
                State::NoToken
            } else if rows.iter().any(|r| !r.revoked) {
                State::Required
            } else {
                State::Closed
            };
            if state == State::Closed && cache.as_ref().is_some_and(|s| s.state == State::Required)
            {
                tracing::warn!("{}", State::Closed.warning().unwrap_or_default());
            }
            let by_hash = rows
                .into_iter()
                .map(|r| (r.token_hash.clone(), r))
                .collect();
            *cache = Some(Snapshot {
                at: now,
                state,
                by_hash,
            });
        }
        Ok(f(cache.as_ref().expect("set above")))
    }

    /// Decides one request; `presented` is the raw token segment, if any. A read failure refuses.
    pub fn check(&self, presented: Option<&str>) -> Decision {
        self.check_at(presented, Instant::now())
    }

    pub fn check_at(&self, presented: Option<&str>, now: Instant) -> Decision {
        let h = presented.map(format::hash);
        let found = self.with_snapshot(now, |s| {
            let row = h.as_ref().and_then(|h| {
                s.by_hash
                    .get(h)
                    .filter(|r| bool::from(r.token_hash.as_bytes().ct_eq(h.as_bytes())))
                    .cloned()
            });
            (s.state, row)
        });
        let (state, row) = match found {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "the token state cannot be read, the request is refused. Fix: check that <LOUPE_DATA_DIR>/loupe.db is readable. Verify: loupe token list exits 0");
                return Decision::Refuse("token_state_unavailable");
            }
        };
        match (state, presented, row) {
            (State::NoToken, None, _) => Decision::Accept(None),
            (State::NoToken, Some(_), _) => Decision::Refuse("token_mode_off"),
            (_, None, _) => Decision::Refuse("token_missing"),
            (_, Some(_), None) => Decision::Refuse("token_unknown"),
            (_, Some(_), Some(r)) if r.revoked => Decision::Refuse("token_revoked"),
            (_, Some(_), Some(r)) => {
                self.touch(r.id, now);
                Decision::Accept(Some(Accepted {
                    id: r.id,
                    name: r.name,
                }))
            }
        }
    }

    fn touch(&self, id: i64, now: Instant) {
        let mut last = self.last_touch.lock().unwrap_or_else(|p| p.into_inner());
        if last
            .get(&id)
            .is_some_and(|t| now.duration_since(*t) < TOUCH_EVERY)
        {
            return;
        }
        last.insert(id, now);
        self.touches.fetch_add(1, Ordering::Relaxed);
        let _ = self.touch_tx.send(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn gate(dir: &std::path::Path) -> Gate {
        let open = || crate::store::open::open(dir).unwrap();
        Gate::new(open(), open())
    }

    #[test]
    fn gate_states() {
        let (d, conn) = store::tests::db();
        let t = Instant::now();
        let g = gate(d.path());
        let valid = format::generate();
        let other = format::generate();
        assert_eq!(g.check_at(None, t), Decision::Accept(None));
        assert_eq!(
            g.check_at(Some(&valid), t),
            Decision::Refuse("token_mode_off")
        );
        store::create(&conn, "a", &BTreeMap::new(), &format::hash(&valid)).unwrap();
        let old = format::generate();
        store::create(&conn, "old", &BTreeMap::new(), &format::hash(&old)).unwrap();
        store::revoke(&conn, "old").unwrap();
        let t = t + TTL;
        assert_eq!(g.check_at(None, t), Decision::Refuse("token_missing"));
        assert_eq!(
            g.check_at(Some(&other), t),
            Decision::Refuse("token_unknown")
        );
        assert_eq!(g.check_at(Some(&old), t), Decision::Refuse("token_revoked"));
        let id: i64 = conn
            .query_row("SELECT id FROM proxy_tokens WHERE name = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            g.check_at(Some(&valid), t),
            Decision::Accept(Some(Accepted {
                id,
                name: "a".into()
            }))
        );
        store::revoke(&conn, "a").unwrap();
        let t = t + TTL;
        assert_eq!(g.state_at(t), State::Closed);
        assert_eq!(g.check_at(None, t), Decision::Refuse("token_missing"));
        assert_eq!(
            g.check_at(Some(&valid), t),
            Decision::Refuse("token_revoked")
        );
    }

    #[test]
    fn last_used_written_once_per_minute() {
        let (d, conn) = store::tests::db();
        let raw = format::generate();
        store::create(&conn, "a", &BTreeMap::new(), &format::hash(&raw)).unwrap();
        let g = gate(d.path());
        let t = Instant::now();
        for i in 0..50 {
            let at = t + Duration::from_millis(200 * i);
            assert!(matches!(
                g.check_at(Some(&raw), at),
                Decision::Accept(Some(_))
            ));
        }
        assert_eq!(g.touches.load(Ordering::Relaxed), 1);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let used: Option<String> = conn
                .query_row("SELECT last_used_at FROM proxy_tokens", [], |r| r.get(0))
                .unwrap();
            if used.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "last_used_at was not written");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    impl Gate {
        fn state_at(&self, now: Instant) -> State {
            self.with_snapshot(now, |s| s.state).unwrap()
        }
    }
}
