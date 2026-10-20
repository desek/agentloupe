//! @agents-index The rate limit rule of the GitHub client: the wait of a 403 or 429 with retry-after or no remaining limit, at most 120 seconds.

use reqwest::header::HeaderMap;

/// The longest wait before one retry, in seconds.
pub const MAX_WAIT_SECS: u64 = 120;
/// The retries after the first request; after them the fetch fails with `rate_limited`.
pub const MAX_RETRIES: u32 = 3;

fn header_u64(h: &HeaderMap, name: &str) -> Option<u64> {
    h.get(name)?.to_str().ok()?.trim().parse().ok()
}

/// The seconds to wait before a retry, or None when the response is not a rate limit.
/// `now_secs` is the Unix time, used for `x-ratelimit-reset`.
pub fn wait_secs(status: u16, h: &HeaderMap, now_secs: u64) -> Option<u64> {
    if !matches!(status, 403 | 429) {
        return None;
    }
    if let Some(s) = header_u64(h, "retry-after") {
        return Some(s.min(MAX_WAIT_SECS));
    }
    if header_u64(h, "x-ratelimit-remaining") == Some(0) {
        let reset = header_u64(h, "x-ratelimit-reset").unwrap_or(now_secs + 60);
        return Some(reset.saturating_sub(now_secs).clamp(1, MAX_WAIT_SECS));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_only_for_rate_limits() {
        let mut h = HeaderMap::new();
        assert_eq!(wait_secs(429, &h, 0), None);
        h.insert("retry-after", "500".parse().unwrap());
        assert_eq!(wait_secs(429, &h, 0), Some(120));
        assert_eq!(wait_secs(500, &h, 0), None);
        let mut r = HeaderMap::new();
        r.insert("x-ratelimit-remaining", "0".parse().unwrap());
        r.insert("x-ratelimit-reset", "110".parse().unwrap());
        assert_eq!(wait_secs(403, &r, 100), Some(10));
    }
}
