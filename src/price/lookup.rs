//! @agents-index Finds the price entry of a model under one provider, also under the key <provider>/<model>, with a fallback that removes a trailing -YYYYMMDD.

use super::index::{Entry, Index};

/// The model name without one trailing hyphen and eight digits, or None if it has no such suffix.
fn strip_date(model: &str) -> Option<&str> {
    let (head, tail) = model.rsplit_once('-')?;
    (tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit())).then_some(head)
}

/// The entry of the model name or of the key `<provider>/<model>`, because the price file keys some providers (github_copilot, groq) with the provider prefix.
fn exact<'a>(index: &'a Index, model: &str, provider: &str) -> Option<&'a Entry> {
    index
        .get(model, provider)
        .or_else(|| index.get(&format!("{provider}/{model}"), provider))
}

/// Exact match on (model, provider), then the same with the date suffix removed.
/// Never matches another provider or a similar name.
pub fn find<'a>(index: &'a Index, model: &str, provider: &str) -> Option<&'a Entry> {
    exact(index, model, provider)
        .or_else(|| strip_date(model).and_then(|m| exact(index, m, provider)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, provider: &str) -> Entry {
        Entry {
            key: key.into(),
            provider: provider.into(),
            ..Default::default()
        }
    }

    #[test]
    fn exact_model_and_provider() {
        let e = find(Index::embedded(), "claude-haiku-4-5", "anthropic").unwrap();
        assert_eq!(e.key, "claude-haiku-4-5");
    }

    #[test]
    fn date_suffix_fallback() {
        let i = Index::from_entries(vec![entry("m", "anthropic")]);
        assert_eq!(find(&i, "m-20250101", "anthropic").unwrap().key, "m");
        assert!(find(&i, "m-2025010", "anthropic").is_none());
    }

    #[test]
    fn provider_prefixed_key() {
        let e = find(Index::embedded(), "claude-haiku-5.5", "github_copilot").unwrap();
        assert_eq!(e.key, "github_copilot/claude-haiku-5.5");
        let i = Index::from_entries(vec![entry("groq/m", "groq")]);
        assert!(find(&i, "m", "openai").is_none());
    }

    #[test]
    fn other_provider_does_not_match() {
        let i = Index::from_entries(vec![entry("only-bedrock", "bedrock")]);
        assert!(find(&i, "only-bedrock", "anthropic").is_none());
    }
}
