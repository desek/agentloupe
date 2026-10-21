//! @agents-index Maps the lines of a Claude Code log file to calls: assistant lines with usage, one call per message.id group.

use super::discover::LogFile;
use super::entry::{Candidate, FileCalls, ms};
use super::key;
use crate::parse::anthropic_json::counts;
use crate::parse::usage::{Usage, text};
use serde_json::Value;

const SOURCE: &str = "claude-code";

/// The raw session ID of a line: the stem of a subagent file, else the `sessionId` of the line.
fn session_id(file: &LogFile, line: &Value) -> Option<String> {
    if file.subagent_of.is_some() {
        return file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned());
    }
    line["sessionId"].as_str().map(str::to_string)
}

/// The session key hash of a line, or None when the line has no session ID.
pub fn line_key(k: &[u8; 32], file: &LogFile, line: &Value) -> Option<String> {
    session_id(file, line).map(|s| key::session(k, &s))
}

/// The calls of one file. Lines with one `message.id` make one call: the usage of the last line,
/// the time of the first. A `<synthetic>` model or five zero counts make no call.
pub fn calls(
    k: &[u8; 32],
    file_index: usize,
    file: &LogFile,
    lines: &[(usize, Value)],
) -> FileCalls {
    let mut out = FileCalls::default();
    let mut groups: std::collections::HashMap<String, usize> = Default::default();
    for (_, l) in lines {
        let Some(sid) = session_id(file, l) else {
            continue;
        };
        let scope_key = key::session(k, &sid);
        if !out.keys.contains(&scope_key) {
            out.keys.push(scope_key.clone());
        }
        let m = &l["message"];
        let (Some(id), Some(at)) = (m["id"].as_str(), ms(&l["timestamp"])) else {
            continue;
        };
        if l["type"] != "assistant" || !m["usage"].is_object() {
            continue;
        }
        let mut u = Usage::default();
        counts(&m["usage"], &mut u);
        u.model = text(&m["model"]);
        if let Some(i) = groups.get(id) {
            out.calls[*i].usage = u;
            continue;
        }
        let main_key = match &file.subagent_of {
            Some(main) => key::session(k, main),
            None => scope_key.clone(),
        };
        // The line sessionId, not the subagent stem, is the stable session part of the entry ID.
        let entry_session = l["sessionId"].as_str().unwrap_or(&sid);
        groups.insert(id.to_string(), out.calls.len());
        out.calls.push(Candidate {
            file: file_index,
            scope_key,
            main_key,
            log_key: key::log(k, SOURCE, entry_session, id),
            at_ms: at,
            upstream: "anthropic".into(),
            usage: u,
            captured: false,
        });
    }
    out.calls.retain(|c| {
        let u = &c.usage;
        u.model.as_deref() != Some("<synthetic>")
            && [
                u.input,
                u.cache_read,
                u.cache_write_5m,
                u.cache_write_1h,
                u.output,
            ] != [0; 5]
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const K: [u8; 32] = [1u8; 32];

    fn main_file() -> LogFile {
        LogFile {
            path: "/x/p/s.jsonl".into(),
            subagent_of: None,
        }
    }

    fn line(id: &str, ts: &str, model: &str, usage: Value) -> (usize, Value) {
        (
            1,
            json!({"type":"assistant","sessionId":"s1","timestamp":ts,"message":{"id":id,"model":model,"usage":usage}}),
        )
    }

    #[test]
    fn one_call_per_message_id() {
        let lines = vec![
            line(
                "m1",
                "2026-01-01T10:00:00Z",
                "claude-x",
                json!({"input_tokens":1,"output_tokens":1}),
            ),
            line(
                "m1",
                "2026-01-01T10:00:01Z",
                "claude-x",
                json!({"input_tokens":1,"output_tokens":5}),
            ),
            line(
                "m1",
                "2026-01-01T10:00:02Z",
                "claude-x",
                json!({"input_tokens":3,"output_tokens":9}),
            ),
        ];
        let c = calls(&K, 0, &main_file(), &lines).calls;
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].usage.input, c[0].usage.output), (3, 9));
        assert_eq!(c[0].at_ms, 1_767_261_600_000);
        assert_eq!(c[0].scope_key, key::session(&K, "s1"));
    }

    #[test]
    fn split_and_legacy_cache_writes() {
        let lines = vec![
            line(
                "a",
                "2026-01-01T10:00:00Z",
                "m",
                json!({"input_tokens":1,"cache_read_input_tokens":2,"cache_creation_input_tokens":30,
                "cache_creation":{"ephemeral_5m_input_tokens":10,"ephemeral_1h_input_tokens":20},"output_tokens":4,"service_tier":"standard"}),
            ),
            line(
                "b",
                "2026-01-01T10:00:01Z",
                "m",
                json!({"input_tokens":1,"cache_creation_input_tokens":7,"output_tokens":4}),
            ),
        ];
        let c = calls(&K, 0, &main_file(), &lines).calls;
        let u = &c[0].usage;
        assert_eq!(
            (
                u.input,
                u.cache_read,
                u.cache_write_5m,
                u.cache_write_1h,
                u.output
            ),
            (1, 2, 10, 20, 4)
        );
        assert_eq!(u.service_tier.as_deref(), Some("standard"));
        assert_eq!(
            (c[1].usage.cache_write_5m, c[1].usage.cache_write_1h),
            (7, 0)
        );
    }

    #[test]
    fn synthetic_or_zero_usage_gives_no_call() {
        let lines = vec![
            line(
                "a",
                "2026-01-01T10:00:00Z",
                "<synthetic>",
                json!({"input_tokens":5,"output_tokens":4}),
            ),
            line(
                "b",
                "2026-01-01T10:00:01Z",
                "claude-x",
                json!({"input_tokens":0,"output_tokens":0}),
            ),
        ];
        assert!(calls(&K, 0, &main_file(), &lines).calls.is_empty());
    }
}
