//! @agents-index The summary client: one POST /v1/messages to LOUPE_SUMMARY_BASE_URL, never through the proxy, mapped to an outcome.

use super::credential::Credential;
use serde_json::{Value, json};
use std::time::Duration;

/// The timeout of one summary request.
const TIMEOUT: Duration = Duration::from_secs(30);

/// The `max_tokens` of a session line and of a day paragraph.
pub const LINE_TOKENS: u32 = 64;
pub const DAY_TOKENS: u32 = 600;

/// The result of one request. A retryable kind gets a backoff; a fatal kind gives `failed` at once.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Text(String),
    Retry {
        kind: &'static str,
        retry_after: Option<u64>,
    },
    Fatal {
        kind: &'static str,
    },
}

/// What the client needs: the target, the model, and the active credential.
#[derive(Debug, Clone)]
pub struct Client {
    pub base_url: String,
    pub model: String,
    pub credential: Credential,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base_url: &str, model: &str, credential: Credential) -> Client {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("the TLS backend of reqwest is built in");
        Client {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            credential,
            http,
        }
    }

    /// The request body: the rule text as the system prompt and the user message. No `temperature`:
    /// the API refuses it for the current Haiku model. The OAuth path puts the billing block first.
    pub fn body(&self, system: &str, user: &str, max_tokens: u32) -> Value {
        let mut blocks = vec![json!({"type": "text", "text": system})];
        if matches!(self.credential, Credential::OauthToken(_)) {
            blocks.insert(
                0,
                json!({"type": "text", "text": super::headers::BILLING_BLOCK}),
            );
        }
        json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": blocks,
            "messages": [{"role": "user", "content": user}],
        })
    }

    /// Sends one request and maps the response. Logs the actionable credential error where needed;
    /// the log line never holds the credential or the text.
    pub async fn send(&self, system: &str, user: &str, max_tokens: u32) -> Outcome {
        let mut req = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("content-type", "application/json")
            .json(&self.body(system, user, max_tokens));
        for (name, value) in self.credential.headers() {
            req = req.header(name, value);
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() => {
                return Outcome::Retry {
                    kind: "timeout",
                    retry_after: None,
                };
            }
            Err(_) => {
                return Outcome::Retry {
                    kind: "network",
                    retry_after: None,
                };
            }
        };
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let oauth = matches!(self.credential, Credential::OauthToken(_));
        if matches!(status, 401 | 403) || (status == 429 && retry_after.is_none() && oauth) {
            self.log_credential_error(status);
        }
        match status {
            200..=299 => match resp.json::<Value>().await {
                Ok(v) => text_of(&v).map_or(
                    Outcome::Fatal {
                        kind: "invalid_response",
                    },
                    Outcome::Text,
                ),
                Err(e) if e.is_timeout() => Outcome::Retry {
                    kind: "timeout",
                    retry_after: None,
                },
                Err(_) => Outcome::Fatal {
                    kind: "invalid_response",
                },
            },
            429 => Outcome::Retry {
                kind: "rate_limited",
                retry_after,
            },
            529 => Outcome::Retry {
                kind: "overloaded",
                retry_after,
            },
            500..=599 => Outcome::Retry {
                kind: "server_error",
                retry_after,
            },
            400 => Outcome::Fatal {
                kind: "bad_request",
            },
            401 => Outcome::Fatal {
                kind: "unauthorized",
            },
            403 => Outcome::Fatal { kind: "forbidden" },
            404 => Outcome::Fatal { kind: "not_found" },
            _ => Outcome::Fatal {
                kind: "client_error",
            },
        }
    }

    fn log_credential_error(&self, status: u16) {
        tracing::error!(
            status,
            credential = self.credential.kind(),
            "the summary request was refused for the credential in {var}. Fix 1: use the model claude-haiku-5-5 with LOUPE_SUMMARY_OAUTH_TOKEN (remove LOUPE_SUMMARY_MODEL), and renew the token with claude setup-token. Fix 2: set LOUPE_SUMMARY_API_KEY to a Console API key. Reference: docs/specification/summaries.md, section Credentials. Verify: loupe summarize --once prints done 1 or more and failed 0",
            var = self.credential.variable(),
        );
    }
}

/// The text of a valid Messages response: the joined text blocks; None for another shape.
pub fn text_of(v: &Value) -> Option<String> {
    if v["type"] != "message" {
        return None;
    }
    let blocks = v["content"].as_array()?;
    Some(
        blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
    )
}

/// True when the text is the word SKIP after trim, in any case.
pub fn is_skip(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("skip")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_and_skip() {
        let v = json!({"type":"message","content":[{"type":"text","text":"fixed a bug"}]});
        assert_eq!(text_of(&v).as_deref(), Some("fixed a bug"));
        assert_eq!(text_of(&json!({"error":{}})), None);
        assert!(is_skip(" Skip\n"));
        assert!(!is_skip("skipped tests"));
    }

    #[test]
    fn body_shape_per_credential() {
        let c = Client::new(
            "http://127.0.0.1:1/",
            "m",
            Credential::ApiKey(crate::config::secret::Secret::new("k".into())),
        );
        let b = c.body("S", "U", LINE_TOKENS);
        assert!(b.get("temperature").is_none());
        assert_eq!(b["system"].as_array().unwrap().len(), 1);
        assert_eq!(b["max_tokens"], 64);
        assert_eq!(c.base_url, "http://127.0.0.1:1");
        let o = Client::new(
            "http://127.0.0.1:1",
            "m",
            Credential::OauthToken(crate::config::secret::Secret::new("t".into())),
        );
        let b = o.body("S", "U", DAY_TOKENS);
        assert_eq!(b["system"][0]["text"], super::super::headers::BILLING_BLOCK);
        assert_eq!(b["system"][1]["text"], "S");
    }
}
