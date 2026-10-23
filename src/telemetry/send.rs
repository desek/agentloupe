//! @agents-index The one start GET: its own reqwest client with rustls, no body, no redirect, one attempt with a timeout, a debug line with the host only.

use std::time::Duration;

/// The timeout of the one attempt, from the start of the connect to the end of the response.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// The result of the attempt: a status code of any value, or the kind of the error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Status(u16),
    Error(&'static str),
}

/// The kind of a reqwest error; the error itself is never logged, because its text holds the URL.
fn kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_redirect() {
        "redirect"
    } else if e.is_request() {
        "request"
    } else {
        "other"
    }
}

/// Sends one GET of `url` with no body and `user_agent`, and gives the outcome; no error reaches the
/// caller and nothing retries. The debug line holds the host name and the status or the error kind,
/// never the scheme, the port, the path, the query, or the user agent.
pub async fn send(url: &str, user_agent: &str, timeout: Duration) -> Outcome {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let outcome = attempt(url, user_agent, timeout).await;
    match outcome {
        Outcome::Status(status) => tracing::debug!(host = %host, status, "telemetry result"),
        Outcome::Error(error) => tracing::debug!(host = %host, error, "telemetry result"),
    }
    outcome
}

async fn attempt(url: &str, user_agent: &str, timeout: Duration) -> Outcome {
    let client = match reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(user_agent)
        .build()
    {
        Ok(c) => c,
        Err(_) => return Outcome::Error("client"),
    };
    // The response body is not read: the outcome is the status, and a large body costs nothing.
    match client.get(url).send().await {
        Ok(r) => Outcome::Status(r.status().as_u16()),
        Err(e) => Outcome::Error(kind(&e)),
    }
}
