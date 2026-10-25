//! @agents-index Integration tests of LOUPE_ROLE: precedence, the bad-value error, keys not required in public, and the log file per role.

use std::process::{Command, Output};

fn loupe(args: &[&str], dir: &std::path::Path, env: &[(&str, &str)]) -> Output {
    std::fs::create_dir_all(dir.join("config")).unwrap();
    let local = dir.join("config/local.toml");
    if !local.exists() {
        std::fs::write(local, "").unwrap();
    }
    let mut c = Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(args)
        .current_dir(dir)
        .env_clear()
        .env("LOUPE_LISTEN", "127.0.0.1:0")
        .env("LOUPE_DATA_DIR", dir.join("data"));
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// The value and the source of the line role in the JSON output of config show.
fn role_line(out: &Output) -> (String, String) {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["name"] == "role")
        .map(|v| {
            (
                v["value"].as_str().unwrap().into(),
                v["source"].as_str().unwrap().into(),
            )
        })
        .expect("no line role")
}

#[test]
fn role_resolves_env_then_file_then_default() {
    let d = tempfile::tempdir().unwrap();
    let show = ["config", "show", "--env", "t"];
    std::fs::create_dir_all(d.path().join("config")).unwrap();
    std::fs::write(d.path().join("config/t.toml"), "").unwrap();
    assert_eq!(
        role_line(&loupe(&show, d.path(), &[])),
        ("full".into(), "default".into())
    );
    std::fs::write(d.path().join("config/t.toml"), "LOUPE_ROLE = \"public\"\n").unwrap();
    let file = loupe(&show, d.path(), &[]);
    assert!(
        file.status.success(),
        "{}",
        String::from_utf8_lossy(&file.stderr)
    );
    assert_eq!(role_line(&file), ("public".into(), "file".into()));
    let env = loupe(&show, d.path(), &[("LOUPE_ROLE", "full")]);
    assert_eq!(role_line(&env), ("full".into(), "env".into()));
}

#[test]
fn bad_role_stops_serve_and_config_show_with_the_fix() {
    let d = tempfile::tempdir().unwrap();
    for args in [&["config", "show"][..], &["serve"][..]] {
        let out = loupe(args, d.path(), &[("LOUPE_ROLE", "mirror")]);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        for want in [
            "LOUPE_ROLE",
            "full",
            "public",
            "fix 1:",
            ".env.example",
            "loupe config show",
        ] {
            assert!(err.contains(want), "{args:?} lacks {want}: {err}");
        }
    }
}

#[test]
fn public_role_needs_no_keys_but_checks_plain_values() {
    let d = tempfile::tempdir().unwrap();
    let out = loupe(&["config", "show"], d.path(), &[("LOUPE_ROLE", "public")]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(r#""name":"LOUPE_DATA_KEY""#) && text.contains(r#""value":"unset""#));
    // No key error in the role public: serve passes the validation.
    let serve = loupe(&["serve"], d.path(), &[("LOUPE_ROLE", "public")]);
    assert!(!String::from_utf8_lossy(&serve.stderr).contains("LOUPE_DATA_KEY"));
    let bad = loupe(
        &["serve"],
        d.path(),
        &[("LOUPE_ROLE", "public"), ("LOUPE_TZ", "Nowhere/x")],
    );
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("LOUPE_TZ"));
    // The role full still needs the keys.
    let full = loupe(&["serve"], d.path(), &[]);
    assert!(String::from_utf8_lossy(&full.stderr).contains("LOUPE_DATA_KEY"));
}

#[test]
fn other_subcommands_ignore_the_role() {
    let d = tempfile::tempdir().unwrap();
    let out = loupe(&["gen-key"], d.path(), &[("LOUPE_ROLE", "mirror")]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let key = "0".repeat(64);
    let agg = loupe(
        &["aggregate", "--dry-run"],
        d.path(),
        &[
            ("LOUPE_ROLE", "mirror"),
            ("LOUPE_DATA_KEY", &key),
            ("LOUPE_ACCOUNT_KEY", &key),
        ],
    );
    assert!(!String::from_utf8_lossy(&agg.stderr).contains("LOUPE_ROLE"));
}
