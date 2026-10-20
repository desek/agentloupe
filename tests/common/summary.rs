//! @agents-index Test helper for summaries: sends the calls of one session through the proxy and reads summary rows.
#![allow(dead_code)]

use super::loupe::Loupe;

/// Sends `n` Anthropic Messages calls of session `sid` through the proxy; each body grows the history.
pub async fn send_session(l: &Loupe, sid: &str, n: usize) {
    let c = reqwest::Client::new();
    let before = l.rows();
    let mut messages = vec![];
    for i in 0..n {
        messages.push(serde_json::json!({"role":"user","content":format!("<system-reminder>hidden</system-reminder>please fix the parser step {i}")}));
        let body = serde_json::json!({"model":"claude-haiku-5-5","system":"SYSTEM PROMPT","messages":messages});
        let r = c
            .post(l.url("/pi/fake/v1/messages"))
            .header("x-session-id", sid)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        r.bytes().await.unwrap();
        messages.push(serde_json::json!({"role":"assistant","content":"done"}));
    }
    l.wait_rows(before + n as i64);
}

pub fn scalar(l: &Loupe, sql: &str) -> i64 {
    l.db().query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn text(l: &Loupe, sql: &str) -> Option<String> {
    l.db().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// The base URL of a summary mode of the fake upstream.
pub fn base(fake: u16, mode: &str) -> String {
    format!("http://127.0.0.1:{fake}/summary/{mode}")
}

/// Env values that keep the serve workers from running again during a test.
pub const QUIET: &[(&str, &str)] = &[
    ("LOUPE_PARSE_INTERVAL_SECS", "86400"),
    ("LOUPE_SUMMARY_INTERVAL_SECS", "86400"),
];
