//! @agents-index The ping worker: one ping of each provider at start and then once per interval, written to the status cache.

use super::Detected;
use super::status::{Cache, Status};
use std::future::Future;
use std::time::Duration;

/// Pings every provider, then sleeps `interval`, until the task ends. One request at most per
/// provider per interval. `ping` is the real ping in the server and a counter in the tests.
pub async fn run<F, Fut>(all: Vec<Detected>, cache: Cache, interval: Duration, ping: F)
where
    F: Fn(Detected) -> Fut,
    Fut: Future<Output = Status>,
{
    loop {
        for d in &all {
            let s = ping(d.clone()).await;
            if s.status != "ok" && s.status != "not_configured" {
                tracing::warn!(
                    provider = d.provider.name(),
                    status = s.status,
                    "the provider ping did not get ok. Fix: check the credential and the base URL of the provider in .env. Reference: docs/specification/operations.md, section Provider ping. Verify: GET /private/api/providers shows ok"
                );
            }
            cache.set(s);
        }
        tokio::time::sleep(interval).await;
    }
}

/// Starts the worker on the runtime of `loupe serve` with the real ping; it never blocks the listener.
pub fn spawn(
    rt: &tokio::runtime::Runtime,
    all: Vec<Detected>,
    cache: Cache,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let http = super::ping::http();
    rt.spawn(run(all, cache, interval, move |d| {
        let http = http.clone();
        async move { super::ping::ping(&http, &d).await }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Source;
    use crate::config::secret::Secret;
    use crate::summary::credential::Credential;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(start_paused = true)]
    async fn ping_schedule_one_request_per_interval() {
        let all: Vec<Detected> = super::super::KNOWN
            .into_iter()
            .map(|p| Detected {
                provider: p,
                credential: Some(Credential::ApiKey(Secret::new("k".into()))),
                base_url: "http://127.0.0.1:9".into(),
                source: Source::Default,
            })
            .collect();
        let cache = Cache::new(&all);
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let interval = Duration::from_secs(300);
        tokio::spawn(run(all, cache.clone(), interval, move |d| {
            c.fetch_add(1, Ordering::SeqCst);
            async move {
                Status {
                    provider: d.provider,
                    status: "ok",
                    checked_at: super::super::status::now(),
                }
            }
        }));
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(count.load(Ordering::SeqCst), 2, "one per provider at start");
        tokio::time::sleep(interval).await;
        assert_eq!(count.load(Ordering::SeqCst), 4);
        tokio::time::sleep(interval).await;
        assert_eq!(count.load(Ordering::SeqCst), 6);
        assert!(cache.read().iter().all(|s| s.status == "ok"));
    }
}
