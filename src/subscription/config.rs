//! @agents-index Parses LOUPE_SUBSCRIPTION_<NAME>=<USD per month> into a named subscription, with an actionable error.

use crate::config::error::ConfigError;
use rust_decimal::Decimal;
use std::str::FromStr;

pub const PREFIX: &str = "LOUPE_SUBSCRIPTION_";

/// One subscription: the lowercase name and the real monthly price in US dollars.
#[derive(Debug, Clone, PartialEq)]
pub struct Subscription {
    pub name: String,
    pub usd_per_month: Decimal,
}

fn invalid(var: &str, problem: &str) -> ConfigError {
    ConfigError::new(
        var,
        format!(
            "{problem}; the expected shape is LOUPE_SUBSCRIPTION_<NAME>=<USD per month>, with <NAME> in [A-Z0-9_]+ and a non-negative decimal with at most 2 fraction digits, for example LOUPE_SUBSCRIPTION_CLAUDE_MAX=200"
        ),
        vec![
            format!("set {var} to a price such as 200 or 19.99 in .env or in the environment"),
            format!("unset {var} if you have no such subscription"),
        ],
    )
}

/// Parses one variable `var` with its `value`.
pub fn parse(var: &str, value: &str) -> Result<Subscription, ConfigError> {
    let name = var.strip_prefix(PREFIX).unwrap_or("");
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(invalid(var, "has a name that does not match [A-Z0-9_]+"));
    }
    let digits_ok = value.split_once('.').map_or(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        |(i, f)| {
            !i.is_empty()
                && !f.is_empty()
                && f.len() <= 2
                && i.bytes().chain(f.bytes()).all(|b| b.is_ascii_digit())
        },
    );
    let usd = digits_ok
        .then(|| Decimal::from_str(value).ok())
        .flatten()
        .ok_or_else(|| {
            invalid(
                var,
                "is not a non-negative decimal with at most 2 fraction digits",
            )
        })?;
    Ok(Subscription {
        name: name.to_ascii_lowercase(),
        usd_per_month: usd,
    })
}

/// Parses every subscription variable of `pairs`, in name order.
pub fn parse_all<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Vec<Subscription>, ConfigError> {
    let mut out: Vec<Subscription> = pairs
        .into_iter()
        .filter(|(k, _)| k.starts_with(PREFIX))
        .map(|(k, v)| parse(k, v))
        .collect::<Result<_, _>>()?;
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_vars_parse() {
        let s = parse_all([
            ("LOUPE_SUBSCRIPTION_CLAUDE_MAX", "200"),
            ("LOUPE_SUBSCRIPTION_CHATGPT_PRO", "20"),
        ])
        .unwrap();
        let names: Vec<&str> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["chatgpt_pro", "claude_max"]);
        assert_eq!(s[1].usd_per_month, Decimal::from(200));
    }

    #[test]
    fn invalid_subscription_is_actionable() {
        for value in ["-1", "abc", "1.234"] {
            let e = parse("LOUPE_SUBSCRIPTION_X", value)
                .unwrap_err()
                .to_string();
            for needle in [
                "LOUPE_SUBSCRIPTION_X",
                "<USD per month>",
                "loupe config show",
            ] {
                assert!(e.contains(needle), "missing {needle} in {e}");
            }
        }
        assert!(parse("LOUPE_SUBSCRIPTION_bad", "1").is_err());
    }
}
