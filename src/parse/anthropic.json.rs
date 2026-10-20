//! @agents-index Reads the usage of an Anthropic Messages JSON response, and maps an Anthropic usage object to the five counts.

use super::usage::{Usage, int, text};
use serde_json::Value;

/// Fills the counts from an Anthropic `usage` object. The legacy total cache write goes to the 5 minute column.
pub fn counts(u: &Value, out: &mut Usage) {
    out.input = int(&u["input_tokens"]);
    out.cache_read = int(&u["cache_read_input_tokens"]);
    let split = &u["cache_creation"];
    if split.is_object() {
        out.cache_write_5m = int(&split["ephemeral_5m_input_tokens"]);
        out.cache_write_1h = int(&split["ephemeral_1h_input_tokens"]);
    } else {
        out.cache_write_5m = int(&u["cache_creation_input_tokens"]);
        out.cache_write_1h = 0;
    }
    out.output = int(&u["output_tokens"]);
    if let Some(t) = text(&u["service_tier"]) {
        out.service_tier = Some(t);
    }
}

/// The call of a JSON body, or None when the body has no usage object.
pub fn parse(body: &[u8]) -> Option<Usage> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let u = v.get("usage").filter(|u| u.is_object())?;
    let mut out = Usage {
        model: text(&v["model"]),
        service_tier: text(&v["service_tier"]),
        ..Usage::default()
    };
    counts(u, &mut out);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_usage_with_split_cache_writes() {
        let u = parse(include_bytes!(
            "../../tests/fixtures/parse/anthropic.split.json"
        ))
        .unwrap();
        assert_eq!(
            (
                u.input,
                u.cache_read,
                u.cache_write_5m,
                u.cache_write_1h,
                u.output
            ),
            (10, 20, 4, 3, 5)
        );
        assert_eq!(u.model.as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(u.service_tier.as_deref(), Some("standard"));
    }

    #[test]
    fn legacy_cache_write_goes_to_5m() {
        let u = parse(include_bytes!(
            "../../tests/fixtures/parse/anthropic.legacy.json"
        ))
        .unwrap();
        assert_eq!((u.cache_write_5m, u.cache_write_1h), (40, 0));
        assert_eq!(u.service_tier, None);
    }
}
