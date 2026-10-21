//! @agents-index The provider status cache: the last ping result of each known provider, read without a ping.

use super::{Detected, Provider};
use std::sync::{Arc, RwLock};

/// The result of the last ping of one provider.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub provider: Provider,
    /// `ok`, `unauthorized`, `unreachable`, or `not_configured`.
    pub status: &'static str,
    /// UTC, RFC 3339.
    pub checked_at: String,
}

/// The current UTC time in RFC 3339, the format of `checked_at`.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The shared cache. The ping worker writes it; a read never starts a ping.
#[derive(Clone, Debug)]
pub struct Cache(Arc<RwLock<Vec<Status>>>);

impl Cache {
    /// A provider with no credential is `not_configured` at once. A provider with a credential is
    /// `unreachable` until the first ping, which the worker sends at start.
    pub fn new(all: &[Detected]) -> Cache {
        let at = now();
        let first = all
            .iter()
            .map(|d| Status {
                provider: d.provider,
                status: if d.credential.is_some() {
                    "unreachable"
                } else {
                    "not_configured"
                },
                checked_at: at.clone(),
            })
            .collect();
        Cache(Arc::new(RwLock::new(first)))
    }

    pub fn read(&self) -> Vec<Status> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Replaces the status of one provider.
    pub fn set(&self, s: Status) {
        let mut all = self.0.write().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = all.iter_mut().find(|x| x.provider == s.provider) {
            *slot = s;
        }
    }

    /// The body of `GET /private/api/providers`: name, status, and time only, never a credential or a URL.
    pub fn json(&self) -> serde_json::Value {
        let list: Vec<_> = self
            .read()
            .into_iter()
            .map(|s| {
                serde_json::json!({"name": s.provider.name(), "status": s.status, "checked_at": s.checked_at})
            })
            .collect();
        serde_json::json!({ "providers": list })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Source;

    fn all() -> Vec<Detected> {
        super::super::KNOWN
            .into_iter()
            .map(|p| Detected {
                provider: p,
                credential: None,
                base_url: "https://x".into(),
                source: Source::Default,
            })
            .collect()
    }

    #[test]
    fn checked_at_is_utc_rfc3339() {
        let c = Cache::new(&all());
        for s in c.read() {
            let t = chrono::DateTime::parse_from_rfc3339(&s.checked_at).unwrap();
            assert_eq!(t.offset().local_minus_utc(), 0);
            assert!(s.checked_at.ends_with('Z'));
            assert_eq!(s.status, "not_configured");
        }
    }

    #[test]
    fn provider_status_read_is_cached() {
        let c = Cache::new(&all());
        c.set(Status {
            provider: Provider::Anthropic,
            status: "ok",
            checked_at: now(),
        });
        let mut times = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let t = std::time::Instant::now();
            let body = c.json();
            times.push(t.elapsed());
            assert_eq!(body["providers"][0]["status"], "ok");
        }
        times.sort();
        let p95 = times[949];
        println!("provider_status_read_p95_us: {}", p95.as_micros());
        assert!(p95 < std::time::Duration::from_millis(5));
        let text = c.json().to_string();
        assert!(!text.contains("https://x"));
    }
}
