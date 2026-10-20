//! @agents-index Reads the usage of an OpenAI Responses body: the response.completed event, or the top-level usage.

use super::openai_counts::from_object;
use super::sse::data_values;
use super::usage::Usage;

pub fn parse(body: &[u8], stream: bool) -> Option<Usage> {
    if !stream {
        return from_object(&serde_json::from_slice(body).ok()?);
    }
    data_values(body)
        .iter()
        .rev()
        .find(|v| v["type"] == "response.completed")
        .and_then(|v| from_object(&v["response"]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_completed_event_usage() {
        let u = super::parse(
            include_bytes!("../../tests/fixtures/parse/openai.responses.sse"),
            true,
        )
        .unwrap();
        assert_eq!(
            (
                u.input,
                u.cache_read,
                u.cache_write_5m,
                u.cache_write_1h,
                u.output
            ),
            (60, 40, 0, 0, 9)
        );
        assert_eq!(u.model.as_deref(), Some("gpt-5"));
        assert_eq!(u.service_tier.as_deref(), Some("default"));
    }
}
