//! @agents-index Parses a proxy path into the typed route: agent label, upstream name, rest of the path, query.

use crate::serve::router::RESERVED;

/// The parsed proxy route; the forwarder and the capture read only this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub agent: String,
    pub upstream: String,
    /// The path after the upstream segment, with its leading slash, byte for byte; may be empty.
    pub rest: String,
    /// The raw query without the `?`; empty if absent.
    pub query: String,
}

/// Why a path is not a proxy route.
#[derive(Debug, PartialEq, Eq)]
pub struct RouteError(pub String);

/// The name rule `^[a-z0-9][a-z0-9-]{0,31}$`, and not a reserved segment.
pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && !RESERVED.contains(&name)
}

/// The one parse function; a later change can add an optional leading segment here only.
pub fn parse(path: &str, query: Option<&str>) -> Result<Route, RouteError> {
    let mut parts = path.strip_prefix('/').unwrap_or(path).splitn(3, '/');
    let agent = parts.next().unwrap_or("");
    let upstream = parts.next().unwrap_or("");
    for (what, name) in [("agent label", agent), ("upstream name", upstream)] {
        if !valid_name(name) {
            return Err(RouteError(format!(
                "the {what} {name:?} must match ^[a-z0-9][a-z0-9-]{{0,31}}$ and must not be reserved; the path form is /<agent>/<upstream>/<path>"
            )));
        }
    }
    Ok(Route {
        agent: agent.into(),
        upstream: upstream.into(),
        rest: parts.next().map(|r| format!("/{r}")).unwrap_or_default(),
        query: query.unwrap_or("").into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_upstream_and_rest() {
        let r = parse("/claude-code/anthropic/v1/messages", Some("beta=true")).unwrap();
        assert_eq!(r.agent, "claude-code");
        assert_eq!(r.upstream, "anthropic");
        assert_eq!(r.rest, "/v1/messages");
        assert_eq!(r.query, "beta=true");
    }

    #[test]
    fn rejects_reserved_and_invalid_names() {
        assert!(parse("/v1/anthropic/x", None).is_err());
        assert!(parse("/Bad/anthropic/x", None).is_err());
        assert!(parse("/a/", None).is_err());
    }
}
