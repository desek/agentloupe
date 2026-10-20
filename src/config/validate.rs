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
            | "LOUPE_RAW_RETENTION_HOURS"
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
        ("LOUPE_PUBLISH_AT", Some(v)) if chrono::NaiveTime::parse_from_str(v, "%H:%M").is_err() => {
            Err(bad(n, "is not a time in HH:MM", "a time such as 01:00"))
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
            idle_secs: num("LOUPE_IDLE_SECS"),
            parse_interval: std::time::Duration::from_secs(num("LOUPE_PARSE_INTERVAL_SECS")),
        })
    }
}
