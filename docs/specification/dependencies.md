<!-- @agents-index Specification of the dependency flow: Dependabot configuration, the daily autofix workflow and its script, the headless Claude Code run, the autofix branch and its merge, the release pull request step, the autofix issue, the hold time, the tokens, the limits, the weekly freshness scan with its classes, the freshness update and the breaking updates issue, the zero-dep assessment, the freshness workflow and its script, and the known gap of the proxy. -->
# Dependencies

This file states how the project keeps its dependencies current with no human step. Dependabot finds the updates and the vulnerable dependencies. A daily workflow calls one script. The script lets Claude Code in headless mode fix each item on one branch, merges each green fix, and merges the release pull request of `release.md` when its check is green. Thus a dependency fix becomes a patch release.

A weekly workflow calls a second script, the freshness script. It measures the age of each direct dependency, lets Claude Code raise each stale dependency to its latest version, and finds out with the verdict of the script if the app breaks. A green update goes through the same pull request flow as a fix of the daily autofix. A red update goes into one issue. For each dependency with no release for a long time, the script writes an assessment of a replacement with own code into one issue.

## Dependabot issue

A Dependabot issue is one of these two items:

1. An open Dependabot security alert of the repository.
2. An open Dependabot version update pull request.

The script handles each Dependabot issue as one item. An item has one of two paths: the path `alert` or the path `pull_request`. When an alert and an open Dependabot pull request name the same package, and the pull request moves the package to a version at or above the first patched version, the item has the path `pull_request` only. Thus one change does not start two fixes.

## Dependabot configuration

The file `.github/dependabot.yml` declares the version updates. Dependabot security alerts are on for the repository. `repo.settings.sh` turns them on (`release.md`, Repository settings). The Dependabot security updates stay off, because the script fixes the alerts.

| Ecosystem | Directory | Interval | Commit type of the pull request title | Effect on the release |
|---|---|---|---|---|
| `cargo` | `/` | weekly | `fix(deps)` | a patch release |
| `github-actions` | `/` | weekly | `ci(deps)` | none, the type is hidden, because a pin of an action does not change the binary or the image |

1. The version updates run each Monday at 03:00 UTC. Thus the daily run of Monday finds the new pull requests.
2. The minor and patch updates of one ecosystem come in one grouped pull request each week. Each major update has its own pull request.
3. Dependabot updates the pin of an action by its full 40-character commit SHA and keeps the version comment.
4. Dependabot does not read `.tool-versions`, the digests of the base images in the `Dockerfile`, or the vendored price file under `vendor/litellm/`. The owner updates these by hand. `prices.vendor.sh` refreshes the price file.
5. The project has no npm code, so the file has no `npm` entry. The dependency flows have the ecosystems `cargo` and `github-actions` only.

## Autofix workflow

The workflow `.github/workflows/deps-autofix.yml` is a thin caller (`operations.md`, Hosted workflows).

1. It starts each day at 04:17 UTC and on a manual start. A manual start can ask for a dry run, and the workflow then passes the dry-run flag to the script, so the run plans and writes nothing. A manual start can also end the hold time of the failed items, and the workflow then passes the retry flag. The time is before the working day of the owner and off the full hour, where GitHub delays scheduled runs.
2. It does not start on a push or on a pull request. Thus its own merges do not start it again.
3. It shares one concurrency group with the freshness workflow (see "Freshness workflow"). At most one run of the two workflows is active. A new run waits and does not cancel the active run. GitHub keeps at most one pending run in a concurrency group: a new pending run cancels the older pending run. When a pending daily run is cancelled, the next daily run does its work.
4. Its one job has a timeout of 180 minutes. This timeout is the one limit in the workflow file.
5. The job checks out the repository with no credential in the git configuration. It installs the Rust toolchain from `rust-toolchain.toml`, and Node, `cargo-nextest`, cargo-deny, and Claude Code from `.tool-versions`, as the verdict workflow does. It caches the cargo registry and the target directory with the same key as the verdict workflow.
6. It calls `.agents/scripts/deps.autofix.sh` in exactly one step. The workflow file holds no tool version, no model name, no branch name, no label, and no limit other than the job timeout.
7. Only the step of the script gets the secret `CLAUDE_CODE_OAUTH_TOKEN`, as an environment variable. No other step gets it.

The job requests these permissions of the `GITHUB_TOKEN`, and no others:

| Permission | Use |
|---|---|
| `contents: write` | the checkout and the reads of the tree and the branches; the push itself uses the release token |
| `pull-requests: write` | the reads of the pull requests; the writes use the release token |
| `issues: write` | the autofix issue and its label |
| `security-events: read` | the reads of the security data |
| `actions: read` | the check that the verdict workflow is active, and the wait for the release-please run |
| `checks: read` | the read of the check `verdict` |

## Tokens

| Token | Used for | Never used for |
|---|---|---|
| The release token, secret `RELEASE_PLEASE_TOKEN` (`release.md`, Release token) | the read of the Dependabot alerts; the push of the autofix branch and of the freshness branch; the autofix pull request and the freshness pull request; the enable of auto-merge; the rebase command comment on a Dependabot pull request; each merge of the two flows | the `claude -p` process; the verdict of the script; the job `assess` of the freshness workflow |
| The `GITHUB_TOKEN` of the workflow | the autofix issue, the breaking updates issue, the zero-dep assessment issues, their comments, and the label `zero-dep`; the reads of the pull requests, the checks, the workflow runs, the tags, and the releases of the pinned actions | a push or a merge, because GitHub starts no workflow from an event that this token makes |
| The subscription token, secret `CLAUDE_CODE_OAUTH_TOKEN` | the `claude -p` process only: the fix runs of the autofix, and the update runs and the assessment runs of the freshness flow | a GitHub call; a registry call; the verdict of the script; the freshness scan, which needs no subscription token |

1. The script reads the alerts with one token, named in one place of the script. Dependabot does not act on a command from the bot of the `GITHUB_TOKEN`, and an event of the `GITHUB_TOKEN` starts no verdict workflow. Thus each write that must start a workflow or a Dependabot action uses the release token.
2. The script gives a GitHub token to git only for its own push. While `claude -p` runs, no file in the tree or in `.git/` holds a GitHub token, and the environment of the `claude -p` process holds no GitHub token. The freshness script follows the same rule.
3. The owner makes the subscription token with `claude setup-token`. The token has a fixed lifetime. Expiry date: not recorded yet. The owner records the date here when the secret is made.
4. The crates.io API and the public GitHub releases need no token for the freshness scan. The scan reads the GitHub data with the `GITHUB_TOKEN` of the job, or with the `gh` login of the person who runs it.

To rotate the subscription token, do these steps:

1. Run `claude setup-token` on the machine of the owner.
2. Replace the value of the repository secret `CLAUDE_CODE_OAUTH_TOKEN` with the new token.
3. Start the workflow by hand and check that the item lines show no reason `claude_auth`.
4. Record the new expiry date in this section.

## Run of the script

The script holds all the logic of the flow. A run does these steps in this order:

1. **Precondition.** When the verdict workflow is absent from the checked-out tree, or is not active on the repository, the script stops with exit code 5 before any API write and before any `claude -p` run. The error names the file of the verdict workflow and the release governance that adds it.
2. **Tools and credentials.** When `gh`, `git`, `claude`, or `cargo` is missing, or when `CLAUDE_CODE_OAUTH_TOKEN` or the GitHub token is empty, the script stops with exit code 3 before any run. The error names the missing item and never its value.
3. **Alerts on.** The script lists the Dependabot alerts with the alert read token. When the error of the list call names disabled alerts, the script stops with exit code 4 and an actionable error that names the setting.
4. **Discovery.** The script lists the open alerts and the open pull requests whose author is Dependabot, through `gh`.
   1. For each alert, it reads the alert number, the advisory identifier (GHSA), the ecosystem, the package, the vulnerable version range, the first patched version, the manifest path, and the severity.
   2. For each Dependabot pull request, it reads the number, the head commit, the title, the changed files, and the state of the check `verdict`.
5. **Nothing open.** When no alert and no Dependabot pull request is open, the script prints one count line with zero alerts and zero pull requests and exits with code 0. It makes no branch, no pull request, and no issue, and starts no `claude -p` run. It still does the release step for an open release pull request whose changelog lists a `fix(deps)` change, and closes an open autofix issue.
6. **No patched version.** An alert with no first patched version starts no `claude -p` run. The script records it in the autofix issue with the reason `no_patched_version`.
7. **Order and cap.** The script handles the items with the highest severity first, then the oldest. An item above the run cap waits for the next run.
8. **Autofix branch.** The script makes the autofix branch from the head of `main`, or reuses it when it exists from an earlier run on the same day.
9. **Items.** The script handles each alert (see "Fix of an alert") and each Dependabot pull request (see "Dependabot pull requests").
10. **Push and merge.** When the autofix branch holds one or more fix commits, the script pushes it and merges it (see "Autofix branch and merge").
11. **Release.** The script does the release step (see "Release pull request step").
12. **Issue.** The script opens, updates, or closes the autofix issue (see "Autofix issue").

Each run ends in one of three states:

1. Nothing open: exit code 0.
2. Each item merged or set to auto-merge: exit code 0.
3. One or more items failed, and the autofix issue holds each one: exit code 1.

A run that ends with exit code 3, 4, or 5 shows as failed in GitHub Actions.

## Fix of an alert

For each alert, the script starts at most one `claude -p` run in one workflow run.

### Limits

| Limit | Value |
|---|---|
| Model | the Sonnet alias of Claude Code, one constant in the script; the script records the resolved model ID of each run from the result |
| Turns of one run | at most 30 |
| Budget of one run | at most 5 US dollars, API-equivalent |
| Process timeout of one run | 40 minutes; then the script ends the process, and the item fails with the reason `timeout` |
| Bash tool timeout in Claude Code | 20 minutes, so that one verdict fits |
| `claude -p` runs of one workflow run | at most 4 |
| Budget of one workflow run | at most 15 US dollars, API-equivalent; when the sum passes the cap, the script starts no more `claude -p` runs |
| Wait for a check `verdict` | at most 30 minutes |
| Wait for the new head commit after the rebase command of Dependabot | at most 5 minutes; then the run continues with the current head |
| Job timeout of the workflow | 180 minutes |

A documented flag of the script lowers the turn limit for a test run. It cannot raise a limit.

The freshness flow keeps the limits of one `claude -p` run of the table above, and adds limits of its own for one workflow run. No flag and no configuration file changes a limit of the freshness flow, and no flag raises one.

| Limit | Update run (job `update`) | Assessment run (job `assess`) |
|---|---|---|
| Model | the Sonnet alias, as above | the Sonnet alias, as above |
| Turns of one run | at most 30 | at most 30 |
| Budget of one run | at most 5 US dollars, API-equivalent | at most 5 US dollars, API-equivalent |
| Process timeout of one run | 40 minutes; then the item fails with the reason `timeout` | 15 minutes, because an assessment only reads; then the item fails with the reason `timeout` |
| Bash tool timeout in Claude Code | 20 minutes | none, because an assessment run has no Bash tool |
| `claude -p` runs of one job | at most 4 | at most 10 |
| Budget of one job | at most 15 US dollars, API-equivalent | at most 20 US dollars, API-equivalent |
| Job timeout of the workflow | 330 minutes | 180 minutes |
| Remaining job time | the script starts no new update run when less than 75 minutes of the job time remain | no rule |

An item that waits because of the run cap or the remaining job time carries the reason `run_cap`. An item that waits because of the budget of the job carries the reason `budget_cap`. Neither is a failure. The next weekly run handles the item.

### Prompt

The prompt comes from the template `.agents/scripts/deps.autofix.prompt.md`. The script fills in the package, the ecosystem, the vulnerable range, the first patched version, the manifest path, and the advisory identifier. For a Dependabot pull request, the script fills in the package and the target version of the pull request.

The prompt tells Claude Code to do these steps:

1. Update the dependency in the manifest and in the lock file.
2. Run `.agents/scripts/pipeline.verdict.sh`.
3. Change code only when the verdict fails because of the update.
4. End with one JSON object, the structured result.

### Tools of Claude Code

Claude Code gets the tools Read, Edit, Write, Grep, and Glob. It gets the Bash tool only for `cargo`, for `git diff`, `git status`, and `git log`, and for `.agents/scripts/pipeline.*.sh`. It gets no web fetch, no web search, no `git commit`, no `git push`, no `gh`, no `curl`, no `env`, and no `printenv`. Claude Code does not commit, does not push, and does not call the GitHub API.

### Structured result

| Field | Value |
|---|---|
| `status` | `fixed` or `failed` |
| `package` | the package name |
| `from_version` | the version before the update |
| `to_version` | the version after the update |
| `files_changed` | the list of the changed paths |
| `verdict` | `passed` or `failed`, as Claude Code saw it |
| `reason` | a short text |

A result that is missing, that cannot be parsed, or that lacks a field counts as `failed` with the reason `invalid_result`.

### Checks of the script

The script, not Claude Code, is the authority for the diff and the verdict. After each `claude -p` run, the script does these checks in this order:

1. **Allowed paths.** The diff can always change `Cargo.toml` and `Cargo.lock`. It can change `src/` and `tests/` only when the verdict failed after the version change. It can never change `.github/`, `.agents/`, `docs/`, `vendor/`, `web/`, `Dockerfile`, `compose.yaml`, `rust-toolchain.toml`, or `.tool-versions`. A diff with another path fails with the reason `path_not_allowed`.
2. **No credential.** A diff that holds a match of a credential pattern (see "Credentials and logs"), or the value of a token of the run, fails with the reason `credential_in_diff`. The script writes the matched text nowhere.
3. **Verdict.** For a result with the status `fixed`, the script runs `pipeline.verdict.sh` itself, with no GitHub token and no `CLAUDE_CODE_OAUTH_TOKEN` in its environment. Only exit code 0 of this run counts as passed. The `verdict` field of the result does not count.

The script builds the crate one time before the first `claude -p` run, so later verdicts reuse the target directory.

### Commit

When each check passes, the script makes one commit on the autofix branch. The script writes the message from the alert data. The summary has the form `<type>(deps): bump <package> to <version> (<GHSA id>)`.

| Fix | Type |
|---|---|
| The fix changes a `cargo` dependency. | `fix` |

### Failure

When a check fails, the script resets the autofix branch to its last good commit, records the item as failed, and continues with the next item. It does not start a second `claude -p` run for the same item in the same workflow run.

When `claude -p` fails with an authentication error, the script starts no more `claude -p` runs, writes the reason `claude_auth` and the command `claude setup-token` into the autofix issue, and exits with code 3. The merge of the green Dependabot pull requests continues, because it needs no Claude Code.

## Dependabot pull requests

For each open Dependabot pull request, the script does these steps:

1. When the pull request is behind `main`, the script posts the rebase command of Dependabot with the release token, and waits for the new head commit for at most 5 minutes (the rebase cap). When the new head does not come before that cap, the script continues with the current head, and the item line records `rebase: dependabot_no_response`: a green check `verdict` of that head merges with squash as in item 2 (a squash merge of a branch behind `main` is valid, and the verdict runs on `main` after the merge); a red check goes to the `claude -p` run of item 3, which works on the autofix branch made from `main`, never on the Dependabot branch.
2. When the check `verdict` of the up-to-date head commit is green, the script merges the pull request with squash (see "Merge rule").
3. When the check `verdict` fails, the script hands the update to one `claude -p` run on the autofix branch, with the rules of "Fix of an alert".
   1. When that fix passes, the script closes the Dependabot pull request with a comment that links the autofix pull request.
   2. When that fix fails, the Dependabot pull request stays open with a comment that links the autofix issue, and the item waits for the hold time. The script does not tell Dependabot to ignore the version.
4. The script never asks Claude Code to change a workflow file. A red `github-actions` pull request goes to the autofix issue.
5. The script does not merge a `github-actions` pull request whose diff replaces a 40-character commit SHA with a tag or a branch name. It records the reason `pin_not_sha`.

## Autofix branch and merge

All fixes of one workflow run land on one autofix branch.

| Item | Form |
|---|---|
| Branch | `deps/autofix-<YYYY-MM-DD>`, the date of the run in UTC, made from the head of `main` |
| Title, one fix | `<type>(deps): bump <package> to <version>` |
| Title, more fixes | `<type>(deps): update <n> dependencies` |
| Body | one line for each fix with the type of that fix, the advisories, and the link to the run |

1. When the branch holds one or more fix commits, the script pushes it and opens one pull request to `main`, or updates the open pull request of that branch.
2. The type of the title, and thus of the squash commit on `main`, is `fix` when the pull request holds one or more `cargo` fixes, and `chore` when it holds no fix. The squash body keeps one line for each fix, with the type of that fix, so that each fix shows in the changelog.
3. One branch for each day gives at most one autofix release for each day.
4. The script never pushes to `main`. Each change to `main` comes through the merge of a pull request.

### Merge rule

Each merge of the flow is a squash merge after a green check `verdict` of the head commit. The main ruleset of `release.md` (Repository settings) requires the check `verdict`, and the merge settings allow auto-merge. The flow makes no ruleset of its own.

1. When auto-merge waits for the check `verdict` for the release token, the script enables auto-merge with squash on the pull request.
2. When the release token bypasses the main ruleset, so that auto-merge does not wait for the check, the script does not enable auto-merge. It waits for the check `verdict` of the head commit with the wait cap and merges with squash only when the check is green.
3. After the script enables auto-merge on the autofix pull request, it waits for the merge with the wait cap. When the merge does not come before the cap, the script records `release_pending`.

After the merge, GitHub closes each alert that the fix resolves.

## Release pull request step

The script does this step when the run merged at least one `fix(deps)` commit, or when an open release pull request lists a `fix(deps)` change in its changelog section:

1. The script waits for the end of the release-please run on the merge commit.
2. It finds the one open release pull request by its title form `chore(release): release <version>`, and checks that its base holds the merge commit.
3. It waits for the check `verdict` of the head commit of the release pull request with the wait cap.
4. When the check is green, it merges the release pull request with squash. Then the release flow of `release.md` makes the tag and the release.
5. When the check is not green before the cap, the script records `release_pending` and leaves the pull request open. The next run does the step again.

The script does not merge a release pull request that proposes `1.0.0` or a new major version. It records the case in the autofix issue as `release_needs_human`. It also does not merge a release pull request while the repository is private, because the release flow stops on a private repository and a merge would make a tag with no assets. It records that case as `release_needs_human` and leaves the pull request open.

A release that the script merges carries each unreleased releasable commit on `main`, also the `feat` and `fix` commits of the owner.

## Autofix issue

The autofix issue holds the failed items. It has the title "Dependency autofix: failed items" and the label `dependencies`, which Dependabot also uses.

1. At most one autofix issue is open. The script finds it by the title and the label.
2. When one or more items fail in a run, the script opens the issue, or comments on the open one. For each failed item, the text holds the alert or the pull request, the package, the reason, the structured result, the link to the workflow run, and the last 200 lines of the item log after the redaction (see "Credentials and logs"). The full log stays in the run log.
3. The issue holds one machine block with one line for each failed item and its date. The hold time reads this block.
4. When a run ends with no failure and the issue is open, the script comments with the link of the run and closes the issue.

## Hold time

A failed item starts no new `claude -p` run in a later workflow run until one of these events:

1. 7 days pass from the date of the failure.
2. The input of the item changes: a new head commit of the Dependabot pull request, a new patched version of the advisory, or a new commit on `main` that changes the manifest of the item.
3. A person starts the script with the flag `--retry-failed`, which ends the hold for that run.

A held item keeps its line and its first date in the machine block, and the run prints it with the status `held`. A held item counts as a failed item that the autofix issue holds, so the run ends with exit code 1 and the issue stays open.

## Credentials and logs

The script never prints `CLAUDE_CODE_OAUTH_TOKEN` or a GitHub token. No token goes to stdout, to stderr, to a log file, to an issue, to a pull request, or to a commit. An error names a missing credential by its name only.

Before the script writes a log extract to an issue, it removes each match of these credential patterns: `sk-ant-`, `ghp_`, `github_pat_`, `gho_`, `ghs_`, and the value of each token of the run. The repository is public, so each issue and each pull request is public. GitHub masks the literal value of a secret in the run log.

## Script interface

`deps.autofix.sh` follows the same rules as the other scripts in `.agents/scripts/`: the index line, a docstring with the purpose, the usage, and the parameters, a complete `--help`, a machine output on stdout by default, `--human` for the friendly view, actionable errors on stderr, and documented exit codes.

1. The machine output has one `key: value` line for each item and one summary line with the counts.
2. The item line of a `claude -p` run holds the model, the four token counts `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, and `output_tokens`, and the API-equivalent cost of the result.
3. With `--dry-run`, the script does the discovery and prints each action that it would do: the item, the path, the branch, the prompt fields, the merge, and the issue action. It starts no `claude -p` run, writes nothing to the tree, pushes nothing, and writes nothing to the GitHub API. A dry run needs no `claude`, `cargo`, or `CLAUDE_CODE_OAUTH_TOKEN`. It reads with the release token when it is set, and else with the `gh` login of the person who runs it.
4. An unknown flag gives an actionable error on stderr and exit code 2.
5. A documented flag for the tests names a fixture directory, which takes the place of the GitHub API and of `claude`. A run in this mode uses no network.

| Exit code | Meaning |
|---|---|
| 0 | No failure: nothing open, or each item fixed or merged. |
| 1 | One or more items failed, and the autofix issue holds them. |
| 2 | A usage error. |
| 3 | A tool or a credential is missing, or `claude -p` failed with an authentication error. |
| 4 | A GitHub API call failed. |
| 5 | The verdict workflow is absent or not active. |

| Item reason | Meaning |
|---|---|
| `no_patched_version` | The alert has no first patched version. No `claude -p` run started. |
| `timeout` | The `claude -p` run passed the process timeout. |
| `invalid_result` | The structured result is missing, cannot be parsed, or lacks a field. |
| `path_not_allowed` | The diff changes a path outside the allowed paths. |
| `credential_in_diff` | The diff holds a credential pattern or a token value. |
| `pin_not_sha` | A `github-actions` pull request replaces a commit SHA with a tag or a branch name. |
| `claude_auth` | `claude -p` failed with an authentication error. |
| `release_needs_human` | The release pull request proposes `1.0.0` or a new major version. |
| `release_pending` | A merge or a check did not come before the wait cap. The next run continues. |
| `claude_failed` | The structured result has the status `failed`. |
| `no_change` | The structured result has the status `fixed`, and the diff is empty. |
| `verdict_failed` | The verdict of the script failed, or the check `verdict` of a pull request that no `claude -p` run may fix is red. |
| `run_cap` | The item waits for the next run, because this run reached its cap of `claude -p` runs. Not a failure. |
| `budget_cap` | The item waits for the next run, because this run reached its budget. Not a failure. |
| `ecosystem_removed` | The item has the ecosystem `npm`, for example an old alert. The project has no npm code. The item line records it with the status `skipped`. No `claude -p` run started, and the item is not a failure. |

## Freshness scan

The freshness script `.agents/scripts/deps.freshness.sh` has three actions: `scan`, `update`, and `assess`. Each action starts with the scan. The scan measures the age of each direct dependency and gives it one freshness class.

### Inputs

The scan reads only the direct dependencies of three ecosystems:

| Ecosystem | Dependencies | Pinned version |
|---|---|---|
| `cargo` | each entry of `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]` of `Cargo.toml`; a crate in more than one table counts as one dependency | the resolved version of `Cargo.lock`; when the lock file holds more than one version of the crate, the version that the root package uses |
| `github-actions` | each external action of the files under `.github/workflows/`; one dependency for each distinct pin; a local workflow or a local action does not count | the tag of the version comment of the pin |

1. A transitive dependency is not scanned. It follows its parent: when the parent rises, the lock file brings the new transitive versions. The dependency check of the lint stage still checks each crate of `Cargo.lock` (`operations.md`, Dependency check).
2. For an action, the scan checks through the GitHub API that the tag of the version comment resolves to the pinned commit SHA. When it does not, the line holds `pin_check: mismatch`, and the class is `stale`.
3. The scan does not read `.tool-versions`, `rust-toolchain.toml`, the digests of the base images in the `Dockerfile`, or the vendored price file under `vendor/litellm/`. The owner updates these by hand.

### Age sources

| Ecosystem | Source of the versions and of the publish dates |
|---|---|
| `cargo` | the crates.io API, one request for each crate, with the creation time of each version |
| `github-actions` | the releases API of the GitHub repository of the action, through `gh`, with the publish time of the release; for a tag with no release, the commit date of the tagged commit |

1. The age of a version is the count of whole days from its publish time to the scan time, both in UTC.
2. Each crates.io request has a `User-Agent` header that names the project and its repository URL: `agentloupe-deps-freshness (https://github.com/desek/agentloupe)`.
3. The scan sends at most one request in each second to each registry. This is the crawler policy of crates.io.
4. On an HTTP 429 or a 5xx answer, the scan tries again at most 3 times, with a longer wait each time. After the last try, the dependency gets the class `unknown`. An `unknown` dependency gets no `claude -p` run. The run does the other work and then ends with exit code 6. A failed lookup never gives a class other than `unknown`.

### Latest version

The latest version is the highest version by semantic version order that is not a pre-release and not withdrawn:

1. A yanked crate version and a draft GitHub release are never the latest.
2. A pre-release is never the latest. The update never goes to a pre-release. When the pinned version is a pre-release, the scan compares it with the highest release by the same rules.
3. For an action, a tag that is not a full semantic version, for example `v4`, does not count.
4. The scan does not take the order of the API answer or the first entry of a list as the latest. The first entry of the crates.io answer is not always the highest release.
5. A pinned version that is yanked or deprecated gets the class `stale` with the field `yanked: yes`, whatever its age, because its authors withdrew it.

### Freshness classes

Each dependency has exactly one freshness class. Three cases come first:

1. A dependency whose lookup failed has the class `unknown`.
2. An action with a pin mismatch has the class `stale`.
3. A dependency with a yanked or deprecated pinned version has the class `stale`.

Each other dependency gets one class, with this order of precedence:

| Order | Class | Rule |
|---|---|---|
| 1 | `aged` | The latest release is older than 180 days. |
| 2 | `stale` | The pinned version is older than 90 days and is behind the latest. |
| 3 | `fresh` | Each other case: the pinned version is at most 90 days old, or it equals the latest. |

The two thresholds have these reasons:

| Threshold | Value | Reason |
|---|---|---|
| Freshness limit | 90 days | The owner wants no pinned version older than 3 months when a newer version exists. |
| Zero-dep limit | 180 days | When a dependency has no release for more than 6 months, the project considers a replacement with its own code. The limit is measured on the latest release. |

1. The line of each dependency also holds the field `behind`, `yes` or `no`. A dependency of the class `aged` with `behind: yes` gets an update run, as a stale dependency does, because a newer version exists. It also gets an assessment.
2. The script states the two thresholds as two named constants, each with a comment that states its reason. No other script, no prompt template, and no workflow file states them as thresholds. In the specification, this file states them.

### Scan output

The action `scan` prints one line for each dependency with these fields: `ecosystem`, `name`, `kind` (`normal`, `dev`, `build`, or `action`), `pinned`, `pinned_date`, `pinned_age_days`, `latest`, `latest_date`, `latest_age_days`, `behind`, `class`, and `yanked`. The line of an action also holds `pin_check`. Then it prints one summary line with the count of each class, the count of all dependencies, and the scan time in seconds.

The action `scan` only reads. It starts no `claude -p` run, writes nothing to the tree, and makes no write call to the GitHub API. It needs no `CLAUDE_CODE_OAUTH_TOKEN`.

## Freshness update

The action `update` does the scan, then raises each stale dependency to its latest version, one `claude -p` run for each dependency, with the verdict of the script between two runs.

### Items

1. An update item is a `cargo` dependency of the class `stale`, or of the class `aged` with `behind: yes`. Each item gets at most one `claude -p` update run in one workflow run.
2. A `github-actions` dependency gets no `claude -p` run, because Claude Code never changes a workflow file. Dependabot updates the pins of the actions, and the daily autofix merges a green pin pull request. The script records a stale action with the reason `dependabot_path` when a Dependabot pull request for that action is open, and with the reason `actions_no_pr` when none is open. An item with `actions_no_pr` goes into the breaking updates issue.
3. A dependency of the class `unknown` gets no run.
4. The scan reads no npm manifest, because the project has no npm code. Thus an npm dependency is never an item and gets no run. An old npm alert goes to the autofix, which skips it with the reason `ecosystem_removed`.

### Order

The script handles the items in two groups, in this order:

1. `cargo` dependencies of `[dependencies]` and `[build-dependencies]`, because they change the shipped binary.
2. `cargo` dependencies of `[dev-dependencies]`.

In each group, the oldest pinned version comes first, because it has the largest distance to the latest. Items above the run cap of the job, or after the remaining job time rule (see "Limits"), carry the reason `run_cap`.

### Update run

Each update run uses the subscription token, the model, the turn limit, the budget, the Bash tool timeout, and the process timeout of one `claude -p` run of the autofix (see "Limits"). The credential rules of the autofix apply: the process gets no GitHub token, the checkout persists no credential, the process and the verdict of the script run with the fixed allowlist of environment variables, and the tools are the tools of the autofix (see "Tools of Claude Code").

The prompt comes from the template `.agents/scripts/deps.freshness.update.prompt.md`. The script fills in the ecosystem, the name, the kind, the pinned version, the latest version, and the manifest path. The prompt tells Claude Code to do these steps:

1. Raise the dependency to the latest version, also across a major version, in the manifest and in the lock file.
2. Run `.agents/scripts/pipeline.verdict.sh`.
3. Change code in the allowed paths only when the verdict fails because of the update.
4. Raise another direct dependency only when the update cannot build without it.
5. End with one structured result.

An example of a coupled raise is `sha2` and `hmac`: both use the trait crate `digest`, and the project uses an HMAC of SHA-256. The commit names each raised dependency. A dependency that a run raised gets no own run in the same workflow run.

The structured result of an update run has these fields:

| Field | Value |
|---|---|
| `status` | `updated`, `breaks`, or `failed` |
| `name` | the dependency name |
| `from_version` | the pinned version before the run |
| `to_version` | the version after the run |
| `also_raised` | a list of name and version pairs of the other raised direct dependencies |
| `files_changed` | the list of the changed paths |
| `code_changed` | true or false |
| `verdict` | `passed` or `failed`, as Claude Code saw it |
| `failing_stage` | the failing stage, as Claude Code saw it |
| `first_error` | the first error line, as Claude Code saw it |
| `reason` | a short text |

A result that is missing, that cannot be parsed, or that lacks a field counts as `failed` with the reason `invalid_result`.

### Checks of the script

The script, not Claude Code, decides if the app breaks. After each update run, the script does these checks in this order.

The guards:

1. **Allowed paths.** The diff follows the allowed paths of the autofix (see "Checks of the script" under "Fix of an alert"). Another path fails the item with the reason `path_not_allowed`.
2. **No credential.** A match of a credential pattern, or the value of a token of the run, fails the item with the reason `credential_in_diff`.
3. **No removed dependency.** A diff that removes a direct dependency from `Cargo.toml` fails the item with the reason `dependency_removed`. Thus an update run never replaces a dependency, also a cryptography crate, with own code.
4. **No removed test.** The script counts the test attributes `#[test]` and `#[tokio::test]` under `src/` and `tests/`. When the count is lower than the count on the branch before the run, the item fails with the reason `tests_removed`.

The break checks:

1. **Verdict.** The script runs `pipeline.verdict.sh` itself, with no GitHub token and no `CLAUDE_CODE_OAUTH_TOKEN` in its environment. Only exit code 0 counts as passed.
2. **Smoke check of a `cargo` update.** The `loupe` binary of a debug build prints its version with `--version`, writes the line `listening` after `loupe serve`, answers `GET /healthz` with status 200 and the body `{"status":"ok"}`, and on SIGTERM writes the line `database closed, exit` and exits with code 0. These are the items of the binary check of `release.md` (Release build profile, Unwinding check) that run on one host.

The app breaks when a break check fails. The verdict of Claude Code does not count. A guard failure, or a `timeout`, or an `invalid_result`, is a failed run, not a finding that the app breaks. The issue shows it with its reason.

### Two outcomes

Each update item ends green or red.

**Green.** Each guard and each break check passes. The script makes one commit on the freshness branch. The script, not Claude Code, writes the message:

1. The summary has the form `<type>(deps): bump <name> to <version>`.
2. The type follows the ecosystem rule of "Commit" under "Fix of an alert": `fix` for a `cargo` update, also of a dev dependency. A `cargo` update of a dev dependency thus makes a patch release with no change of the binary.
3. The body holds the pinned age and the latest age, one line for each dependency of `also_raised` with its version, and the line `code changed: yes` when the result has `code_changed` true.

The next update run starts from this commit. Thus the verdict runs between two updates.

**Red.** A guard or a break check fails. The script resets the freshness branch to its last good commit. It records the item as `breaks` when a break check failed, or as `failed` with the reason of the guard. A `breaks` record holds the failing check (`verdict` or `smoke`), the failing stage, and the first error line of the verdict output of the script, after the redaction of "Credentials and logs". Then the script continues with the next item.

### Freshness branch and merge

All green updates of one workflow run land on one freshness branch.

1. The branch is `deps/freshness-<YYYY-MM-DD>`, with the date of the run in UTC, made from the head of `main`. A separate branch keeps the weekly major updates apart from the daily security fixes, so a red update never holds back a security fix.
2. When the branch holds one or more commits, the script pushes it, opens one pull request to `main`, and merges it after a green check `verdict`, with the rules of "Autofix branch and merge" and "Merge rule". The title of the pull request follows the title forms of the autofix. The type of the title is `fix` when the pull request holds one or more `cargo` updates, and `chore` when it holds none.
3. Then the script does the "Release pull request step" with the same rules as the autofix.
4. The branch, push, pull request, merge rule, and release pull request steps are in one shared helper, `.agents/scripts/deps.common.sh`, which the autofix script and the freshness script both source. The helper takes the branch name and the item lines as inputs. The behaviour of the autofix does not change.
5. One weekly run gives at most one freshness release each week.

### Breaking updates issue

The breaking updates issue holds the red update items and the actions with `actions_no_pr`. It has the title "Dependency freshness: breaking updates" and the label `dependencies`. The title differs from the title of the autofix issue, so each script finds only its own issue.

1. At most one breaking updates issue is open. The script finds it by the title and the label.
2. When one or more update items break or fail in a run, the script opens the issue, or comments on the open one. For each item, the text holds the ecosystem and the name, the pinned and the latest version, the failing check, the failing stage, the first error, the structured result, and the link to the workflow run. The text also holds the summary of the scan.
3. The issue holds one machine block with one line for each held item and its date.
4. When a later run has no breaking or failed item and the issue is open, the script comments with the link of the run and closes the issue.
5. A run with one or more broken or failed items that the issue holds ends with exit code 1.

### Hold time of a breaking item

An item that broke starts no new update run until one of these events:

1. The latest version of the dependency changes.
2. 28 days pass from the date in the machine block.
3. A person starts the script with the flag `--retry-failed`.

A weekly try of the same version gives the same result and costs a run each week. A held item prints with the status `held` and gets no `claude -p` run.

## Zero-dep assessment

The action `assess` does the scan, then writes one zero-dep assessment for each dependency of the class `aged`, of each ecosystem. An assessment is only a recommendation. A replacement is a separate change request for that dependency, which the owner approves before any code changes.

### Counts

Before each assessment, the script counts the use of the dependency with a fixed search: the import lines, the distinct imported symbols, the call sites, and the count of files. The search covers `src/`, `tests/`, `build.rs`, and, for an action, `.github/workflows/`. These counts are the authority. The assessment uses them.

### Cryptography rule

A cryptography dependency always gets the recommendation `keep` with the reason "cryptography: the project does not write its own cryptography". It gets no `claude -p` run. Own cryptography code has a high risk and no review. A small use does not change the rule, because constant-time and protocol rules come with the crate.

A cryptography dependency is one of these:

1. The crates `aes-gcm`, `sha2`, `hmac`, `subtle`, and `getrandom`, by name.
2. Each crate whose crates.io field `repository` points to the GitHub organisation `RustCrypto`. When crates.io does not give this field, the list of item 1 applies, and the owner adds a crate to it by hand.

A cryptography dependency still gets an update run when it is stale, because an update is not own code.

### Assessment run

For each other aged dependency, the script starts at most one `claude -p` assessment run.

1. The run uses the model, the turn limit, and the budget of one run of "Limits", and the process timeout of 15 minutes.
2. Its tools are Read, Grep, and Glob only. It gets no Bash, no Edit, no Write, and no web tool.
3. After the run, the work tree must be unchanged. When the run changed a file, the item fails with the reason `tree_changed`.
4. The prompt comes from the template `.agents/scripts/deps.freshness.assess.prompt.md`. The script fills in the scan line and the counts.

The structured result of an assessment run has these fields:

| Field | Value |
|---|---|
| `name` | the dependency name |
| `symbols` | the used symbols |
| `call_sites` | the call sites |
| `replacement_lines` | an estimate of the lines of own code that a replacement needs |
| `risks` | a list of risks |
| `recommendation` | `keep`, `replace`, or `watch` |
| `reason` | a short text |

The recommendations mean:

| Recommendation | Meaning |
|---|---|
| `keep` | The project keeps the dependency. |
| `replace` | A replacement with own code is small and safe enough to propose in a change request. |
| `watch` | The project keeps the dependency for now and assesses it again later. |

A `replace` with `replacement_lines` above 500 becomes `watch` with the reason `estimate_too_large`, because a larger replacement holds more risk than the dependency. The owner can still ask for it through a change request. A small dependency can still hold data that changes, for example the time zone data of `chrono-tz`. The assessment names such risks.

### Assessment issue

The script writes each assessment into one issue for each dependency.

1. The title is "Zero-dep assessment: <ecosystem> <name>". The labels are `dependencies` and `zero-dep`. The script makes the label `zero-dep` when it is absent.
2. The issue holds the scan line, the counts, the symbols, the estimate in lines, the risks, the recommendation, the reason, the link to the run, the summary of the scan, and this statement: "A replacement is a separate change request for this dependency. The owner approves it before any code changes."

The script finds the assessment issue of a dependency by its title and its labels, in each state:

1. When no issue exists, it opens one.
2. When the issue is open and the latest version, the counts, and the class did not change, it starts no new `claude -p` run and changes nothing. After 90 days from the last assessment, it starts a new run and comments.
3. When the latest version or the counts changed, it starts a new run and comments.
4. When the dependency is no longer aged, or no longer a dependency, it comments with the reason and closes the issue.
5. When the owner closed the issue, it does not reopen it and does not comment until the latest version changes.

## Freshness workflow

The workflow `.github/workflows/deps-freshness.yml` is a thin caller (`operations.md`, Hosted workflows).

1. It starts each Tuesday at 07:43 UTC and on a manual start. A manual start can ask for a dry run, and the workflow then passes the dry-run flag to the script. The time is after the Dependabot run of Monday at 03:00 UTC, so the scan sees its pull requests, and after the latest end of the daily autofix at 07:17 UTC. The time is off the full hour, where GitHub delays scheduled runs.
2. It uses the concurrency group of the daily autofix, with no cancel of an active run. Thus the weekly run and the daily run never run at once. A daily run that starts while the weekly run is active waits until the weekly run ends (see "Autofix workflow").
3. It has two jobs, `update` and `assess`, which run in parallel, because their runs together do not fit in one hosted job. The job `update` has a timeout of 330 minutes, and the job `assess` has a timeout of 180 minutes.
4. Each job calls `deps.freshness.sh` in exactly one step, with the action `update` or `assess`.
5. The other steps of a job are only these: a checkout with no credential in the git configuration, the tool installs from the pin files as in the autofix workflow, the step of `.agents/scripts/claude.ensure.sh` that makes the Claude Code binary runnable (see "Install of Claude Code"), a cache, and one upload of the report as a workflow artifact. The upload action is pinned by its full commit SHA.
6. The workflow file holds no tool version, no model name, no branch name, no label, no threshold, and no limit other than the job timeouts.
7. Only the step of the script gets `CLAUDE_CODE_OAUTH_TOKEN`. Only the step of the script of the job `update` gets the release token. The workflow adds no repository secret.
8. Until the repository is public and the secret `CLAUDE_CODE_OAUTH_TOKEN` exists, the owner keeps the workflow off after one manual dry run, as for the daily autofix.

| Job | Permissions of the `GITHUB_TOKEN` |
|---|---|
| `update` | the permissions of the autofix job (see "Autofix workflow"), and no others |
| `assess` | `contents: read` and `issues: write` only |

### Report artifact

Each action writes its report as one JSON file to the path of the flag `--report`. Each job uploads its report as one workflow artifact, which GitHub keeps for 30 days.

1. The report holds the scan lines, the result of each run, the four token counts and the API-equivalent cost of each `claude -p` run, and the summary.
2. When the variable of the job summary is set, the script writes the summary of the action to the job summary of the workflow run.
3. Each issue that the run writes holds the summary of the scan.
4. The report, the artifact, the job summary, each issue, each pull request, and each commit hold no credential. The script never prints `CLAUDE_CODE_OAUTH_TOKEN` or a GitHub token. Before it writes a log extract or a first error, it removes each match of the credential patterns of "Credentials and logs".
5. The known gap of the proxy applies: a run of the freshness flow does not show on the dashboard. The report records the tokens and the cost of each run instead.

## Freshness script interface

`deps.freshness.sh` follows the same rules as the other scripts in `.agents/scripts/`: the index line, a docstring with the purpose, the usage, and the parameters, a complete `--help`, a machine output of `key: value` lines on stdout by default, `--human` for a table view, `--json` for one JSON document with the same fields, actionable errors on stderr, and documented exit codes.

| Action | Does |
|---|---|
| `scan` | the scan only; it only reads |
| `update` | the scan, then the update runs, the commits, the push and merge, the release pull request step, and the breaking updates issue |
| `assess` | the scan, then the assessments and the assessment issues |

| Flag | Effect |
|---|---|
| `--help` | prints the purpose, the actions, the flags, and the exit codes |
| `--human` | prints a table view |
| `--json` | prints one JSON document |
| `--report PATH` | writes the report file |
| `--dry-run` | plans and writes nothing (see "Dry run") |
| `--retry-failed` | ends the hold time of the breaking items for this run |
| `--fixtures DIR` | for the tests: recorded answers take the place of the registries, the GitHub API, and `claude`; a run in this mode uses no network |

An unknown flag gives an actionable error on stderr and exit code 2.

### Dry run

With `--dry-run`, the actions `update` and `assess` do the scan and print the plan: each planned run with its prompt fields, the order, the caps, the branch, the issue actions, and the cryptography rule. They start no `claude -p` run, write nothing to the tree, push nothing, and make no write call to the GitHub API. A dry run needs no `claude`, no `cargo`, and no `CLAUDE_CODE_OAUTH_TOKEN`.

### Exit codes and reasons

| Exit code | Meaning |
|---|---|
| 0 | Success: the scan is done, each run is green or no run started, and each assessment is written. |
| 1 | One or more items broke or failed, and an issue holds them. |
| 2 | A usage error. |
| 3 | A tool or a credential is missing. |
| 4 | A GitHub API call failed. |
| 5 | The verdict workflow is absent or not active. Action `update` only, as in the autofix. |
| 6 | A registry lookup failed after its retries. |

| Item status or reason | Meaning |
|---|---|
| `updated` | A green update with one commit on the freshness branch. |
| `breaks` | A break check failed: the verdict or the smoke check. |
| `held` | A breaking item waits for its hold time. No `claude -p` run started. |
| `unknown` | The registry lookup failed. No class and no run. |
| `dependabot_path` | A stale action with an open Dependabot pull request. No `claude -p` run. |
| `actions_no_pr` | A stale action with no open Dependabot pull request. It goes into the breaking updates issue. |
| `timeout` | The `claude -p` run passed its process timeout. |
| `invalid_result` | The structured result is missing, cannot be parsed, or lacks a field. |
| `path_not_allowed` | The diff changes a path outside the allowed paths. |
| `credential_in_diff` | The diff holds a credential pattern or a token value. |
| `dependency_removed` | The diff removes a direct dependency from a manifest. |
| `tests_removed` | The count of test attributes fell. |
| `tree_changed` | An assessment run changed the work tree. |
| `estimate_too_large` | A `replace` with an estimate above 500 lines became `watch`. |
| `run_cap` | The item waits for the next run, because the job reached its run cap or its remaining job time. Not a failure. |
| `budget_cap` | The item waits for the next run, because the job reached its budget. Not a failure. |

### Checks at implementation

These points are checked when the freshness flow is built. Each has a stated rule, so none blocks the flow. The result of each check goes into this file.

1. **Repository field of crates.io.** A check shows whether the crates.io answer holds the field `repository` for each RustCrypto crate. When it does not, the fixed list of the cryptography rule applies.
2. **Pending runs in the shared concurrency group.** Two manual starts in dry run show that a daily run waits behind an active weekly run, and how GitHub handles a second pending run.
3. **Actions with no release.** The first scan counts the pins whose tag has no GitHub release. For these, the commit date of the tagged commit applies.
4. **Time of an update run on the runner.** One update run with its verdict and its smoke check is timed on the runner. The remaining job time rule of "Limits" bounds the job when 4 runs do not fit.

Results, 2026-10-24:

1. Repository field: the crates.io answer holds `repository` for each RustCrypto crate of the project: `aes-gcm` (`RustCrypto/AEADs`), `sha2` (`RustCrypto/hashes`), and `hmac` (`RustCrypto/MACs`). `subtle` (`dalek-cryptography/subtle`) and `getrandom` (`rust-random/getrandom`) are not in the organisation, so the name list holds them.
2. Pending runs: not checked yet. The manual dry run of the freshness workflow needs the workflow file on the default branch; the check follows it.
3. Actions with no release: 0 of the 8 distinct pins; each pinned tag has a GitHub release, so each line holds `date_source: release`.
4. Time of an update run on the runner: not checked yet; it waits for the first real run.

## Install of Claude Code

`.tool-versions` pins Claude Code by its npm package, and mise installs it like the other tools. The install on a hosted runner runs the package scripts off, so the platform binary of Claude Code is missing after it; one repository script runs the postinstall of the package when `claude --version` fails, and each workflow of the two flows calls that script in one step before its script. A hosted runner has about 14 GB of free disk, and one job builds the crate for the verdict and again for each Claude Code run with a changed lock file, which filled the disk in a run of 2026-10-24; one repository script frees the preinstalled software that the project never uses, about 20 GB, at the start of the job, and reports the free space before and after. Off a hosted runner it removes nothing. The update job of the weekly freshness workflow calls the same script, because it builds the crate the same way. The workflow states no version. Dependabot does not update this pin. The owner updates it by hand.

## Known gap: the proxy

A `claude -p` run on a GitHub runner does not go through the proxy of this product. Thus the dashboard does not show its calls and its cost. The item line of the script records the tokens and the cost of each run instead. A route of the run through a hosted proxy needs a public host of the proxy, which the project does not have.

## Cost and free plan bounds

The flow uses no paid service other than the Claude Code subscription of the owner. It uses no API key.

1. The repository is public, so the minutes of the standard GitHub-hosted runners are free. The budget of 0 USD for GitHub Actions stays (`operations.md`, Hosted workflows).
2. Dependabot alerts and Dependabot version updates are free.
3. A job of a GitHub-hosted runner can run for at most 6 hours. The job timeout of 180 minutes of the autofix, and the job timeouts of 330 and 180 minutes of the freshness workflow, are below this bound.
4. GitHub can start a scheduled run late at a time of high load, and can drop it. The next daily run, the next weekly run, or a manual start, does the work.
5. GitHub turns off a scheduled workflow of a public repository after 60 days with no activity in the repository. Then a person turns the workflow on again.
6. Each `claude -p` run uses part of the usage limit of the subscription. The budget of one run, the budget of one workflow run or job, and the run cap limit this use. In one week, the daily autofix starts at most 4 runs each day, and the freshness workflow starts at most 4 update runs and at most 10 assessment runs. The first real freshness run starts the most runs. A later weekly run starts fewer, because a fresh dependency starts no run, a cryptography dependency and an action start no run, a held breaking item starts no run, and an open assessment starts no new run until its input changes or 90 days pass.
7. The API-equivalent cost of a run follows the rules of `cost.md` and the vendored price file. The script compares it with the field `total_cost_usd` of the result. A difference above 5 percent has a stated reason in this file.

The result of `claude -p` holds the token counts per model only as totals of the run (`modelUsage`), not per call. The script prices each total with the base fields `input_cost_per_token`, `output_cost_per_token`, `cache_read_input_token_cost`, and `cache_creation_input_token_cost`, in binary floating point. It applies no prompt-size tier and no 1-hour cache write field, because a total does not show the prompt size or the cache lifetime of each call. A difference above 5 percent from `total_cost_usd` comes from these two rules.

## Checks at implementation

These points are checked when the flow is built. Each has a stated rule, so none blocks the flow. The result of each check goes into this file.

1. **Read of the alerts.** The workflow token may not read the Dependabot alerts. The release token reads them, with the added repository permission "Dependabot alerts: read".
2. **Write permissions.** The release token has "Contents: write" and "Pull requests: write". It also gets "Workflows: write" when the merge of a `github-actions` pull request needs it.
3. **Budget flag.** When the pinned Claude Code has no budget flag for `claude -p`, the script checks the budget after each run from `total_cost_usd`, and the process timeout bounds each run.
4. **Auto-merge and the bypass.** A check shows whether auto-merge with the release token waits for the check `verdict`. The result selects the item of "Merge rule".
5. **Lifetime of the subscription token.** The expiry date goes into "Tokens".
6. **Many fixes in one squash commit.** A check shows whether release-please reads each line of the squash body as its own change. When it does not, a merge with rebase keeps each fix commit, but the merge settings allow only squash. Then the owner decides the method, and this file states it.

Results on 2026-10-24, on the private repository:

| Check | Result |
|---|---|
| 1. Read of the alerts | Not checked yet: the Dependabot alerts are off on the private repository; the list call answers HTTP 403 "Dependabot alerts are disabled for this repository.". The script reads the alerts with the release token, as the rule states. |
| 2. Write permissions | Not checked yet: the first real run follows the public repository and the secret. |
| 3. Budget flag | Claude Code 2.1.310 has `--max-budget-usd` and `--max-turns` for `claude -p`, and reads `BASH_DEFAULT_TIMEOUT_MS` and `BASH_MAX_TIMEOUT_MS`. The script passes the budget of one run, the turn limit, and the Bash tool timeout of 20 minutes with them, and also checks the sum of `total_cost_usd` against the budget of one workflow run. |
| 4. Auto-merge and the bypass | Not checked yet: the ruleset exists only on the public repository. Until the check, the script uses item 2 of "Merge rule": it waits for the green check `verdict` and merges with squash. A constant of the script selects item 1. |
| 5. Lifetime of the subscription token | Not checked yet: the owner has not made the token. |
| 6. Many fixes in one squash commit | The release-please documentation states that each Conventional Commit line in the body of one commit becomes its own changelog entry. The script writes the squash body with one fix line per paragraph. Not checked yet on a real release. |
