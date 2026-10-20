//! @agents-index The token, call, session, and cost sums of a group of calls, with the half-to-even cost rounding.

use rust_decimal::{Decimal, RoundingStrategy};
use std::collections::BTreeSet;
use std::str::FromStr;

/// One call as the builder reads it.
pub struct Call {
    pub agent_label: String,
    pub model: String,
    pub session_id: i64,
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    /// Exact decimal text, or None for an unpriced call.
    pub cost_usd: Option<String>,
}

/// The running sums of a group; the cost stays exact until `cost_rounded`.
#[derive(Default)]
pub struct Sums {
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    pub calls: i64,
    pub sessions: BTreeSet<i64>,
    pub cost: Decimal,
    pub unpriced: i64,
}

impl Sums {
    pub fn add(&mut self, c: &Call) {
        self.input += c.input;
        self.cache_read += c.cache_read;
        self.cache_write += c.cache_write;
        self.output += c.output;
        self.calls += 1;
        self.sessions.insert(c.session_id);
        match c
            .cost_usd
            .as_deref()
            .and_then(|t| Decimal::from_str(t).ok())
        {
            Some(d) => self.cost += d,
            None => self.unpriced += 1,
        }
    }

    pub fn input_total(&self) -> i64 {
        self.input + self.cache_read + self.cache_write
    }

    pub fn cost_rounded(&self) -> Decimal {
        round4(self.cost)
    }
}

/// Rounds half to even to 4 fraction digits.
pub fn round4(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(4, RoundingStrategy::MidpointNearestEven)
        .normalize()
}
