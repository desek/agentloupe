//! @agents-index The rule of an outbound base URL: https, or http only for a loopback host that the tests use.

/// True for an https URL, or an http URL of `127.0.0.1` or `localhost`; the same rule as an upstream.
pub fn allowed(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "localhost")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_or_loopback_only() {
        assert!(allowed("https://api.openai.com/v1"));
        assert!(allowed("http://127.0.0.1:9/x"));
        assert!(allowed("http://localhost:9"));
        assert!(!allowed("http://api.anthropic.com"));
        assert!(!allowed("not a url"));
    }
}
