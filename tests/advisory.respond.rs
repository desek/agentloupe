//! @agents-index Integration test: runs the shell tests of advisory.respond.sh against a fake gh, cargo deny, and docker, and asserts that every test passes.

use std::path::Path;
use std::process::Command;

/// The shell tests put their own fakes first on PATH, so no test reaches GitHub, the registry, or the
/// advisory database, and no test deletes anything.
#[test]
fn advisory_respond_script_shell_tests_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/advisory.respond.test.sh"))
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("ADVISORY_TOKEN")
        .env_remove("GH_REPO")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(stdout.matches("status: passed").count(), 11, "{stdout}");
}
