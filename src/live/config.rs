//! @agents-index The typed live configuration and the ranges of LOUPE_LIVE_MAX_RATE, LOUPE_LIVE_MAX_CONNECTIONS, and LOUPE_LIVE_BUFFER.

/// The validated live values that serve reads.
#[derive(Clone, Debug)]
pub struct LiveConfig {
    /// False: `GET /live` returns 404 and the hub does not start.
    pub enabled: bool,
    /// One more accepted origin, for a front proxy that changes the Host header.
    pub allowed_origin: Option<String>,
    /// Frames per second to one client.
    pub max_rate: u32,
    pub max_connections: usize,
    /// The capacity of the hub in events.
    pub buffer: usize,
}

/// The inclusive range of a numeric live variable.
pub fn range(name: &str) -> (u64, u64) {
    match name {
        "LOUPE_LIVE_MAX_RATE" => (1, 50),
        "LOUPE_LIVE_MAX_CONNECTIONS" => (1, 1024),
        _ => (64, 65536),
    }
}

/// True if `value` is a whole number inside the range of `name`.
pub fn in_range(name: &str, value: &str) -> bool {
    let (lo, hi) = range(name);
    value.parse::<u64>().is_ok_and(|v| (lo..=hi).contains(&v))
}
