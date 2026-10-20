//! @agents-index Redacts credential header values for storage; the header names stay, the values become "redacted".

use axum::http::HeaderMap;

const REQUEST_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "cookie",
    "chatgpt-account-id",
    "x-claude-code-agent-id",
];
const REQUEST_PARTS: &[&str] = &["key", "token", "secret", "session"];
/// The substring `token` is not here: rate-limit and usage headers name tokens as counts.
const RESPONSE_PARTS: &[&str] = &["key", "secret", "session"];
const RATE_LIMIT_PREFIXES: &[&str] = &["anthropic-ratelimit-", "x-ratelimit-"];

fn request_secret(n: &str) -> bool {
    REQUEST_NAMES.contains(&n) || REQUEST_PARTS.iter().any(|p| n.contains(p))
}

fn response_secret(n: &str) -> bool {
    if RATE_LIMIT_PREFIXES.iter().any(|p| n.starts_with(p)) {
        return false;
    }
    n == "set-cookie" || RESPONSE_PARTS.iter().any(|p| n.contains(p))
}

fn to_json(h: &HeaderMap, secret: fn(&str) -> bool) -> serde_json::Value {
    let pairs: Vec<serde_json::Value> = h
        .iter()
        .map(|(k, v)| {
            let name = k.as_str();
            let value = if secret(name) {
                "redacted".to_string()
            } else {
                String::from_utf8_lossy(v.as_bytes()).into_owned()
            };
            serde_json::json!([name, value])
        })
        .collect();
    serde_json::Value::Array(pairs)
}

/// The request headers as a JSON array of [name, value] pairs, with the credentials redacted.
pub fn request(h: &HeaderMap) -> serde_json::Value {
    to_json(h, request_secret)
}

/// The response headers as a JSON array of [name, value] pairs, with the secrets redacted.
pub fn response(h: &HeaderMap) -> serde_json::Value {
    to_json(h, response_secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    fn value(j: &serde_json::Value, name: &str) -> String {
        j.as_array().unwrap().iter().find(|p| p[0] == name).unwrap()[1]
            .as_str()
            .unwrap()
            .into()
    }

    #[test]
    fn redacts_credential_headers() {
        let names = [
            "authorization",
            "x-api-key",
            "cookie",
            "chatgpt-account-id",
            "x-claude-code-agent-id",
            "anthropic-session-id",
            "x-goog-api-key",
        ];
        let pairs: Vec<(&str, &str)> = names.iter().map(|n| (*n, "s3cret")).collect();
        let j = request(&map(&pairs));
        for n in names {
            assert_eq!(value(&j, n), "redacted", "{n}");
        }
        assert!(!j.to_string().contains("s3cret"));
    }

    #[test]
    fn keeps_rate_limit_response_headers() {
        let j = response(&map(&[
            ("anthropic-ratelimit-tokens-remaining", "9000"),
            ("x-ratelimit-remaining-tokens", "5"),
            ("set-cookie", "a=b"),
        ]));
        assert_eq!(value(&j, "anthropic-ratelimit-tokens-remaining"), "9000");
        assert_eq!(value(&j, "x-ratelimit-remaining-tokens"), "5");
        assert_eq!(value(&j, "set-cookie"), "redacted");
    }
}
