//! @agents-index Renders the effective configuration as NDJSON or a table, with every secret shown only as set or unset.

use super::loader::{Entry, Loaded};

/// The printable value: secrets become set or unset, a missing plain value becomes unset.
fn shown(e: &Entry) -> String {
    match (&e.value, e.secret) {
        (None, _) => "unset".into(),
        (Some(_), true) => "set".into(),
        (Some(v), false) => v.expose().to_string(),
    }
}

/// The derived lines after the variables: the price commit, the size of the price index, and
/// each subscription with the total. A subscription that is not valid prints as invalid.
fn derived(cfg: &Loaded) -> Vec<(String, String, &'static str)> {
    let mut out = vec![
        (
            "price_commit".to_string(),
            crate::price::embed::PRICE_COMMIT.to_string(),
            "build",
        ),
        (
            "price_entries".to_string(),
            crate::price::index::Index::embedded().len().to_string(),
            "build",
        ),
    ];
    let pairs = cfg
        .entries
        .iter()
        .filter_map(|e| e.value.as_ref().map(|v| (e.name.as_str(), v.expose())));
    match crate::subscription::config::parse_all(pairs) {
        Ok(subs) => {
            let total: rust_decimal::Decimal = subs.iter().map(|s| s.usd_per_month).sum();
            for s in subs {
                out.push((
                    format!("subscription.{}", s.name),
                    s.usd_per_month.to_string(),
                    "derived",
                ));
            }
            out.push((
                "subscription_total_per_month".into(),
                total.to_string(),
                "derived",
            ));
        }
        Err(e) => out.push((
            "subscription_total_per_month".into(),
            format!("invalid: {}", e.variable),
            "derived",
        )),
    }
    out
}

/// One JSON object per line when `table` is false, else an aligned table for a human.
pub fn render(cfg: &Loaded, table: bool) -> String {
    let extra = derived(cfg);
    if !table {
        let vars = cfg.entries.iter().map(|e| {
            serde_json::json!({"name": e.name, "value": shown(e), "source": e.source.to_string(), "secret": e.secret})
        });
        let more = extra.iter().map(
            |(n, v, s)| serde_json::json!({"name": n, "value": v, "source": s, "secret": false}),
        );
        return vars.chain(more).map(|l| format!("{l}\n")).collect();
    }
    let width = cfg
        .entries
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let width = extra
        .iter()
        .map(|(n, _, _)| n.len())
        .max()
        .unwrap_or(0)
        .max(width);
    let mut out = format!("{:width$}  {:8}  VALUE\n", "NAME", "SOURCE");
    for e in &cfg.entries {
        out.push_str(&format!(
            "{:width$}  {:8}  {}\n",
            e.name,
            e.source.to_string(),
            shown(e)
        ));
    }
    for (n, v, s) in &extra {
        out.push_str(&format!("{n:width$}  {s:8}  {v}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::resolve;
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn masks_every_secret() {
        let secrets = [
            ("LOUPE_DATA_KEY", "Q7zK1pXvR9mW3nT2"),
            ("LOUPE_ACCOUNT_KEY", "h8Jd2LqP0sYv4Rx"),
            ("LOUPE_BOOTSTRAP_TOKEN", "Zt5Nw3Kb7Hc1Gm9"),
            ("LOUPE_GITHUB_TOKEN_PERSONAL", "p2Vx8Qr4Lk6Dj0F"),
            ("LOUPE_SUMMARY_API_KEY", "Mw9Ty3Bn5Hq7Cz1"),
            ("LOUPE_SUMMARY_OAUTH_TOKEN", "r6Fk2Jp8Xs4Wd0L"),
            ("LOUPE_ADMIN_TOKEN", "Yb3Gn7Qt1Kv5Pm9"),
        ];
        let env: BTreeMap<String, String> = secrets
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let cfg = resolve(&[], &env, &BTreeMap::new());
        for table in [false, true] {
            let out = render(&cfg, table);
            for (name, value) in secrets {
                assert!(out.contains(name));
                // Every window of 6 characters of the secret must be absent.
                for i in 0..=value.len() - 6 {
                    assert!(!out.contains(&value[i..i + 6]), "leaked part of {name}");
                }
            }
        }
        let json = render(&cfg, false);
        let value = |name: &str| {
            json.lines()
                .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
                .find(|v| v["name"] == name)
                .map(|v| v["value"].as_str().unwrap().to_string())
                .unwrap_or_else(|| panic!("no {name} line"))
        };
        assert_eq!(value("price_commit"), crate::price::embed::PRICE_COMMIT);
        assert!(value("price_entries").parse::<usize>().unwrap() > 4000);
        assert_eq!(value("subscription_total_per_month"), "0");
        assert!(
            json.lines()
                .filter(|l| l.contains("\"secret\":true"))
                .all(|l| l.contains("\"value\":\"set\""))
        );
    }
}
