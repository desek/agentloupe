//! @agents-index Finds the detected providers from the loaded configuration: a provider is detected when one of its credentials is set.

use super::{Detected, KNOWN, Provider};
use crate::config::loader::{Loaded, Source};
use crate::config::secret::Secret;
use crate::summary::credential::Credential;

/// The value of `name`, or None when it is unset, empty, or only white space.
pub fn set<'a>(cfg: &'a Loaded, name: &str) -> Option<&'a str> {
    cfg.get(name)
        .and_then(|e| e.value.as_ref())
        .map(|v| v.expose())
        .filter(|v| !v.trim().is_empty())
}

/// The active credential of `p`. For `anthropic`, the API key wins over the OAuth token.
fn credential(cfg: &Loaded, p: Provider) -> Option<Credential> {
    let wrap = |v: &str| Secret::new(v.trim().to_string());
    match p {
        Provider::Anthropic => match (
            set(cfg, "ANTHROPIC_API_KEY"),
            set(cfg, "CLAUDE_CODE_OAUTH_TOKEN"),
        ) {
            (Some(k), _) => Some(Credential::ApiKey(wrap(k))),
            (None, Some(t)) => Some(Credential::OauthToken(wrap(t))),
            (None, None) => None,
        },
        Provider::Openai => set(cfg, "OPENAI_API_KEY").map(|k| Credential::OpenaiKey(wrap(k))),
    }
}

/// Every known provider with its credential and base URL, in the order of `KNOWN`.
pub fn detect(cfg: &Loaded) -> Vec<Detected> {
    KNOWN
        .into_iter()
        .map(|p| {
            let e = cfg.get(p.base_var());
            Detected {
                provider: p,
                credential: credential(cfg, p),
                base_url: e
                    .and_then(|e| e.value.as_ref())
                    .map(|v| v.expose().trim_end_matches('/').to_string())
                    .unwrap_or_default(),
                source: e.map_or(Source::Default, |e| e.source),
            }
        })
        .collect()
}

/// The providers that have a credential.
pub fn names(all: &[Detected]) -> Vec<Provider> {
    all.iter()
        .filter(|d| d.credential.is_some())
        .map(|d| d.provider)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::resolve;
    use std::collections::BTreeMap;

    fn load(pairs: &[(&str, &str)]) -> Loaded {
        let env: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        resolve(&[], &env, &BTreeMap::new())
    }

    #[test]
    fn detects_providers_from_set_variables() {
        assert!(names(&detect(&load(&[]))).is_empty());
        let a = detect(&load(&[("CLAUDE_CODE_OAUTH_TOKEN", "t")]));
        assert_eq!(names(&a), vec![Provider::Anthropic]);
        assert_eq!(a[0].base_url, "https://api.anthropic.com");
        assert_eq!(a[1].base_url, "https://api.openai.com/v1");
        let both = detect(&load(&[
            ("ANTHROPIC_API_KEY", "k"),
            ("OPENAI_API_KEY", "o"),
            ("OPENAI_BASE_URL", "http://127.0.0.1:9/v1/"),
        ]));
        assert_eq!(names(&both), vec![Provider::Anthropic, Provider::Openai]);
        assert_eq!(both[1].base_url, "http://127.0.0.1:9/v1");
        assert_eq!(both[1].source, Source::Env);
        let blank = detect(&load(&[("OPENAI_API_KEY", "  ")]));
        assert!(names(&blank).is_empty());
    }
}
