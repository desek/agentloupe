//! @agents-index Maps the entries of a Pi session file to calls: assistant messages with usage, usage entries, and compactions with usage.

use super::entry::{Candidate, FileCalls, ms};
use super::key;
use crate::parse::usage::{Usage, int, text};
use serde_json::Value;

const SOURCE: &str = "pi";

/// The five counts of a Pi usage object. `cacheWrite` includes `cacheWrite1h`, so the 5 minute part is the rest.
fn usage(u: &Value, model: Option<String>) -> Usage {
    let one_hour = int(&u["cacheWrite1h"]);
    let write = int(&u["cacheWrite"]);
    Usage {
        input: int(&u["input"]),
        cache_read: int(&u["cacheRead"]),
        cache_write_5m: if u["cacheWrite1h"].is_number() {
            write - one_hour
        } else {
            write
        },
        cache_write_1h: one_hour,
        output: int(&u["output"]),
        model,
        service_tier: None,
    }
}

/// The usage object, model, and provider of an entry that is a call, or None.
fn call_of(e: &Value) -> Option<(&Value, Option<String>, Option<String>)> {
    let m = &e["message"];
    match e["type"].as_str()? {
        "message" if m["role"] == "assistant" && m["usage"].is_object() => {
            Some((&m["usage"], text(&m["model"]), text(&m["provider"])))
        }
        "usage" => {
            let u = if e["usage"].is_object() {
                &e["usage"]
            } else {
                e
            };
            Some((
                u,
                text(&e["model"]).or_else(|| text(&u["model"])),
                text(&e["provider"]).or_else(|| text(&u["provider"])),
            ))
        }
        "compaction" | "branch_summary" if e["usage"].is_object() => {
            Some((&e["usage"], text(&e["model"]), text(&e["provider"])))
        }
        _ => None,
    }
}

/// The calls of one session file. The session ID is the `id` of the first line. An entry without a
/// model or provider of its own (a compaction) takes those of the latest model before it in the file.
pub fn calls(k: &[u8; 32], file_index: usize, lines: &[(usize, Value)]) -> FileCalls {
    let mut out = FileCalls::default();
    let Some(sid) = lines.first().and_then(|(_, l)| l["id"].as_str()) else {
        return out;
    };
    let scope_key = key::session(k, sid);
    out.keys.push(scope_key.clone());
    let (mut model, mut provider): (Option<String>, Option<String>) = (None, None);
    for (_, e) in lines {
        if let Some(t) = ms(&e["timestamp"]) {
            out.last_ms = Some(out.last_ms.map_or(t, |m| m.max(t)));
        }
        if e["type"] == "model_change" {
            model = text(&e["modelId"]).or(model);
            provider = text(&e["provider"]).or(provider);
        }
        let Some((u, m, p)) = call_of(e) else {
            continue;
        };
        model = m.or(model);
        provider = p.or(provider);
        let (Some(id), Some(at), Some(up)) =
            (e["id"].as_str(), ms(&e["timestamp"]), provider.clone())
        else {
            continue;
        };
        out.calls.push(Candidate {
            file: file_index,
            scope_key: scope_key.clone(),
            main_key: scope_key.clone(),
            log_key: key::log(k, SOURCE, sid, id),
            at_ms: at,
            upstream: up,
            usage: usage(u, model.clone()),
            captured: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const K: [u8; 32] = [2u8; 32];

    fn lines(v: Vec<Value>) -> Vec<(usize, Value)> {
        v.into_iter().enumerate().map(|(i, v)| (i + 1, v)).collect()
    }

    #[test]
    fn maps_message_usage_and_usage_entry() {
        let l = lines(vec![
            json!({"type":"session","id":"sess","timestamp":"2026-01-01T10:00:00Z"}),
            json!({"type":"message","id":"e1","timestamp":"2026-01-01T10:00:01Z","message":{"role":"assistant","provider":"anthropic","model":"claude-x","usage":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"cost":{"total":9}}}}),
            json!({"type":"message","id":"e2","timestamp":"2026-01-01T10:00:02Z","message":{"role":"user","content":[]}}),
            json!({"type":"usage","id":"e3","timestamp":"2026-01-01T10:00:03Z","provider":"openai","model":"gpt-x","usage":{"input":5,"output":6,"cacheRead":0,"cacheWrite":0}}),
            json!({"type":"compaction","id":"e4","timestamp":"2026-01-01T10:00:04Z","usage":{"input":7,"output":8,"cacheRead":0,"cacheWrite":0}}),
            json!({"type":"compaction","id":"e5","timestamp":"2026-01-01T10:00:05Z"}),
        ]);
        let f = calls(&K, 0, &l);
        assert_eq!(f.calls.len(), 3);
        assert_eq!(f.calls[0].upstream, "anthropic");
        assert_eq!(f.calls[0].usage.model.as_deref(), Some("claude-x"));
        assert_eq!(f.calls[0].usage.service_tier, None);
        assert_eq!(
            (f.calls[1].upstream.as_str(), f.calls[1].usage.input),
            ("openai", 5)
        );
        assert_eq!(
            (f.calls[2].upstream.as_str(), f.calls[2].usage.output),
            ("openai", 8)
        );
        assert_eq!(f.last_ms, Some(1_767_261_605_000));
        assert_eq!(f.calls[0].scope_key, key::session(&K, "sess"));
    }

    #[test]
    fn cache_write_1h_is_split_out() {
        let u = usage(
            &json!({"input":1,"output":1,"cacheRead":0,"cacheWrite":100,"cacheWrite1h":30}),
            None,
        );
        assert_eq!((u.cache_write_5m, u.cache_write_1h), (70, 30));
        let u = usage(&json!({"cacheWrite":100}), None);
        assert_eq!((u.cache_write_5m, u.cache_write_1h), (100, 0));
    }
}
