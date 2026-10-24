<!-- @agents-index Specification of the dependency flow: Dependabot configuration, the daily autofix workflow and its script, the headless Claude Code run, the autofix branch and its merge, the release pull request step, the autofix issue, the hold time, the tokens, the limits, and the known gap of the proxy. -->
# Dependencies

This file states how the project keeps its dependencies current with no human step. Dependabot finds the updates and the vulnerable dependencies. A daily workflow calls one script. The script lets Claude Code in headless mode fix each item on one branch, merges each green fix, and merges the release pull request of `release.md` when its check is green. Thus a dependency fix becomes a patch release.

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
| `npm` | `/prototype` | weekly | `chore(deps)` | none, the type is hidden, because the prototype is not shipped |
| `github-actions` | `/` | weekly | `ci(deps)` | none, the type is hidden, because a pin of an action does not change the binary or the image |

1. The version updates run each Monday at 03:00 UTC. Thus the daily run of Monday finds the new pull requests.
2. The minor and patch updates of one ecosystem come in one grouped pull request each week. Each major update has its own pull request.
3. Dependabot updates the pin of an action by its full 40-character commit SHA and keeps the version comment.
4. Dependabot does not read `.tool-versions`, the digests of the base images in the `Dockerfile`, or the vendored price file under `vendor/litellm/`. The owner updates these by hand. `prices.vendor.sh` refreshes the price file.
5. The `npm` entry stays in the file also when Dependabot does not support the pnpm version that `prototype/package.json` names. Then the failure shows in the Dependabot log, and the script still fixes the alerts of `prototype/`.

## Autofix workflow

The workflow `.github/workflows/deps-autofix.yml` is a thin caller (`operations.md`, Hosted workflows).

1. It starts each day at 04:17 UTC and on a manual start. The time is before the working day of the owner and off the full hour, where GitHub delays scheduled runs.
2. It does not start on a push or on a pull request. Thus its own merges do not start it again.
3. It has one concurrency group for all its runs. At most one run is active. A new run waits and does not cancel the active run.
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
| The release token, secret `RELEASE_PLEASE_TOKEN` (`release.md`, Release token) | the read of the Dependabot alerts; the push of the autofix branch; the autofix pull request; the enable of auto-merge; the rebase command comment on a Dependabot pull request; each merge of the flow | the `claude -p` process; the verdict of the script |
| The `GITHUB_TOKEN` of the workflow | the autofix issue and its comments; the reads of the pull requests, the checks, and the workflow runs | a push or a merge, because GitHub starts no workflow from an event that this token makes |
| The subscription token, secret `CLAUDE_CODE_OAUTH_TOKEN` | the `claude -p` process only | a GitHub call; the verdict of the script |

1. The script reads the alerts with one token, named in one place of the script. Dependabot does not act on a command from the bot of the `GITHUB_TOKEN`, and an event of the `GITHUB_TOKEN` starts no verdict workflow. Thus each write that must start a workflow or a Dependabot action uses the release token.
2. The script gives a GitHub token to git only for its own push. While `claude -p` runs, no file in the tree or in `.git/` holds a GitHub token, and the environment of the `claude -p` process holds no GitHub token.
3. The owner makes the subscription token with `claude setup-token`. The token has a fixed lifetime. Expiry date: not recorded yet. The owner records the date here when the secret is made.

To rotate the subscription token, do these steps:

1. Run `claude setup-token` on the machine of the owner.
2. Replace the value of the repository secret `CLAUDE_CODE_OAUTH_TOKEN` with the new token.
3. Start the workflow by hand and check that the item lines show no reason `claude_auth`.
4. Record the new expiry date in this section.

## Run of the script

The script holds all the logic of the flow. A run does these steps in this order:

1. **Precondition.** When the verdict workflow is absent from the checked-out tree, or is not active on the repository, the script stops with exit code 5 before any API write and before any `claude -p` run. The error names the file of the verdict workflow and the release governance that adds it.
2. **Tools and credentials.** When `gh`, `git`, `claude`, `cargo`, or `corepack` is missing, or when `CLAUDE_CODE_OAUTH_TOKEN` or the GitHub token is empty, the script stops with exit code 3 before any run. The error names the missing item and never its value. pnpm comes through corepack, with the version of the `packageManager` field of `prototype/package.json`.
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
| Job timeout of the workflow | 180 minutes |

A documented flag of the script lowers the turn limit for a test run. It cannot raise a limit.

### Prompt

The prompt comes from the template `.agents/scripts/deps.autofix.prompt.md`. The script fills in the package, the ecosystem, the vulnerable range, the first patched version, the manifest path, and the advisory identifier. For a Dependabot pull request, the script fills in the package and the target version of the pull request.

The prompt tells Claude Code to do these steps:

1. Update the dependency in the manifest and in the lock file.
2. Run `.agents/scripts/pipeline.verdict.sh`.
3. Change code only when the verdict fails because of the update.
4. End with one JSON object, the structured result.

For `prototype/`, the update runs with pnpm through corepack, in `prototype/`. When no update of a direct dependency brings the patched version of a transitive package, the update adds an override entry in `prototype/package.json`.

### Tools of Claude Code

Claude Code gets the tools Read, Edit, Write, Grep, and Glob. It gets the Bash tool only for `cargo`, for `pnpm` in `prototype/`, for `git diff`, `git status`, and `git log`, and for `.agents/scripts/pipeline.*.sh`. It gets no web fetch, no web search, no `git commit`, no `git push`, no `gh`, no `curl`, no `env`, and no `printenv`. Claude Code does not commit, does not push, and does not call the GitHub API.

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

1. **Allowed paths.** The diff can always change `Cargo.toml`, `Cargo.lock`, `prototype/package.json`, and `prototype/pnpm-lock.yaml`. It can change `src/`, `tests/`, and `prototype/src/` only when the verdict failed after the version change. It can never change `.github/`, `.agents/`, `docs/`, `vendor/`, `web/`, `Dockerfile`, `compose.yaml`, `rust-toolchain.toml`, or `.tool-versions`. A diff with another path fails with the reason `path_not_allowed`.
2. **No credential.** A diff that holds a match of a credential pattern (see "Credentials and logs"), or the value of a token of the run, fails with the reason `credential_in_diff`. The script writes the matched text nowhere.
3. **Verdict.** For a result with the status `fixed`, the script runs `pipeline.verdict.sh` itself, with no GitHub token and no `CLAUDE_CODE_OAUTH_TOKEN` in its environment. Only exit code 0 of this run counts as passed. The `verdict` field of the result does not count.
4. **Prototype.** For an update in `prototype/`, the script also installs with the frozen lock file and builds the prototype, with the same environment rule. The verdict does not read `prototype/`.

The script builds the crate one time before the first `claude -p` run, so later verdicts reuse the target directory.

### Commit

When each check passes, the script makes one commit on the autofix branch. The script writes the message from the alert data. The summary has the form `<type>(deps): bump <package> to <version> (<GHSA id>)`.

| Fix | Type |
|---|---|
| The fix changes a `cargo` dependency. | `fix` |
| The fix changes only `prototype/`. | `chore` |

### Failure

When a check fails, the script resets the autofix branch to its last good commit, records the item as failed, and continues with the next item. It does not start a second `claude -p` run for the same item in the same workflow run.

When `claude -p` fails with an authentication error, the script starts no more `claude -p` runs, writes the reason `claude_auth` and the command `claude setup-token` into the autofix issue, and exits with code 3. The merge of the green Dependabot pull requests continues, because it needs no Claude Code.

## Dependabot pull requests

For each open Dependabot pull request, the script does these steps:

1. When the pull request is behind `main`, the script posts the rebase command of Dependabot with the release token, and waits for the new head commit with the wait cap.
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
2. The type of the title, and thus of the squash commit on `main`, is `fix` when the pull request holds one or more `cargo` fixes, and `chore` when it holds only `prototype/` fixes. The squash body keeps one line for each fix, with the type of that fix, so that each fix shows in the changelog.
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

The script does not merge a release pull request that proposes `1.0.0` or a new major version. It records the case in the autofix issue as `release_needs_human`.

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
3. With `--dry-run`, the script does the discovery and prints each action that it would do: the item, the path, the branch, the prompt fields, the merge, and the issue action. It starts no `claude -p` run, writes nothing to the tree, pushes nothing, and writes nothing to the GitHub API. A dry run needs no `claude`, `cargo`, `corepack`, or `CLAUDE_CODE_OAUTH_TOKEN`. It reads with the release token when it is set, and else with the `gh` login of the person who runs it.
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
| `prototype_failed` | The frozen install or the build of the prototype failed. |
| `run_cap` | The item waits for the next run, because this run reached its cap of `claude -p` runs. Not a failure. |
| `budget_cap` | The item waits for the next run, because this run reached its budget. Not a failure. |

## Install of Claude Code

`.tool-versions` pins Claude Code by its npm package, and mise installs it like the other tools. The workflow states no version. Dependabot does not update this pin. The owner updates it by hand.

## Known gap: the proxy

A `claude -p` run on a GitHub runner does not go through the proxy of this product. Thus the dashboard does not show its calls and its cost. The item line of the script records the tokens and the cost of each run instead. A route of the run through a hosted proxy needs a public host of the proxy, which the project does not have.

## Cost and free plan bounds

The flow uses no paid service other than the Claude Code subscription of the owner. It uses no API key.

1. The repository is public, so the minutes of the standard GitHub-hosted runners are free. The budget of 0 USD for GitHub Actions stays (`operations.md`, Hosted workflows).
2. Dependabot alerts and Dependabot version updates are free.
3. A job of a GitHub-hosted runner can run for at most 6 hours. The job timeout of 180 minutes is below this bound.
4. GitHub can start a scheduled run late at a time of high load, and can drop it. The next daily run, or a manual start, does the work.
5. GitHub turns off a scheduled workflow of a public repository after 60 days with no activity in the repository. Then a person turns the workflow on again.
6. Each `claude -p` run uses part of the usage limit of the subscription. The budget of one run, the budget of one workflow run, and the run cap limit this use.
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
7. **pnpm of the prototype.** A check shows whether Dependabot supports the pnpm version of `prototype/package.json`.

Results on 2026-10-24, on the private repository:

| Check | Result |
|---|---|
| 1. Read of the alerts | Not checked yet: the Dependabot alerts are off on the private repository; the list call answers HTTP 403 "Dependabot alerts are disabled for this repository.". The script reads the alerts with the release token, as the rule states. |
| 2. Write permissions | Not checked yet: the first real run follows the public repository and the secret. |
| 3. Budget flag | Claude Code 2.1.310 has `--max-budget-usd` and `--max-turns` for `claude -p`, and reads `BASH_DEFAULT_TIMEOUT_MS` and `BASH_MAX_TIMEOUT_MS`. The script passes the budget of one run, the turn limit, and the Bash tool timeout of 20 minutes with them, and also checks the sum of `total_cost_usd` against the budget of one workflow run. |
| 4. Auto-merge and the bypass | Not checked yet: the ruleset exists only on the public repository. Until the check, the script uses item 2 of "Merge rule": it waits for the green check `verdict` and merges with squash. A constant of the script selects item 1. |
| 5. Lifetime of the subscription token | Not checked yet: the owner has not made the token. |
| 6. Many fixes in one squash commit | The release-please documentation states that each Conventional Commit line in the body of one commit becomes its own changelog entry. The script writes the squash body with one fix line per paragraph. Not checked yet on a real release. |
| 7. pnpm of the prototype | Not checked yet: the first weekly Dependabot run follows the merge of `.github/dependabot.yml`. |
