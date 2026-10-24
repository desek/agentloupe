<!-- @agents-index Recorded GitHub API and claude -p answers for the shell tests of deps.autofix.sh; written by hand from the documented shapes, with no credential. -->
# Fixtures of the dependency autofix

The shell tests of `.agents/scripts/deps.autofix.sh` (`tests/scripts/deps.autofix.test.sh`) serve these files through a fake `gh` and a fake `claude` on `PATH`. Each file follows the shape of the documented answer; the numbers, commit SHAs, advisory identifiers, and logins are made up. No file holds a credential. The values in `item.log.txt` that look like tokens are short test values of each credential pattern, for the redaction test.

| File | Shape of |
|---|---|
| `alerts.json` | `GET /repos/{owner}/{repo}/dependabot/alerts?state=open`: a cargo alert with a patched version, an npm alert of the prototype, and a cargo alert with no patched version |
| `alerts.empty.json` | the same call with no open alert |
| `alerts.disabled.json` | the 403 answer of the same call when the alerts are off |
| `pulls.json` | `GET /repos/{owner}/{repo}/pulls?state=open`: a cargo bump, an action pin bump, and a grouped cargo bump, each by Dependabot |
| `pulls.release.json` | the same call with the release pull request of release-please |
| `files.cargo.json`, `files.actions_sha.json`, `files.actions_tag.json` | `GET /repos/{owner}/{repo}/pulls/{n}/files`: a lock file bump, a pin that keeps a SHA, and a pin that becomes a tag |
| `checks.success.json`, `checks.failure.json`, `checks.pending.json` | `GET /repos/{owner}/{repo}/commits/{sha}/check-runs?check_name=verdict` |
| `compare.current.json`, `compare.behind.json` | `GET /repos/{owner}/{repo}/compare/main...{sha}` |
| `workflow.verdict.json`, `workflow.verdict.disabled.json` | `GET /repos/{owner}/{repo}/actions/workflows/verdict.yml` |
| `runs.release_please.json` | `GET /repos/{owner}/{repo}/actions/workflows/release-please.yml/runs?head_sha={sha}` |
| `claude.result.json` | the `claude -p --output-format json` result of a fix, with the structured result at the end of the text |
| `claude.result.invalid.json` | the same with no structured result |
| `claude.result.auth.json` | the same after an authentication error |
| `item.log.txt` | an item log with a test value of each credential pattern |
