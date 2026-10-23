//! @agents-index Integration test: runs the shell tests of verdict.reuse.sh against a fake gh on recorded run records and asserts that every test passes.

use std::path::Path;
use std::process::Command;

/// The shell tests put their own fake gh first on PATH, so no test reaches GitHub; every call is a GET.
#[test]
fn verdict_reuse_script_shell_tests_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/verdict.reuse.test.sh"))
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_REPO")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{stderr}");
    assert_eq!(stdout.matches("status: passed").count(), 21, "{stdout}");
}
