//! @agents-index Loads and validates the allowlist file of upstreams before the port opens.

use super::route::valid_name;
use std::collections::BTreeMap;
use std::path::Path;

/// Upstream name to base URL, without a trailing slash.
pub type Upstreams = BTreeMap<String, String>;

fn fail(file: &Path, entry: &str, problem: &str, fix: &str) -> String {
    format!(
        "error: the upstreams file {} is not valid: entry {entry}: {problem}. Fix: {fix}, or set LOUPE_UPSTREAMS_FILE to a valid file. Reference: docs/specification/proxy.md, section Allowlist. Verify: run loupe config show to see the path of LOUPE_UPSTREAMS_FILE and its source, then start loupe serve again.",
        file.display()
    )
}

/// Checks one base URL: https, or http only for a loopback host that the tests use.
fn check_url(url: &str) -> Result<String, &'static str> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "the base URL is not a URL")?;
    let host = parsed.host_str().unwrap_or("");
    match parsed.scheme() {
        "https" => {}
        "http" if host == "127.0.0.1" || host == "localhost" => {}
        _ => return Err("the base URL is not https"),
    }
    Ok(url.trim_end_matches('/').to_string())
}

/// Parses the file text; the error is actionable and names the file and the entry.
pub fn parse(file: &Path, text: &str) -> Result<Upstreams, String> {
    let table: toml::Table = text.parse().map_err(|e| {
        fail(
            file,
            "(file)",
            &format!("not valid TOML: {e}"),
            "correct the TOML syntax",
        )
    })?;
    let mut out = Upstreams::new();
    for (name, value) in table {
        if !valid_name(&name) {
            return Err(fail(
                file,
                &name,
                "the name does not match ^[a-z0-9][a-z0-9-]{0,31}$ or is reserved",
                "rename the entry",
            ));
        }
        let Some(url) = value.as_str() else {
            return Err(fail(
                file,
                &name,
                "the value is not a string",
                "write the base URL in quotes",
            ));
        };
        let url = check_url(url).map_err(|p| fail(file, &name, p, "use an https base URL"))?;
        out.insert(name, url);
    }
    Ok(out)
}

/// Reads and validates the file at `file`.
pub fn load(file: &Path) -> Result<Upstreams, String> {
    let text = std::fs::read_to_string(file).map_err(|e| {
        fail(
            file,
            "(file)",
            &format!("cannot be read: {e}"),
            "create the file, or run loupe from the directory that holds upstreams.toml",
        )
    })?;
    parse(file, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_http_except_loopback() {
        let f = Path::new("u.toml");
        assert!(parse(f, r#"x = "http://example.com""#).is_err());
        assert!(parse(f, r#"x = "http://127.0.0.1:9""#).is_ok());
    }

    #[test]
    fn invalid_file_is_actionable() {
        let e = parse(Path::new("bad.toml"), "= = =").unwrap_err();
        for needle in ["bad.toml", "Fix:", "loupe config show"] {
            assert!(e.contains(needle), "missing {needle} in {e}");
        }
    }
}
