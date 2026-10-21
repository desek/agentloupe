//! @agents-index The call candidate that a log reader finds: keys, start time, upstream, and usage; no log content.

use crate::parse::usage::Usage;

/// One call that a log entry describes. It holds only hashes and mapped columns, never log content.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The index of the file in the discovery list.
    pub file: usize,
    /// The HMAC of the session ID of the log: the key hash of the session scope.
    pub scope_key: String,
    /// The key that the processed-session rule tests; for a Claude Code subagent, the key of its main session.
    pub main_key: String,
    pub log_key: String,
    pub at_ms: i64,
    pub upstream: String,
    pub usage: Usage,
    /// Set by the run when the proxy capture already holds the session.
    pub captured: bool,
}

/// What a reader finds in one file.
#[derive(Debug, Default)]
pub struct FileCalls {
    pub calls: Vec<Candidate>,
    /// Every session key with an entry in the file, so a summary can find the files of a segment again.
    pub keys: Vec<String>,
    /// The latest entry time of the file, for the Pi rule.
    pub last_ms: Option<i64>,
}

/// Unix milliseconds of an ISO 8601 timestamp field.
pub fn ms(v: &serde_json::Value) -> Option<i64> {
    v.as_str()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp_millis())
}
