//! @agents-index Parses a proxy path into the typed route: optional token segment, agent label, upstream name, rest, query.

use crate::serve::router::RESERVED;

/// The parsed proxy route; the forwarder and the capture read only this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The leading `lpt_` segment, if the path had one; never logged or stored.
    pub token: Option<Presented>,
    pub agent: String,
    pub upstream: String,
    /// The path after the upstream segment, with its leading slash, byte for byte; may be empty.
    pub rest: String,
    /// The raw query without the `?`; empty if absent.
    pub query: String,
}

/// A presented raw token; its Debug form hides the value so a stray log cannot leak it.
#[derive(Clone, PartialEq, Eq)]
pub struct Presented(pub String);

impl std::fmt::Debug for Presented {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Presented(hidden)")
    }
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

/// The one parse function. A first segment that starts with `lpt_` is the presented token; the
/// agent label pattern has no `_`, so it is never an agent label.
pub fn parse(path: &str, query: Option<&str>) -> Result<Route, RouteError> {
    let mut path = path.strip_prefix('/').unwrap_or(path);
    let mut token = None;
    if path.starts_with(crate::token::format::PREFIX) {
        let (t, rest) = path.split_once('/').unwrap_or((path, ""));
        token = Some(Presented(t.to_string()));
        path = rest;
    }
    let mut parts = path.splitn(3, '/');
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
        token,
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
        assert!(r.token.is_none());
    }

    #[test]
    fn takes_a_leading_token_segment() {
        let r = parse("/lpt_abc/claude-code/anthropic/v1/messages", None).unwrap();
        assert_eq!(r.token, Some(Presented("lpt_abc".into())));
        assert_eq!(r.agent, "claude-code");
        assert_eq!(r.rest, "/v1/messages");
        assert!(!format!("{r:?}").contains("lpt_"));
        assert!(parse("/lpt_abc", None).is_err());
    }

    #[test]
    fn rejects_reserved_and_invalid_names() {
        assert!(parse("/v1/anthropic/x", None).is_err());
        assert!(parse("/Bad/anthropic/x", None).is_err());
        assert!(parse("/a/", None).is_err());
    }
}
