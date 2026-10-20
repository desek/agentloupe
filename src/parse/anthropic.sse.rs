//! @agents-index Reads the usage of an Anthropic Messages stream: input counts from message_start, output from the last message_delta.

use super::anthropic_json::counts;
use super::sse::data_values;
use super::usage::{Usage, int, text};

/// The call of a stream, or None when no message_start carries a usage object.
pub fn parse(body: &[u8]) -> Option<Usage> {
    let mut out: Option<Usage> = None;
    for v in data_values(body) {
        match v["type"].as_str() {
            Some("message_start") if v["message"]["usage"].is_object() => {
                let mut u = Usage {
                    model: text(&v["message"]["model"]),
                    ..Usage::default()
                };
                counts(&v["message"]["usage"], &mut u);
                out = Some(u);
            }
            Some("message_delta") => {
                if let (Some(u), Some(n)) = (out.as_mut(), v["usage"].get("output_tokens")) {
                    u.output = int(n);
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn takes_input_from_start_and_output_from_last_delta() {
        let u = super::parse(include_bytes!(
            "../../tests/fixtures/parse/anthropic.stream.sse"
        ))
        .unwrap();
        assert_eq!(
            (
                u.input,
                u.cache_read,
                u.cache_write_5m,
                u.cache_write_1h,
                u.output
            ),
            (30, 100, 6, 2, 9)
        );
        assert_eq!(u.model.as_deref(), Some("claude-haiku-5-5"));
    }
}
