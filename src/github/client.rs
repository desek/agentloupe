//! @agents-index The GitHub HTTP client: bearer token, JSON media type, API version, user agent, 30-second timeout, and the rate limit retry.

use super::retry;
use serde_json::Value;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);
const API_VERSION: &str = "2022-11-28";

/// Why a fetch failed; the text is the stored and logged cause. It never holds a name or a URL.
#[derive(Debug, Clone, PartialEq)]
pub enum Cause {
    Unauthorized,
    Network,
    RateLimited,
    SearchLimit,
    Http(u16),
}

impl Cause {
    pub fn text(&self) -> String {
        match self {
            Cause::Unauthorized => "unauthorized".into(),
            Cause::Network => "network".into(),
            Cause::RateLimited => "rate_limited".into(),
            Cause::SearchLimit => "search_limit".into(),
            Cause::Http(s) => format!("http_{s}"),
        }
    }
}

/// One client for one token; it lives only for one fetch.
pub struct Client {
    base: String,
    token: String,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(base_url: &str, token: &str) -> Client {
        let http = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(format!("agent-loupe/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("the TLS backend of reqwest is built in");
        Client {
            base: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        }
    }

    /// GET `path` with `query` and returns the JSON body. A rate limit waits and retries at most
    /// three times. A non-success status returns `Cause::Http`; the caller maps it further.
    pub fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Cause> {
        let mut retries = 0;
        loop {
            let resp = self
                .http
                .get(format!("{}{path}", self.base))
                .query(query)
                .bearer_auth(&self.token)
                .header("accept", "application/vnd.github+json")
                .header("x-github-api-version", API_VERSION)
                .send()
                .map_err(|_| Cause::Network)?;
            let status = resp.status().as_u16();
            if (200..300).contains(&status) {
                return resp.json().map_err(|_| Cause::Network);
            }
            let now = chrono::Utc::now().timestamp().max(0) as u64;
            match retry::wait_secs(status, resp.headers(), now) {
                Some(_) if retries >= retry::MAX_RETRIES => return Err(Cause::RateLimited),
                Some(secs) => {
                    retries += 1;
                    std::thread::sleep(Duration::from_secs(secs));
                }
                None => return Err(Cause::Http(status)),
            }
        }
    }
}
