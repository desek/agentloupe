//! @agents-index The header set of the OAuth path of the summary client, as Claude Code sends it on the raw Messages API.

/// The Messages API version that both credential kinds send.
pub const VERSION: &str = "2023-06-01";

/// The beta flags that mark a request as a Claude Code OAuth call; without them a token gets a 401.
pub const OAUTH_BETA: &str = "oauth-2025-04-20,claude-code-20250219";

/// The Claude Code version that the user agent names; the server reads only the product name.
pub const CLI_VERSION: &str = "2.1.304";

/// The headers of a request with a subscription token: Bearer, never `x-api-key`.
pub fn oauth(token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("authorization", format!("Bearer {token}")),
        ("anthropic-version", VERSION.to_string()),
        ("anthropic-beta", OAUTH_BETA.to_string()),
        ("x-app", "cli".to_string()),
        (
            "user-agent",
            format!("claude-cli/{CLI_VERSION} (external, cli)"),
        ),
    ]
}
