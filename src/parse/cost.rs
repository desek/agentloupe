//! @agents-index Prices one parsed call against the embedded index into the three cost columns, and logs the warn line of an unpriced call.

use super::usage::Usage;
use crate::price::compute::{Call, price as compute, text};
use crate::price::embed::PRICE_COMMIT;
use crate::price::index::Index;
use crate::price::unpriced::Unpriced;
use std::collections::BTreeMap;

/// The values of calls.cost_usd, calls.price_commit, and calls.unpriced_reason.
pub struct Priced {
    pub cost_usd: Option<String>,
    pub price_commit: &'static str,
    pub unpriced_reason: Option<String>,
}

/// Prices `u`, a call through `upstream`; an upstream without a provider has no entry.
pub fn price(providers: &BTreeMap<String, String>, upstream: &str, u: &Usage) -> Priced {
    let result = match (u.model.as_deref(), providers.get(upstream)) {
        (Some(model), Some(provider)) => compute(
            Index::embedded(),
            &Call {
                model,
                provider,
                service_tier: u.service_tier.as_deref(),
                input: u.input,
                cache_read: u.cache_read,
                cache_write_5m: u.cache_write_5m,
                cache_write_1h: u.cache_write_1h,
                output: u.output,
            },
        ),
        _ => Err(Unpriced::NoEntry),
    };
    match result {
        Ok(d) => Priced {
            cost_usd: Some(text(d)),
            price_commit: PRICE_COMMIT,
            unpriced_reason: None,
        },
        Err(r) => Priced {
            cost_usd: None,
            price_commit: PRICE_COMMIT,
            unpriced_reason: Some(r.reason()),
        },
    }
}

/// One warn line for an unpriced call, with the fix.
pub fn warn(call_id: i64, model: Option<&str>, upstream: &str, reason: &str) {
    tracing::warn!(
        call_id,
        model = model.unwrap_or(""),
        upstream,
        reason,
        price_commit = PRICE_COMMIT,
        "the call is unpriced, so its cost is null. Fix: run .agents/scripts/prices.vendor.sh and rebuild loupe; new prices apply only to new calls and to calls with a null price_commit (loupe price backfill). Reference: docs/specification/cost.md. Verify: loupe price show --model <model> --upstream <upstream> prints the entry"
    );
}
