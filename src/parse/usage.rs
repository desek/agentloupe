//! @agents-index The usage of one model call: the five token counts, the model, and the service tier.

use serde_json::Value;

/// One call as a format parser found it; the model can still be empty until the request fills it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Usage {
    /// Input tokens that are not cached.
    pub input: i64,
    pub cache_read: i64,
    pub cache_write_5m: i64,
    pub cache_write_1h: i64,
    pub output: i64,
    pub model: Option<String>,
    pub service_tier: Option<String>,
}

/// The integer at `v`, or 0 when it is absent or not a number.
pub fn int(v: &Value) -> i64 {
    v.as_i64().unwrap_or(0)
}

/// The string at `v`, or None.
pub fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}
