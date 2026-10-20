//! @agents-index Converts the stored UTC time text of exchanges, calls, and sessions to and from Unix milliseconds.

/// The stored form, as the capture writes `started_at`.
pub fn text(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Unix milliseconds of a stored time; 0 when the text is not a valid time.
pub fn ms(text: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(text)
        .map(|t| t.timestamp_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        assert_eq!(
            super::ms(&super::text(1_700_000_000_123)),
            1_700_000_000_123
        );
    }
}
