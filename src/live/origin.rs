//! @agents-index The origin rule of the live route: no origin, an origin whose host and port equal Host, or the allowed origin.

/// The host and port of an origin or Host value in lowercase, with a default port made explicit.
/// The scheme only selects the default port; the comparison ignores it.
fn host_port(value: &str, default_port: u16) -> Option<String> {
    let rest = value.split_once("://").map_or(value, |(_, r)| r);
    let rest = rest.trim_end_matches('/').to_ascii_lowercase();
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    // An IPv6 literal keeps its brackets; the port is after the closing bracket.
    let has_port = match rest.rfind(']') {
        Some(i) => rest[i..].contains(':'),
        None => rest.contains(':'),
    };
    Some(if has_port {
        rest
    } else {
        format!("{rest}:{default_port}")
    })
}

fn default_port(origin: &str) -> u16 {
    if origin.to_ascii_lowercase().starts_with("https://") {
        443
    } else {
        80
    }
}

/// True if `v` is one origin: a scheme, `://`, and a host with an optional port, no path.
pub fn is_origin(v: &str) -> bool {
    v.split_once("://")
        .is_some_and(|(s, r)| !s.is_empty() && !r.is_empty() && !r.contains('/'))
}

/// The accept rule: no origin, a host and port equal to `host`, or equal to `allowed`.
pub fn accepted(origin: Option<&str>, host: Option<&str>, allowed: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    if allowed.is_some_and(|a| {
        a.trim_end_matches('/')
            .eq_ignore_ascii_case(origin.trim_end_matches('/'))
    }) {
        return true;
    }
    let port = default_port(origin);
    match (
        host_port(origin, port),
        host.and_then(|h| host_port(h, port)),
    ) {
        (Some(o), Some(h)) => o == h,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::accepted;

    #[test]
    fn rules() {
        assert!(accepted(None, Some("h:1"), None));
        assert!(accepted(Some("https://h:8787"), Some("h:8787"), None));
        assert!(accepted(Some("http://h:8787"), Some("h:8787"), None));
        assert!(accepted(Some("https://h"), Some("h:443"), None));
        assert!(accepted(Some("http://h"), Some("h"), None));
        assert!(!accepted(
            Some("https://evil.example"),
            Some("h:8787"),
            None
        ));
        assert!(accepted(
            Some("https://front.example"),
            Some("h:8787"),
            Some("https://front.example")
        ));
        assert!(!accepted(Some("null"), Some("h:8787"), None));
    }
}
