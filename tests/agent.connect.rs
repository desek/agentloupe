//! @agents-index Integration test: runs the shell tests of agent.connect.sh with a temp HOME and asserts that every test passes.

use std::path::Path;
use std::process::Command;

#[test]
fn connect_script_shell_tests_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let home = tempfile::tempdir().unwrap();
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/agent.connect.test.sh"))
        .env("HOME", home.path())
        .env_remove("XDG_STATE_HOME")
        .env_remove("LOUPE_PROXY_TOKEN")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(stdout.matches("status: passed").count(), 8, "{stdout}");
}
