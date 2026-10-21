//! @agents-index Integration test: the test stage stops with exit 3 and an actionable error when cargo-nextest is absent or below the pin.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

/// Runs pipeline.test.sh with a PATH whose `cargo` is a stand-in that logs its arguments and runs no test.
/// `nextest` is the --version output of a stand-in cargo-nextest, or None for no cargo-nextest on PATH.
fn run_stage(nextest: Option<&str>) -> (Output, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("cargo.log");
    let cargo = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\nif [ \"$1\" = nextest ]; then shift; command -v cargo-nextest >/dev/null || exit 101; exec cargo-nextest nextest \"$@\"; fi\nexit 0\n",
        log.display()
    );
    let mut tools = vec![("cargo", cargo)];
    if let Some(v) = nextest {
        tools.push(("cargo-nextest", format!("#!/bin/sh\necho '{v}'\n")));
    }
    for (name, body) in tools {
        let p = bin.path().join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = Command::new("/bin/bash")
        .arg(root.join(".agents/scripts/pipeline.test.sh"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))
        .output()
        .unwrap();
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    (out, calls)
}

#[test]
fn test_stage_stops_without_nextest() {
    let (out, calls) = run_stage(None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("Fix"), "{stderr}");
    assert!(stderr.contains("cargo install cargo-nextest"), "{stderr}");
    assert!(stderr.contains(".tool-versions"), "{stderr}");
    assert!(!calls.lines().any(|l| l.starts_with("test")), "{calls}");
}

#[test]
fn test_stage_stops_below_pinned_nextest() {
    let (out, calls) = run_stage(Some("cargo-nextest 0.9.1 (stand-in)"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("0.9.1"), "{stderr}");
    assert!(!calls.lines().any(|l| l.contains("run")), "{calls}");
}
