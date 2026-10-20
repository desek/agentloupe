//! @agents-index The backoff of a retryable summary failure: 60 seconds doubled per attempt, at most 1 hour, or a longer retry-after.

/// The first delay, in seconds.
const BASE_SECS: u64 = 60;
/// The longest computed delay, in seconds.
const CAP_SECS: u64 = 3600;

/// The delay before the next attempt, after `attempts` attempts (1 or more).
/// A `retry-after` header wins only when it is longer.
pub fn delay_secs(attempts: u32, retry_after: Option<u64>) -> u64 {
    let exp = attempts.saturating_sub(1).min(16);
    let computed = BASE_SECS.saturating_mul(1u64 << exp).min(CAP_SECS);
    retry_after.map_or(computed, |r| r.max(computed))
}

#[cfg(test)]
mod tests {
    use super::delay_secs;

    #[test]
    fn backoff_and_cap() {
        let got: Vec<u64> = (1..=8).map(|a| delay_secs(a, None)).collect();
        assert_eq!(got, [60, 120, 240, 480, 960, 1920, 3600, 3600]);
        assert_eq!(delay_secs(1, Some(500)), 500);
        assert_eq!(delay_secs(3, Some(10)), 240);
    }
}
