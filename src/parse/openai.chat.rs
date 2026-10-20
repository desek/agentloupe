//! @agents-index Reads the usage of an OpenAI Chat Completions body, or of the last stream chunk that has one.

use super::openai_counts::from_object;
use super::sse::data_values;
use super::usage::Usage;

pub fn parse(body: &[u8], stream: bool) -> Option<Usage> {
    if !stream {
        return from_object(&serde_json::from_slice(body).ok()?);
    }
    data_values(body).iter().rev().find_map(from_object)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_usage_from_last_chunk() {
        let u = super::parse(
            include_bytes!("../../tests/fixtures/parse/openai.chat.sse"),
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
            (40, 10, 0, 0, 6)
        );
    }
}
