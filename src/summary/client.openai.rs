//! @agents-index The OpenAI shape of a summary request: the chat completions body and the text of its response.

use serde_json::{Value, json};

/// The path under `OPENAI_BASE_URL`, which already ends in `/v1`.
pub const PATH: &str = "/chat/completions";

/// The rule text as the system message, the same limit as `max_completion_tokens`, no temperature.
pub fn body(model: &str, system: &str, user: &str, max_tokens: u32) -> Value {
    json!({
        "model": model,
        "max_completion_tokens": max_tokens,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    })
}

/// The text of the first choice; None for another shape.
pub fn text_of(v: &Value) -> Option<String> {
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_first_choice() {
        let v = json!({"choices":[{"message":{"role":"assistant","content":"fixed a bug"}}]});
        assert_eq!(text_of(&v).as_deref(), Some("fixed a bug"));
        assert_eq!(text_of(&json!({"error":{}})), None);
    }
}
