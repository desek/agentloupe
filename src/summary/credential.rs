//! @agents-index Selects the active summary credential (API key first, then OAuth token) and checks the Haiku-only model rule.

use crate::config::error::ConfigError;
use crate::config::secret::Secret;

/// The only model that a subscription OAuth token can use on the raw Messages API.
pub const HAIKU: &str = "claude-haiku-5-5";

/// The active credential of the summarizer; the value stays wrapped so it never reaches a log.
#[derive(Debug, Clone)]
pub enum Credential {
    ApiKey(Secret),
    OauthToken(Secret),
}

impl Credential {
    /// The kind as `config show` prints it.
    pub fn kind(&self) -> &'static str {
        match self {
            Credential::ApiKey(_) => "api_key",
            Credential::OauthToken(_) => "oauth_token",
        }
    }

    /// The variable that holds this credential, for an actionable error.
    pub fn variable(&self) -> &'static str {
        match self {
            Credential::ApiKey(_) => "LOUPE_SUMMARY_API_KEY",
            Credential::OauthToken(_) => "LOUPE_SUMMARY_OAUTH_TOKEN",
        }
    }

    /// The request headers of this credential: `x-api-key` for a key, the OAuth set for a token.
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        match self {
            Credential::ApiKey(k) => vec![
                ("x-api-key", k.expose().to_string()),
                ("anthropic-version", super::headers::VERSION.to_string()),
            ],
            Credential::OauthToken(t) => super::headers::oauth(t.expose()),
        }
    }
}

/// The kind name of an optional credential: `none` when no credential is set.
pub fn kind(c: Option<&Credential>) -> &'static str {
    c.map_or("none", Credential::kind)
}

/// Picks the API key when both are set. A model other than Haiku with no API key is a start error,
/// because a subscription token works on the raw Messages API for Haiku only.
pub fn select(
    api_key: Option<&str>,
    oauth_token: Option<&str>,
    model: &str,
) -> Result<Option<Credential>, ConfigError> {
    if model != HAIKU && api_key.is_none() {
        return Err(ConfigError::new(
            "LOUPE_SUMMARY_MODEL",
            format!(
                "is {model}, and a model other than {HAIKU} needs LOUPE_SUMMARY_API_KEY; LOUPE_SUMMARY_OAUTH_TOKEN works with {HAIKU} only"
            ),
            vec![
                format!("remove LOUPE_SUMMARY_MODEL to use {HAIKU}"),
                "set LOUPE_SUMMARY_API_KEY to a Console API key".into(),
            ],
        ));
    }
    let wrap = |v: &str| Secret::new(v.to_string());
    Ok(match (api_key, oauth_token) {
        (Some(k), _) => Some(Credential::ApiKey(wrap(k))),
        (None, Some(t)) => Some(Credential::OauthToken(wrap(t))),
        (None, None) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_haiku_model_needs_api_key() {
        let e = select(None, Some("tok"), "claude-sonnet-5-5").unwrap_err();
        let text = e.to_string();
        assert!(text.contains("LOUPE_SUMMARY_MODEL") && text.contains("LOUPE_SUMMARY_API_KEY"));
        let ok = select(Some("key"), Some("tok"), "claude-sonnet-5-5").unwrap();
        assert_eq!(kind(ok.as_ref()), "api_key");
    }

    #[test]
    fn api_key_wins_and_headers() {
        let both = select(Some("key1"), Some("tok1"), HAIKU).unwrap().unwrap();
        let h = both.headers();
        assert!(h.iter().any(|(n, v)| *n == "x-api-key" && v == "key1"));
        assert!(!h.iter().any(|(n, _)| *n == "authorization"));
        let token = select(None, Some("tok1"), HAIKU).unwrap().unwrap();
        assert_eq!(token.kind(), "oauth_token");
        let h = token.headers();
        assert!(
            h.iter()
                .any(|(n, v)| *n == "authorization" && v == "Bearer tok1")
        );
        assert!(!h.iter().any(|(n, _)| *n == "x-api-key"));
        assert_eq!(kind(select(None, None, HAIKU).unwrap().as_ref()), "none");
    }
}
