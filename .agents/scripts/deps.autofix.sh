#!/usr/bin/env bash
# @agents-index Daily dependency autofix: lists the Dependabot alerts and pull requests, fixes each with one headless claude -p run, merges green fixes and the release pull request, writes the autofix issue.
#
# Usage: deps.autofix.sh [--repo OWNER/NAME] [--dry-run] [--human] [--retry-failed] [--max-turns N]
#                        [--run-url URL] [--fixtures DIR]
#   --repo R         The repository, OWNER/NAME. Default: the GitHub remote origin of the clone.
#   --dry-run        Do the discovery and print each action that a run would do: the item, its path, the
#                    branch, the prompt fields, the merge, and the issue action. Starts no claude -p run,
#                    writes nothing to the tree, pushes nothing, and writes nothing to the GitHub API. Needs no
#                    CLAUDE_CODE_OAUTH_TOKEN; reads with RELEASE_TOKEN when it is set, else with the gh login.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   --retry-failed   End the hold time of the failed items for this run (a manual run of a person).
#   --max-turns N    Lower the turn limit of each claude -p run, for a test run. It cannot raise the limit of 30.
#   --run-url URL    The link of the workflow run, for the pull request body and the autofix issue.
#   --fixtures DIR   For the tests only. DIR takes the place of the GitHub API and of claude: the script exports
#                    DEPS_AUTOFIX_FIXTURES=DIR to the fake gh and the fake claude that the test puts first on
#                    PATH, polls with no delay, reads the date of the run from DIR/today and a lower process
#                    timeout from DIR/claude_timeout_secs when they exist, and refuses a git remote that is not
#                    a local path, so the run uses no network.
#   -h, --help       Print this help.
#
# Environment:
#   RELEASE_TOKEN            The release token (the secret RELEASE_PLEASE_TOKEN in the workflow): the read of the
#                            Dependabot alerts, the push of the autofix branch, the pull request, the rebase
#                            command, and each merge, so that each of them starts the verdict workflow.
#   GH_TOKEN                 The workflow token: the reads of the pull requests, the checks, and the runs, and
#                            the autofix issue. Outside a workflow, the gh login.
#   CLAUDE_CODE_OAUTH_TOKEN  The subscription token, given only to the claude -p process.
#   No token is printed. The claude -p process and the verdict of the script get a fixed allowlist of variables
#   with no GitHub token; the verdict also gets no CLAUDE_CODE_OAUTH_TOKEN. git gets the release token only for
#   the fetch and the push of the script, through a credential helper that reads it from its environment.
#
# Purpose: the flow of docs/specification/dependencies.md. A run: (1) stops with exit 5 when the verdict workflow
# is absent from the tree or not active; (2) stops with exit 3 when a tool or a credential is missing; (3) lists
# the open Dependabot alerts (an error that names disabled alerts stops the run with the setting named) and the
# open Dependabot pull requests; an alert that a pull request moves to a patched version is handled through the
# pull request; (4) with nothing open, prints a zero count line and exits 0; (5) orders the items by severity,
# then age; an alert with no patched version starts no run; an npm item is skipped with the reason
# ecosystem_removed, because the project has no npm code; a held failed item waits (7 days, a new input, or
# --retry-failed); (6) makes or reuses the branch deps/autofix-<YYYY-MM-DD> (UTC) from main, builds the crate
# once, and gives each alert, and each red Dependabot pull request, one claude -p run with the prompt template
# deps.autofix.prompt.md; (7) checks each result: the structured result, the allowed paths, no credential in the
# diff, its own verdict, then commits
# "<type>(deps): bump <package> to <version> (<GHSA id>)" or resets the branch; (8) merges each green Dependabot
# pull request with squash, after the rebase command of Dependabot when it is behind (when no new head comes
# within 5 minutes, the run continues with the current head: a green one merges, a red one goes to claude -p, and
# the item line holds rebase: dependabot_no_response); refuses a pin update that
# replaces a commit SHA with a tag; (9) pushes the branch, opens or updates its pull request, and squash merges it
# after a green check verdict; (10) merges the release pull request after a green check verdict when a fix(deps)
# change is in it, never 1.0.0 or a new major version, and only when the repository is public; (11) opens, comments on, or closes the autofix issue.
#
# Output (stdout, key: value):
#   repo: <owner/name> visibility: public|private mode: dry-run|run
#   count: alerts: <n> pull_requests: <n>
#   item: <alert:N|pr:N|branch:NAME|release:N> path: alert|pull_request|autofix|release package: <name>
#     ecosystem: <e> status: planned|fixed|merged|failed|held|waiting|closed|skipped [reason: <reason>] [fields...]
#     A claude -p run adds: model, input_tokens, cache_creation_input_tokens, cache_read_input_tokens,
#     output_tokens, turns, cost_usd (API-equivalent, from the vendored price file), total_cost_usd (of the result).
#   action: branch|claude|commit|rebase|merge|pull_request|release|issue|comment target: <t> status: planned|done|failed
#     [detail: <text>]
#   result: alerts: <n> pull_requests: <n> fixed: <n> merged: <n> failed: <n> held: <n> waiting: <n>
#     claude_runs: <n> exit: <code>
# Item reasons: no_patched_version, timeout, invalid_result, path_not_allowed, credential_in_diff, pin_not_sha,
#   claude_auth, release_needs_human, release_pending, and claude_failed (the result says failed), no_change (the
#   result says fixed and the diff is empty), verdict_failed (the verdict of the script, or a red check that no run
#   may fix), run_cap and budget_cap (the item waits for the next run), and ecosystem_removed (status skipped:
#   an npm item, recorded on its item line, not fixed and not a failure).
# Exit codes: 0 no failure: nothing open, or each item fixed or merged; 1 one or more items failed or held, and
#   the autofix issue holds them; 2 bad usage; 3 a tool or a credential is missing, or claude -p failed with an
#   authentication error; 4 a GitHub API call failed, or the Dependabot alerts are off; 5 the verdict workflow is
#   absent or not active.
# Reference: docs/specification/dependencies.md.
set -uo pipefail

# ---- the constants of the flow (docs/specification/dependencies.md, Limits) ---------------------------------
MODEL=sonnet
MAX_TURNS=30
RUN_BUDGET_USD=5
WORKFLOW_BUDGET_USD=15
CLAUDE_TIMEOUT_SECS=2400
BASH_TIMEOUT_MS=1200000
RUN_CAP=4
WAIT_CAP_SECS=1800
# The wait for the new head commit after the rebase command of Dependabot; then the run continues with the current head.
REBASE_WAIT_CAP_SECS=300
POLL_SECS=30
HOLD_DAYS=7
LOG_LINES=200
BRANCH_PREFIX=deps/autofix-
ISSUE_TITLE="Dependency autofix: failed items"
LABEL=dependencies
LABEL_COLOR=0366d6
VERDICT_WORKFLOW=.github/workflows/verdict.yml
RELEASE_PLEASE_WORKFLOW=release-please.yml
# The merge rule: "wait" waits for a green check verdict, then squash merges; "auto" enables auto-merge with
# squash. "wait" holds while the release token can bypass the main ruleset (Checks at implementation, item 4).
MERGE_MODE=wait
# The one place that names the token of the alert read and of every write that must start a workflow.
RELEASE_TOKEN_VAR=RELEASE_TOKEN
COMMIT_NAME="github-actions[bot]"
COMMIT_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"
HELD_OPEN="<!-- deps-autofix:held"
HELD_CLOSE="-->"
# The variables that the claude -p process and the verdict of the script may see; no GitHub token is in it.
ENV_ALLOW="PATH HOME USER LOGNAME LANG LC_ALL TERM TMPDIR SHELL CI CARGO_HOME RUSTUP_HOME RUSTUP_TOOLCHAIN CARGO_TERM_COLOR MISE_DATA_DIR MISE_CONFIG_DIR MISE_CACHE_DIR XDG_CONFIG_HOME XDG_CACHE_HOME XDG_DATA_HOME"
ALWAYS_PATHS="Cargo.toml Cargo.lock"
CODE_PREFIXES="src/ tests/"
CRED_PATTERNS="sk-ant- ghp_ github_pat_ gho_ ghs_"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# The shared steps: GitHub calls, redaction, branch, push, pull request, merge rule, release pull request step.
# shellcheck source=deps.common.sh
. "$ROOT/.agents/scripts/deps.common.sh"
human=0; dry_run=0; retry_failed=0; max_turns=$MAX_TURNS; repo=""; run_url=""; fixtures=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }
need_value() { [[ $1 -ge 2 ]] || die "$2 needs a value. Fix: run --help for the flags. Verify: deps.autofix.sh --help." 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) need_value $# "$1"; repo="$2"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    --human) human=1; shift ;;
    --retry-failed) retry_failed=1; shift ;;
    --max-turns) need_value $# "$1"
      [[ "$2" =~ ^[1-9][0-9]*$ ]] || die "--max-turns $2 is not a positive whole number. Fix: pass a value from 1 to $MAX_TURNS. Verify: deps.autofix.sh --dry-run --max-turns 1." 2
      [[ "$2" -le $MAX_TURNS ]] || die "--max-turns $2 is above the limit of $MAX_TURNS turns, and the flag can only lower it. Fix: pass a value from 1 to $MAX_TURNS. Reference: docs/specification/dependencies.md, section Limits. Verify: deps.autofix.sh --dry-run --max-turns $MAX_TURNS." 2
      max_turns="$2"; shift 2 ;;
    --run-url) need_value $# "$1"; run_url="$2"; shift 2 ;;
    --fixtures) need_value $# "$1"; fixtures="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the flags. Verify: deps.autofix.sh --dry-run prints a count line." 2 ;;
  esac
done

cd "$ROOT" || die "cannot enter $ROOT. Fix: run the script from its checkout. Verify: re-run." 2
today="$(date -u +%Y-%m-%d)"
WAIT_CAP_MIN=$((WAIT_CAP_SECS / 60))
REBASE_WAIT_CAP_MIN=$((REBASE_WAIT_CAP_SECS / 60))
if [[ -n "$fixtures" ]]; then
  [[ -d "$fixtures" ]] || die "the fixture directory $fixtures does not exist. Fix: pass --fixtures <dir> with the recorded answers. Verify: ls $fixtures." 2
  fixtures="$(cd "$fixtures" && pwd)"
  export DEPS_AUTOFIX_FIXTURES="$fixtures"
  POLL_SECS=0; WAIT_CAP_SECS=2; REBASE_WAIT_CAP_SECS=1
  [[ -s "$fixtures/today" ]] && today="$(tr -d ' \n' <"$fixtures/today")"
  if [[ -s "$fixtures/claude_timeout_secs" ]]; then
    t="$(tr -d ' \n' <"$fixtures/claude_timeout_secs")"; [[ "$t" =~ ^[0-9]+$ && "$t" -lt $CLAUDE_TIMEOUT_SECS ]] && CLAUDE_TIMEOUT_SECS="$t"
  fi
  case "$(git remote get-url origin 2>/dev/null)" in
    http*|git@*|ssh:*) die "the fixture mode needs a local git remote, so the run uses no network. Fix: point origin of the scratch repository to a local bare repository. Verify: git remote get-url origin." 2 ;;
  esac
fi
branch="$BRANCH_PREFIX$today"

# ---- (1) precondition: the verdict workflow ------------------------------------------------------------------
[[ -f "$ROOT/$VERDICT_WORKFLOW" ]] || die "the verdict workflow $VERDICT_WORKFLOW is absent from the tree, so no fix can be judged and the run stops before any API write and any claude -p run. Fix: implement the release governance that adds $VERDICT_WORKFLOW (docs/specification/release.md, Release flow; docs/specification/operations.md, Hosted workflows). Verify: ls $VERDICT_WORKFLOW." 5

# ---- (2) tools and credentials -------------------------------------------------------------------------------
tools="gh git jq"; [[ $dry_run -eq 0 ]] && tools="$tools claude cargo"
for t in $tools; do
  command -v "$t" >/dev/null 2>&1 || die "$t is not installed, so the dependency autofix cannot run. Fix: install $t (the workflow installs claude from .tool-versions with mise; gh, git, and jq are on the runner; cargo with rustup). Verify: $t --version." 3
done
release_token="${!RELEASE_TOKEN_VAR:-}"
if [[ $dry_run -eq 0 ]]; then
  [[ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}" ]] || die "CLAUDE_CODE_OAUTH_TOKEN is empty, so no claude -p run can start. Fix: create the repository secret CLAUDE_CODE_OAUTH_TOKEN with claude setup-token and give it to the step of this script (docs/specification/dependencies.md, Tokens), or run with --dry-run. Verify: deps.autofix.sh --dry-run." 3
  [[ -n "$release_token" ]] || die "$RELEASE_TOKEN_VAR is empty, so the alerts cannot be read and no push or merge can start a workflow. Fix: give the step the secret RELEASE_PLEASE_TOKEN as $RELEASE_TOKEN_VAR, with Dependabot alerts read, contents and pull requests write (docs/specification/dependencies.md, Tokens), or run with --dry-run. Verify: deps.autofix.sh --dry-run." 3
  if [[ -z "${GH_TOKEN:-}" ]] && ! gh auth token >/dev/null 2>&1; then
    die "GH_TOKEN is empty and gh is not logged in, so the pull requests and the issue cannot be read or written. Fix: pass the workflow token as GH_TOKEN, or run gh auth login. Verify: gh api user." 3
  fi
  if git config --get-regexp '^http\..*\.extraheader$' >/dev/null 2>&1; then
    die "the git configuration of the checkout holds an authorization header, so a claude -p run could read a GitHub token. Fix: check out with persist-credentials: false, or remove the http.*.extraheader entries. Reference: docs/specification/dependencies.md, Tokens. Verify: git config --get-regexp extraheader prints nothing." 3
  fi
  [[ -z "$(git status --porcelain 2>/dev/null)" ]] || die "the work tree has uncommitted changes, so a fix commit could take them. Fix: run on a clean checkout, or run with --dry-run. Verify: git status --porcelain prints nothing." 2
fi
if [[ -z "$repo" ]]; then
  repo="$(git remote get-url origin 2>/dev/null | sed -nE 's#^(https://github\.com/|git@github\.com:)([^/]+/[^/.]+)(\.git)?$#\2#p')"
fi
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "the repository '${repo:-none}' is not OWNER/NAME. Fix: pass --repo OWNER/NAME. Verify: re-run with --dry-run." 2
owner="${repo%%/*}"

work="$(mktemp -d "${TMPDIR:-/tmp}/deps.autofix.XXXXXX")" || die "mktemp failed. Fix: check TMPDIR. Verify: re-run." 2
claude_pid=""
cleanup() { [[ -n "$claude_pid" ]] && kill "$claude_pid" 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

# ---- output ---------------------------------------------------------------------------------------------------
n_alerts=0; n_prs=0; fixed=0; merged=0; failed=0; held=0; waiting=0; claude_runs=0; auth_failed=0
finish() {
  local code="$1"
  if [[ "$code" -eq 0 ]]; then
    [[ $failed -gt 0 || $held -gt 0 ]] && code=1
    [[ $auth_failed -eq 1 ]] && code=3
  fi
  if [[ $human -eq 1 ]]; then
    echo "Dependency autofix: $n_alerts alerts, $n_prs Dependabot pull requests; $fixed fixed, $merged merged, $failed failed, $held held, $waiting waiting; $claude_runs claude -p runs (exit $code)."
  else
    echo "result: alerts: $n_alerts pull_requests: $n_prs fixed: $fixed merged: $merged failed: $failed held: $held waiting: $waiting claude_runs: $claude_runs exit: $code"
  fi
  exit "$code"
}
# item <key> <path> <package> <ecosystem> <status> [reason] [extra key: value text]
item() {
  local key="$1" path="$2" pkg="$3" eco="$4" status="$5" reason="${6:-}" extra="${7:-}"
  case "$status" in fixed) fixed=$((fixed + 1)) ;; merged) merged=$((merged + 1)) ;; failed) failed=$((failed + 1)) ;;
    held) held=$((held + 1)) ;; waiting) waiting=$((waiting + 1)) ;; esac
  if [[ $human -eq 1 ]]; then echo "$key ($path, $pkg, $eco): $status${reason:+, $reason}${extra:+ [$extra]}"; else
    echo "item: $key path: $path package: $pkg ecosystem: $eco status: $status${reason:+ reason: $reason}${extra:+ $extra}"; fi
}
action() {
  local kind="$1" target="$2" status="$3" detail="${4:-}"
  if [[ $human -eq 1 ]]; then echo "  $kind $target: $status${detail:+ ($detail)}"; else
    echo "action: $kind target: $target status: $status${detail:+ detail: $detail}"; fi
}
api_fail() { echo "error: the GitHub API call $1 failed: $(redact <"$work/err" | head -c 300 | tr '\n' ' '). Fix: check the token permissions and the network, then re-run. Reference: docs/specification/dependencies.md, Tokens. Verify: gh api $1." >&2; finish 4; }

ver_ge() { [[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n 1)" == "$2" ]]; }
sev_rank() { case "$1" in critical) echo 4 ;; high) echo 3 ;; medium|moderate) echo 2 ;; low) echo 1 ;; *) echo 0 ;; esac; }

# ---- the repository and the verdict workflow ------------------------------------------------------------------
get "repos/$repo" "$work/repo.json" || api_fail "repos/$repo"
visibility="$(jq -r 'if .private == true then "private" elif .private == false then "public" else "unknown" end' "$work/repo.json")"
mode=run; [[ $dry_run -eq 1 ]] && mode=dry-run
if [[ $human -eq 1 ]]; then echo "Repository $repo ($visibility), $mode, branch $branch."; else echo "repo: $repo visibility: $visibility mode: $mode"; fi
get "repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")" "$work/wf.json" || {
  grep -q 'HTTP 404' "$work/err" && die "the verdict workflow $VERDICT_WORKFLOW is not known on $repo, so no fix can be judged; the run stops before any API write and any claude -p run. Fix: push the release governance that adds $VERDICT_WORKFLOW to the default branch. Verify: gh api repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")." 5
  api_fail "repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")"; }
wf_state="$(jq -r '.state // "unknown"' "$work/wf.json")"
[[ "$wf_state" == active ]] || die "the verdict workflow $VERDICT_WORKFLOW is $wf_state on $repo, not active, so no fix can be judged; the run stops before any API write and any claude -p run. Fix: enable it with gh workflow enable verdict.yml (the release governance adds it). Verify: gh api repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW") --jq .state prints active." 5

# ---- (3) alerts on, (4) discovery ------------------------------------------------------------------------------
if ! get_release "repos/$repo/dependabot/alerts?state=open&per_page=100" "$work/alerts.json"; then
  if grep -qi 'disabled' "$work/err" "$work/alerts.json" 2>/dev/null; then
    msg="$(jq -r '.message // empty' "$work/alerts.json" 2>/dev/null || true)"; [[ -n "$msg" ]] || msg="$(head -n 1 "$work/err" | head -c 200)"
    echo "error: the Dependabot alerts are off for $repo: the alert list answered '$msg'. Fix: turn on the setting Dependabot alerts (Settings, Advanced Security; repo.settings.sh applies it on the public repository). Reference: docs/specification/dependencies.md, Dependabot configuration. Verify: gh api 'repos/$repo/dependabot/alerts?state=open' answers a list." >&2
    finish 4
  fi
  api_fail "repos/$repo/dependabot/alerts"
fi
jq -e 'type == "array"' "$work/alerts.json" >/dev/null 2>&1 || { echo "the alert answer is not a list" >"$work/err"; api_fail "repos/$repo/dependabot/alerts"; }
get "repos/$repo/pulls?state=open&per_page=100" "$work/pulls.json" || api_fail "repos/$repo/pulls"
jq -e 'type == "array"' "$work/pulls.json" >/dev/null 2>&1 || { echo "the pull request answer is not a list" >"$work/err"; api_fail "repos/$repo/pulls"; }

jq -c '.[] | select(.state == "open") | {kind: "alert", key: ("alert:" + (.number | tostring)), number,
    package: .security_vulnerability.package.name,
    ecosystem: ({"rust": "cargo", "actions": "github-actions"}[.security_vulnerability.package.ecosystem] // .security_vulnerability.package.ecosystem),
    range: (.security_vulnerability.vulnerable_version_range // ""), patched: (.security_vulnerability.first_patched_version.identifier // ""),
    manifest: (.dependency.manifest_path // ""), ghsa: (.security_advisory.ghsa_id // ""),
    severity: (.security_advisory.severity // .security_vulnerability.severity // "low"), created: (.created_at // "")}' \
  "$work/alerts.json" >"$work/alerts.jsonl"
jq -c '.[] | select(.user.login == "dependabot[bot]") | {kind: "pull_request", key: ("pr:" + (.number | tostring)), number,
    title, head: .head.sha, ref: .head.ref, created: (.created_at // ""),
    ecosystem: (.head.ref | if startswith("dependabot/cargo/") then "cargo" elif startswith("dependabot/npm_and_yarn/") then "npm"
      elif startswith("dependabot/github_actions/") then "github-actions" else "unknown" end),
    packages: ([(.body // "") | scan("Updates `([^`]+)` from (\\S+) to (\\S+)") | {name: .[0], to: (.[2] | rtrimstr("."))}] as $g
      | if ($g | length) > 0 then $g else [(.title | capture("bump (?<name>\\S+) from \\S+ to (?<to>\\S+)")?) | {name, to: (.to | rtrimstr("."))}] end)}' \
  "$work/pulls.json" >"$work/prs.jsonl"
n_alerts="$(wc -l <"$work/alerts.jsonl" | tr -d ' ')"; n_prs="$(wc -l <"$work/prs.jsonl" | tr -d ' ')"
if [[ $human -eq 1 ]]; then echo "Open: $n_alerts Dependabot alerts, $n_prs Dependabot pull requests."; else echo "count: alerts: $n_alerts pull_requests: $n_prs"; fi

# ---- the open autofix issue and its held items ------------------------------------------------------------------
get "repos/$repo/issues?state=open&labels=$LABEL&per_page=100" "$work/issues.json" || api_fail "repos/$repo/issues"
issue_number="$(jq -r --arg t "$ISSUE_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .number // empty' "$work/issues.json")"
issue_url="$(jq -r --arg t "$ISSUE_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .html_url // empty' "$work/issues.json")"
jq -r --arg t "$ISSUE_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .body // ""' "$work/issues.json" \
  | awk -v o="$HELD_OPEN" -v c="$HELD_CLOSE" '$0 == o {f = 1; next} f && $0 == c {f = 0} f && NF {print}' >"$work/held.old"

# Nothing open: the steps below make no branch, no pull request, no issue, and no claude -p run; only an open
# release pull request whose changelog lists a fix(deps) change, and the close of an open autofix issue, remain.
nothing_open=0; [[ "$n_alerts" -eq 0 && "$n_prs" -eq 0 ]] && nothing_open=1

# ---- per pull request details, and the match of an alert to a pull request -------------------------------------
: >"$work/prs.full.jsonl"
while IFS= read -r pr; do
  n="$(jq -r .number <<<"$pr")"; head="$(jq -r .head <<<"$pr")"
  get "repos/$repo/pulls/$n/files?per_page=100" "$work/files.$n.json" || api_fail "repos/$repo/pulls/$n/files"
  get "repos/$repo/compare/main...$head" "$work/compare.$n.json" || api_fail "repos/$repo/compare/main...$head"
  state="$(check_state "$head")"
  behind="$(jq -r '.behind_by // 0' "$work/compare.$n.json")"
  # A pin update must keep a full 40-character commit SHA on each added uses: line.
  pin_ok="$(jq -r '[.[].patch // "" | split("\n")[] | select(test("^\\+\\s*-?\\s*uses:\\s*[^@\\s]+@"))
    | capture("@(?<ref>[^\\s#]+)").ref | test("^[0-9a-f]{40}$")] | all' "$work/files.$n.json")"
  jq -c --arg s "$state" --argjson b "$behind" --argjson p "$pin_ok" --slurpfile f "$work/files.$n.json" \
    '. + {check: $s, behind: $b, pin_ok: $p, files: [$f[0][].filename], package: ([.packages[].name] | join(",")),
          target: ([.packages[].to] | join(",")), ghsa: "", severity: "none", alerts: []}' <<<"$pr" >>"$work/prs.full.jsonl"
done <"$work/prs.jsonl"

: >"$work/items.jsonl"
cp "$work/prs.full.jsonl" "$work/prs.match.jsonl"
while IFS= read -r al; do
  pkg="$(jq -r .package <<<"$al")"; patched="$(jq -r .patched <<<"$al")"; matched=""
  if [[ -n "$patched" ]]; then
    while IFS= read -r pr; do
      to="$(jq -r --arg p "$pkg" '[.packages[] | select(.name == $p) | .to] | first // empty' <<<"$pr")"
      if [[ -n "$to" ]] && ver_ge "$to" "$patched"; then matched="$(jq -r .number <<<"$pr")"; break; fi
    done <"$work/prs.match.jsonl"
  fi
  if [[ -n "$matched" ]]; then
    # One item, through the pull request path: the pull request carries the advisory and its severity.
    jq -c --argjson m "$matched" --argjson a "$al" 'if .number == $m then
        .alerts += [$a.key] | .ghsa = ([.ghsa, $a.ghsa] | map(select(. != "")) | join(","))
        | .severity = (if ({"critical":4,"high":3,"medium":2,"moderate":2,"low":1}[$a.severity] // 0) > ({"critical":4,"high":3,"medium":2,"moderate":2,"low":1}[.severity] // 0) then $a.severity else .severity end)
        | .created = (if $a.created < .created then $a.created else .created end)
      else . end' "$work/prs.match.jsonl" >"$work/prs.tmp" && mv "$work/prs.tmp" "$work/prs.match.jsonl"
  else
    echo "$al" >>"$work/items.jsonl"
  fi
done <"$work/alerts.jsonl"
cat "$work/prs.match.jsonl" >>"$work/items.jsonl"
# Highest severity first, then the oldest.
jq -s -c 'sort_by([-({"critical":4,"high":3,"medium":2,"moderate":2,"low":1}[.severity] // 0), .created]) | .[]' "$work/items.jsonl" >"$work/items.sorted.jsonl"

# ---- hold time --------------------------------------------------------------------------------------------------
main_ref=HEAD; git rev-parse --verify -q origin/main >/dev/null && main_ref=origin/main
input_of() {
  local it="$1" kind; kind="$(jq -r .kind <<<"$it")"
  if [[ "$kind" == alert ]]; then
    echo "patched=$(jq -r '.patched | if . == "" then "none" else . end' <<<"$it");main=$(git log -1 --format=%h "$main_ref" -- "$(jq -r .manifest <<<"$it")" 2>/dev/null || true)"
  else echo "head=$(jq -r .head <<<"$it" | cut -c1-12)"; fi
}
# held_reason <key> <input>: prints the reason of a held item, or nothing.
held_reason() {
  [[ $retry_failed -eq 1 ]] && return 0
  awk -v k="$1" -v i="input:$2" '$1 == k && $2 == i {print $3, $4; exit}' "$work/held.old" | while read -r d r; do
    days="$(jq -nr --arg a "${d#date:}" --arg b "$today" '((($b | strptime("%Y-%m-%d") | mktime) - ($a | strptime("%Y-%m-%d") | mktime)) / 86400) | floor' 2>/dev/null || echo 99)"
    [[ "$days" -lt $HOLD_DAYS ]] && echo "${r#reason:} ${d#date:}"
  done
}
: >"$work/held.new"; : >"$work/failures"; : >"$work/fixlines"; : >"$work/closeprs"; : >"$work/stalled_prs"
# record_failure <key> <input> <reason> <package> <text file or empty> <result json or empty>
record_failure() {
  echo "$1 input:$2 date:$today reason:$3" >>"$work/held.new"
  printf '%s\t%s\t%s\t%s\t%s\n' "$1" "$3" "$4" "${5:-}" "${6:-}" >>"$work/failures"
}

# ---- the planned claude -p runs ---------------------------------------------------------------------------------
prompt_fields() {
  jq -r 'if .kind == "alert" then "package=\(.package) ecosystem=\(.ecosystem) range=\(.range | gsub(" "; "")) patched=\(.patched) manifest=\(.manifest) advisory=\(.ghsa)"
    else "package=\(.package) ecosystem=\(.ecosystem) target=\(.target) pull_request=\(.number)\(if .ghsa != "" then " advisory=\(.ghsa)" else "" end)" end' <<<"$1"
}
commit_type_of_eco() { echo fix; }
summary_of() {
  # <type> <item json>: the commit summary from the alert or pull request data, never from Claude Code.
  jq -r --arg t "$1" 'if .kind == "alert" then "\($t)(deps): bump \(.package) to \(.patched) (\(.ghsa))"
    else "\($t)(deps): bump \(.package) to \(.target)\(if .ghsa != "" then " (\(.ghsa))" else "" end)" end' <<<"$2"
}

# ---- one claude -p run and the checks of the script --------------------------------------------------------------
render_prompt() {
  local it="$1"
  DEPS_PACKAGE="$(jq -r .package <<<"$it")" DEPS_ECOSYSTEM="$(jq -r .ecosystem <<<"$it")" \
  DEPS_RANGE="$(jq -r '.range // "not given: a version update of Dependabot"' <<<"$it")" \
  DEPS_PATCHED="$(jq -r 'if .kind == "alert" then .patched else .target end' <<<"$it")" \
  DEPS_MANIFEST="$(jq -r 'if .kind == "alert" then .manifest else "Cargo.toml" end' <<<"$it")" \
  DEPS_ADVISORY="$(jq -r 'if .ghsa != "" then .ghsa else "none: a version update of Dependabot" end' <<<"$it")" \
  DEPS_KIND="$(jq -r 'if .kind == "alert" then "Dependabot security alert \(.number)" else "Dependabot pull request \(.number) whose check verdict failed" end' <<<"$it")" \
  awk 'BEGIN { split("package ecosystem vulnerable_range target_version manifest_path advisory_id item", k, " ")
                split("DEPS_PACKAGE DEPS_ECOSYSTEM DEPS_RANGE DEPS_PATCHED DEPS_MANIFEST DEPS_ADVISORY DEPS_KIND", e, " ") }
       /^<!-- .* -->$/ { next }
       { out = ""; line = $0
         # Literal replacement, so no character of a value acts as a pattern.
         while ((p = index(line, "{{")) > 0) {
           q = index(substr(line, p + 2), "}}"); if (q == 0) break
           name = substr(line, p + 2, q - 1); val = "{{" name "}}"
           for (i = 1; i <= 7; i++) if (k[i] == name) val = ENVIRON[e[i]]
           out = out substr(line, 1, p - 1) val; line = substr(line, p + q + 3)
         }
         print out line }' "$ROOT/.agents/scripts/deps.autofix.prompt.md"
}
# clean_env [--exec] [NAME=value...] -- command...: runs the command with only the allowlisted variables and the
# given pairs; --exec replaces the shell, so the process ID is the one of the command.
clean_env() {
  local args=() v ex=0
  [[ "$1" == --exec ]] && { ex=1; shift; }
  for v in $ENV_ALLOW; do [[ -n "${!v:-}" ]] && args+=("$v=${!v}"); done
  while [[ "$1" != -- ]]; do args+=("$1"); shift; done; shift
  if [[ $ex -eq 1 ]]; then exec env -i "${args[@]}" "$@"; else env -i "${args[@]}" "$@"; fi
}
run_verdict() { clean_env -- "$ROOT/.agents/scripts/pipeline.verdict.sh" >>"$1" 2>&1 </dev/null; }
# The model of the run, the four token counts, the turns, the API-equivalent cost from the vendored price file
# (the base fields of cost.md on the totals per model), and total_cost_usd of the result.
usage_fields() {
  jq -r --slurpfile p "$ROOT/vendor/litellm/model_prices_and_context_window.json" '
    def price($m): ($p[0][$m] // $p[0]["anthropic/" + $m] // $p[0][$m | sub("-[0-9]{8}$"; "")] // null)
      | if . != null and .litellm_provider == "anthropic" then . else null end;
    def cost: [(.modelUsage // {}) | to_entries[] | price(.key) as $q
      | if $q == null then null else
        (.value.inputTokens // 0) * ($q.input_cost_per_token // 0) + (.value.outputTokens // 0) * ($q.output_cost_per_token // 0)
        + (.value.cacheReadInputTokens // 0) * ($q.cache_read_input_token_cost // 0)
        + (.value.cacheCreationInputTokens // 0) * ($q.cache_creation_input_token_cost // 0) end]
      | if length == 0 then 0 elif any(. == null) then null else add end;
    "model: \((.modelUsage // {}) | to_entries | sort_by(-(.value.costUSD // 0)) | (first | .key) // "unknown")"
    + " input_tokens: \(.usage.input_tokens // 0) cache_creation_input_tokens: \(.usage.cache_creation_input_tokens // 0)"
    + " cache_read_input_tokens: \(.usage.cache_read_input_tokens // 0) output_tokens: \(.usage.output_tokens // 0)"
    + " turns: \(.num_turns // 0) cost_usd: \(cost | if . == null then "unpriced" else (. * 1000000 | round / 1000000 | tostring) end)"
    + " total_cost_usd: \(.total_cost_usd // 0)"' "$1" 2>/dev/null || echo "model: unknown"
}
# The last JSON object of the result text with every field of the structured result, or nothing.
structured_of() {
  jq -c '.result // "" | [scan("\\{(?:[^{}\\[\\]]|\\[[^\\]]*\\])*\\}")] | map(fromjson? // empty
    | select(type == "object" and (.status == "fixed" or .status == "failed") and (.package | type) == "string"
      and (.from_version | type) == "string" and (.to_version | type) == "string" and (.files_changed | type) == "array"
      and (.verdict == "passed" or .verdict == "failed") and (.reason | type) == "string")) | last // empty' "$1" 2>/dev/null
}
spent_usd=0
# fix_item <item json> <log file>: sets fix_status (fixed|failed), fix_reason, fix_result, fix_usage.
fix_item() {
  local it="$1" log="$2" last_good out="$work/claude.out" marker="$work/claude.timeout" rc changed p code=0 s
  fix_status=failed; fix_reason=""; fix_result=""; fix_usage=""
  last_good="$(git rev-parse HEAD)"
  render_prompt "$it" >"$work/prompt.txt"
  rm -f "$marker" "$out"
  claude_runs=$((claude_runs + 1))
  ( clean_env --exec CLAUDE_CODE_OAUTH_TOKEN="$CLAUDE_CODE_OAUTH_TOKEN" BASH_DEFAULT_TIMEOUT_MS="$BASH_TIMEOUT_MS" BASH_MAX_TIMEOUT_MS="$BASH_TIMEOUT_MS" -- \
      claude -p --output-format json --model "$MODEL" --max-turns "$max_turns" --max-budget-usd "$RUN_BUDGET_USD" \
      --no-session-persistence \
      --allowedTools Read Edit Write Grep Glob "Bash(cargo:*)" \
        "Bash(git diff:*)" "Bash(git status:*)" "Bash(git log:*)" "Bash(.agents/scripts/pipeline.verdict.sh:*)" \
        "Bash(.agents/scripts/pipeline.format.sh:*)" "Bash(.agents/scripts/pipeline.lint.sh:*)" "Bash(.agents/scripts/pipeline.test.sh:*)" \
        "Bash(.agents/scripts/pipeline.web.sh:*)" "Bash(.agents/scripts/pipeline.image.sh:*)" \
      --disallowedTools WebFetch WebSearch "Bash(git commit:*)" "Bash(git push:*)" "Bash(gh:*)" "Bash(curl:*)" "Bash(env:*)" "Bash(printenv:*)" \
      <"$work/prompt.txt" >"$out" 2>>"$log" ) &
  claude_pid=$!
  ( sleep "$CLAUDE_TIMEOUT_SECS" & s=$!; trap 'kill $s 2>/dev/null; exit 0' TERM; wait $s; touch "$marker"; kill -TERM "$claude_pid" 2>/dev/null ) &
  local wd=$!
  wait "$claude_pid"; rc=$?; claude_pid=""
  kill -TERM "$wd" 2>/dev/null; wait "$wd" 2>/dev/null
  jq -r '.result // empty' "$out" >>"$log" 2>/dev/null
  fix_usage="$(usage_fields "$out")"
  s="$(jq -r '.total_cost_usd // 0' "$out" 2>/dev/null || echo 0)"
  spent_usd="$(jq -nr --argjson a "$spent_usd" --argjson b "${s:-0}" '$a + $b' 2>/dev/null || echo "$spent_usd")"
  if [[ -e "$marker" ]]; then fix_reason=timeout
  elif jq -e '(.api_error_status == 401) or ((.is_error == true) and ((.result // "") | test("authentication_error|invalid api key|oauth token|please run /login"; "i")))' "$out" >/dev/null 2>&1; then
    fix_reason=claude_auth; auth_failed=1
  else
    fix_result="$(structured_of "$out")"
    if [[ -z "$fix_result" ]]; then fix_reason=invalid_result
    elif [[ "$(jq -r .status <<<"$fix_result")" != fixed ]]; then fix_reason=claude_failed
    fi
  fi
  [[ -n "$fix_reason" ]] || [[ $rc -eq 0 ]] || fix_reason=invalid_result
  if [[ -z "$fix_reason" ]]; then
    git add -A >/dev/null 2>&1
    changed="$(git diff --cached --name-only "$last_good")"
    if [[ -z "$changed" ]]; then fix_reason=no_change; fi
  fi
  if [[ -z "$fix_reason" ]]; then
    local code_changed=0
    for p in $changed; do
      case " $ALWAYS_PATHS " in *" $p "*) continue ;; esac
      case "$p" in src/*|tests/*) code_changed=1; continue ;; esac
      fix_reason=path_not_allowed; break
    done
    if [[ -z "$fix_reason" ]] && git diff --cached "$last_good" | grep '^+' | grep -v '^+++' | has_credential; then fix_reason=credential_in_diff; fi
    if [[ -z "$fix_reason" && $code_changed -eq 1 ]]; then
      # A code change is allowed only when the verdict failed after the version change alone.
      git diff --cached --binary "$last_good" >"$work/full.patch"
      git reset -q --hard "$last_good"
      git apply --include=Cargo.toml --include=Cargo.lock "$work/full.patch" 2>>"$log"
      echo "verdict: the version change alone" >>"$log"
      if run_verdict "$log"; then fix_reason=path_not_allowed; fi
      git reset -q --hard "$last_good"; git clean -fdq
      git apply "$work/full.patch" 2>>"$log" && git add -A >/dev/null 2>&1
    fi
    if [[ -z "$fix_reason" ]]; then echo "verdict: the fix" >>"$log"; run_verdict "$log" || fix_reason=verdict_failed; fi
    if [[ -z "$fix_reason" ]]; then
      local type=fix
      local summary; summary="$(summary_of "$type" "$it")"
      if git -c user.name="$COMMIT_NAME" -c user.email="$COMMIT_EMAIL" commit -q -m "$summary" >>"$log" 2>&1; then
        fix_status=fixed; echo "$summary" >>"$work/fixlines"
      else fix_reason=verdict_failed; fi
    fi
  fi
  if [[ "$fix_status" != fixed ]]; then git reset -q --hard "$last_good"; git clean -fdq; fi
}

# ---- the run over the items ----------------------------------------------------------------------------------
branch_ready=0
planned_claude=0; planned_fix_merge=0; planned_branch=0; planned_records=0
while IFS= read -r it; do
  key="$(jq -r .key <<<"$it")"; kind="$(jq -r .kind <<<"$it")"; pkg="$(jq -r .package <<<"$it")"; eco="$(jq -r .ecosystem <<<"$it")"
  input="$(input_of "$it")"; hr="$(held_reason "$key" "$input")"
  if [[ "$kind" == alert ]]; then
    path=alert
    extra="number: $(jq -r .number <<<"$it") ghsa: $(jq -r .ghsa <<<"$it") severity: $(jq -r .severity <<<"$it") range: $(jq -r '.range | gsub(" "; "")' <<<"$it") patched: $(jq -r '.patched | if . == "" then "none" else . end' <<<"$it") manifest: $(jq -r .manifest <<<"$it")"
  else
    path=pull_request
    extra="number: $(jq -r .number <<<"$it") head: $(jq -r .head <<<"$it" | cut -c1-12) check: $(jq -r .check <<<"$it") behind: $(jq -r .behind <<<"$it") target: $(jq -r .target <<<"$it") files: $(jq -r '.files | join(",")' <<<"$it")$(jq -r 'if .ghsa != "" then " ghsa: \(.ghsa) alerts: \(.alerts | join(","))" else "" end' <<<"$it")"
  fi
  if [[ -n "$hr" ]]; then
    # A held item: no new claude -p run; its line stays in the issue with its first date.
    item "$key" "$path" "$pkg" "$eco" held "${hr% *}" "$extra since: ${hr#* }"
    awk -v k="$key" '$1 == k' "$work/held.old" >>"$work/held.new"
    continue
  fi
  if [[ "$eco" == npm ]]; then
    # The project has no npm code, so an npm item from an old alert or pull request is skipped, not fixed.
    item "$key" "$path" "$pkg" "$eco" skipped ecosystem_removed "$extra"
    continue
  fi
  if [[ "$kind" == alert && "$(jq -r .patched <<<"$it")" == "" ]]; then
    if [[ $dry_run -eq 1 ]]; then item "$key" "$path" "$pkg" "$eco" planned no_patched_version "$extra plan: record in the issue, no claude -p run"; planned_records=$((planned_records + 1))
    else item "$key" "$path" "$pkg" "$eco" failed no_patched_version "$extra"; record_failure "$key" "$input" no_patched_version "$pkg" "" ""; fi
    continue
  fi
  needs_claude=0
  if [[ "$kind" == pull_request ]]; then
    n="$(jq -r .number <<<"$it")"; head="$(jq -r .head <<<"$it")"; state="$(jq -r .check <<<"$it")"
    if [[ "$eco" == github-actions && "$(jq -r .pin_ok <<<"$it")" != true ]]; then
      if [[ $dry_run -eq 1 ]]; then item "$key" "$path" "$pkg" "$eco" planned pin_not_sha "$extra plan: no merge, record in the issue"; planned_records=$((planned_records + 1))
      else item "$key" "$path" "$pkg" "$eco" failed pin_not_sha "$extra"; record_failure "$key" "$input" pin_not_sha "$pkg" "" ""; fi
      continue
    fi
    if [[ $dry_run -eq 1 ]]; then
      [[ "$(jq -r .behind <<<"$it")" -gt 0 ]] && action rebase "$key" planned "comment @dependabot rebase with the release token, wait for the new head commit up to $REBASE_WAIT_CAP_MIN minutes, then continue with the current head"
      case "$state" in
        success) item "$key" "$path" "$pkg" "$eco" planned "" "$extra plan: merge"
          action merge "$key" planned "squash merge after a green check verdict of the head commit"
          [[ "$(jq -r .title <<<"$it")" == fix\(deps\)* ]] && planned_fix_merge=1 ;;
        failure) if [[ "$eco" == github-actions ]]; then item "$key" "$path" "$pkg" "$eco" planned verdict_failed "$extra plan: record in the issue, no claude -p run for a workflow file"; planned_records=$((planned_records + 1))
          else needs_claude=1; fi ;;
        *) item "$key" "$path" "$pkg" "$eco" planned "" "$extra plan: wait for the check verdict up to $WAIT_CAP_MIN minutes, then merge when green" ;;
      esac
    else
      if [[ "$(jq -r .behind <<<"$it")" -gt 0 ]]; then
        if write_release --method POST "repos/$repo/issues/$n/comments" -f body="@dependabot rebase" >/dev/null; then
          action rebase "$key" done "asked Dependabot to rebase"
          new_head="$head"; k=0; max="$(polls "$REBASE_WAIT_CAP_SECS")"
          while [[ "$new_head" == "$head" && $k -lt $max ]]; do
            get "repos/$repo/pulls/$n" "$work/pr.json" && new_head="$(jq -r '.head.sha // empty' "$work/pr.json")"; [[ -n "$new_head" ]] || new_head="$head"
            k=$((k + 1)); [[ "$new_head" == "$head" ]] && sleep_poll
          done
          if [[ "$new_head" == "$head" ]]; then
            # No new head before the rebase cap: continue with the current head. A squash merge of a branch
            # behind main is valid (the verdict runs on main after the merge); a red check goes to claude -p
            # on the autofix branch from main, as for any red Dependabot pull request.
            extra="$extra rebase: dependabot_no_response"
          else
            head="$new_head"; state="$(check_state "$head")"
          fi
        else action rebase "$key" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; item "$key" "$path" "$pkg" "$eco" waiting release_pending "$extra"; continue; fi
      fi
      [[ "$state" == success || "$state" == failure ]] || state="$(wait_check "$head")"
      case "$state" in
        success)
          if write_release --method PUT "repos/$repo/pulls/$n/merge" -f merge_method=squash -f sha="$head" >"$work/merge.json"; then
            action merge "$key" done "squash"; item "$key" "$path" "$pkg" "$eco" merged "" "$extra"
            if [[ "$(jq -r .title <<<"$it")" == fix\(deps\)* ]]; then merged_fix_sha="$(jq -r '.sha // empty' "$work/merge.json")"; fi
          else action merge "$key" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; item "$key" "$path" "$pkg" "$eco" waiting release_pending "$extra"; fi ;;
        failure)
          if [[ "$eco" == github-actions ]]; then item "$key" "$path" "$pkg" "$eco" failed verdict_failed "$extra"; record_failure "$key" "$input" verdict_failed "$pkg" "" ""
          else needs_claude=1; fi ;;
        *) item "$key" "$path" "$pkg" "$eco" waiting release_pending "$extra wait: check verdict" ;;
      esac
    fi
    [[ $needs_claude -eq 1 ]] || continue
  fi
  # An alert, or a red Dependabot pull request: one claude -p run on the autofix branch.
  if [[ $auth_failed -eq 1 ]]; then item "$key" "$path" "$pkg" "$eco" waiting claude_auth "$extra"; continue; fi
  if [[ $((planned_claude + claude_runs)) -ge $RUN_CAP ]]; then item "$key" "$path" "$pkg" "$eco" waiting run_cap "$extra"; continue; fi
  if jq -ne --argjson a "$spent_usd" --argjson c "$WORKFLOW_BUDGET_USD" '$a >= $c' >/dev/null; then item "$key" "$path" "$pkg" "$eco" waiting budget_cap "$extra"; continue; fi
  type="$(commit_type_of_eco "$eco")"
  if [[ $dry_run -eq 1 ]]; then
    if [[ $planned_branch -eq 0 ]]; then
      if git ls-remote --exit-code --heads origin "$branch" >/dev/null 2>&1; then action branch "$branch" planned "reuse the branch of an earlier run today"
      else action branch "$branch" planned "from the head of main"; fi
      planned_branch=1
    fi
    planned_claude=$((planned_claude + 1))
    item "$key" "$path" "$pkg" "$eco" planned "" "$extra plan: claude"
    action claude "$key" planned "$(prompt_fields "$it") model=$MODEL max_turns=$max_turns budget_usd=$RUN_BUDGET_USD timeout_min=$((CLAUDE_TIMEOUT_SECS / 60))"
    action commit "$key" planned "$(summary_of "$type" "$it")"
    [[ "$type" == fix ]] && planned_fix_merge=1
    continue
  fi
  prepare_branch "$branch"
  log="$work/item.$(echo "$key" | tr ':/' '__').log"; : >"$log"
  fix_item "$it" "$log"
  if [[ "$fix_status" == fixed ]]; then
    item "$key" "$path" "$pkg" "$eco" fixed "" "$extra $fix_usage"
    [[ "$kind" == pull_request ]] && echo "$(jq -r .number <<<"$it")" >>"$work/closeprs"
  else
    item "$key" "$path" "$pkg" "$eco" failed "$fix_reason" "$extra $fix_usage"
    record_failure "$key" "$input" "$fix_reason" "$pkg" "$log" "$fix_result"
    [[ "$kind" == pull_request ]] && echo "$(jq -r .number <<<"$it")" >>"$work/stalled_prs"
  fi
done <"$work/items.sorted.jsonl"

# ---- push and merge of the autofix branch (deps.common.sh) ------------------------------------------------------
merged_fix_sha="${merged_fix_sha:-}"
if [[ $dry_run -eq 1 ]]; then
  [[ $planned_claude -gt 0 ]] && plan_branch_merge "$branch"
elif [[ $branch_ready -eq 1 ]] && [[ "$(git rev-list --count origin/main..HEAD)" -gt 0 ]]; then
  git log --reverse --format=%s origin/main..HEAD >"$work/alllines"
  branch_merge "$branch" "$work/alllines" autofix fixes fix "Advisories: $(grep -o 'GHSA-[a-z0-9-]*' "$work/alllines" | sort -u | tr '\n' ' ' | sed 's/ $//')"
fi

# ---- the release pull request step (deps.common.sh) -------------------------------------------------------------
release_step

# ---- the autofix issue ----------------------------------------------------------------------------------------
# A held line of an item that is no longer open drops out; a new failure replaces an older line of its key.
awk '!s[$1]++' "$work/held.new" >"$work/held.block"
n_fail="$(grep -c . "$work/failures" || true)"; n_block="$(grep -c . "$work/held.block" || true)"
block="$HELD_OPEN
$(cat "$work/held.block")
$HELD_CLOSE"
issue_body="The dependency autofix (docs/specification/dependencies.md) records here each item that failed. An item waits $HOLD_DAYS days, or until its input changes, before a new claude -p run; a run with --retry-failed ends the hold. The block below is read by the script.

$block"
comment_text() {
  echo "Run: ${run_url:-local run} on $today. Failed items:"
  while IFS=$'\t' read -r key reason pkg logf result; do
    echo
    echo "### $key: $pkg"
    echo "- Reason: \`$reason\`"
    [[ -n "$result" ]] && echo "- Structured result: \`$(redact <<<"$result")\`"
    [[ "$reason" == claude_auth ]] && echo "- Fix: run \`claude setup-token\` and replace the secret CLAUDE_CODE_OAUTH_TOKEN (docs/specification/dependencies.md, Tokens)."
    if [[ -n "$logf" && -s "$logf" ]]; then
      echo
      echo "<details><summary>Last $LOG_LINES lines of the item log</summary>"
      echo
      echo '```'
      tail -n "$LOG_LINES" "$logf" | redact
      echo '```'
      echo "</details>"
    fi
  done <"$work/failures"
}
if [[ $dry_run -eq 1 && $planned_records -gt 0 ]]; then
  action issue autofix-issue planned "${issue_number:+comment on #$issue_number}${issue_number:-open \"$ISSUE_TITLE\" with the label $LABEL}; $planned_records items to record, and each claude -p run that fails"
elif [[ "$n_fail" -gt 0 ]]; then
  if [[ $dry_run -eq 1 ]]; then action issue autofix-issue planned "${issue_number:+comment on #$issue_number}${issue_number:-open \"$ISSUE_TITLE\" with the label $LABEL}"
  else
    comment_text >"$work/comment.md"
    get "repos/$repo/labels/$LABEL" "$work/label.json" || write_issue --method POST "repos/$repo/labels" -f name="$LABEL" -f color="$LABEL_COLOR" \
      -f description="Pull requests that update a dependency file" >/dev/null || true
    if [[ -z "$issue_number" ]]; then
      if write_issue --method POST "repos/$repo/issues" -f title="$ISSUE_TITLE" -f body="$issue_body" -f "labels[]=$LABEL" >"$work/issue.json"; then
        issue_number="$(jq -r '.number // empty' "$work/issue.json")"; issue_url="$(jq -r '.html_url // empty' "$work/issue.json")"
        action issue "#$issue_number" done "opened"
      else action issue autofix-issue failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
    else
      write_issue --method PATCH "repos/$repo/issues/$issue_number" -f body="$issue_body" >/dev/null && action issue "#$issue_number" done "block updated" \
        || action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
    fi
    if [[ -n "$issue_number" ]]; then
      write_issue --method POST "repos/$repo/issues/$issue_number/comments" -F body=@"$work/comment.md" >/dev/null && action comment "#$issue_number" done "$n_fail failed items" \
        || action comment "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
      for n in $(sort -u "$work/stalled_prs"); do
        write_issue --method POST "repos/$repo/issues/$n/comments" -f body="The dependency autofix could not fix this update; see ${issue_url:-#$issue_number}. It waits $HOLD_DAYS days or a new head commit." >/dev/null \
          && action comment "pr:$n" done "linked the autofix issue" || action comment "pr:$n" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
      done
    fi
  fi
elif [[ -n "$issue_number" ]]; then
  if [[ "$n_block" -gt 0 ]]; then
    if [[ $dry_run -eq 1 ]]; then action issue "#$issue_number" planned "keep open: $n_block held items"
    elif ! cmp -s "$work/held.block" "$work/held.old"; then
      write_issue --method PATCH "repos/$repo/issues/$issue_number" -f body="$issue_body" >/dev/null && action issue "#$issue_number" done "block updated, $n_block held items" \
        || action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
    fi
  elif [[ $dry_run -eq 1 ]]; then action issue "#$issue_number" planned "comment with the run link and close"
  elif write_issue --method POST "repos/$repo/issues/$issue_number/comments" -f body="No failed item in this run. ${run_url:+Run: $run_url}" >/dev/null \
    && write_issue --method PATCH "repos/$repo/issues/$issue_number" -f state=closed >/dev/null; then action issue "#$issue_number" done "closed"
  else action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
fi
finish 0
