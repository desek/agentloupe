//! @agents-index Finds the format of an exchange from its path and content type, runs the matching parser, and gives the outcome.

use super::outcome::Outcome;
use super::usage::{Usage, text};
use super::{anthropic_json, anthropic_sse, openai_chat, openai_responses};

/// What the parser needs from one exchange, with the bodies already decrypted.
pub struct Exchange<'a> {
    pub path: &'a str,
    pub content_type: &'a str,
    pub response_truncated: bool,
    pub request: &'a [u8],
    pub response: &'a [u8],
}

/// The format name for the log line, and the call or the reason for no call.
pub struct Parsed {
    pub format: &'static str,
    pub result: Result<Usage, Outcome>,
}

/// `count_tokens` is an Anthropic Messages path that never has a usage block, so it gives no_usage.
fn format(path: &str, stream: bool) -> Option<&'static str> {
    let p = path.trim_end_matches('/');
    if p.ends_with("/v1/messages") || p.ends_with("/v1/messages/count_tokens") {
        Some(if stream {
            "anthropic.sse"
        } else {
            "anthropic.json"
        })
    } else if p.ends_with("/responses") {
        Some(if stream {
            "openai.responses.sse"
        } else {
            "openai.responses.json"
        })
    } else if p.ends_with("/chat/completions") {
        Some(if stream {
            "openai.chat.sse"
        } else {
            "openai.chat.json"
        })
    } else {
        None
    }
}

pub fn parse(e: &Exchange) -> Parsed {
    let stream = e.content_type.contains("text/event-stream");
    let Some(name) = format(e.path, stream) else {
        return Parsed {
            format: "unknown",
            result: Err(Outcome::UnknownFormat),
        };
    };
    if e.response_truncated {
        return Parsed {
            format: name,
            result: Err(Outcome::Truncated),
        };
    }
    let found = match name {
        "anthropic.sse" => anthropic_sse::parse(e.response),
        "anthropic.json" => anthropic_json::parse(e.response),
        "openai.responses.sse" | "openai.responses.json" => {
            openai_responses::parse(e.response, stream)
        }
        _ => openai_chat::parse(e.response, stream),
    };
    let result = found
        .map(|mut u| {
            if u.model.is_none() {
                u.model = serde_json::from_slice::<serde_json::Value>(e.request)
                    .ok()
                    .and_then(|r| text(&r["model"]));
            }
            u
        })
        .ok_or(Outcome::NoUsage);
    Parsed {
        format: name,
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(path: &str, body: &[u8], truncated: bool) -> Result<Usage, Outcome> {
        parse(&Exchange {
            path,
            content_type: "application/json",
            response_truncated: truncated,
            request: br#"{"model":"from-request"}"#,
            response: body,
        })
        .result
    }

    #[test]
    fn no_usage_gives_no_call() {
        let count = include_bytes!("../../tests/fixtures/parse/anthropic.count_tokens.json");
        let error = include_bytes!("../../tests/fixtures/parse/anthropic.error.json");
        let full = include_bytes!("../../tests/fixtures/parse/anthropic.split.json");
        assert_eq!(
            run("/v1/messages/count_tokens", count, false),
            Err(Outcome::NoUsage)
        );
        assert_eq!(run("/v1/messages", error, false), Err(Outcome::NoUsage));
        assert_eq!(
            run("/v1/messages", &full[..40], true),
            Err(Outcome::Truncated)
        );
    }

    #[test]
    fn unknown_path_and_request_model_fallback() {
        assert_eq!(run("/v1/models", b"{}", false), Err(Outcome::UnknownFormat));
        let u = run(
            "/v1/chat/completions",
            br#"{"usage":{"prompt_tokens":3}}"#,
            false,
        )
        .unwrap();
        assert_eq!(u.model.as_deref(), Some("from-request"));
    }
}
