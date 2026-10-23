//! @agents-index The host rule of LOUPE_TELEMETRY_HOST: a host name with an optional port; std only, so build.rs and the tests share it.
//
// build.rs includes this file with #[path], so it must use the standard library only and no item of
// the crate.

/// What a host value holds that the rule refuses. The text names the part, never the value, so a build
/// error cannot print the value.
pub fn check(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("is empty");
    }
    if value.contains("://") {
        return Err("holds a scheme (for example https://)");
    }
    if value.contains('@') {
        return Err("holds a user name or a password (an @)");
    }
    if value.contains('/') {
        return Err("holds a path (a /)");
    }
    if value.contains('?') {
        return Err("holds a query (a ?)");
    }
    if value.contains('#') {
        return Err("holds a fragment (a #)");
    }
    let (name, port) = match value.rsplit_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (value, None),
    };
    if let Some(p) = port {
        let ok = !p.is_empty()
            && p.len() <= 5
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n));
        if !ok {
            return Err("holds a port that is not a number from 1 to 65535, or a second colon");
        }
    }
    if name.is_empty() || name.len() > 253 {
        return Err("has no host name, or a host name longer than 253 characters");
    }
    for label in name.split('.') {
        if label.is_empty() {
            return Err("has an empty label (a leading, trailing, or double dot)");
        }
        if label.len() > 63 {
            return Err("has a label longer than 63 characters");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("holds a character other than a letter, a digit, a hyphen, or a dot");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("has a label that starts or ends with a hyphen");
        }
    }
    Ok(())
}

/// The host name of a valid value, without the port: the only part of the target that a log line shows.
pub fn name(value: &str) -> &str {
    value.rsplit_once(':').map_or(value, |(n, _)| n)
}

/// True for the two loopback names that get `http`, so a test can use a local fake endpoint.
pub fn is_loopback(value: &str) -> bool {
    matches!(name(value), "127.0.0.1" | "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_host_validation() {
        for ok in [
            "agentloupe-telemetry.example.workers.dev",
            "example.workers.dev:8443",
            "127.0.0.1",
            "127.0.0.1:8899",
            "localhost",
            "localhost:1",
        ] {
            assert_eq!(check(ok), Ok(()), "{ok}");
        }
        for (bad, part) in [
            ("https://example.dev", "scheme"),
            ("http://127.0.0.1:8899", "scheme"),
            ("example.dev/x", "path"),
            ("example.dev?a=b", "query"),
            ("example.dev#f", "fragment"),
            ("user@example.dev", "user name"),
            ("user:pw@example.dev", "password"),
            ("example..dev", "empty label"),
            (".example.dev", "empty label"),
            ("example.dev.", "empty label"),
            ("example.dev:", "port"),
            ("example.dev:0", "port"),
            ("example.dev:65536", "port"),
            ("example dev", "character"),
            ("", "empty"),
        ] {
            let err = check(bad).expect_err(bad);
            assert!(err.contains(part), "{bad}: {err}");
            if bad.len() > 3 {
                assert!(!err.contains(bad), "the reason must not echo the value");
            }
        }
        assert_eq!(name("example.dev:8443"), "example.dev");
        assert_eq!(name("localhost"), "localhost");
        assert!(is_loopback("127.0.0.1:9") && is_loopback("localhost"));
        assert!(!is_loopback("example.dev") && !is_loopback("127.0.0.2"));
    }
}
