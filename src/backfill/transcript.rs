//! @agents-index Reads the log entries of a backfilled session in a time window as messages, in memory only, for the one transcript builder.

use super::discover::LogFile;
use super::entry::ms;
use super::{Source, claude};
use serde_json::{Value, json};

/// True when the entry time is inside the window: after `after` (exclusive), up to `until` (inclusive).
/// A segment line uses the segment window; a session text uses the whole session up to the end of a day,
/// or the day only.
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

/// The messages of one session in the window from the files that hold its session key, in line order.
pub fn messages(
    source: Source,
    k: &[u8; 32],
    files: &[&LogFile],
    key_hash: &str,
    after: Option<i64>,
    until: i64,
) -> Vec<Value> {
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
    messages
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
        let t = crate::summary::transcript::build_messages(&msgs, 400);
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
            crate::summary::transcript::build_messages(&pi_messages(&pi, None, until), 400),
            "user: [tool result] [attachment]"
        );
    }

    fn at(day: u32) -> String {
        format!("2026-01-0{day}T10:00:00Z")
    }

    #[test]
    fn transcript_ends_at_end_of_day() {
        let k = [4u8; 32];
        let f = LogFile {
            path: "/p/s.jsonl".into(),
            subagent_of: None,
        };
        let lines: Vec<(usize, Value)> = (1..=3)
            .map(|d| json!({"type":"user","sessionId":"s","timestamp":at(d),"message":{"role":"user","content":format!("work of day {d}")}}))
            .enumerate()
            .collect();
        let key = super::super::key::session(&k, "s");
        let end_day2 = ms(&json!("2026-01-03T00:00:00Z")).unwrap() - 1;
        let full = claude_messages(&k, &f, &lines, &key, None, end_day2);
        let t = crate::summary::transcript::build_messages(&full, 400);
        assert_eq!(t, "user: work of day 1\nuser: work of day 2");
        let start_day2 = ms(&json!("2026-01-02T00:00:00Z")).unwrap() - 1;
        let day = claude_messages(&k, &f, &lines, &key, Some(start_day2), end_day2);
        assert_eq!(
            crate::summary::transcript::build_messages(&day, 400),
            "user: work of day 2"
        );
    }

    #[test]
    fn subagent_entries_stay_out_of_main_transcript() {
        let k = [4u8; 32];
        let main = LogFile {
            path: "/p/s.jsonl".into(),
            subagent_of: None,
        };
        let sub = LogFile {
            path: "/p/s/subagents/agent-1.jsonl".into(),
            subagent_of: Some("s".into()),
        };
        let line = |text: &str| {
            vec![(
                0,
                json!({"type":"user","sessionId":"s","timestamp":at(1),"message":{"role":"user","content":text}}),
            )]
        };
        let key = super::super::key::session(&k, "s");
        let until = ms(&json!(at(2))).unwrap();
        assert!(claude_messages(&k, &sub, &line("SUBAGENT TEXT"), &key, None, until).is_empty());
        let sub_key = super::super::key::session(&k, "agent-1");
        assert_eq!(
            claude_messages(&k, &sub, &line("SUBAGENT TEXT"), &sub_key, None, until).len(),
            1
        );
        assert_eq!(
            claude_messages(&k, &main, &line("MAIN"), &key, None, until).len(),
            1
        );
    }
}
