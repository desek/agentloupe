//! @agents-index Integration test: runs the shell tests of pi.session.sh with a temp HOME and asserts that every test passes.

use std::path::Path;
use std::process::Command;

/// The shell tests own their temp HOME per test; this wrapper only makes them part of the test stage.
#[test]
fn pi_session_script_shell_tests_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let home = tempfile::tempdir().unwrap();
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/pi.session.test.sh"))
        .env("HOME", home.path())
        .env_remove("PI_CC_AUTH_EXT")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(stdout.matches("status: passed").count(), 4, "{stdout}");
}
