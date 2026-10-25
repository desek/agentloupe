//! @agents-index Integration test: runs the shell tests of deps.freshness.sh against a fake curl, gh, claude, and cargo on recorded registry answers, in two parallel groups, and asserts that every test passes.

use std::path::Path;
use std::process::Command;

/// Runs the named shell tests and asserts that each of them passed. The shell tests put their own fakes
/// first on PATH and give the script a fixture directory, so no test reaches a registry or GitHub, starts a
/// real claude process, or runs the real verdict. The tokens of the shell of the developer are removed, so a
/// test sees only its own test values. A failure prints the whole output of the shell tests.
fn shell_tests(names: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("/bin/bash")
        .arg(root.join("tests/scripts/deps.freshness.test.sh"))
        .args(names)
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("RELEASE_TOKEN")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("GH_REPO")
        .env_remove("GITHUB_STEP_SUMMARY")
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

/// The scan and the update runs: the classes, the latest rule, yanked and pre-release versions, the two
/// tables, the action pins, the polite rate, a registry error, the JSON output, the dry run, the order
/// and the cap, a green commit, a red verdict, the smoke check, and the guards of the script. Two groups, not
/// four: the shell tests start many short processes, and fewer parallel groups keep the timing tests of the
/// same test stage within their thresholds on a busy machine.
#[test]
fn deps_freshness_scan_and_update() {
    shell_tests(&[
        "scan_classes",
        "latest_is_highest_release",
        "yanked_latest_skipped",
        "yanked_pinned_is_stale",
        "crate_in_two_tables_counted_once",
        "action_pin_resolved",
        "action_pin_mismatch",
        "user_agent_and_rate",
        "registry_error_exit_6",
        "json_output",
        "dry_run_starts_no_claude",
        "update_order_and_cap",
        "update_green_commits",
        "update_red_records_stage",
        "smoke_failure_breaks",
        "tests_removed_fails",
        "dependency_removed_fails",
        "invalid_result_fails",
    ]);
}

/// The coupled raise, the stale action, the hold time, the redaction, the interface, the workflow, and the
/// zero-dep assessment: the cryptography rule, the issue form, a large estimate, a run that writes, and the
/// life of an assessment issue.
#[test]
fn deps_freshness_items_and_assessment() {
    shell_tests(&[
        "also_raised_no_second_run",
        "stale_action_no_claude",
        "held_breaking_item",
        "redaction_and_no_token",
        "interface_help_and_usage",
        "thresholds_and_limits",
        "workflow_is_thin_caller",
        "crypto_is_keep_without_claude",
        "assessment_issue_form",
        "large_replace_becomes_watch",
        "assessment_tree_changed_fails",
        "assessment_issue_life",
    ]);
}
