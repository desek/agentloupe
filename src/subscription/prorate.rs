//! @agents-index The pure subscription cost of one calendar day: each monthly price divided by the days of that month.

use super::config::Subscription;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

/// The number of days of the calendar month of `date`.
fn days_in_month(date: NaiveDate) -> u32 {
    let (y, m) = (date.year(), date.month());
    let next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    };
    next.and_then(|n| n.pred_opt())
        .map_or(30, |last| last.day())
}

/// The subscription cost of `date` (a calendar day in LOUPE_TZ) as decimal text with at most 12
/// fraction digits. It uses the current subscriptions for every day, because the configuration
/// holds no history of prices; the day record reads it in a later change.
#[allow(dead_code)]
pub fn subscription_cost_for_day(date: NaiveDate, subscriptions: &[Subscription]) -> String {
    let days = Decimal::from(days_in_month(date));
    let sum: Decimal = subscriptions
        .iter()
        .map(|s| (s.usd_per_month / days).round_dp(12))
        .sum();
    sum.round_dp(12).normalize().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn claude_max() -> Vec<Subscription> {
        vec![Subscription {
            name: "claude_max".into(),
            usd_per_month: Decimal::from(200),
        }]
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::from_str(s).unwrap()
    }

    #[test]
    fn prorate_by_days_in_month() {
        assert_eq!(
            subscription_cost_for_day(day("2026-02-10"), &claude_max()),
            "7.142857142857"
        );
        assert_eq!(
            subscription_cost_for_day(day("2026-10-10"), &claude_max()),
            "6.451612903226"
        );
    }

    #[test]
    fn range_sum_of_full_month_is_monthly_price() {
        let sum: Decimal = (1..=31)
            .map(|d| {
                let text =
                    subscription_cost_for_day(day(&format!("2026-10-{d:02}")), &claude_max());
                Decimal::from_str(&text).unwrap()
            })
            .sum();
        assert!(
            (sum - Decimal::from(200)).abs() < Decimal::new(1, 9),
            "{sum}"
        );
    }

    #[test]
    fn no_subscription_is_zero() {
        assert_eq!(subscription_cost_for_day(day("2026-10-10"), &[]), "0");
    }
}
