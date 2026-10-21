//! @agents-index Integration test: the upstreams file loads tables with base_url and litellm_provider, and each bad entry gets an actionable error.

use std::process::{Command, Output};

/// Runs `loupe price show` for `upstream` against an upstreams file with `text`; the command loads the file like serve.
fn show(text: &str, upstream: &str) -> Output {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("upstreams.toml");
    std::fs::write(&file, text).unwrap();
    Command::new(env!("CARGO_BIN_EXE_loupe"))
        .args([
            "price",
            "show",
            "--model",
            "claude-haiku-5-5",
            "--upstream",
            upstream,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_UPSTREAMS_FILE", &file)
        .output()
        .unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn assert_actionable(o: &Output, needles: &[&str]) {
    assert!(!o.status.success());
    let e = stderr(o);
    for n in needles
        .iter()
        .chain(&["upstreams.toml", "Fix:", "loupe config show"])
    {
        assert!(e.contains(n), "missing {n} in {e}");
    }
}

#[test]
fn valid_table_entry_loads() {
    let o = show(
        "[x]\nbase_url = \"https://api.anthropic.com\"\nlitellm_provider = \"anthropic\"\n",
        "x",
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("match: claude-haiku-5-5"));
}

#[test]
fn inline_table_entry_loads() {
    let o = show(
        "x = { base_url = \"http://127.0.0.1:9\", litellm_provider = \"anthropic\" }\n",
        "x",
    );
    assert!(o.status.success(), "{}", stderr(&o));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("match: claude-haiku-5-5") && out.contains("provider: anthropic"),
        "{out}"
    );
}

#[test]
fn missing_litellm_provider_is_actionable() {
    let o = show("[x]\nbase_url = \"https://a.example\"\n", "x");
    assert_actionable(&o, &["entry x", "litellm_provider"]);
}

#[test]
fn string_entry_is_actionable() {
    let o = show("x = \"https://a.example\"\n", "x");
    assert_actionable(
        &o,
        &[
            "entry x",
            "string",
            "a table with exactly base_url and litellm_provider",
        ],
    );
}

#[test]
fn unknown_key_is_actionable() {
    let o = show(
        "[x]\nbase_url = \"https://a.example\"\nlitellm_provider = \"anthropic\"\nextra = 1\n",
        "x",
    );
    assert_actionable(&o, &["entry x", "key extra"]);
}

#[test]
fn empty_provider_is_actionable() {
    let o = show(
        "[x]\nbase_url = \"https://a.example\"\nlitellm_provider = \"\"\n",
        "x",
    );
    assert_actionable(&o, &["entry x", "litellm_provider"]);
}

#[test]
fn invalid_toml_is_actionable() {
    assert_actionable(&show("= = =", "x"), &["not valid TOML"]);
}

#[test]
fn bad_name_is_actionable() {
    let o = show(
        "[X_]\nbase_url = \"https://a.example\"\nlitellm_provider = \"a\"\n",
        "x",
    );
    assert_actionable(&o, &["entry X_"]);
}

#[test]
fn non_https_url_is_actionable_except_localhost() {
    let o = show(
        "[x]\nbase_url = \"http://a.example\"\nlitellm_provider = \"a\"\n",
        "x",
    );
    assert_actionable(&o, &["entry x", "base_url", "https"]);
    let o = show(
        "[x]\nbase_url = \"http://localhost:9\"\nlitellm_provider = \"a\"\n",
        "x",
    );
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn chatgpt_uses_openai_prices() {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/upstreams.toml")).unwrap();
    let t: toml::Table = text.parse().unwrap();
    assert_eq!(t["chatgpt"]["litellm_provider"].as_str(), Some("openai"));
    assert_eq!(
        t["github-copilot"]["litellm_provider"].as_str(),
        Some("github_copilot")
    );
    assert_eq!(t["groq"]["litellm_provider"].as_str(), Some("groq"));
    assert_eq!(t["cerebras"]["litellm_provider"].as_str(), Some("cerebras"));
    assert_eq!(t["openai"]["litellm_provider"].as_str(), Some("openai"));
    assert_eq!(
        t["anthropic"]["litellm_provider"].as_str(),
        Some("anthropic")
    );
}
