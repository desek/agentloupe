//! @agents-index Puts the short price commit into the cost footnote of index.html at start, so the page code holds no literal commit.

use crate::price::embed::PRICE_COMMIT;

/// The marker in web/index.html that the short price commit replaces.
pub const PLACEHOLDER: &str = "{{price_commit}}";

/// The short form of the price commit: its first 7 hex characters, as git prints it.
pub fn short() -> &'static str {
    &PRICE_COMMIT[..7]
}

pub fn apply(html: &str) -> String {
    html.replace(PLACEHOLDER, short())
}
