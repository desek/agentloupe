//! @agents-index Validates every resolved configuration value and builds the typed Config that serve reads.

use super::Config;
use super::error::ConfigError;
use super::key::parse_key;
use super::loader::{Entry, Loaded};
use super::vars::{GITHUB_TOKEN_PREFIX, SUBSCRIPTION_PREFIX};

fn bad(name: &str, problem: &str, expected: &str) -> ConfigError {
    ConfigError::new(
        name,
        problem,
        vec![
            format!("set {name} to {expected} in .env or in the environment"),
            format!("unset {name} to use the default"),
        ],
    )
}

fn text(e: &Entry) -> Option<&str> {
    e.value.as_ref().map(|v| v.expose())
}

/// The rule of the summary and GitHub base URLs: https, or http only for a loopback host that the tests use.
fn https_or_loopback(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "localhost")))
    })
}

/// Checks one entry; secrets never appear in the problem text.
fn check(e: &Entry) -> Result<(), ConfigError> {
    let n = e.name.as_str();
    let v = text(e);
    match (n, v) {
        ("LOUPE_DATA_KEY" | "LOUPE_ACCOUNT_KEY", None) => Err(ConfigError::key(n, "is missing")),
        ("LOUPE_DATA_KEY" | "LOUPE_ACCOUNT_KEY", Some(v)) => parse_key(v)
            .map(|_| ())
            .map_err(|why| ConfigError::key(n, format!("is not valid: {why}"))),
        ("LOUPE_LISTEN", Some(v)) if v.parse::<std::net::SocketAddr>().is_err() => Err(bad(
            n,
            "is not a host:port address",
            "an address such as 127.0.0.1:8787",
        )),
        (
            "LOUPE_IDLE_SECS"
            | "LOUPE_PARSE_INTERVAL_SECS"
            | "LOUPE_SUMMARY_INTERVAL_SECS"
            | "LOUPE_SUMMARY_MAX_ATTEMPTS"
            | "LOUPE_PUBLISH_INTERVAL_SECS"
            | "LOUPE_CAPTURE_MAX_BYTES"
            | "LOUPE_CAPTURE_QUEUE"
            | "LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS"
            | "LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS",
            Some(v),
        ) if v.parse::<u64>().map_or(true, |x| x == 0) => Err(bad(
            n,
            "is not a positive whole number",
            "a positive whole number",
        )),
        ("LOUPE_RAW_RETENTION_HOURS", Some(v)) if v.parse::<u64>().is_err() => Err(bad(
            n,
            "is not a whole number",
            "a whole number of hours, 0 or more",
        )),
        ("LOUPE_SUMMARY_MIN_CONVERSATION_CHARS", Some(v)) if v.parse::<u64>().is_err() => Err(bad(
            n,
            "is not a whole number of 0 or more",
            "a whole number of characters, 0 or more, such as 200",
        )),
        ("LOUPE_SUMMARY_BASE_URL", Some(v)) if !https_or_loopback(v) => Err(bad(
            n,
            "is not an https URL or an http URL of 127.0.0.1 or localhost",
            "an https URL such as https://api.anthropic.com",
        )),
        ("LOUPE_GITHUB_API_URL", Some(v)) if !https_or_loopback(v) => Err(bad(
            n,
            "is not an https URL or an http URL of 127.0.0.1 or localhost",
            "an https URL such as https://api.github.com",
        )),
        ("LOUPE_TZ", Some(v)) if v.parse::<chrono_tz::Tz>().is_err() => Err(bad(
            n,
            "is not a time zone name",
            "an IANA time zone name such as Europe/Stockholm",
        )),
        ("LOUPE_BOOTSTRAP_TOKEN", Some(v)) => crate::token::format::parse(v).map_err(|why| {
            ConfigError::new(
                n,
                format!("is not a valid proxy token: it {why}"),
                vec![
                    format!("run `loupe token generate` and put the value in .env as {n}=<value>"),
                    format!("unset {n} to revoke the bootstrap token at the next start"),
                ],
            )
        }),
        // An empty value turns the private page off; a short value would be easy to guess.
        ("LOUPE_ADMIN_TOKEN", Some(v)) if !v.is_empty() && v.chars().count() < 32 => {
            Err(ConfigError::new(
                n,
                "has fewer than 32 characters, so the private page would be easy to open",
                vec![
                    format!("run `loupe token generate` and put the value in .env as {n}=<value>"),
                    format!("unset {n} to turn the private page off"),
                ],
            ))
        }
        ("LOUPE_PUBLISH_AT", Some(v)) if super::publish_at(v).is_none() => {
            Err(bad(n, "is not a time in HH:MM", "a time such as 01:00"))
        }
        ("LOUPE_SITE_TITLE", Some(v)) if v.trim().is_empty() || v.trim().chars().count() > 80 => {
            Err(bad(
                n,
                "is empty after trim or longer than 80 characters",
                "a title of 1 to 80 characters such as AI agent activity",
            ))
        }
        ("LOUPE_LIVE_ENABLED", Some(v)) if v != "true" && v != "false" => {
            Err(bad(n, "is not true or false", "true or false"))
        }
        ("LOUPE_LIVE_ALLOWED_ORIGIN", Some(v)) if !crate::live::origin::is_origin(v) => Err(bad(
            n,
            "is not one origin of the form scheme://host or scheme://host:port",
            "an origin such as https://loupe.example.com",
        )),
        ("LOUPE_LIVE_MAX_RATE" | "LOUPE_LIVE_MAX_CONNECTIONS" | "LOUPE_LIVE_BUFFER", Some(v))
            if !crate::live::config::in_range(n, v) =>
        {
            let (lo, hi) = crate::live::config::range(n);
            Err(bad(
                n,
                &format!("is not a whole number from {lo} to {hi}"),
                &format!("a whole number from {lo} to {hi}"),
            ))
        }
        ("LOUPE_DAY_REVIEW", Some(v)) if v != "auto" && v != "manual" => {
            Err(bad(n, "is not auto or manual", "auto or manual"))
        }
        (
            "LOUPE_TZ"
            | "LOUPE_SUMMARY_MODEL"
            | "LOUPE_DATA_DIR"
            | "LOUPE_ENV"
            | "LOUPE_UPSTREAMS_FILE",
            Some(""),
        ) => Err(bad(n, "is empty", "a value that is not empty")),
        _ if n.starts_with(SUBSCRIPTION_PREFIX) => {
            crate::subscription::config::parse(n, v.unwrap_or("")).map(|_| ())
        }
        _ if n.starts_with(GITHUB_TOKEN_PREFIX)
            && (n.len() == GITHUB_TOKEN_PREFIX.len() || v == Some("")) =>
        {
            Err(bad(
                n,
                "needs a label after the prefix and a token value",
                "a GitHub token, under a name LOUPE_GITHUB_TOKEN_<LABEL>",
            ))
        }
        _ => Ok(()),
    }
}

impl Loaded {
    /// Validates every value and returns the first actionable error, or the typed configuration.
    pub fn validate(&self) -> Result<Config, ConfigError> {
        for e in &self.entries {
            check(e)?;
        }
        let get = |n: &str| self.get(n).and_then(text).unwrap_or_default().to_string();
        let num = |n: &str| get(n).parse::<u64>().expect("checked above");
        let key = |n: &str| parse_key(&get(n)).expect("checked above");
        let opt = |n: &str| self.get(n).and_then(text);
        let model = get("LOUPE_SUMMARY_MODEL");
        let credential = crate::summary::credential::select(
            opt("LOUPE_SUMMARY_API_KEY"),
            opt("LOUPE_SUMMARY_OAUTH_TOKEN"),
            &model,
        )?;
        Ok(Config {
            listen: get("LOUPE_LISTEN").parse().expect("checked above"),
            data_dir: get("LOUPE_DATA_DIR").into(),
            data_key: key("LOUPE_DATA_KEY"),
            account_key: key("LOUPE_ACCOUNT_KEY"),
            upstreams_file: get("LOUPE_UPSTREAMS_FILE").into(),
            capture_max_bytes: num("LOUPE_CAPTURE_MAX_BYTES") as usize,
            capture_queue: num("LOUPE_CAPTURE_QUEUE") as usize,
            connect_timeout: std::time::Duration::from_secs(num(
                "LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS",
            )),
            headers_timeout: std::time::Duration::from_secs(num(
                "LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS",
            )),
            bootstrap_token: self
                .get("LOUPE_BOOTSTRAP_TOKEN")
                .and_then(text)
                .map(|v| super::secret::Secret::new(v.to_string())),
            admin_token: opt("LOUPE_ADMIN_TOKEN")
                .filter(|v| !v.is_empty())
                .map(|v| super::secret::Secret::new(v.to_string())),
            idle_secs: num("LOUPE_IDLE_SECS"),
            site_title: get("LOUPE_SITE_TITLE").trim().to_string(),
            parse_interval: std::time::Duration::from_secs(num("LOUPE_PARSE_INTERVAL_SECS")),
            live: crate::live::config::LiveConfig {
                enabled: get("LOUPE_LIVE_ENABLED") == "true",
                allowed_origin: opt("LOUPE_LIVE_ALLOWED_ORIGIN").map(str::to_string),
                max_rate: num("LOUPE_LIVE_MAX_RATE") as u32,
                max_connections: num("LOUPE_LIVE_MAX_CONNECTIONS") as usize,
                buffer: num("LOUPE_LIVE_BUFFER") as usize,
            },
            days: super::DayConfig {
                publish_at: super::publish_at(&get("LOUPE_PUBLISH_AT")).expect("checked above"),
                publish_interval: std::time::Duration::from_secs(num(
                    "LOUPE_PUBLISH_INTERVAL_SECS",
                )),
                review: crate::day::state::Review::parse(&get("LOUPE_DAY_REVIEW")),
                subscriptions: crate::subscription::config::parse_all(
                    self.entries
                        .iter()
                        .filter_map(|e| text(e).map(|v| (e.name.as_str(), v))),
                )?,
            },
            github: super::GithubConfig {
                api_url: get("LOUPE_GITHUB_API_URL"),
                tokens: self
                    .entries
                    .iter()
                    .filter(|e| e.name.starts_with(GITHUB_TOKEN_PREFIX))
                    .filter_map(|e| {
                        e.value
                            .clone()
                            .map(|v| (e.name[GITHUB_TOKEN_PREFIX.len()..].to_uppercase(), v))
                    })
                    .collect(),
            },
            summary: super::SummaryConfig {
                credential,
                model,
                base_url: get("LOUPE_SUMMARY_BASE_URL"),
                interval: std::time::Duration::from_secs(num("LOUPE_SUMMARY_INTERVAL_SECS")),
                max_attempts: num("LOUPE_SUMMARY_MAX_ATTEMPTS").min(u32::MAX as u64) as u32,
                raw_retention_hours: num("LOUPE_RAW_RETENTION_HOURS"),
                min_conversation_chars: num("LOUPE_SUMMARY_MIN_CONVERSATION_CHARS") as usize,
                public_projects: crate::summary::scrub::public_projects(&get(
                    "LOUPE_PUBLIC_PROJECTS",
                )),
                tz: get("LOUPE_TZ").parse().expect("checked above"),
            },
        })
    }
}
