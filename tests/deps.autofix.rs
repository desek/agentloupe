//! @agents-index Integration test: runs the shell tests of deps.autofix.sh against a fake gh, claude, cargo, and corepack, in four parallel groups, and asserts that every test passes.

use std::path::Path;
use std::process::Command;

/// Runs the named shell tests and asserts that each of them passed. The shell tests put their own fakes
/// first on PATH and give the script a fixture directory, so no test reaches GitHub, starts a real claude
/// process, or runs the real verdict. The tokens of the shell of the developer are removed, so a test sees
/// only its own test values. A failure prints the whole output of the shell tests.
fn shell_tests(names: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/deps.autofix.test.sh"))
        .args(names)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("RELEASE_TOKEN")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("GH_REPO")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}\n{stderr}");
    for name in names {
        let line = format!("test: {name} status: passed");
        assert!(
            stdout.lines().any(|l| l == line),
            "{name} did not pass:\n{stdout}\n{stderr}"
        );
    }
}

/// Discovery and the dry run: nothing open, the planned actions, one item for an alert with a pull request,
/// the precondition, the credentials, the alerts setting, the interface, and the thin caller workflow.
#[test]
fn deps_autofix_discovery() {
    shell_tests(&[
        "nothing_open_prints_zero_and_exits_0",
        "dry_run_lists_actions_for_alert",
        "alert_with_pr_is_handled_once",
        "absent_verdict_workflow_exits_5",
        "missing_token_exits_3_without_value",
        "alerts_off_names_the_setting",
        "interface_help_usage_and_turn_limit",
        "workflow_is_thin_caller",
    ]);
}

/// The fix of an alert: the full run to the release, the credential rule, and the checks of the script.
#[test]
fn deps_autofix_fix_of_an_alert() {
    shell_tests(&[
        "one_alert_fixed_merged_and_released",
        "credentials_stay_out_of_claude_verdict_and_output",
        "invalid_result_counts_as_failed",
        "path_outside_allowlist_fails",
        "code_change_only_after_failed_version_verdict",
    ]);
}

/// The failure paths: the issue, the redaction, the hold time, the authentication error, and the timeout.
#[test]
fn deps_autofix_failures() {
    shell_tests(&[
        "no_patched_version_starts_no_run",
        "credential_in_diff_fails",
        "redaction_removes_credentials",
        "hold_time_blocks_retry",
        "claude_auth_stops_runs_and_exits_3",
        "claude_timeout_fails_item",
    ]);
}

/// The Dependabot pull requests and the release pull request step.
#[test]
fn deps_autofix_pull_requests_and_release() {
    shell_tests(&[
        "dependabot_pr_merges_and_release_step",
        "private_repo_release_stays_with_human",
        "release_pr_with_deps_fix_merged_later",
        "behind_pr_is_rebased_then_merged",
        "red_dependabot_pr_goes_to_claude",
        "actions_pr_sha_to_tag_refused",
    ]);
}
