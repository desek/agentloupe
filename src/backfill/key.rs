//! @agents-index The log key of a backfilled call and the session key of a log session, as HMACs under the account key.

use crate::capture::credential::hmac_hex;

/// The session key hash of a log session ID; the same HMAC as a proxied session hint of the same ID.
pub fn session(key: &[u8; 32], session_id: &str) -> String {
    hmac_hex(key, session_id.as_bytes())
}

/// The log key: the HMAC of the log source, the session ID, and the entry ID. It makes a log call unique,
/// so a second run adds nothing, and it holds no raw ID.
pub fn log(key: &[u8; 32], source: &str, session_id: &str, entry_id: &str) -> String {
    hmac_hex(
        key,
        format!("{source}\n{session_id}\n{entry_id}").as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_key_is_stable_and_hashed() {
        let k = [3u8; 32];
        let a = log(&k, "claude-code", "sess-raw-1", "msg-raw-1");
        assert_eq!(a, log(&k, "claude-code", "sess-raw-1", "msg-raw-1"));
        assert_ne!(a, log(&k, "pi", "sess-raw-1", "msg-raw-1"));
        assert_ne!(a, log(&k, "claude-code", "sess-raw-1", "msg-raw-2"));
        assert_eq!(a.len(), 64);
        for raw in ["sess-raw-1", "msg-raw-1", "claude-code"] {
            assert!(!a.contains(raw));
        }
        assert_eq!(session(&k, "s"), hmac_hex(&k, b"s"));
    }
}
