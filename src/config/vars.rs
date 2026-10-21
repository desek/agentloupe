//! @agents-index The table of known LOUPE_ variables with their defaults and secrecy, plus the dynamic name prefixes.

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
    var("LOUPE_SUMMARY_API_KEY", None, true),
    var("LOUPE_SUMMARY_OAUTH_TOKEN", None, true),
    var("LOUPE_SUMMARY_MODEL", Some("claude-haiku-5-5"), false),
    var(
        "LOUPE_SUMMARY_BASE_URL",
        Some("https://api.anthropic.com"),
        false,
    ),
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
