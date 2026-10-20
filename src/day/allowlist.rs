//! @agents-index The one allowlist of public field names: the record at each level and the envelope of GET /v1/days.

/// The top-level fields of a record, in their order.
pub const RECORD: &[&str] = &[
    "day",
    "input_total",
    "input",
    "cache_read",
    "cache_write",
    "output",
    "calls",
    "sessions",
    "cost_usd",
    "unpriced_calls",
    "subscription_cost_usd",
    "prs_opened",
    "prs_merged",
    "lines_added",
    "lines_removed",
    "summary",
    "detail",
];

pub const DETAIL: &[&str] = &["by_agent", "by_model", "github"];

/// The fields of detail.github.
pub const GITHUB: &[&str] = &["failed_tokens"];

/// The fields of one value of by_agent.
pub const AGENT: &[&str] = &[
    "input_total",
    "input",
    "cache_read",
    "cache_write",
    "output",
    "calls",
    "sessions",
    "cost_usd",
    "unpriced_calls",
];

/// The fields of one value of by_model; no sessions.
pub const MODEL: &[&str] = &[
    "input_total",
    "input",
    "cache_read",
    "cache_write",
    "output",
    "calls",
    "cost_usd",
    "unpriced_calls",
];

/// The fields of the envelope of GET /v1/days.
pub const ENVELOPE: &[&str] = &["from", "to", "updated_at", "subscription_cost_usd", "days"];

/// The field names of `record` that are not in the allowlist, as dotted paths; the keys of
/// by_agent and by_model are free.
pub fn violations(record: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let check = |v: &serde_json::Value, allowed: &[&str], at: &str, out: &mut Vec<String>| {
        if let Some(m) = v.as_object() {
            for k in m.keys() {
                if !allowed.contains(&k.as_str()) {
                    out.push(format!("{at}{k}"));
                }
            }
        }
    };
    check(record, RECORD, "", &mut out);
    let detail = &record["detail"];
    check(detail, DETAIL, "detail.", &mut out);
    check(&detail["github"], GITHUB, "detail.github.", &mut out);
    for (map, allowed) in [("by_agent", AGENT), ("by_model", MODEL)] {
        if let Some(m) = detail[map].as_object() {
            for (k, v) in m {
                check(v, allowed, &format!("detail.{map}.{k}."), &mut out);
            }
        }
    }
    out
}

/// The envelope fields outside the allowlist, plus the violations of each record in `days`.
pub fn envelope_violations(body: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = body
        .as_object()
        .map(|m| {
            m.keys()
                .filter(|k| !ENVELOPE.contains(&k.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for r in body["days"].as_array().into_iter().flatten() {
        out.extend(violations(r));
    }
    out
}
