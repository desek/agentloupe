//! @agents-index Prints the shape of captured requests: header names, body field paths, and booleans; never a value.
//!
//! Usage: LOUPE_DATA_KEY=<key> cargo run --example capture_shape -- <data dir>
//! For the verification of the session and account sources with real agents. The output holds
//! only names, paths, booleans, and the masked shape of metadata.user_id.

#[allow(dead_code)]
#[path = "../src/capture/seal.rs"]
mod seal;

use base64::Engine;
use serde_json::Value;
use std::collections::BTreeSet;

/// Object key paths to depth 2; array items collapse to `[]`, and message content is not walked.
fn paths(v: &Value, prefix: &str, depth: usize, out: &mut BTreeSet<String>) {
    if depth == 0 {
        return;
    }
    match v {
        Value::Object(m) => {
            for (k, child) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.insert(p.clone());
                if !matches!(
                    k.as_str(),
                    "messages" | "tools" | "system" | "content" | "input_schema"
                ) {
                    paths(child, &p, depth - 1, out);
                }
            }
        }
        Value::Array(a) => {
            if let Some(first) = a.first() {
                paths(first, &format!("{prefix}[]"), depth, out);
            }
        }
        _ => {}
    }
}

/// The shape of `metadata.user_id` with every value part masked: known words stay, the rest is `<id>`.
fn user_id_shape(raw: &str) -> String {
    if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(raw) {
        return format!("json object with keys {:?}", m.keys().collect::<Vec<_>>());
    }
    let words = ["user", "account", "session"];
    let parts: Vec<&str> = raw
        .split('_')
        .map(|p| {
            if words.contains(&p) {
                p
            } else if p.is_empty() {
                ""
            } else {
                "<id>"
            }
        })
        .collect();
    format!("string {}", parts.join("_"))
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("error: the data dir is missing. Fix: pass it as the first argument. Verify: the program prints one line per exchange.");
        std::process::exit(2);
    });
    let key_b64 = std::env::var("LOUPE_DATA_KEY").unwrap_or_default();
    let key: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .ok()
        .and_then(|k| k.try_into().ok())
        .unwrap_or_else(|| {
            eprintln!("error: LOUPE_DATA_KEY is missing or not 32 bytes. Fix: export the key of the data dir. Verify: the program prints one line per exchange.");
            std::process::exit(2);
        });
    let conn = rusqlite::Connection::open(format!("{dir}/loupe.db")).expect("the database opens");
    let mut st = conn
        .prepare(
            "SELECT e.id, e.agent, e.path, e.request_headers, e.request_body, e.parse_outcome, e.session_hint_source, \
             s.id, s.key_source, a.source, c.input + c.cache_read + c.output, c.model \
             FROM exchanges e LEFT JOIN calls c ON c.exchange_id = e.id LEFT JOIN sessions s ON s.id = c.session_id \
             LEFT JOIN accounts a ON a.id = c.account_id ORDER BY e.rowid",
        )
        .unwrap();
    let rows = st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Vec<u8>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<i64>>(10)?,
                r.get::<_, Option<String>>(11)?,
            ))
        })
        .unwrap();
    for row in rows.flatten() {
        let (
            id,
            agent,
            path,
            headers,
            body,
            outcome,
            hint,
            session,
            key_source,
            account,
            tokens,
            model,
        ) = row;
        let names: Vec<String> = serde_json::from_str::<Value>(&headers)
            .ok()
            .and_then(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|p| p[0].as_str().map(str::to_string))
                        .collect()
                })
            })
            .unwrap_or_default();
        let req: Value = seal::open(&key, &id, "request", &body)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut p = BTreeSet::new();
        paths(&req, "", 2, &mut p);
        let shape = req["metadata"]["user_id"].as_str().map(user_id_shape);
        let line = serde_json::json!({
            "agent": agent, "path": path, "parse_outcome": outcome, "model": model,
            "tokens_above_0": tokens.map(|t| t > 0), "session_hint_source": hint,
            "session_row": session, "key_source": key_source, "account_source": account,
            "header_names": names, "body_paths": p, "metadata_user_id_shape": shape,
            "prompt_cache_key_present": req.get("prompt_cache_key").is_some(),
        });
        println!("{line}");
    }
}
