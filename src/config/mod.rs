//! @agents-index Configuration of loupe: resolution by precedence, validation, secret handling, and the show output.

pub mod error;
pub mod key;
pub mod loader;
pub mod secret;
pub mod show;
mod validate;
mod vars;

/// The validated values that the running binary reads. Values for later product areas are
/// validated by the loader but not kept, so they cannot change behaviour yet.
pub struct Config {
    pub listen: std::net::SocketAddr,
    pub data_dir: std::path::PathBuf,
    /// Seals the captured bodies and the spill files.
    pub data_key: [u8; key::KEY_LEN],
    /// Keys the HMAC of a credential and of an account ID header.
    pub account_key: [u8; key::KEY_LEN],
    /// The allowlist of upstreams; a relative path resolves against the working directory.
    pub upstreams_file: std::path::PathBuf,
    /// The capture limit: the maximum bytes of one body that the capture keeps.
    pub capture_max_bytes: usize,
    /// The count of exchange records that the queue to the capture writer holds.
    pub capture_queue: usize,
    pub connect_timeout: std::time::Duration,
    /// From the end of the request send to the response headers.
    pub headers_timeout: std::time::Duration,
    /// The raw bootstrap token, already checked against the token format.
    pub bootstrap_token: Option<secret::Secret>,
    /// The admin token of the private page, at least 32 characters; None (unset or empty) turns the page off.
    pub admin_token: Option<secret::Secret>,
    /// Seconds with no call after which the parser closes a session segment.
    pub idle_secs: u64,
    /// The time between two passes of the parser worker.
    pub parse_interval: std::time::Duration,
    /// The live route and the live hub.
    pub live: crate::live::config::LiveConfig,
    /// The trimmed site title of the dashboard, 1 to 80 characters.
    pub site_title: String,
    pub summary: SummaryConfig,
    pub days: DayConfig,
    pub github: GithubConfig,
}

/// The values of the GitHub worker; no token means the worker does not start.
pub struct GithubConfig {
    /// The base URL of the GitHub API: https, or http for a loopback host in tests.
    pub api_url: String,
    /// Each label in uppercase with its token, sorted by label.
    pub tokens: Vec<(String, secret::Secret)>,
}

/// The values of the day records and the publisher; the time zone is `summary.tz`.
pub struct DayConfig {
    /// The local time on D plus one at which the day D is due.
    pub publish_at: chrono::NaiveTime,
    pub publish_interval: std::time::Duration,
    pub review: crate::day::state::Review,
    pub subscriptions: Vec<crate::subscription::config::Subscription>,
}

/// Parses LOUPE_PUBLISH_AT: exactly HH:MM, hour 00 to 23, minute 00 to 59.
pub fn publish_at(v: &str) -> Option<chrono::NaiveTime> {
    let b = v.as_bytes();
    let shape = b.len() == 5 && b[2] == b':' && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit());
    shape
        .then(|| chrono::NaiveTime::parse_from_str(v, "%H:%M").ok())
        .flatten()
}

/// The values of the summary worker, the day line, and the raw-body retention.
pub struct SummaryConfig {
    /// The active credential; None means every closed segment gets the status disabled.
    pub credential: Option<crate::summary::credential::Credential>,
    pub model: String,
    /// The target of the summary client; never a proxy route of loupe serve.
    pub base_url: String,
    pub interval: std::time::Duration,
    pub max_attempts: u32,
    /// Hours after the later of the parse and the summary before the bodies go; 0 is the next pass.
    pub raw_retention_hours: u64,
    /// Below this count of conversation characters, the transcript also holds the clipped tool parts.
    pub min_conversation_chars: usize,
    pub public_projects: Vec<String>,
    /// The time zone that cuts the days.
    pub tz: chrono_tz::Tz,
}
