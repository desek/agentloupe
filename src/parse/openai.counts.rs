//! @agents-index Maps an OpenAI usage object of Responses or Chat Completions to the five counts; cache writes are 0.

use super::usage::{Usage, int, text};
use serde_json::Value;

/// `input` is the input total minus the cached tokens; Responses and Chat Completions name the fields differently.
pub fn counts(u: &Value, out: &mut Usage) {
    let total = int(u.get("input_tokens").unwrap_or(&u["prompt_tokens"]));
    let details = u
        .get("input_tokens_details")
        .unwrap_or(&u["prompt_tokens_details"]);
    out.cache_read = int(&details["cached_tokens"]);
    out.input = total - out.cache_read;
    out.cache_write_5m = 0;
    out.cache_write_1h = 0;
    out.output = int(u.get("output_tokens").unwrap_or(&u["completion_tokens"]));
    if let Some(t) = text(&u["service_tier"]) {
        out.service_tier = Some(t);
    }
}

/// The call of one response or chunk object that has a usage object.
pub fn from_object(v: &serde_json::Value) -> Option<Usage> {
    let u = v.get("usage").filter(|u| u.is_object())?;
    let mut out = Usage {
        model: text(&v["model"]),
        service_tier: text(&v["service_tier"]),
        ..Usage::default()
    };
    counts(u, &mut out);
    Some(out)
}
