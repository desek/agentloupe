//! @agents-index The table of known variables (LOUPE_ and the provider names) with their defaults and secrecy, plus the dynamic name prefixes.

/// One fixed variable: its name, its built-in default, and whether it is a secret.
pub struct Var {
    pub name: &'static str,
    pub default: Option<&'static str>,
    pub secret: bool,
}

const fn var(name: &'static str, default: Option<&'static str>, secret: bool) -> Var {
    Var {
        name,
        default,
        secret,
    }
}

/// Every fixed variable, in the order that config show prints them.
pub const VARS: &[Var] = &[
    var("LOUPE_ENV", Some("local"), false),
    var("LOUPE_LISTEN", Some("127.0.0.1:8787"), false),
    var("LOUPE_DATA_DIR", Some("./data"), false),
    var("LOUPE_DATA_KEY", None, true),
    var("LOUPE_ACCOUNT_KEY", None, true),
    var("LOUPE_BOOTSTRAP_TOKEN", None, true),
    var(
        "LOUPE_GITHUB_API_URL",
        Some("https://api.github.com"),
        false,
    ),
    var("CLAUDE_CODE_OAUTH_TOKEN", None, true),
    var("ANTHROPIC_API_KEY", None, true),
    var("OPENAI_API_KEY", None, true),
    var(
        "ANTHROPIC_BASE_URL",
        Some("https://api.anthropic.com"),
        false,
    ),
    var("OPENAI_BASE_URL", Some("https://api.openai.com/v1"), false),
    var("LOUPE_SUMMARY_PROVIDER", None, false),
    var("LOUPE_PROVIDER_PING_SECS", Some("300"), false),
    var("LOUPE_SUMMARY_MODEL", Some("claude-haiku-5-5"), false),
    var("LOUPE_SUMMARY_INTERVAL_SECS", Some("30"), false),
    var("LOUPE_SUMMARY_MAX_ATTEMPTS", Some("5"), false),
    var("LOUPE_SUMMARY_MIN_CONVERSATION_CHARS", Some("200"), false),
    var("LOUPE_TZ", Some("Europe/Stockholm"), false),
    var("LOUPE_PUBLISH_AT", Some("01:00"), false),
    var("LOUPE_PUBLISH_INTERVAL_SECS", Some("60"), false),
    var("LOUPE_IDLE_SECS", Some("3600"), false),
    var("LOUPE_PARSE_INTERVAL_SECS", Some("5"), false),
    var("LOUPE_RAW_RETENTION_HOURS", Some("168"), false),
    var("LOUPE_SITE_TITLE", Some("AI agent activity"), false),
    var("LOUPE_PUBLIC_PROJECTS", Some(""), false),
    var("LOUPE_DAY_REVIEW", Some("auto"), false),
    var("LOUPE_ADMIN_TOKEN", None, true),
    var("LOUPE_UPSTREAMS_FILE", Some("upstreams.toml"), false),
    var("LOUPE_CAPTURE_MAX_BYTES", Some("33554432"), false),
    var("LOUPE_CAPTURE_QUEUE", Some("256"), false),
    var("LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS", Some("10"), false),
    var("LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS", Some("120"), false),
    var("LOUPE_LIVE_ENABLED", Some("true"), false),
    var("LOUPE_LIVE_ALLOWED_ORIGIN", None, false),
    var("LOUPE_LIVE_MAX_RATE", Some("10"), false),
    var("LOUPE_LIVE_MAX_CONNECTIONS", Some("32"), false),
    var("LOUPE_LIVE_BUFFER", Some("1024"), false),
];

/// Prefix of the GitHub token variables; the rest of the name is the label. Secret, env only.
pub const GITHUB_TOKEN_PREFIX: &str = "LOUPE_GITHUB_TOKEN_";
/// Prefix of the subscription price variables; the rest of the name is the subscription.
pub const SUBSCRIPTION_PREFIX: &str = "LOUPE_SUBSCRIPTION_";

/// True for a name that the loader reads from the environment of the process.
pub fn known(name: &str) -> bool {
    name.starts_with("LOUPE_") || VARS.iter().any(|v| v.name == name)
}

#[cfg(test)]
mod tests {
    use super::super::loader::{Loaded, resolve};
    use std::collections::BTreeMap;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    fn load(pairs: &[(&str, &str)]) -> Loaded {
        let mut env: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        env.insert("LOUPE_DATA_KEY".into(), KEY.into());
        env.insert("LOUPE_ACCOUNT_KEY".into(), KEY.into());
        resolve(&[], &env, &BTreeMap::new())
    }

    fn error(pairs: &[(&str, &str)]) -> String {
        match load(pairs).validate() {
            Ok(_) => panic!("{pairs:?} passed"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn industry_names_are_read() {
        let cfg = load(&[
            ("CLAUDE_CODE_OAUTH_TOKEN", "t1"),
            ("ANTHROPIC_API_KEY", "k1"),
            ("OPENAI_API_KEY", "o1"),
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:9/a"),
            ("OPENAI_BASE_URL", "https://example.com/v1"),
        ])
        .validate()
        .unwrap();
        let p = &cfg.providers;
        assert_eq!(p[0].credential.as_ref().unwrap().kind(), "api_key");
        assert_eq!(p[0].base_url, "http://127.0.0.1:9/a");
        assert_eq!(
            p[1].credential.as_ref().unwrap().variable(),
            "OPENAI_API_KEY"
        );
        assert_eq!(p[1].base_url, "https://example.com/v1");
        assert_eq!(cfg.summary.base_url, "http://127.0.0.1:9/a");
        assert_eq!(cfg.provider_ping, std::time::Duration::from_secs(300));
    }

    #[test]
    fn empty_credential_counts_as_unset() {
        let cfg = load(&[("CLAUDE_CODE_OAUTH_TOKEN", " "), ("OPENAI_API_KEY", "")])
            .validate()
            .unwrap();
        assert!(cfg.providers.iter().all(|d| d.credential.is_none()));
        assert!(cfg.summary.credential.is_none());
        let shown = super::super::show::render(&load(&[("ANTHROPIC_API_KEY", "  ")]), false);
        assert!(shown.contains("credential=none"));
    }

    #[test]
    fn base_url_requires_https_or_loopback() {
        for var in ["ANTHROPIC_BASE_URL", "OPENAI_BASE_URL"] {
            let e = error(&[(var, "http://api.example.com")]);
            assert!(e.contains(var) && e.contains("https"), "{e}");
            assert!(load(&[(var, "https://api.example.com")]).validate().is_ok());
            assert!(load(&[(var, "http://localhost:9")]).validate().is_ok());
        }
    }

    #[test]
    fn legacy_name_stops_start_and_names_new_variable() {
        for (old, new) in crate::provider::legacy::RENAMED {
            let e = crate::provider::legacy::check(|n| (n == old).then(|| "Wv4Qx8Lp".into()))
                .unwrap_err()
                .to_string();
            assert!(e.contains(old) && e.contains(new), "{e}");
            assert!(!e.contains("Wv4Qx8Lp"));
        }
        for kept in [
            "LOUPE_SUMMARY_MODEL",
            "LOUPE_SUMMARY_INTERVAL_SECS",
            "LOUPE_SUMMARY_MAX_ATTEMPTS",
        ] {
            assert!(crate::provider::legacy::check(|n| (n == kept).then(|| "7".into())).is_ok());
        }
        assert!(
            load(&[
                ("LOUPE_SUMMARY_INTERVAL_SECS", "7"),
                ("LOUPE_SUMMARY_MAX_ATTEMPTS", "3")
            ])
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn ping_secs_minimum_is_60() {
        for bad in ["59", "0", "abc", "-1", "60.5"] {
            let e = error(&[("LOUPE_PROVIDER_PING_SECS", bad)]);
            assert!(e.contains("LOUPE_PROVIDER_PING_SECS"), "{e}");
        }
        let cfg = load(&[("LOUPE_PROVIDER_PING_SECS", "60")])
            .validate()
            .unwrap();
        assert_eq!(cfg.provider_ping, std::time::Duration::from_secs(60));
    }

    #[test]
    fn openai_summary_without_model_stops_start() {
        let e = error(&[("OPENAI_API_KEY", "o1")]);
        assert!(
            e.contains("LOUPE_SUMMARY_MODEL") && e.contains("openai"),
            "{e}"
        );
        let cfg = load(&[
            ("OPENAI_API_KEY", "o1"),
            ("LOUPE_SUMMARY_MODEL", "gpt-5-mini"),
        ])
        .validate()
        .unwrap();
        assert_eq!(cfg.summary.model, "gpt-5-mini");
        assert_eq!(cfg.summary.base_url, "https://api.openai.com/v1");
        let e = error(&[
            ("ANTHROPIC_API_KEY", "k"),
            ("LOUPE_SUMMARY_PROVIDER", "openai"),
        ]);
        assert!(
            e.contains("LOUPE_SUMMARY_PROVIDER") && e.contains("anthropic"),
            "{e}"
        );
    }
}
