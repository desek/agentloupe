//! @agents-index The one render of the effective configuration for config show and the private config route; secrets only as set or unset.

use super::loader::{Entry, Loaded};

/// One printed entry. `value` is already masked, so a surface that prints it cannot leak a secret.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub name: String,
    pub value: String,
    pub source: String,
    pub secret: bool,
}

/// The printable value: secrets become set or unset, a missing plain value becomes unset. A secret
/// of only white space counts as unset, as the provider detection reads it.
fn shown(e: &Entry) -> String {
    match (&e.value, e.secret) {
        (None, _) => "unset".into(),
        (Some(v), true) if v.expose().trim().is_empty() => "unset".into(),
        (Some(_), true) => "set".into(),
        (Some(v), false) => v.expose().to_string(),
    }
}

/// The derived lines after the variables: the price commit, the size of the price index, the
/// telemetry build state, and each subscription with the total. A subscription that is not valid
/// prints as invalid.
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
        (
            "role".to_string(),
            cfg.get("LOUPE_ROLE").map_or("full".into(), shown),
            cfg.get("LOUPE_ROLE")
                .map_or("default", |e| source_name(&e.source.to_string())),
        ),
        // The build state only; the line reads no run-time variable.
        (
            "telemetry".to_string(),
            crate::telemetry::build_state(),
            "build",
        ),
    ];
    let get = |n: &str| {
        cfg.get(n)
            .and_then(|e| e.value.as_ref())
            .map(|v| v.expose())
    };
    let providers = crate::provider::detect::detect(cfg);
    // An invalid selection still prints a line; the start error names the fix.
    let summary = crate::summary::credential::select_provider(
        get("LOUPE_SUMMARY_PROVIDER"),
        &crate::provider::detect::names(&providers),
    )
    .map_or("invalid: LOUPE_SUMMARY_PROVIDER".to_string(), |p| {
        p.map_or("none".to_string(), |p| p.name().to_string())
    });
    let tz = get("LOUPE_TZ").and_then(|v| v.parse::<chrono_tz::Tz>().ok());
    let at = get("LOUPE_PUBLISH_AT").and_then(super::publish_at);
    let (local, utc) = match (tz, at) {
        (Some(tz), Some(at)) => {
            let t = crate::publish::due::next_due(chrono::Utc::now().timestamp_millis(), tz, at);
            (t.to_rfc3339(), crate::publish::due::utc(t).to_rfc3339())
        }
        _ => (
            "invalid: LOUPE_TZ or LOUPE_PUBLISH_AT".into(),
            "invalid".into(),
        ),
    };
    out.push(("publish_next_due_local".into(), local, "derived"));
    out.push((
        "site_title".into(),
        get("LOUPE_SITE_TITLE")
            .unwrap_or_default()
            .trim()
            .to_string(),
        "derived",
    ));
    out.push(("publish_next_due_utc".into(), utc, "derived"));
    // One line per known provider; the line names the kind, never the credential.
    for d in &providers {
        let source = source_name(&d.source.to_string());
        out.push((
            format!("provider.{}", d.provider.name()),
            format!(
                "credential={} base_url={} source={source}",
                crate::summary::credential::kind(d.credential.as_ref()),
                d.base_url
            ),
            source,
        ));
    }
    out.push(("summary_provider".into(), summary, "derived"));
    // The live lines carry the source of their variable, so an override is visible.
    for (line, var) in [
        ("live_enabled", "LOUPE_LIVE_ENABLED"),
        ("live_allowed_origin", "LOUPE_LIVE_ALLOWED_ORIGIN"),
        ("live_max_rate", "LOUPE_LIVE_MAX_RATE"),
        ("live_max_connections", "LOUPE_LIVE_MAX_CONNECTIONS"),
        ("live_buffer", "LOUPE_LIVE_BUFFER"),
    ] {
        let e = cfg.get(var);
        out.push((
            line.into(),
            e.map_or("unset".into(), shown),
            e.map_or("default", |e| source_name(&e.source.to_string())),
        ));
    }
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

/// The static name of a source, so a derived line can carry it.
fn source_name(s: &str) -> &'static str {
    match s {
        "flag" => "flag",
        "env" => "env",
        "file" => "file",
        _ => "default",
    }
}

/// Every entry in print order: each variable with its source, then the derived lines.
pub fn lines(cfg: &Loaded) -> Vec<Line> {
    let vars = cfg.entries.iter().map(|e| Line {
        name: e.name.clone(),
        value: shown(e),
        source: e.source.to_string(),
        secret: e.secret,
    });
    let more = derived(cfg).into_iter().map(|(name, value, source)| Line {
        name,
        value,
        source: source.to_string(),
        secret: false,
    });
    vars.chain(more).collect()
}

/// The JSON object of one entry, as both surfaces print it.
pub fn json(l: &Line) -> serde_json::Value {
    serde_json::json!({"name": l.name, "value": l.value, "source": l.source, "secret": l.secret})
}

/// One JSON object per line when `table` is false, else an aligned table for a human.
pub fn render(cfg: &Loaded, table: bool) -> String {
    let all = lines(cfg);
    if !table {
        return all.iter().map(|l| format!("{}\n", json(l))).collect();
    }
    let width = all.iter().map(|l| l.name.len()).max().unwrap_or(4).max(4);
    let mut out = format!("{:width$}  {:8}  VALUE\n", "NAME", "SOURCE");
    for l in &all {
        out.push_str(&format!("{:width$}  {:8}  {}\n", l.name, l.source, l.value));
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
            ("ANTHROPIC_API_KEY", "Mw9Ty3Bn5Hq7Cz1"),
            ("CLAUDE_CODE_OAUTH_TOKEN", "r6Fk2Jp8Xs4Wd0L"),
            ("OPENAI_API_KEY", "c4Hs8Nv2Rt6Kx0B"),
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
        assert_eq!(value("telemetry"), crate::telemetry::build_state());
        assert_eq!(value("subscription_total_per_month"), "0");
        assert_eq!(value("summary_provider"), "anthropic");
        assert_eq!(
            value("provider.anthropic"),
            "credential=api_key base_url=https://api.anthropic.com source=default"
        );
        assert_eq!(
            value("provider.openai"),
            "credential=api_key base_url=https://api.openai.com/v1 source=default"
        );
        assert!(!json.contains("summary_credential"));
        assert_eq!(value("LOUPE_GITHUB_API_URL"), "https://api.github.com");
        assert_eq!(value("LOUPE_SUMMARY_INTERVAL_SECS"), "30");
        assert_eq!(value("LOUPE_SUMMARY_MAX_ATTEMPTS"), "5");
        assert_eq!(value("LOUPE_RAW_RETENTION_HOURS"), "168");
        assert_eq!(value("LOUPE_PUBLIC_PROJECTS"), "");
        assert_eq!(value("LOUPE_TZ"), "UTC");
        assert_eq!(value("LOUPE_PUBLISH_AT"), "01:00");
        assert_eq!(value("LOUPE_PUBLISH_INTERVAL_SECS"), "60");
        assert_eq!(value("LOUPE_DAY_REVIEW"), "auto");
        assert_eq!(value("site_title"), "Agentloupe");
        assert!(value("publish_next_due_local").contains("T01:00:00+0"));
        assert!(value("publish_next_due_utc").ends_with("+00:00"));
        assert!(
            json.lines()
                .filter(|l| l.contains("\"secret\":true"))
                .all(|l| l.contains("\"value\":\"set\""))
        );
    }
}
