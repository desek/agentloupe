//! @agents-index Integration tests of loupe config show: one line per provider, summary_provider, and no credential in the output.

#[path = "common/fake_provider.rs"]
mod fake_provider;
#[path = "common/loupe.rs"]
mod loupe;
#[path = "common/private.rs"]
mod private;
#[path = "common/providers.rs"]
mod providers;

use std::collections::BTreeMap;

fn show(env: &[(&str, &str)], human: bool) -> std::process::Output {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(["config", "show"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear();
    if human {
        c.arg("--human");
    }
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn values(out: &std::process::Output) -> BTreeMap<String, String> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .map(|v| {
            let s = |k: &str| v[k].as_str().unwrap().to_string();
            (s("name"), s("value"))
        })
        .collect()
}

#[test]
fn config_show_lists_providers() {
    let out = show(
        &[
            ("ANTHROPIC_API_KEY", providers::ANTHROPIC_KEY),
            ("OPENAI_API_KEY", providers::OPENAI_KEY),
            ("OPENAI_BASE_URL", "http://127.0.0.1:9/v1"),
        ],
        false,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = values(&out);
    assert_eq!(
        v["provider.anthropic"],
        "credential=api_key base_url=https://api.anthropic.com source=default"
    );
    assert_eq!(
        v["provider.openai"],
        "credential=api_key base_url=http://127.0.0.1:9/v1 source=env"
    );
    assert_eq!(v["summary_provider"], "anthropic");
    assert!(!v.contains_key("summary_credential"));
    let none = values(&show(&[], false));
    assert_eq!(none["summary_provider"], "none");
    assert!(none["provider.openai"].starts_with("credential=none "));
}

#[test]
fn config_show_never_prints_a_credential() {
    let env = [
        ("ANTHROPIC_API_KEY", providers::ANTHROPIC_KEY),
        ("CLAUDE_CODE_OAUTH_TOKEN", providers::OAUTH_TOKEN),
        ("OPENAI_API_KEY", providers::OPENAI_KEY),
    ];
    for human in [false, true] {
        let out = show(&env, human);
        let text = String::from_utf8_lossy(&out.stdout);
        let hits = providers::SECRETS
            .iter()
            .filter(|s| text.contains(**s))
            .count();
        println!("config_show_credential_hits: {hits}");
        assert_eq!(hits, 0);
        assert!(providers::clean(&text));
        assert!(text.contains("provider.anthropic"));
    }
}

#[test]
fn config_show_stops_on_a_legacy_name() {
    let out = show(&[("LOUPE_SUMMARY_API_KEY", "legacy-Kp4Wn7")], false);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        err.contains("ANTHROPIC_API_KEY") && !err.contains("legacy-Kp4Wn7"),
        "{err}"
    );
}

#[test]
fn config_show_prints_the_role_and_its_source() {
    let out = show(&[], false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(values(&out)["role"], "full");
    let out = show(&[("LOUPE_ROLE", "public")], false);
    assert_eq!(values(&out)["role"], "public");
}
