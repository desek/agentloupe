//! @agents-index The pure cost function: the sum of the five token terms in exact decimal arithmetic, or the unpriced reason.

use super::index::Index;
use super::lookup::find;
use super::tier::select;
use super::unpriced::Unpriced;
use rust_decimal::Decimal;

/// The fields of one call that the cost reads.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    pub model: &'a str,
    /// The litellm_provider of the upstream of the call.
    pub provider: &'a str,
    pub service_tier: Option<&'a str>,
    pub input: i64,
    pub cache_read: i64,
    pub cache_write_5m: i64,
    pub cache_write_1h: i64,
    /// Already holds the reasoning tokens, so they are not added again.
    pub output: i64,
}

/// The fraction digits of a stored cost.
pub const SCALE: u32 = 12;

/// The cost in US dollars, rounded to at most 12 fraction digits. Pure: no input or output.
pub fn price(index: &Index, c: &Call) -> Result<Decimal, Unpriced> {
    let entry = find(index, c.model, c.provider).ok_or(Unpriced::NoEntry)?;
    let prompt = c.input + c.cache_read + c.cache_write_5m + c.cache_write_1h;
    let terms = [
        (c.input, "input_cost_per_token"),
        (c.cache_read, "cache_read_input_token_cost"),
        (c.cache_write_5m, "cache_creation_input_token_cost"),
        (
            c.cache_write_1h,
            "cache_creation_input_token_cost_above_1hr",
        ),
        (c.output, "output_cost_per_token"),
    ];
    let mut sum = Decimal::ZERO;
    for (count, base) in terms {
        if count == 0 {
            continue;
        }
        let (_, p) =
            select(&entry.fields, base, c.service_tier, prompt).map_err(Unpriced::MissingField)?;
        sum += Decimal::from(count) * p;
    }
    Ok(sum.round_dp(SCALE).normalize())
}

/// The cost as the decimal text that calls.cost_usd stores.
pub fn text(d: Decimal) -> String {
    d.round_dp(SCALE).normalize().to_string()
}

#[cfg(test)]
mod tests {
    use super::super::index::Entry;
    use super::*;
    use std::str::FromStr;

    fn index(fields: &[(&str, &str)]) -> Index {
        Index::from_entries(vec![Entry {
            key: "m".into(),
            provider: "p".into(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), Decimal::from_str(v).unwrap()))
                .collect(),
        }])
    }

    fn call(counts: [i64; 5]) -> Call<'static> {
        Call {
            model: "m",
            provider: "p",
            service_tier: None,
            input: counts[0],
            cache_read: counts[1],
            cache_write_5m: counts[2],
            cache_write_1h: counts[3],
            output: counts[4],
        }
    }

    const FIELDS: &[(&str, &str)] = &[
        ("input_cost_per_token", "0.000003"),
        ("cache_read_input_token_cost", "0.0000003"),
        ("cache_creation_input_token_cost", "0.00000375"),
        ("cache_creation_input_token_cost_above_1hr", "0.000006"),
        ("output_cost_per_token", "0.000015"),
    ];

    #[test]
    fn five_terms_sum() {
        // 100*0.000003 + 200*0.0000003 + 300*0.00000375 + 400*0.000006 + 500*0.000015
        // = 0.0003 + 0.00006 + 0.001125 + 0.0024 + 0.0075 = 0.011385
        let d = price(&index(FIELDS), &call([100, 200, 300, 400, 500])).unwrap();
        assert_eq!(text(d), "0.011385");
    }

    #[test]
    fn one_hour_writes_use_1hr_field() {
        let d = price(&index(FIELDS), &call([0, 0, 0, 1000, 0])).unwrap();
        assert_eq!(text(d), "0.006");
    }
}
