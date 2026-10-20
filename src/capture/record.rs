//! @agents-index The exchange record that the forwarder hands to the capture writer, with plain bodies until it is sealed.

use serde::{Deserialize, Serialize};

/// One exchange as the forwarder saw it. Header JSON is already redacted; bodies are the kept copies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExchangeRecord {
    /// 32 lowercase hex characters, random.
    pub id: String,
    /// Unix time in milliseconds.
    pub started_ms: i64,
    pub ended_ms: i64,
    pub ttfb_ms: Option<i64>,
    pub agent: String,
    pub upstream: String,
    pub method: String,
    pub path: String,
    pub query: String,
    pub status: u16,
    pub request_size: u64,
    pub response_size: u64,
    pub request_headers: serde_json::Value,
    pub response_headers: serde_json::Value,
    #[serde(with = "b64")]
    pub request_body: Vec<u8>,
    #[serde(with = "b64")]
    pub response_body: Vec<u8>,
    pub request_truncated: bool,
    pub response_truncated: bool,
    pub credential_hash: Option<String>,
    pub credential_source: Option<String>,
    pub account_id_hash: Option<String>,
    pub account_id_source: Option<String>,
    pub outcome: String,
    pub error: Option<String>,
}

/// A new random exchange id.
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS random source works");
    hex::encode(b)
}

/// Bodies as base64 inside a spill file, so the JSON stays near the body size.
mod b64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}
