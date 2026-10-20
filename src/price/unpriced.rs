//! @agents-index The reasons that a call has no price: no matching entry, or a missing price field for a non-zero count.

/// Why a call is unpriced; its cost is null, never zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unpriced {
    NoEntry,
    /// The base field name that the entry lacks.
    MissingField(String),
}

impl Unpriced {
    /// The text stored in calls.unpriced_reason.
    pub fn reason(&self) -> String {
        match self {
            Unpriced::NoEntry => "no_entry".into(),
            Unpriced::MissingField(f) => format!("missing_field:{f}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::compute::{Call, price};
    use super::super::index::{Entry, Index};
    use rust_decimal::Decimal;

    fn index() -> Index {
        let fields = [
            "input_cost_per_token",
            "output_cost_per_token",
            "cache_read_input_token_cost",
            "cache_creation_input_token_cost",
        ]
        .iter()
        .map(|k| (k.to_string(), Decimal::new(1, 6)))
        .collect();
        Index::from_entries(vec![Entry {
            key: "m".into(),
            provider: "p".into(),
            fields,
        }])
    }

    fn call(write_1h: i64) -> Call<'static> {
        Call {
            model: "m",
            provider: "p",
            service_tier: None,
            input: 1,
            cache_read: 1,
            cache_write_5m: 1,
            cache_write_1h: write_1h,
            output: 1,
        }
    }

    #[test]
    fn missing_field_nonzero_is_unpriced() {
        let r = price(&index(), &call(5)).unwrap_err();
        assert_eq!(
            r.reason(),
            "missing_field:cache_creation_input_token_cost_above_1hr"
        );
    }

    #[test]
    fn missing_field_zero_is_priced() {
        assert!(price(&index(), &call(0)).is_ok());
    }
}
