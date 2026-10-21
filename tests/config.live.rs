//! @agents-index Integration test of the live variables: out-of-range values stop the start with the variable named; config show prints five lines.

#[path = "common/loupe.rs"]
mod loupe;

use std::process::Command;

fn loupe(args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let data = tempfile::tempdir().unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_loupe"));
    c.args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_clear()
        .env("LOUPE_DATA_DIR", data.path())
        .env("LOUPE_DATA_KEY", loupe::KEY)
        .env("LOUPE_ACCOUNT_KEY", loupe::KEY)
        .env("LOUPE_LISTEN", format!("127.0.0.1:{}", loupe::free_port()));
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

#[test]
fn live_config_ranges_and_show() {
    for (k, v) in [
        ("LOUPE_LIVE_ENABLED", "yes"),
        ("LOUPE_LIVE_ALLOWED_ORIGIN", "front.example/x"),
        ("LOUPE_LIVE_MAX_RATE", "0"),
        ("LOUPE_LIVE_MAX_RATE", "51"),
        ("LOUPE_LIVE_MAX_CONNECTIONS", "1025"),
        ("LOUPE_LIVE_BUFFER", "63"),
        ("LOUPE_LIVE_BUFFER", "65537"),
    ] {
        let out = loupe(&["serve"], &[(k, v)]);
        assert!(!out.status.success(), "{k}={v} started");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(k), "{k}: {err}");
    }
    let out = loupe(&["config", "show"], &[("LOUPE_LIVE_MAX_RATE", "20")]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let line = |n: &str| {
        lines
            .iter()
            .find(|v| v["name"] == n)
            .unwrap_or_else(|| panic!("no {n}"))
            .clone()
    };
    assert_eq!(line("live_max_rate")["value"], "20");
    assert_eq!(line("live_max_rate")["source"], "env");
    assert_eq!(line("live_enabled")["value"], "true");
    assert_eq!(line("live_enabled")["source"], "default");
    assert_eq!(line("live_allowed_origin")["value"], "unset");
    assert_eq!(line("live_max_connections")["value"], "32");
    assert_eq!(line("live_buffer")["value"], "1024");
}
