//! @agents-index Integration test: with no variable set, each timer that a test drives through a clock or a short variable keeps its value.

use agent_loupe::config::vars::VARS;
use std::time::Duration;

fn default_of(name: &str) -> &'static str {
    VARS.iter()
        .find(|v| v.name == name)
        .and_then(|v| v.default)
        .unwrap_or_else(|| panic!("{name} has no default"))
}

#[test]
fn timer_defaults_unchanged() {
    use agent_loupe::{live::client, provider::ping, serve::shutdown, token::gate};
    assert_eq!(gate::TTL, Duration::from_secs(1));
    assert_eq!(gate::TOUCH_EVERY, Duration::from_secs(60));
    assert_eq!(client::HEARTBEAT, Duration::from_secs(15));
    assert_eq!(client::WRITE_TIMEOUT, Duration::from_secs(5));
    assert_eq!(ping::TIMEOUT, Duration::from_secs(5));
    assert_eq!(shutdown::DRAIN, Duration::from_secs(10));
    for (name, value) in [
        ("LOUPE_IDLE_SECS", "3600"),
        ("LOUPE_PARSE_INTERVAL_SECS", "5"),
        ("LOUPE_PUBLISH_INTERVAL_SECS", "60"),
        ("LOUPE_SUMMARY_INTERVAL_SECS", "30"),
        ("LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS", "120"),
        ("LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS", "10"),
        ("LOUPE_RAW_RETENTION_HOURS", "168"),
    ] {
        assert_eq!(default_of(name), value, "{name}");
    }
}
