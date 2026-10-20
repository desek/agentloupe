//! @agents-index Selects the price field of one token term: the service tier field first, then the highest prompt-size tier exceeded.

use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// The suffix of the service tier fields, or None for a tier that uses the base fields.
fn service_suffix(service_tier: Option<&str>) -> Option<&'static str> {
    match service_tier {
        Some("priority") => Some("_priority"),
        Some("flex") => Some("_flex"),
        _ => None,
    }
}

/// The threshold in tokens of `name` if it is `<base>_above_<N>k_tokens<suffix>`.
fn threshold(name: &str, base: &str, suffix: &str) -> Option<i64> {
    let n = name
        .strip_prefix(base)?
        .strip_prefix("_above_")?
        .strip_suffix(suffix)?
        .strip_suffix("k_tokens")?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse::<i64>().ok().map(|k| k * 1000))
        .flatten()
}

/// The prompt-size tier field with the highest threshold that `prompt` is above.
fn size_tier<'a>(
    fields: &'a BTreeMap<String, Decimal>,
    base: &str,
    suffix: &str,
    prompt: i64,
) -> Option<(&'a String, &'a Decimal)> {
    fields
        .iter()
        .filter_map(|(k, v)| threshold(k, base, suffix).map(|t| (t, k, v)))
        .filter(|(t, _, _)| prompt > *t)
        .max_by_key(|(t, _, _)| *t)
        .map(|(_, k, v)| (k, v))
}

/// True if `select` can read the field `name` for the base field `base`.
pub fn is_read(name: &str, base: &str) -> bool {
    ["", "_priority", "_flex"]
        .iter()
        .any(|s| name == format!("{base}{s}") || threshold(name, base, s).is_some())
}

/// The field name and price for the base field `base`. On a missing field, Err holds the base field name.
pub fn select(
    fields: &BTreeMap<String, Decimal>,
    base: &str,
    service_tier: Option<&str>,
    prompt: i64,
) -> Result<(String, Decimal), String> {
    let suffix = service_suffix(service_tier)
        .filter(|s| fields.contains_key(&format!("{base}{s}")))
        .unwrap_or("");
    if let Some((k, v)) = size_tier(fields, base, suffix, prompt) {
        return Ok((k.clone(), *v));
    }
    let name = format!("{base}{suffix}");
    fields
        .get(&name)
        .map(|v| (name, *v))
        .ok_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, i64)]) -> BTreeMap<String, Decimal> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Decimal::from(*v)))
            .collect()
    }

    const B: &str = "input_cost_per_token";

    #[test]
    fn prompt_above_threshold_uses_tier() {
        let f = fields(&[(B, 1), ("input_cost_per_token_above_200k_tokens", 2)]);
        assert_eq!(select(&f, B, None, 200_001).unwrap().1, Decimal::from(2));
    }

    #[test]
    fn two_tiers_selects_highest_exceeded() {
        let f = fields(&[
            (B, 1),
            ("input_cost_per_token_above_128k_tokens", 2),
            ("input_cost_per_token_above_200k_tokens", 3),
        ]);
        assert_eq!(select(&f, B, None, 150_000).unwrap().1, Decimal::from(2));
        assert_eq!(select(&f, B, None, 200_001).unwrap().1, Decimal::from(3));
    }

    #[test]
    fn prompt_at_threshold_uses_base() {
        let f = fields(&[(B, 1), ("input_cost_per_token_above_200k_tokens", 2)]);
        assert_eq!(select(&f, B, None, 200_000).unwrap().1, Decimal::from(1));
    }

    #[test]
    fn priority_and_flex_fields() {
        let f = fields(&[
            (B, 1),
            ("input_cost_per_token_priority", 2),
            ("input_cost_per_token_flex", 3),
        ]);
        assert_eq!(
            select(&f, B, Some("priority"), 10).unwrap().1,
            Decimal::from(2)
        );
        assert_eq!(select(&f, B, Some("flex"), 10).unwrap().1, Decimal::from(3));
        assert_eq!(
            select(&f, B, Some("default"), 10).unwrap().1,
            Decimal::from(1)
        );
    }

    #[test]
    fn service_then_size_tier() {
        let f = fields(&[
            (B, 1),
            ("input_cost_per_token_priority", 2),
            ("input_cost_per_token_above_272k_tokens", 3),
            ("input_cost_per_token_above_272k_tokens_priority", 4),
        ]);
        assert_eq!(
            select(&f, B, Some("priority"), 300_000).unwrap().1,
            Decimal::from(4)
        );
        assert_eq!(
            select(&f, B, Some("priority"), 10).unwrap().1,
            Decimal::from(2)
        );
    }
}
