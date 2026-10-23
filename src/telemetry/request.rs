//! @agents-index The pure builds of the start request: the URL https://<host>/<commit> and the user agent agentloupe/<version> (<target>).

use super::host;

/// The URL of the start request: `https`, or `http` for a loopback host, then the host and the commit
/// as the whole path. No query and no fragment: the URL is the schema.
pub fn url(host_value: &str, commit: &str) -> String {
    let scheme = if host::is_loopback(host_value) {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{host_value}/{commit}")
}

/// The `user-agent` value: the lowercase product name, the version, and the target triple.
pub fn user_agent(version: &str, target: &str) -> String {
    format!("agentloupe/{version} ({target})")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn telemetry_request_shape() {
        let u = url("agentloupe-telemetry.example.workers.dev", COMMIT);
        assert_eq!(
            u,
            format!("https://agentloupe-telemetry.example.workers.dev/{COMMIT}")
        );
        assert_eq!(
            url("127.0.0.1:8899", COMMIT),
            format!("http://127.0.0.1:8899/{COMMIT}")
        );
        assert_eq!(
            url("localhost", COMMIT),
            format!("http://localhost/{COMMIT}")
        );
        for u in [u, url("127.0.0.1:1", COMMIT)] {
            let path = u.rsplit_once('/').unwrap().1;
            assert_eq!(path.len(), 40);
            assert!(
                path.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
            assert!(!u.contains('?') && !u.contains('#'));
        }
        assert_eq!(
            user_agent("0.2.0", "aarch64-apple-darwin"),
            "agentloupe/0.2.0 (aarch64-apple-darwin)"
        );
        assert_eq!(
            user_agent(env!("CARGO_PKG_VERSION"), "x86_64-unknown-linux-gnu"),
            format!(
                "agentloupe/{} (x86_64-unknown-linux-gnu)",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}
