//! @agents-index The provider credentials of the summarizer, the summary provider selection, and the Haiku-only rule of the OAuth token.

use crate::config::error::ConfigError;
use crate::config::secret::Secret;
use crate::provider::{Detected, KNOWN, Provider};

/// The only model that a subscription OAuth token can use on the raw Messages API.
pub const HAIKU: &str = "claude-haiku-5-5";

/// A provider credential; the value stays wrapped so it never reaches a log.
#[derive(Debug, Clone)]
pub enum Credential {
    /// `ANTHROPIC_API_KEY`, a Console API key.
    ApiKey(Secret),
    /// `CLAUDE_CODE_OAUTH_TOKEN`, a Claude subscription OAuth token.
    OauthToken(Secret),
    /// `OPENAI_API_KEY`.
    OpenaiKey(Secret),
}

impl Credential {
    /// The kind as `config show` prints it.
    pub fn kind(&self) -> &'static str {
        match self {
            Credential::ApiKey(_) | Credential::OpenaiKey(_) => "api_key",
            Credential::OauthToken(_) => "oauth_token",
        }
    }

    /// The variable that holds this credential, for an actionable error.
    pub fn variable(&self) -> &'static str {
        match self {
            Credential::ApiKey(_) => "ANTHROPIC_API_KEY",
            Credential::OauthToken(_) => "CLAUDE_CODE_OAUTH_TOKEN",
            Credential::OpenaiKey(_) => "OPENAI_API_KEY",
        }
    }

    /// The request headers of a summary request: `x-api-key` for an Anthropic key, the OAuth set
    /// for a token, Bearer for an OpenAI key.
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        match self {
            Credential::ApiKey(k) => vec![
                ("x-api-key", k.expose().to_string()),
                ("anthropic-version", super::headers::VERSION.to_string()),
            ],
            Credential::OauthToken(t) => super::headers::oauth(t.expose()),
            Credential::OpenaiKey(k) => super::headers::openai(k.expose()),
        }
    }
}

/// The kind name of an optional credential: `none` when no credential is set.
pub fn kind(c: Option<&Credential>) -> &'static str {
    c.map_or("none", Credential::kind)
}

/// The summary provider: the requested one, else `anthropic`, else `openai`, among the detected
/// providers. A request for an unknown or undetected provider is a start error.
pub fn select_provider(
    requested: Option<&str>,
    detected: &[Provider],
) -> Result<Option<Provider>, ConfigError> {
    let Some(name) = requested.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(KNOWN.into_iter().find(|p| detected.contains(p)));
    };
    match Provider::parse(name) {
        Some(p) if detected.contains(&p) => Ok(Some(p)),
        _ => {
            let list: Vec<&str> = detected.iter().map(|p| p.name()).collect();
            let list = if list.is_empty() {
                "none".to_string()
            } else {
                list.join(", ")
            };
            Err(ConfigError::new(
                "LOUPE_SUMMARY_PROVIDER",
                format!(
                    "is {name}, which is not a detected provider; the detected providers are: {list}"
                ),
                vec![
                    "set LOUPE_SUMMARY_PROVIDER to a detected provider, anthropic or openai".into(),
                    "set the credential of that provider: CLAUDE_CODE_OAUTH_TOKEN, ANTHROPIC_API_KEY, or OPENAI_API_KEY".into(),
                    "unset LOUPE_SUMMARY_PROVIDER to select from the detected providers".into(),
                ],
            ))
        }
    }
}

/// The provider of the summarizer with its checks: the Haiku-only rule of the OAuth token, and
/// a model that `openai` needs. `model_set` is false when the model is the built-in default.
pub fn select(
    all: &[Detected],
    requested: Option<&str>,
    model: &str,
    model_set: bool,
) -> Result<Option<Detected>, ConfigError> {
    let detected = crate::provider::detect::names(all);
    let Some(p) = select_provider(requested, &detected)? else {
        return Ok(None);
    };
    let d = all
        .iter()
        .find(|d| d.provider == p)
        .cloned()
        .expect("a selected provider is detected");
    if matches!(d.credential, Some(Credential::OauthToken(_))) && model != HAIKU {
        return Err(ConfigError::new(
            "LOUPE_SUMMARY_MODEL",
            format!(
                "is {model}, and CLAUDE_CODE_OAUTH_TOKEN works with {HAIKU} only, so a different model needs ANTHROPIC_API_KEY"
            ),
            vec![
                format!("remove LOUPE_SUMMARY_MODEL to use {HAIKU}"),
                "set ANTHROPIC_API_KEY to a Console API key".into(),
            ],
        ));
    }
    if p == Provider::Openai && !model_set {
        return Err(ConfigError::new(
            "LOUPE_SUMMARY_MODEL",
            "is not set, and the summary provider openai has no default model",
            vec![
                "set LOUPE_SUMMARY_MODEL to an OpenAI model name in .env".into(),
                "set LOUPE_SUMMARY_PROVIDER=anthropic with an Anthropic credential".into(),
            ],
        ));
    }
    Ok(Some(d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::loader::resolve;
    use crate::provider::detect::detect;
    use std::collections::BTreeMap;

    fn all(pairs: &[(&str, &str)]) -> Vec<Detected> {
        let env: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        detect(&resolve(&[], &env, &BTreeMap::new()))
    }

    fn chosen(pairs: &[(&str, &str)], model: &str, set: bool) -> Option<Detected> {
        select(&all(pairs), None, model, set).unwrap()
    }

    #[test]
    fn anthropic_api_key_wins_over_oauth_token() {
        let both = &[
            ("ANTHROPIC_API_KEY", "key1"),
            ("CLAUDE_CODE_OAUTH_TOKEN", "tok1"),
        ];
        let c = chosen(both, HAIKU, false).unwrap().credential.unwrap();
        assert_eq!(c.kind(), "api_key");
        let h = c.headers();
        assert!(h.iter().any(|(n, v)| *n == "x-api-key" && v == "key1"));
        assert!(!h.iter().any(|(n, _)| *n == "authorization"));
    }

    #[test]
    fn non_haiku_model_needs_api_key() {
        let tok = &[("CLAUDE_CODE_OAUTH_TOKEN", "tok")];
        let ok = chosen(
            &[("ANTHROPIC_API_KEY", "key"), tok[0]],
            "claude-sonnet-5-5",
            true,
        );
        assert_eq!(kind(ok.unwrap().credential.as_ref()), "api_key");
        let text = select(&all(tok), None, "claude-sonnet-5-5", true)
            .unwrap_err()
            .to_string();
        assert!(text.contains("LOUPE_SUMMARY_MODEL") && text.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn oauth_token_with_non_haiku_model_stops_start() {
        let text = select(
            &all(&[("CLAUDE_CODE_OAUTH_TOKEN", "tok")]),
            None,
            "claude-opus-5-5",
            true,
        )
        .unwrap_err()
        .to_string();
        for n in [
            "CLAUDE_CODE_OAUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "LOUPE_SUMMARY_MODEL",
        ] {
            assert!(text.contains(n), "missing {n}");
        }
        assert!(!text.contains("tok\n"));
    }

    #[test]
    fn api_key_wins_and_headers() {
        let token = chosen(&[("CLAUDE_CODE_OAUTH_TOKEN", "tok1")], HAIKU, false)
            .unwrap()
            .credential
            .unwrap();
        assert_eq!(token.kind(), "oauth_token");
        let h = token.headers();
        assert!(
            h.iter()
                .any(|(n, v)| *n == "authorization" && v == "Bearer tok1")
        );
        assert!(!h.iter().any(|(n, _)| *n == "x-api-key"));
        assert!(chosen(&[], HAIKU, false).is_none());
    }

    #[test]
    fn summary_provider_selection_rule() {
        let a = Provider::Anthropic;
        let o = Provider::Openai;
        assert_eq!(select_provider(None, &[]).unwrap(), None);
        assert_eq!(select_provider(None, &[a, o]).unwrap(), Some(a));
        assert_eq!(select_provider(None, &[o]).unwrap(), Some(o));
        assert_eq!(select_provider(Some("openai"), &[a, o]).unwrap(), Some(o));
        let e = select_provider(Some("openai"), &[a])
            .unwrap_err()
            .to_string();
        assert!(e.contains("LOUPE_SUMMARY_PROVIDER") && e.contains("anthropic"));
        assert!(select_provider(Some("mistral"), &[a, o]).is_err());
        let open = all(&[("OPENAI_API_KEY", "o")]);
        assert!(select(&open, None, HAIKU, false).is_err());
        let d = select(&open, None, "gpt-5-mini", true).unwrap().unwrap();
        assert_eq!(d.provider, o);
    }
}
