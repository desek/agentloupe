//! @agents-index The price file and the price commit that the build embeds in the binary; nothing is read from disk at run time.

/// The vendored LiteLLM price file, embedded at compile time.
pub const PRICE_FILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/litellm/model_prices_and_context_window.json"
));

/// The commit of the vendored price file, from the `Commit` row of vendor/litellm/SOURCE.md (see build.rs).
pub const PRICE_COMMIT: &str = env!("LOUPE_PRICE_COMMIT");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_commit_is_40_hex() {
        assert_eq!(PRICE_COMMIT.len(), 40);
        assert!(
            PRICE_COMMIT
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
    }
}
