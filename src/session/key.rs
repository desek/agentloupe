//! @agents-index Selects the session key of a call from the first present source and hashes it under the account key.

use crate::capture::credential::hmac_hex;
use crate::parse::metadata::user_id;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The HMAC of the key value as lowercase hex, and the source name stored in key_source.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionKey {
    pub hash: String,
    pub source: &'static str,
}

/// The system prompt or `instructions`, plus the first message or input item, hashed with SHA-256.
fn opening(request: &Value) -> String {
    let head = request
        .get("system")
        .or_else(|| request.get("instructions"));
    let first = request["messages"]
        .get(0)
        .or_else(|| request["input"].get(0))
        .or_else(|| request.get("input").filter(|i| i.is_string()));
    let mut h = Sha256::new();
    h.update(head.map(|v| v.to_string()).unwrap_or_default());
    h.update([0u8]);
    h.update(first.map(|v| v.to_string()).unwrap_or_default());
    hex::encode(h.finalize())
}

/// The order is: the session hint, the session ID of `metadata.user_id`, `prompt_cache_key`, the opening hash.
/// The session hint is already an HMAC under the same key, so it is the key hash as it is.
pub fn select(key: &[u8; 32], hint: Option<&str>, request: &Value) -> SessionKey {
    if let Some(h) = hint {
        return SessionKey {
            hash: h.to_string(),
            source: "header",
        };
    }
    if let Some(s) = user_id(request).session {
        return SessionKey {
            hash: hmac_hex(key, s.as_bytes()),
            source: "metadata",
        };
    }
    if let Some(c) = request["prompt_cache_key"]
        .as_str()
        .filter(|c| !c.is_empty())
    {
        return SessionKey {
            hash: hmac_hex(key, c.as_bytes()),
            source: "cache_key",
        };
    }
    SessionKey {
        hash: hmac_hex(key, opening(request).as_bytes()),
        source: "opening",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const K: [u8; 32] = [9u8; 32];

    #[test]
    fn key_source_order() {
        let meta = json!({"metadata":{"user_id":"user_a_account_b_session_s1"},"prompt_cache_key":"c1","messages":[{"role":"user","content":"x"}]});
        let cache = json!({"prompt_cache_key":"c1","messages":[{"role":"user","content":"x"}]});
        let none = json!({"system":"sys","messages":[{"role":"user","content":"x"}]});
        assert_eq!(select(&K, Some("abc"), &meta).source, "header");
        assert_eq!(
            select(&K, None, &meta),
            SessionKey {
                hash: hmac_hex(&K, b"s1"),
                source: "metadata"
            }
        );
        assert_eq!(
            select(&K, None, &cache),
            SessionKey {
                hash: hmac_hex(&K, b"c1"),
                source: "cache_key"
            }
        );
        let o = select(&K, None, &none);
        assert_eq!(o.source, "opening");
        assert_eq!(o, select(&K, None, &none));
        assert_ne!(
            o.hash,
            select(
                &K,
                None,
                &json!({"system":"other","messages":[{"role":"user","content":"x"}]})
            )
            .hash
        );
    }
}
