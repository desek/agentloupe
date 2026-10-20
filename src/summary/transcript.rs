//! @agents-index Builds the clipped transcript of a segment from the request body with the longest message history.

use serde_json::Value;

/// The most characters of one tool input or tool result.
const CLIP: usize = 300;
/// The most characters of a transcript before the middle is cut.
const MAX: usize = 24000;
const HEAD: usize = 6000;
const TAIL: usize = 18000;
const OMITTED: &str = "[... earlier turns omitted ...]";

/// The length of the message history of a request body: `messages`, or `input` of OpenAI Responses.
pub fn history_len(body: &Value) -> usize {
    body.get("messages")
        .or_else(|| body.get("input"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// The index of the body to use: the longest history, a tie to the later body.
pub fn pick_longest(lens: &[usize]) -> Option<usize> {
    lens.iter()
        .enumerate()
        .max_by_key(|(i, n)| (**n, *i))
        .map(|(i, _)| i)
}

fn clip(s: &str) -> String {
    if s.chars().count() <= CLIP {
        return s.to_string();
    }
    format!("{} [clipped]", s.chars().take(CLIP).collect::<String>())
}

/// Removes every `<system-reminder>...</system-reminder>` block; an unclosed block runs to the end.
fn strip_reminders(s: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find(OPEN) {
        out.push_str(&rest[..at]);
        rest = match rest[at..].find(CLOSE) {
            Some(end) => &rest[at + end + CLOSE.len()..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => flat(&strip_reminders(s)),
        Value::Array(a) => a
            .iter()
            .map(part)
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// One content part of any supported format as transcript text.
fn part(p: &Value) -> String {
    if let Value::String(s) = p {
        return flat(&strip_reminders(s));
    }
    let kind = p["type"].as_str().unwrap_or("");
    match kind {
        "text" | "input_text" | "output_text" => {
            flat(&strip_reminders(p["text"].as_str().unwrap_or("")))
        }
        "tool_use" => format!("[tool input] {}", clip(&flat(&p["input"].to_string()))),
        "function_call" => format!("[tool input] {}", clip(&flat(&text_of(&p["arguments"])))),
        "tool_result" => format!(
            "[tool result] {}",
            clip(&flat(&strip_reminders(&text_of(&p["content"]))))
        ),
        "function_call_output" => format!("[tool result] {}", clip(&flat(&text_of(&p["output"])))),
        "thinking" | "redacted_thinking" | "reasoning" => String::new(),
        "image" | "document" | "input_image" | "input_file" | "image_url" => "[attachment]".into(),
        _ if p.get("text").is_some() => flat(&strip_reminders(p["text"].as_str().unwrap_or(""))),
        _ => "[attachment]".into(),
    }
}

/// One turn line, or None for a system turn or an empty turn.
fn turn(m: &Value) -> Option<String> {
    let role = match m["role"].as_str() {
        Some("assistant") => "assistant",
        Some("system" | "developer") => return None,
        Some(_) => "user",
        // An OpenAI Responses item without a role: a function call is the assistant, an output the user.
        None => match m["type"].as_str() {
            Some("function_call") => "assistant",
            Some("reasoning") => return None,
            _ => "user",
        },
    };
    let text = match (m.get("content"), m["role"].is_null()) {
        (_, true) => part(m),
        (Some(c), false) => text_of(c),
        (None, false) => String::new(),
    };
    let mut text = text.trim().to_string();
    if let Some(calls) = m["tool_calls"].as_array() {
        for c in calls {
            text.push_str(&format!(
                " [tool input] {}",
                clip(&flat(&text_of(&c["function"]["arguments"])))
            ));
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| format!("{role}: {text}"))
}

/// Cuts the middle of a long transcript: the first 6000 and the last 18000 characters stay.
fn cut(t: String) -> String {
    let n = t.chars().count();
    if n <= MAX {
        return t;
    }
    let head: String = t.chars().take(HEAD).collect();
    let tail: String = t.chars().skip(n - TAIL).collect();
    format!("{head}\n{OMITTED}\n{tail}")
}

/// The transcript of one request body: no system prompt, no tool definitions, no reminders.
/// Empty when no turn has text.
pub fn build(body: &Value) -> String {
    let turns = body
        .get("messages")
        .or_else(|| body.get("input"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(turn).collect::<Vec<_>>())
        .unwrap_or_default();
    cut(turns.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msgs(n: usize) -> Value {
        let m: Vec<Value> = (0..n).map(|i| json!({"role": if i % 2 == 0 {"user"} else {"assistant"}, "content": format!("m{i}")})).collect();
        json!({"system": "SECRET SYSTEM", "messages": m})
    }

    #[test]
    fn picks_longest_history() {
        let bodies = [msgs(2), msgs(6), msgs(4)];
        let lens: Vec<usize> = bodies.iter().map(history_len).collect();
        let i = pick_longest(&lens).unwrap();
        assert_eq!(i, 1);
        assert_eq!(build(&bodies[i]).lines().count(), 6);
        assert_eq!(pick_longest(&[3, 5, 5]), Some(2));
    }

    #[test]
    fn removes_system_and_reminders() {
        let b = json!({"system": [{"type":"text","text":"SYSPROMPT"}], "tools": [{"name":"TOOLDEF"}],
            "messages": [{"role":"user","content":[{"type":"text","text":"<system-reminder>HIDDEN</system-reminder>fix the bug"}]},
                         {"role":"assistant","content":"done"}]});
        let t = build(&b);
        for bad in ["SYSPROMPT", "TOOLDEF", "HIDDEN", "system-reminder"] {
            assert!(!t.contains(bad), "{t}");
        }
        assert_eq!(t, "user: fix the bug\nassistant: done");
        let r = json!({"instructions":"INSTR","input":[{"role":"developer","content":"DEV"},{"role":"user","content":[{"type":"input_text","text":"hi"}]}]});
        assert_eq!(build(&r), "user: hi");
    }

    #[test]
    fn clips_tool_output() {
        let long = "x".repeat(2000);
        let b = json!({"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":long}]},
            {"role":"assistant","content":[{"type":"image","source":{}}]}]});
        let t = build(&b);
        let first = t.lines().next().unwrap();
        assert_eq!(first.matches('x').count(), 300);
        assert!(first.ends_with("[clipped]"));
        assert_eq!(t.lines().nth(1).unwrap(), "assistant: [attachment]");
    }

    #[test]
    fn omits_long_middle() {
        let m: Vec<Value> = (0..60)
            .map(|i| json!({"role":"user","content": format!("{:0>998}", i)}))
            .collect();
        let full: String = m.iter().filter_map(turn).collect::<Vec<_>>().join("\n");
        assert!(full.chars().count() >= 60000);
        let t = build(&json!({ "messages": m }));
        let expect = format!(
            "{}\n{OMITTED}\n{}",
            &full[..6000],
            &full[full.len() - 18000..]
        );
        assert_eq!(t, expect);
    }

    #[test]
    fn empty_when_no_text() {
        assert_eq!(
            build(
                &json!({"messages":[{"role":"user","content":"<system-reminder>x</system-reminder>"}]})
            ),
            ""
        );
    }
}
