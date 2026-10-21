//! @agents-index The provider ping: one GET of the models list with the exact header set per credential kind, mapped to a status.

use super::status::{Status, now};
use super::{Detected, Provider};
use crate::summary::credential::Credential;
use crate::summary::headers::{OAUTH_BETA, VERSION};
use std::time::Duration;

/// The timeout of one ping.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// The client of the pings; the timeout covers the whole request.
pub fn http() -> reqwest::Client {
    http_with(TIMEOUT)
}

/// The client with `timeout`, so a test asserts the timeout path without a wait of 5 seconds.
fn http_with(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("the TLS backend of reqwest is built in")
}

/// The models list of the provider; it lists models and calls none, so it spends no tokens.
pub fn url(p: Provider, base_url: &str) -> String {
    match p {
        Provider::Anthropic => format!("{base_url}/v1/models"),
        Provider::Openai => format!("{base_url}/models"),
    }
}

/// The exact headers of a ping. The OAuth set has no `x-app` and no billing block, because the
/// billing block belongs to a Messages body.
pub fn headers(c: &Credential) -> Vec<(&'static str, String)> {
    match c {
        Credential::ApiKey(k) => vec![
            ("x-api-key", k.expose().to_string()),
            ("anthropic-version", VERSION.to_string()),
        ],
        Credential::OauthToken(t) => vec![
            ("authorization", format!("Bearer {}", t.expose())),
            ("anthropic-version", VERSION.to_string()),
            ("anthropic-beta", OAUTH_BETA.to_string()),
        ],
        Credential::OpenaiKey(k) => vec![("authorization", format!("Bearer {}", k.expose()))],
    }
}

/// The status of an HTTP answer: 2xx is ok, 401 and 403 are unauthorized, the rest is unreachable.
pub fn map(status: u16) -> &'static str {
    match status {
        200..=299 => "ok",
        401 | 403 => "unauthorized",
        _ => "unreachable",
    }
}

/// Pings one provider. No credential gives `not_configured` and sends no request; a network
/// error or a timeout gives `unreachable`.
pub async fn ping(http: &reqwest::Client, d: &Detected) -> Status {
    let status = match &d.credential {
        None => "not_configured",
        Some(c) => {
            let mut req = http.get(url(d.provider, &d.base_url));
            for (name, value) in headers(c) {
                req = req.header(name, value);
            }
            match req.send().await {
                Ok(r) => map(r.status().as_u16()),
                Err(_) => "unreachable",
            }
        }
    };
    Status {
        provider: d.provider,
        status,
        checked_at: now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::Source;
    use crate::config::secret::Secret;
    use std::sync::{Arc, Mutex};

    fn names(h: &[(&'static str, String)]) -> Vec<&'static str> {
        h.iter().map(|(n, _)| *n).collect()
    }

    #[test]
    fn ping_header_sets() {
        let s = |v: &str| Secret::new(v.into());
        let k = headers(&Credential::ApiKey(s("k1")));
        assert_eq!(names(&k), ["x-api-key", "anthropic-version"]);
        assert_eq!(k[1].1, "2023-06-01");
        let t = headers(&Credential::OauthToken(s("t1")));
        assert_eq!(
            names(&t),
            ["authorization", "anthropic-version", "anthropic-beta"]
        );
        assert_eq!(t[0].1, "Bearer t1");
        assert_eq!(t[2].1, "oauth-2025-04-20,claude-code-20250219");
        let o = headers(&Credential::OpenaiKey(s("o1")));
        assert_eq!(o, vec![("authorization", "Bearer o1".to_string())]);
        for set in [k, t, o] {
            assert!(set.iter().all(|(_, v)| !v.contains("billing")));
        }
    }

    #[test]
    fn status_maps_from_response() {
        assert_eq!(map(200), "ok");
        assert_eq!(map(204), "ok");
        assert_eq!(map(401), "unauthorized");
        assert_eq!(map(403), "unauthorized");
        assert_eq!(map(404), "unreachable");
        assert_eq!(map(500), "unreachable");
    }

    /// A fake provider on a free port that records each path and answers `code`.
    async fn fake(code: u16) -> (String, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let rec = seen.clone();
        let app = axum::Router::new().fallback(move |req: axum::extract::Request| {
            let rec = rec.clone();
            async move {
                rec.lock().unwrap().push(req.uri().path().to_string());
                axum::http::StatusCode::from_u16(code).unwrap()
            }
        });
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        (base, seen)
    }

    fn target(p: Provider, base: &str, c: Option<Credential>) -> Detected {
        Detected {
            provider: p,
            credential: c,
            base_url: base.into(),
            source: Source::Env,
        }
    }

    #[tokio::test]
    async fn provider_ping_never_calls_a_model() {
        let (base, seen) = fake(200).await;
        let http = http();
        let k = |v: &str| Secret::new(v.into());
        let a = ping(
            &http,
            &target(
                Provider::Anthropic,
                &base,
                Some(Credential::OauthToken(k("t"))),
            ),
        )
        .await;
        let o = ping(
            &http,
            &target(
                Provider::Openai,
                &format!("{base}/v1"),
                Some(Credential::OpenaiKey(k("o"))),
            ),
        )
        .await;
        let n = ping(&http, &target(Provider::Openai, &base, None)).await;
        assert_eq!(
            (a.status, o.status, n.status),
            ("ok", "ok", "not_configured")
        );
        let paths = seen.lock().unwrap().clone();
        assert_eq!(paths, ["/v1/models", "/v1/models"]);
        let model_calls = paths
            .iter()
            .filter(|p| p.ends_with("/messages") || p.ends_with("/chat/completions"))
            .count();
        println!("provider_ping_model_requests: {model_calls}");
        assert_eq!(model_calls, 0);
        let (bad, _) = fake(401).await;
        let k = Some(Credential::ApiKey(Secret::new("k".into())));
        let u = ping(&http, &target(Provider::Anthropic, &bad, k)).await;
        assert_eq!(u.status, "unauthorized");
    }

    #[tokio::test]
    async fn provider_ping_times_out_unreachable() {
        // A listener that accepts and never answers.
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((s, _)) = l.accept().await {
                held.push(s);
            }
        });
        let k = Some(Credential::ApiKey(Secret::new("k".into())));
        let client = http_with(Duration::from_millis(200));
        let s = ping(&client, &target(Provider::Anthropic, &base, k)).await;
        assert_eq!(s.status, "unreachable");
    }
}
