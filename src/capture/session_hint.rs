//! @agents-index The ordered list of session headers and the HMAC of the first present one, read before redaction.

use super::credential::{Hashed, hmac_hex};
use axum::http::HeaderMap;

/// The session headers in order of preference; the first present one is the session hint.
pub const SESSION_HEADERS: &[&str] = &[
    "x-claude-code-session-id",
    "x-session-id",
    "session_id",
    "session-id",
];

/// The HMAC under the account key of the first present session header, with its lowercase name.
pub fn session_hint(key: &[u8; 32], h: &HeaderMap) -> Hashed {
    SESSION_HEADERS.iter().find_map(|n| {
        h.get(*n)
            .map(|v| (hmac_hex(key, v.as_bytes()), n.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: [u8; 32] = [5u8; 32];

    #[test]
    fn hashes_first_session_header() {
        let mut h = HeaderMap::new();
        h.insert("session-id", "later".parse().unwrap());
        h.insert("x-claude-code-session-id", "first".parse().unwrap());
        assert_eq!(
            session_hint(&K, &h),
            Some((hmac_hex(&K, b"first"), "x-claude-code-session-id".into()))
        );
        assert_eq!(session_hint(&K, &HeaderMap::new()), None);
    }
}
