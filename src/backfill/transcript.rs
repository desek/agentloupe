//! @agents-index Builds the transcript of a backfilled segment from its log entries, in memory only, with the summary transcript rules.

use super::discover::LogFile;
use super::entry::ms;
use super::{Source, claude};
use serde_json::{Value, json};

/// True when the entry time is inside the window of the segment: after the previous segment, up to the last call.
fn inside(e: &Value, after: Option<i64>, until: i64) -> bool {
    ms(&e["timestamp"]).is_some_and(|t| after.is_none_or(|a| t > a) && t <= until)
}

/// The user and assistant messages of a Claude Code file for one session key, in line order.
/// The lines of one assistant message (one `message.id`) become one turn.
pub fn claude_messages(
    k: &[u8; 32],
    file: &LogFile,
    lines: &[(usize, Value)],
    key_hash: &str,
    after: Option<i64>,
    until: i64,
) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    let mut last_id: Option<String> = None;
    for (_, l) in lines {
        let role = match l["type"].as_str() {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };
        if !inside(l, after, until) || claude::line_key(k, file, l).as_deref() != Some(key_hash) {
            continue;
        }
        let m = &l["message"];
        let content = match &m["content"] {
            Value::String(s) => json!([{"type": "text", "text": s}]),
            Value::Array(a) => Value::Array(a.clone()),
            _ => continue,
        };
        let id = m["id"].as_str().map(str::to_string);
        if role == "assistant"
            && id.is_some()
            && id == last_id
            && let Some(prev) = out.last_mut().and_then(|p| p["content"].as_array_mut())
        {
            prev.extend(content.as_array().cloned().unwrap_or_default());
            continue;
        }
        last_id = if role == "assistant" { id } else { None };
        out.push(json!({"role": role, "content": content}));
    }
    out
}

/// One Pi content part in the Anthropic shape that the transcript rules read.
fn pi_part(p: &Value) -> Value {
    match p["type"].as_str() {
        Some("toolCall") => json!({"type": "tool_use", "input": p["arguments"]}),
        Some("thinking") => json!({"type": "thinking"}),
        _ => p.clone(),
    }
}

/// The user, assistant, and tool result messages of a Pi file, in line order.
pub fn pi_messages(lines: &[(usize, Value)], after: Option<i64>, until: i64) -> Vec<Value> {
    let mut out = vec![];
    for (_, e) in lines {
        if e["type"] != "message" || !inside(e, after, until) {
            continue;
        }
        let m = &e["message"];
        let parts: Vec<Value> = match &m["content"] {
            Value::String(s) => vec![json!({"type": "text", "text": s})],
            Value::Array(a) => a.iter().map(pi_part).collect(),
            _ => continue,
        };
        out.push(match m["role"].as_str() {
            Some("user") => json!({"role": "user", "content": parts}),
            Some("assistant") => json!({"role": "assistant", "content": parts}),
            Some("toolResult") => {
                json!({"role": "user", "content": [{"type": "tool_result", "content": parts}]})
            }
            _ => continue,
        });
    }
    out
}

/// The transcript of one segment from the files that hold its session key. Empty when no turn has text.
pub fn segment(
    source: Source,
    k: &[u8; 32],
    files: &[&LogFile],
    key_hash: &str,
    after: Option<i64>,
    until: i64,
) -> String {
    let mut messages = vec![];
    for f in files {
        let Ok(lines) = super::read::file(&f.path, source.name(), 0, false) else {
            continue;
        };
        messages.extend(match source {
            Source::ClaudeCode => claude_messages(k, f, &lines.entries, key_hash, after, until),
            Source::Pi => pi_messages(&lines.entries, after, until),
        });
    }
    crate::summary::transcript::build_messages(&messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_transcript_follows_summary_rules() {
        let k = [4u8; 32];
        let f = LogFile {
            path: "/p/s.jsonl".into(),
            subagent_of: None,
        };
        let ts = "2026-01-01T10:00:00Z";
        let lines: Vec<(usize, Value)> = vec![
            json!({"type":"user","sessionId":"s","timestamp":ts,"message":{"role":"user","content":"<system-reminder>HIDDEN</system-reminder>fix the parser"}}),
            json!({"type":"assistant","sessionId":"s","timestamp":ts,"message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"looking"}]}}),
            json!({"type":"assistant","sessionId":"s","timestamp":ts,"message":{"id":"m1","role":"assistant","content":[{"type":"tool_use","input":{"cmd":"ls"}}]}}),
            json!({"type":"user","sessionId":"s","timestamp":ts,"message":{"role":"user","content":[{"type":"tool_result","content":"y".repeat(1000)}]}}),
            json!({"type":"user","sessionId":"s","timestamp":ts,"message":{"role":"user","content":[{"type":"image","source":{}}]}}),
            json!({"type":"user","sessionId":"other","timestamp":ts,"message":{"role":"user","content":"NOT THIS SESSION"}}),
            json!({"type":"system","sessionId":"s","timestamp":ts,"content":"SYSTEM LINE"}),
        ]
        .into_iter()
        .enumerate()
        .collect();
        let until = ms(&json!(ts)).unwrap();
        let msgs = claude_messages(
            &k,
            &f,
            &lines,
            &super::super::key::session(&k, "s"),
            None,
            until,
        );
        let t = crate::summary::transcript::build_messages(&msgs);
        let rows: Vec<&str> = t.lines().collect();
        assert_eq!(rows.len(), 4, "{t}");
        assert_eq!(rows[0], "user: fix the parser");
        assert!(rows[1].starts_with("assistant: looking [tool input]"));
        assert!(rows[2].starts_with("user: [tool result] ") && rows[2].ends_with("[clipped]"));
        assert_eq!(rows[2].matches('y').count(), 300);
        assert_eq!(rows[3], "user: [attachment]");
        for bad in ["HIDDEN", "NOT THIS", "SYSTEM LINE"] {
            assert!(!t.contains(bad));
        }
        assert!(claude_messages(&k, &f, &lines, "x", None, until).is_empty());
        let pi: Vec<(usize, Value)> = vec![(
            1,
            json!({"type":"message","timestamp":ts,"message":{"role":"toolResult","content":[{"type":"image"}]}}),
        )];
        assert_eq!(
            crate::summary::transcript::build_messages(&pi_messages(&pi, None, until)),
            "user: [tool result] [attachment]"
        );
    }
}
