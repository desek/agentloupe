//! @agents-index The day record shape: the fields in their fixed order, the detail maps in byte order, and the JSON bytes with their hash.

use super::sum::Sums;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Serialize, Serializer};
use sha2::Digest;
use std::collections::BTreeMap;

/// Writes a decimal as a JSON number: an integer when it has no fraction, else a float whose
/// shortest form is the 4-digit text.
pub fn number<S: Serializer>(d: &Decimal, s: S) -> Result<S::Ok, S::Error> {
    if d.fract().is_zero() {
        s.serialize_i64(d.to_i64().unwrap_or(0))
    } else {
        s.serialize_f64(d.to_f64().unwrap_or(0.0))
    }
}

/// One record of a day. The field order is the public order; a struct keeps it.
#[derive(Serialize, Debug, Clone)]
pub struct Record {
    pub day: String,
    pub input_total: i64,
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    pub calls: i64,
    pub sessions: i64,
    #[serde(serialize_with = "number")]
    pub cost_usd: Decimal,
    pub unpriced_calls: i64,
    #[serde(serialize_with = "number")]
    pub subscription_cost_usd: Decimal,
    pub prs_opened: Option<i64>,
    pub prs_merged: Option<i64>,
    pub lines_added: Option<i64>,
    pub lines_removed: Option<i64>,
    pub summary: Option<String>,
    pub detail: Detail,
}

/// The detail maps; a BTreeMap keeps the keys in byte order, so the bytes are deterministic.
#[derive(Serialize, Debug, Clone, Default)]
pub struct Detail {
    pub by_agent: BTreeMap<String, AgentTotals>,
    pub by_model: BTreeMap<String, ModelTotals>,
    /// Present only when a GitHub token is set; it never holds a label name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubDetail>,
}

/// The number of labels with no ok fetch for the day.
#[derive(Serialize, Debug, Clone, Default)]
pub struct GithubDetail {
    pub failed_tokens: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct AgentTotals {
    pub input_total: i64,
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    pub calls: i64,
    pub sessions: i64,
    #[serde(serialize_with = "number")]
    pub cost_usd: Decimal,
    pub unpriced_calls: i64,
}

/// No sessions field: one segment can use more than one model.
#[derive(Serialize, Debug, Clone)]
pub struct ModelTotals {
    pub input_total: i64,
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    pub calls: i64,
    #[serde(serialize_with = "number")]
    pub cost_usd: Decimal,
    pub unpriced_calls: i64,
}

impl AgentTotals {
    pub fn from(s: &Sums) -> AgentTotals {
        AgentTotals {
            input_total: s.input_total(),
            input: s.input,
            cache_read: s.cache_read,
            cache_write: s.cache_write,
            output: s.output,
            calls: s.calls,
            sessions: s.sessions.len() as i64,
            cost_usd: s.cost_rounded(),
            unpriced_calls: s.unpriced,
        }
    }
}

impl ModelTotals {
    pub fn from(s: &Sums) -> ModelTotals {
        ModelTotals {
            input_total: s.input_total(),
            input: s.input,
            cache_read: s.cache_read,
            cache_write: s.cache_write,
            output: s.output,
            calls: s.calls,
            cost_usd: s.cost_rounded(),
            unpriced_calls: s.unpriced,
        }
    }
}

impl Record {
    /// The stored JSON text.
    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("a record always serializes")
    }
}

/// The SHA-256 of the record text, lowercase hex.
pub fn content_hash(json: &str) -> String {
    hex::encode(sha2::Sha256::digest(json.as_bytes()))
}
