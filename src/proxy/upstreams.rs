//! @agents-index Loads and validates the allowlist file of upstreams, each a table with base_url and litellm_provider, before the port opens.

use super::route::valid_name;
use std::collections::BTreeMap;
use std::path::Path;

/// One allowlisted upstream.
#[derive(Debug, Clone, PartialEq)]
pub struct Upstream {
    /// Without a trailing slash.
    pub base_url: String,
    /// The LiteLLM provider whose prices apply to calls through this upstream.
    pub litellm_provider: String,
}

/// Upstream name to its entry.
pub type Upstreams = BTreeMap<String, Upstream>;

const SHAPE: &str = "a table with exactly base_url and litellm_provider, for example [anthropic] base_url = \"https://api.anthropic.com\" litellm_provider = \"anthropic\"";

fn fail(file: &Path, entry: &str, problem: &str, fix: &str) -> String {
    format!(
        "error: the upstreams file {} is not valid: entry {entry}: {problem}. Expected shape: {SHAPE}. Fix: {fix}, or set LOUPE_UPSTREAMS_FILE to a valid file. Reference: docs/specification/proxy.md, section Allowlist. Verify: run loupe config show to see the path of LOUPE_UPSTREAMS_FILE and its source, then start loupe serve again.",
        file.display()
    )
}

/// Checks one base URL: https, or http only for a loopback host that the tests use.
fn check_url(url: &str) -> Result<String, &'static str> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "key base_url: the base URL is not a URL")?;
    let host = parsed.host_str().unwrap_or("");
    match parsed.scheme() {
        "https" => {}
        "http" if host == "127.0.0.1" || host == "localhost" => {}
        _ => return Err("key base_url: the base URL is not https"),
    }
    Ok(url.trim_end_matches('/').to_string())
}

/// Checks one entry value; the error text is the problem and the fix.
fn entry(value: &toml::Value) -> Result<Upstream, (String, &'static str)> {
    let Some(t) = value.as_table() else {
        let problem = if value.is_str() {
            "the entry is a string, the old shape; the entry must be a table"
        } else {
            "the entry is not a table"
        };
        return Err((
            problem.into(),
            "write the entry as a table with base_url and litellm_provider",
        ));
    };
    if let Some(k) = t
        .keys()
        .find(|k| *k != "base_url" && *k != "litellm_provider")
    {
        return Err((
            format!("key {k} is not known"),
            "remove the key; only base_url and litellm_provider are allowed",
        ));
    }
    let Some(url) = t.get("base_url") else {
        return Err((
            "key base_url is missing".into(),
            "add base_url with the https base URL",
        ));
    };
    let url = url.as_str().ok_or_else(|| {
        (
            "key base_url is not a string".to_string(),
            "write the base URL in quotes",
        )
    })?;
    let Some(provider) = t.get("litellm_provider") else {
        return Err((
            "key litellm_provider is missing".into(),
            "add litellm_provider, for example \"anthropic\" or \"openai\"",
        ));
    };
    let provider = provider.as_str().filter(|p| !p.is_empty()).ok_or_else(|| {
        (
            "key litellm_provider is not a non-empty string".to_string(),
            "set litellm_provider to a LiteLLM provider name in quotes",
        )
    })?;
    let base_url = check_url(url).map_err(|p| (p.to_string(), "use an https base URL"))?;
    Ok(Upstream {
        base_url,
        litellm_provider: provider.to_string(),
    })
}

/// Parses the file text; the error is actionable and names the file, the entry, and the key.
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
        let u = entry(&value).map_err(|(p, fix)| fail(file, &name, &p, fix))?;
        out.insert(name, u);
    }
    Ok(out)
}

/// Upstream name to its litellm_provider, the map that the pricing of a call reads.
pub fn providers(u: &Upstreams) -> BTreeMap<String, String> {
    u.iter()
        .map(|(k, v)| (k.clone(), v.litellm_provider.clone()))
        .collect()
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

    fn one(url: &str) -> String {
        format!("[x]\nbase_url = \"{url}\"\nlitellm_provider = \"anthropic\"\n")
    }

    #[test]
    fn rejects_http_except_loopback() {
        let f = Path::new("u.toml");
        assert!(parse(f, &one("http://example.com")).is_err());
        assert!(parse(f, &one("http://127.0.0.1:9")).is_ok());
        assert!(parse(f, &one("https://example.com")).is_ok());
    }

    #[test]
    fn rejects_bad_name() {
        assert!(
            parse(
                Path::new("u.toml"),
                &one("https://a.b").replace("[x]", "[X_]")
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_file_is_actionable() {
        let e = parse(Path::new("bad.toml"), "= = =").unwrap_err();
        for needle in ["bad.toml", "Fix:", "loupe config show"] {
            assert!(e.contains(needle), "missing {needle} in {e}");
        }
    }
}
