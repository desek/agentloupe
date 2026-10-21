//! @agents-index The header sets of the summary client: the OAuth set with its billing block, and the OpenAI Bearer set.

/// The Messages API version that both credential kinds send.
pub const VERSION: &str = "2023-06-01";

/// The beta flags that mark a request as a Claude Code OAuth call; without them a token gets a 401.
pub const OAUTH_BETA: &str = "oauth-2025-04-20,claude-code-20250219";

/// The first system block of an OAuth request. Without it a request with a system prompt that is not
/// Claude Code's gets a 429 with no retry-after; the server checks only that both keys are present.
pub const BILLING_BLOCK: &str =
    "x-anthropic-billing-header: cc_version=2.1.304; cc_entrypoint=sdk-cli;";

/// The headers of a request with a subscription token: Bearer, never `x-api-key`.
pub fn oauth(token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("authorization", format!("Bearer {token}")),
        ("anthropic-version", VERSION.to_string()),
        ("anthropic-beta", OAUTH_BETA.to_string()),
        ("x-app", "cli".to_string()),
    ]
}

/// The headers of an OpenAI request: Bearer only.
pub fn openai(key: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {key}"))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_bearer_headers() {
        assert_eq!(
            openai("sk-o1"),
            vec![("authorization", "Bearer sk-o1".to_string())]
        );
    }
}
