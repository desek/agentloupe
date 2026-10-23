//! @agents-index Integration test: loupe --version prints the Cargo.toml version, the first serve log line names it, and /healthz is unchanged.

#[path = "common/loupe.rs"]
mod loupe;

/// The version that Cargo.toml gives the crate at build time; the test states no version itself.
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn version_flag_prints_cargo_version() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_loupe"))
        .arg("--version")
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("loupe {VERSION}\n")
    );
}

/// The first JSON line of a log text, parsed.
fn first_line(text: &str) -> serde_json::Value {
    let line = text.lines().next().expect("the log has no line");
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not one JSON line ({e}): {line}"))
}

#[test]
fn serve_start_line_names_version() {
    let l = loupe::Loupe::start(1, &[], &[]);
    // The log file and stderr get the same line; both must start with the version.
    for (channel, text) in [("log file", l.log()), ("stderr", l.stderr())] {
        let first = first_line(&text);
        assert_eq!(
            first["fields"]["version"], VERSION,
            "the first {channel} line holds no version field: {first}"
        );
    }
}

#[test]
fn healthz_body_unchanged() {
    let l = loupe::Loupe::start(1, &[], &[]);
    let res = reqwest::blocking::get(l.url("/healthz")).unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().unwrap(), r#"{"status":"ok"}"#);
}
