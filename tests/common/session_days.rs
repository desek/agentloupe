//! @agents-index Test helper for the session texts: log lines on chosen days, a backfill run in UTC, and the recorded summary requests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// A phrase of the system prompt of a full session summary.
pub const FULL_MARK: &str = "from its start up to the end of one day";
/// A phrase of the system prompt of a session day summary.
pub const DAY_MARK: &str = "what one coding-agent session did on one day";

/// One Claude Code user text line.
pub fn user(session: &str, text: &str, at: &str) -> String {
    serde_json::json!({"type":"user","sessionId":session,"timestamp":at,
        "message":{"role":"user","content":text}})
    .to_string()
}

/// One Claude Code assistant line with usage and only a tool input: a call with no text.
pub fn tool_call(session: &str, msg: &str, at: &str) -> String {
    serde_json::json!({"type":"assistant","sessionId":session,"timestamp":at,
        "message":{"id":msg,"role":"assistant","model":"claude-haiku-5-5",
        "content":[{"type":"tool_use","input":{"cmd":"ls"}}],
        "usage":{"input_tokens":2,"output_tokens":3}}})
    .to_string()
}

/// One Claude Code assistant line with usage and text: a call.
pub fn answer(session: &str, msg: &str, text: &str, at: &str) -> String {
    serde_json::json!({"type":"assistant","sessionId":session,"timestamp":at,
        "message":{"id":msg,"role":"assistant","model":"claude-haiku-5-5",
        "content":[{"type":"text","text":text}],
        "usage":{"input_tokens":2,"output_tokens":3}}})
    .to_string()
}

/// A Claude Code root that holds one session file with `lines`.
pub fn tree(root: &Path, session: &str, lines: &[String]) -> PathBuf {
    let path = root
        .join("claude")
        .join("-Users-x-proj")
        .join(format!("{session}.jsonl"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    root.join("claude")
}

/// One user turn and one call on each of the days 2026-01-01 to 2026-01-0N, with the marker DAYTEXT<d>.
pub fn days(session: &str, n: u32) -> Vec<String> {
    (1..=n)
        .flat_map(|d| {
            let at = format!("2026-01-0{d}T10:00:00Z");
            [
                user(
                    session,
                    &format!("please do the work of DAYTEXT{d} now"),
                    &at,
                ),
                answer(session, &format!("m{d}"), &format!("did DAYTEXT{d}"), &at),
            ]
        })
        .collect()
}

/// Runs `loupe backfill` over `root` in UTC; `mode` is the fake summary mode, None for no credential.
pub fn backfill(data: &Path, root: &Path, fake: u16, mode: Option<&str>) -> std::process::Output {
    let url = format!("http://127.0.0.1:{fake}/summary/{}", mode.unwrap_or("none"));
    let mut env = vec![
        ("LOUPE_TZ", "UTC"),
        ("LOUPE_SUMMARY_BASE_URL", url.as_str()),
    ];
    if mode.is_some() {
        env.push(("LOUPE_SUMMARY_API_KEY", "sk-test-Sd4Qr7"));
    }
    super::bf::ok(
        data,
        &["--source", "claude-code", "--path", root.to_str().unwrap()],
        &env,
    )
}

/// The system and user text of each request to `mode` whose system prompt holds `mark`.
pub fn requests(mode: &str, mark: &str) -> Vec<(String, String)> {
    super::fake::summary_bodies(mode)
        .into_iter()
        .map(|b| {
            (
                b["system"].to_string(),
                b["messages"][0]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .filter(|(s, _)| s.contains(mark))
        .collect()
}

/// The status of each row of `table` for the days in order.
pub fn statuses(data: &Path, table: &str) -> Vec<(String, String)> {
    super::bf::db(data)
        .prepare(&format!("SELECT day, status FROM {table} ORDER BY day"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
