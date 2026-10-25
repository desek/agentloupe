#!/usr/bin/env bash
# @agents-index Weekly dependency freshness: scans the direct cargo and action dependencies against their registries, updates each stale one with one claude -p run judged by the verdict, and writes zero-dep assessments of the aged ones.
#
# Usage: deps.freshness.sh scan|update|assess [--repo OWNER/NAME] [--human] [--json] [--report PATH] [--dry-run]
#                          [--retry-failed] [--run-url URL] [--fixtures DIR]
#   scan             Only reads: one line per direct dependency with its freshness class, then one summary line.
#                    Starts no claude -p run, writes nothing to the tree or to GitHub, needs no
#                    CLAUDE_CODE_OAUTH_TOKEN.
#   update           The scan, then one claude -p update run per stale cargo dependency (and per aged one
#                    that is behind its latest), the checks of the script, one commit per green update on the
#                    branch deps/freshness-<YYYY-MM-DD> (UTC), its pull request and merge, the release pull
#                    request step, and the issue "Dependency freshness: breaking updates".
#   assess           The scan, then one zero-dep assessment per aged dependency in one issue per dependency,
#                    "Zero-dep assessment: <ecosystem> <name>", with the labels dependencies and zero-dep.
#   --repo R         The repository, OWNER/NAME (update and assess). Default: the GitHub remote origin.
#   --human          Print a table for a person. Also the default when stdout is a terminal.
#   --json           Print one JSON document with the same fields (the report) in place of the lines.
#   --report PATH    Write the report as one JSON file: the scan lines, the result of each run with its four
#                    token counts and its API-equivalent cost, the items, and the summary.
#   --dry-run        update and assess: do the scan and print the plan (each planned run with its prompt fields,
#                    the order, the caps, the branch, the issue actions, the cryptography rule). Starts no
#                    claude -p run, writes nothing to the tree, pushes nothing, and writes nothing to GitHub.
#                    Needs no claude, cargo, or CLAUDE_CODE_OAUTH_TOKEN.
#   --retry-failed   update: end the hold time of the breaking items for this run (a manual run of a person).
#   --run-url URL    The link of the workflow run, for the pull request, the issues, and the report.
#   --fixtures DIR   For the tests only. Exports DEPS_FRESHNESS_FIXTURES=DIR to the fake curl, gh, claude, and
#                    cargo that the test puts first on PATH; reads the scan time from DIR/now, a lower
#                    request interval from DIR/registry_interval_ms, lower process timeouts from
#                    DIR/claude_timeout_secs and DIR/smoke_timeout_secs, and the elapsed job time from
#                    DIR/job_elapsed_min when they exist; polls with no delay; refuses a git remote that is not
#                    a local path. A run in this mode uses no network.
#   -h, --help       Print this help.
#
# Environment:
#   RELEASE_TOKEN            update: the push of the freshness branch, its pull request, and each merge (the
#                            secret RELEASE_PLEASE_TOKEN in the workflow), so that each starts the verdict workflow.
#   GH_TOKEN                 The workflow token: the GitHub reads, the tags and releases of the pinned actions, the
#                            issues, and the label. Outside a workflow, the gh login.
#   CLAUDE_CODE_OAUTH_TOKEN  update and assess: the subscription token, given only to the claude -p process.
#   GITHUB_STEP_SUMMARY      When set, the summary of the action is appended to the job summary.
#   No token is printed. The claude -p process and the verdict of the script get the fixed allowlist of variables
#   of the autofix with no GitHub token; the verdict also gets no CLAUDE_CODE_OAUTH_TOKEN.
#
# Purpose: the flow of docs/specification/dependencies.md, sections "Freshness scan", "Freshness update",
# "Zero-dep assessment", "Freshness workflow", and "Freshness script interface". The scan reads Cargo.toml with
# Cargo.lock and the action pins of .github/workflows/; it asks crates.io (with a User-Agent) and the GitHub
# releases of each action,
# at most one request per second per registry, with at most 3 retries; the latest version is the highest
# release that is not a pre-release and not withdrawn. Each dependency gets one class: unknown (lookup failed),
# stale (pin mismatch, or a yanked or deprecated pinned version), else aged, stale, or fresh by the two
# thresholds below. The script, not Claude Code, decides if the app breaks: its own verdict, the smoke check of
# the debug binary. The scan reads no npm manifest, so no npm item exists.
#
# Output (stdout, key: value):
#   ecosystem: <cargo|github-actions> name: <n> kind: normal|dev|build|action pinned: <v> pinned_date: <d>
#     pinned_age_days: <n> latest: <v> latest_date: <d> latest_age_days: <n> behind: yes|no
#     class: fresh|stale|aged|unknown yanked: yes|no [pin_check: ok|mismatch date_source: release|commit]
#   summary: fresh: <n> stale: <n> aged: <n> unknown: <n> aged_behind: <n> total: <n> scan_secs: <n>
#   repo: <owner/name> visibility: public|private mode: dry-run|run action: update|assess    (update, assess)
#   item: <ecosystem:name|branch:NAME|release:N> path: update|assess|freshness|release package: <name>
#     ecosystem: <e> status: planned|updated|breaks|failed|held|raised|waiting|unknown|assessed|unchanged|closed
#     [reason: <reason>] [fields...]; a claude -p run adds model, the four token counts, turns, run_secs,
#     cost_usd (API-equivalent, from the vendored price file), and total_cost_usd (of the result).
#   action: branch|claude|commit|push|pull_request|merge|release|issue|comment|label target: <t>
#     status: planned|done|failed [detail: <text>]
#   result: action: <a> items: <n> updated: <n> breaks: <n> failed: <n> held: <n> waiting: <n> assessed: <n>
#     claude_runs: <n> cost_usd: <n> exit: <code>
# Reasons: unknown, dependabot_path, actions_no_pr, also_raised, timeout, invalid_result, claude_auth, no_change,
#   path_not_allowed, credential_in_diff, dependency_removed, tests_removed, tree_changed, estimate_too_large,
#   cryptography, run_cap, budget_cap, release_needs_human, release_pending, verdict_failed.
# Exit codes: 0 success: the scan is done, each run green or none, each assessment written; 1 one or more items
#   broke or failed (or wait their hold time), and an issue holds them; 2 bad usage; 3 a tool or a credential is
#   missing, or claude -p failed with an authentication error; 4 a GitHub API call failed; 5 the verdict workflow
#   is absent or not active (update only); 6 a registry lookup failed after its retries (wins over 1).
# Reference: docs/specification/dependencies.md.
set -uo pipefail

# ---- the two thresholds (docs/specification/dependencies.md, Freshness classes) --------------------------------
# D1, freshness limit: the owner wants no pinned version older than 3 months when a newer version exists.
FRESH_DAYS=90
# D2, zero-dep limit: with no release for more than 6 months, the project considers a replacement with own code;
# measured on the latest release.
AGED_DAYS=180

# ---- the limits of the flow (docs/specification/dependencies.md, Limits) ---------------------------------------
MODEL=sonnet
MAX_TURNS=30
RUN_BUDGET_USD=5
UPDATE_TIMEOUT_SECS=2400
ASSESS_TIMEOUT_SECS=900
BASH_TIMEOUT_MS=1200000
UPDATE_RUN_CAP=4
ASSESS_RUN_CAP=10
UPDATE_JOB_BUDGET_USD=15
ASSESS_JOB_BUDGET_USD=20
# The job timeout of the job update, and the job time that must remain to start one more update run.
UPDATE_JOB_MIN=330
MIN_REMAINING_MIN=75
HOLD_DAYS=28
REASSESS_DAYS=90
REPLACE_MAX_LINES=500
# The polite rate: at most one request per second per registry; 1100 ms keeps a margin for the clock.
REGISTRY_INTERVAL_MS=1100
RETRIES=3
USER_AGENT="agentloupe-deps-freshness (https://github.com/desek/agentloupe)"
SMOKE_TIMEOUT_SECS=60
# The cryptography rule: these crates by name, and each crate whose crates.io repository is in this organisation.
CRYPTO_CRATES="aes-gcm sha2 hmac subtle getrandom"
CRYPTO_ORG=RustCrypto
CRYPTO_REASON="cryptography: the project does not write its own cryptography"
BRANCH_PREFIX=deps/freshness-
BREAK_TITLE="Dependency freshness: breaking updates"
ASSESS_TITLE_PREFIX="Zero-dep assessment: "
LABEL=dependencies
LABEL_COLOR=0366d6
ZERO_LABEL=zero-dep
ZERO_LABEL_COLOR=c5def5
HELD_OPEN="<!-- deps-freshness:held"
HELD_CLOSE="-->"
ASSESS_MARK="<!-- deps-freshness:assessment"
REPLACE_STATEMENT="A replacement is a separate change request for this dependency. The owner approves it before any code changes."
# The merge rule, the wait caps, and the commit identity of the autofix (the shared steps of deps.common.sh).
WAIT_CAP_SECS=1800
POLL_SECS=30
MERGE_MODE=wait
VERDICT_WORKFLOW=.github/workflows/verdict.yml
RELEASE_PLEASE_WORKFLOW=release-please.yml
RELEASE_TOKEN_VAR=RELEASE_TOKEN
COMMIT_NAME="github-actions[bot]"
COMMIT_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"
# The variables that the claude -p process and the verdict of the script may see: the allowlist of the autofix.
ENV_ALLOW="PATH HOME USER LOGNAME LANG LC_ALL TERM TMPDIR SHELL CI CARGO_HOME RUSTUP_HOME RUSTUP_TOOLCHAIN CARGO_TERM_COLOR MISE_DATA_DIR MISE_CONFIG_DIR MISE_CACHE_DIR XDG_CONFIG_HOME XDG_CACHE_HOME XDG_DATA_HOME"
ALWAYS_PATHS="Cargo.toml Cargo.lock"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# The shared steps: GitHub calls, redaction, branch, push, pull request, merge rule, release pull request step.
# shellcheck source=deps.common.sh
. "$ROOT/.agents/scripts/deps.common.sh"

act=""; human=0; json=0; report=""; dry_run=0; retry_failed=0; repo=""; run_url=""; fixtures=""
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }
need_value() { [[ $1 -ge 2 ]] || die "$2 needs a value. Fix: run --help for the flags. Verify: deps.freshness.sh --help." 2; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    scan|update|assess) [[ -z "$act" ]] || die "two actions, $act and $1. Fix: pass one of scan, update, assess. Verify: deps.freshness.sh scan --dry-run." 2; act="$1"; shift ;;
    --repo) need_value $# "$1"; repo="$2"; shift 2 ;;
    --human) human=1; json=0; shift ;;
    --json) json=1; human=0; shift ;;
    --report) need_value $# "$1"; report="$2"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    --retry-failed) retry_failed=1; shift ;;
    --run-url) need_value $# "$1"; run_url="$2"; shift 2 ;;
    --fixtures) need_value $# "$1"; fixtures="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the actions and the flags. Verify: deps.freshness.sh scan prints a summary line." 2 ;;
  esac
done
[[ -n "$act" ]] || die "no action. Fix: pass one of scan, update, assess. Verify: deps.freshness.sh scan prints a summary line." 2
[[ "$act" == scan && $retry_failed -eq 1 ]] && die "--retry-failed is a flag of the action update. Fix: run deps.freshness.sh update --retry-failed. Verify: deps.freshness.sh --help." 2
if [[ -n "$report" ]]; then
  case "$report" in /*) ;; *) report="$PWD/$report" ;; esac
  [[ -d "$(dirname "$report")" ]] || die "the directory of --report $report does not exist. Fix: create it or pass another path. Verify: ls $(dirname "$report")." 2
fi

cd "$ROOT" || die "cannot enter $ROOT. Fix: run the script from its checkout. Verify: re-run." 2
claude_update_timeout=$UPDATE_TIMEOUT_SECS; claude_assess_timeout=$ASSESS_TIMEOUT_SECS; interval_ms=$REGISTRY_INTERVAL_MS
smoke_timeout=$SMOKE_TIMEOUT_SECS; job_elapsed_extra=0; now_iso=""
if [[ -n "$fixtures" ]]; then
  [[ -d "$fixtures" ]] || die "the fixture directory $fixtures does not exist. Fix: pass --fixtures <dir> with the recorded answers. Verify: ls $fixtures." 2
  fixtures="$(cd "$fixtures" && pwd)"
  export DEPS_FRESHNESS_FIXTURES="$fixtures"
  POLL_SECS=0; WAIT_CAP_SECS=2
  [[ -s "$fixtures/now" ]] && now_iso="$(tr -d ' \n' <"$fixtures/now")"
  fx_num() { local v; [[ -s "$fixtures/$1" ]] && v="$(tr -d ' \n' <"$fixtures/$1")" && [[ "$v" =~ ^[0-9]+$ ]] && echo "$v"; }
  v="$(fx_num registry_interval_ms)" && [[ "$v" -lt $REGISTRY_INTERVAL_MS ]] && interval_ms="$v"
  v="$(fx_num claude_timeout_secs)" && { [[ "$v" -lt $claude_update_timeout ]] && claude_update_timeout="$v"; [[ "$v" -lt $claude_assess_timeout ]] && claude_assess_timeout="$v"; }
  v="$(fx_num smoke_timeout_secs)" && [[ "$v" -lt $SMOKE_TIMEOUT_SECS ]] && smoke_timeout="$v"
  v="$(fx_num job_elapsed_min)" && job_elapsed_extra="$v"
  case "$(git remote get-url origin 2>/dev/null)" in
    http*|git@*|ssh:*) die "the fixture mode needs a local git remote, so the run uses no network. Fix: point origin of the scratch repository to a local bare repository. Verify: git remote get-url origin." 2 ;;
  esac
fi
now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
sleep_ms() { [[ "$1" -gt 0 ]] && perl -e 'select(undef, undef, undef, $ARGV[0] / 1000)' "$1"; return 0; }
[[ -n "$now_iso" ]] || now_iso="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
now_epoch="$(jq -nr --arg t "$now_iso" '$t[0:19] + "Z" | fromdateiso8601' 2>/dev/null)" || die "the scan time $now_iso is not an ISO 8601 UTC time. Fix: write it as YYYY-MM-DDTHH:MM:SSZ. Verify: cat the now file." 2
today="${now_iso:0:10}"
branch="$BRANCH_PREFIX$today"
start_s="$(date +%s)"
WAIT_CAP_MIN=$((WAIT_CAP_SECS / 60))

# ---- (1) precondition of update: the verdict workflow in the tree ----------------------------------------------
if [[ "$act" == update ]]; then
  [[ -f "$ROOT/$VERDICT_WORKFLOW" ]] || die "the verdict workflow $VERDICT_WORKFLOW is absent from the tree, so no update can be judged; the run stops before any API write and any claude -p run. Fix: restore $VERDICT_WORKFLOW (docs/specification/operations.md, Hosted workflows). Verify: ls $VERDICT_WORKFLOW." 5
fi

# ---- (2) tools and credentials ---------------------------------------------------------------------------------
tools="curl gh git jq perl"; real_run=0
[[ "$act" != scan && $dry_run -eq 0 ]] && real_run=1
[[ $real_run -eq 1 ]] && tools="$tools claude"
[[ $real_run -eq 1 && "$act" == update ]] && tools="$tools cargo"
for t in $tools; do
  command -v "$t" >/dev/null 2>&1 || die "$t is not installed, so deps.freshness.sh $act cannot run. Fix: install $t (the workflow installs claude from .tool-versions with mise; curl, gh, git, jq, and perl are on the runner; cargo with rustup). Verify: $t --version." 3
done
release_token="${!RELEASE_TOKEN_VAR:-}"
if [[ $real_run -eq 1 ]]; then
  [[ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}" ]] || die "CLAUDE_CODE_OAUTH_TOKEN is empty, so no claude -p run can start. Fix: create the repository secret CLAUDE_CODE_OAUTH_TOKEN with claude setup-token and give it to the step of this script (docs/specification/dependencies.md, Tokens), or run with --dry-run. Verify: deps.freshness.sh $act --dry-run." 3
  if [[ "$act" == update ]]; then
    [[ -n "$release_token" ]] || die "$RELEASE_TOKEN_VAR is empty, so no push or merge can start the verdict workflow. Fix: give the step of the job update the secret RELEASE_PLEASE_TOKEN as $RELEASE_TOKEN_VAR (docs/specification/dependencies.md, Tokens), or run with --dry-run. Verify: deps.freshness.sh update --dry-run." 3
  fi
  if git config --get-regexp '^http\..*\.extraheader$' >/dev/null 2>&1; then
    die "the git configuration of the checkout holds an authorization header, so a claude -p run could read a GitHub token. Fix: check out with persist-credentials: false, or remove the http.*.extraheader entries. Reference: docs/specification/dependencies.md, Tokens. Verify: git config --get-regexp extraheader prints nothing." 3
  fi
  [[ -z "$(git status --porcelain 2>/dev/null)" ]] || die "the work tree has uncommitted changes, so a commit or the tree check of an assessment could take them. Fix: run on a clean checkout, or run with --dry-run. Verify: git status --porcelain prints nothing." 2
fi
if [[ -z "${GH_TOKEN:-}" ]] && ! gh auth token >/dev/null 2>&1 && [[ -z "$fixtures" ]]; then
  die "GH_TOKEN is empty and gh is not logged in, so the tags and releases of the actions cannot be read. Fix: pass the workflow token as GH_TOKEN, or run gh auth login. Verify: gh api rate_limit." 3
fi
if [[ "$act" != scan ]]; then
  [[ -n "$repo" ]] || repo="$(git remote get-url origin 2>/dev/null | sed -nE 's#^(https://github\.com/|git@github\.com:)([^/]+/[^/.]+)(\.git)?$#\2#p')"
  [[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "the repository '${repo:-none}' is not OWNER/NAME. Fix: pass --repo OWNER/NAME. Verify: re-run with --dry-run." 2
fi
owner="${repo%%/*}"

work="$(mktemp -d "${TMPDIR:-/tmp}/deps.freshness.XXXXXX")" || die "mktemp failed. Fix: check TMPDIR. Verify: re-run." 2
claude_pid=""; serve_pid=""
cleanup() { [[ -n "$claude_pid" ]] && kill "$claude_pid" 2>/dev/null; [[ -n "$serve_pid" ]] && kill "$serve_pid" 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT
mkdir -p "$work/reg"
: >"$work/items.jsonl"; : >"$work/actions.jsonl"; : >"$work/runs.jsonl"; : >"$work/closeprs"; : >"$work/failures.jsonl"; : >"$work/held.new"

# ---- output ------------------------------------------------------------------------------------------------------
n_items=0; n_updated=0; n_breaks=0; n_failed=0; n_held=0; n_waiting=0; n_assessed=0; claude_runs=0; auth_failed=0
spent_usd=0; n_unknown=0; visibility=unknown; mode=run; [[ $dry_run -eq 1 ]] && mode=dry-run
say() { [[ $json -eq 1 ]] || printf '%s\n' "$1"; }
# item <key> <path> <package> <ecosystem> <status> [reason] [extra key: value text]: one item line, kept for the report.
item() {
  local key="$1" path="$2" pkg="$3" eco="$4" status="$5" reason="${6:-}" extra="${7:-}"
  n_items=$((n_items + 1))
  case "$status" in updated|merged) n_updated=$((n_updated + 1)) ;; breaks) n_breaks=$((n_breaks + 1)) ;; failed) n_failed=$((n_failed + 1)) ;;
    held) n_held=$((n_held + 1)) ;; waiting) n_waiting=$((n_waiting + 1)) ;; assessed) n_assessed=$((n_assessed + 1)) ;; esac
  if [[ $human -eq 1 ]]; then say "$key ($path, $pkg, $eco): $status${reason:+, $reason}${extra:+ [$extra]}"
  else say "item: $key path: $path package: $pkg ecosystem: $eco status: $status${reason:+ reason: $reason}${extra:+ $extra}"; fi
  jq -nc --arg k "$key" --arg p "$path" --arg n "$pkg" --arg e "$eco" --arg s "$status" --arg r "$reason" --arg x "$(redact <<<"$extra")" \
    '{key: $k, path: $p, package: $n, ecosystem: $e, status: $s, reason: (if $r == "" then null else $r end), fields: $x}' >>"$work/items.jsonl"
}
action() {
  local kind="$1" target="$2" status="$3" detail="${4:-}"
  if [[ $human -eq 1 ]]; then say "  $kind $target: $status${detail:+ ($detail)}"; else
    say "action: $kind target: $target status: $status${detail:+ detail: $detail}"; fi
  jq -nc --arg k "$kind" --arg t "$target" --arg s "$status" --arg d "$(redact <<<"$detail")" '{action: $k, target: $t, status: $s, detail: $d}' >>"$work/actions.jsonl"
}
api_fail() { echo "error: the GitHub API call $1 failed: $(redact <"$work/err" | head -c 300 | tr '\n' ' '). Fix: check the token permissions and the network, then re-run. Reference: docs/specification/dependencies.md, Tokens. Verify: gh api $1." >&2; finish 4; }
summary_line() { jq -r '"summary: fresh: \(.fresh) stale: \(.stale) aged: \(.aged) unknown: \(.unknown) aged_behind: \(.aged_behind) total: \(.total) scan_secs: \(.scan_secs)"' "$work/summary.json" 2>/dev/null; }
write_report() {
  local code="$1"
  jq -n --arg action "$act" --arg mode "$mode" --arg repo "$repo" --arg now "$now_iso" --arg run "$run_url" --argjson code "$code" \
    --argjson fresh "$FRESH_DAYS" --argjson aged "$AGED_DAYS" --argjson spent "$spent_usd" --argjson runs "$claude_runs" \
    --slurpfile deps "$work/scan.json" --slurpfile sum "$work/summary.json" --slurpfile items "$work/items.jsonl" --slurpfile acts "$work/actions.jsonl" --slurpfile runs "$work/runs.jsonl" \
    '{action: $action, mode: $mode, repo: (if $repo == "" then null else $repo end), scan_time: $now, run_url: (if $run == "" then null else $run end),
      thresholds_days: {fresh: $fresh, aged: $aged}, dependencies: ($deps[0] // []), summary: ($sum[0] // null),
      items: $items, actions: $acts, runs: $runs, result: {claude_runs: $runs, cost_usd: $spent, exit: $code}}' 2>/dev/null | redact
}
finish() {
  local code="$1"
  [[ -s "$work/scan.json" ]] || echo '[]' >"$work/scan.json"
  [[ -s "$work/summary.json" ]] || echo 'null' >"$work/summary.json"
  if [[ "$code" -eq 0 ]]; then
    [[ $n_failed -gt 0 || $n_breaks -gt 0 || $n_held -gt 0 ]] && code=1
    [[ $n_unknown -gt 0 ]] && code=6
    [[ $auth_failed -eq 1 ]] && code=3
  fi
  local res="result: action: $act items: $n_items updated: $n_updated breaks: $n_breaks failed: $n_failed held: $n_held waiting: $n_waiting assessed: $n_assessed claude_runs: $claude_runs cost_usd: $spent_usd exit: $code"
  if [[ $human -eq 1 ]]; then say "Dependency freshness $act: $n_items items; $n_updated updated, $n_breaks break, $n_failed failed, $n_held held, $n_waiting waiting, $n_assessed assessed; $claude_runs claude -p runs, $spent_usd USD (exit $code)."
  else say "$res"; fi
  if [[ -n "$report" ]]; then write_report "$code" >"$report" || echo "error: the report $report could not be written. Fix: check the path. Verify: ls $report." >&2; fi
  [[ $json -eq 1 ]] && write_report "$code"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    { echo "### Dependency freshness: $act ($mode)"; echo; echo '```'; summary_line; echo "$res"; echo '```'
      [[ -s "$work/items.jsonl" ]] && { echo; jq -r '"- `\(.key)` \(.status)\(if .reason then " (\(.reason))" else "" end)"' "$work/items.jsonl"; }
    } | redact >>"$GITHUB_STEP_SUMMARY" 2>/dev/null || true
  fi
  exit "$code"
}

# ---- versions and dates (jq) ---------------------------------------------------------------------------------------
# vkey orders by semantic version: the build metadata after + is ignored, a pre-release sorts below its release.
JQ_LIB='
def vcore: split("+")[0];
def ispre: vcore | test("-");
def vkey: vcore | ltrimstr("v") | split("-") as $p | ((($p[0] | split(".") | map(tonumber? // 0)) + [0, 0, 0])[0:3]) + [if ($p | length) > 1 then 0 else 1 end];
def fullsemver: test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$");
def ts: if . == null or . == "" then null else (.[0:19] + "Z" | fromdateiso8601) end;
def day: if . == null then null else (todate[0:10]) end;
def age($now): if . == null then null else ((($now - .) / 86400) | floor | if . < 0 then 0 else . end) end;
'

# ---- (3) the scan -------------------------------------------------------------------------------------------------
# The direct cargo dependencies: name and kind; a crate in more than one table is one dependency, kind normal
# before build before dev.
list_cargo() {
  awk '/^\[/ { s = $0; gsub(/[][ ]/, "", s); next }
    (s == "dependencies" || s == "dev-dependencies" || s == "build-dependencies") && /^[A-Za-z0-9_-]+[ ]*=/ {
      n = $0; sub(/[ ]*=.*/, "", n); print n, s; next }
    s ~ /^(dev-|build-)?dependencies\.[A-Za-z0-9_-]+$/ && !seen[s]++ { n = s; sub(/^[^.]*\./, "", n); k = s; sub(/\..*$/, "", k); print n, k }' Cargo.toml \
  | awk '{ r = ($2 == "dependencies") ? 1 : ($2 == "build-dependencies") ? 2 : 3
           if (!($1 in best) || r < best[$1]) best[$1] = r }
         END { for (n in best) print n, (best[n] == 1 ? "normal" : best[n] == 2 ? "build" : "dev") }' | sort
}
# The resolved version of each direct crate: the version that the root package uses in Cargo.lock.
lock_version() {
  awk -v want="$1" -v root="$2" '
    /^\[\[package\]\]/ { name = ""; indeps = 0; next }
    /^name = / { name = $3; gsub(/"/, "", name); next }
    /^version = / { v = $3; gsub(/"/, "", v); if (name == want) vers[++nv] = v; next }
    name == root && /^dependencies = \[/ { indeps = 1; next }
    indeps && /^\]/ { indeps = 0; next }
    indeps { x = $0; gsub(/[",]/, "", x); sub(/^ +/, "", x); n = split(x, p, " "); if (p[1] == want && n >= 2) pinned = p[2] }
    END { if (pinned != "") print pinned; else if (nv == 1) print vers[1]; else print "-" }' Cargo.lock
}
# The distinct external action pins: name, commit SHA, and the tag of the version comment.
list_actions() {
  local f
  for f in .github/workflows/*.yml .github/workflows/*.yaml; do [[ -f "$f" ]] && cat "$f"; done \
    | sed -nE 's/^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]*["'\'']?([^@"'\''[:space:]]+)@([^"'\''[:space:]#]+)["'\'']?[[:space:]]*(#[[:space:]]*([^[:space:]]+))?.*$/\2 \3 \5/p' \
    | awk '$1 !~ /^\.\// && $1 !~ /^docker:/ { if (NF < 3) $3 = "-"; if (!seen[$1 " " $2]++) print $1, $2, $3 }' | sort
}

# The polite rate of one lane: at most one request per interval to its registry. The interval counts from the
# end of the last request, so two requests never start within one interval, whatever the network time.
lane_last=0
throttle() {
  local n w; n="$(now_ms)"; w=$((lane_last + interval_ms - n))
  [[ $w -gt 0 ]] && sleep_ms "$w"
  lane_last="$(now_ms)"
}
backoff() { sleep_ms $((interval_ms * (1 << $1))); }
# fetch_url <url> <out>: one registry GET with the User-Agent; retries a 429, a 5xx, or a network error at most
# RETRIES times with a growing wait. Sets lane_error on failure.
fetch_url() {
  local url="$1" out="$2" try=0 code
  while :; do
    throttle
    code="$(curl -sS -A "$USER_AGENT" -H 'Accept: application/json' --max-time 60 -o "$out" -w '%{http_code}' "$url" 2>"$out.err")" || code="${code:-000}"
    lane_last="$(now_ms)"
    case "$code" in
      200) return 0 ;;
      429|5[0-9][0-9]|000) ;;
      *) lane_error="HTTP $code"; return 1 ;;
    esac
    try=$((try + 1))
    [[ $try -gt $RETRIES ]] && { lane_error="HTTP $code after $RETRIES retries"; return 1; }
    backoff "$try"
  done
}
# gh_lane <endpoint> <out>: one GitHub GET in the lane of the actions, with the same rate and retries.
# Returns 0, 4 for HTTP 404, or 1 with lane_error.
gh_lane() {
  local ep="$1" out="$2" try=0
  while :; do
    throttle
    gh api -H "Accept: application/vnd.github+json" "$ep" </dev/null >"$out" 2>"$out.err" && { lane_last="$(now_ms)"; return 0; }
    lane_last="$(now_ms)"
    grep -q 'HTTP 404' "$out.err" && return 4
    if ! grep -qiE 'HTTP (429|5[0-9][0-9])|rate limit|timeout|connection' "$out.err"; then lane_error="$(redact <"$out.err" | head -c 200 | tr '\n' ' ')"; return 1; fi
    try=$((try + 1))
    [[ $try -gt $RETRIES ]] && { lane_error="$(grep -oE 'HTTP [0-9]+' "$out.err" | head -n 1) after $RETRIES retries"; return 1; }
    backoff "$try"
  done
}
lane_record_error() { jq -nc --arg e "$1" --arg n "$2" --arg k "$3" --arg p "$4" --arg x "$5" '{ecosystem: $e, name: $n, kind: $k, pinned: $p, error: $x}'; }

lane_cargo() {
  local name kind pinned f
  while read -r name kind; do
    pinned="$(lock_version "$name" "$root_pkg")"
    if [[ -z "$pinned" || "$pinned" == - ]]; then lane_record_error cargo "$name" "$kind" "-" "the resolved version is not in Cargo.lock"; continue; fi
    f="$work/reg/cargo.$name.json"
    if fetch_url "https://crates.io/api/v1/crates/$name" "$f"; then
      jq -c --arg name "$name" --arg kind "$kind" --arg pinned "$pinned" "$JQ_LIB"'
        . as $a | [$a.versions[]? | select((.num | ispre | not) and (.yanked != true))] as $rel
        | ($rel | max_by(.num | vkey)) as $l | ([$a.versions[]? | select(.num == $pinned)] | first) as $p
        | {ecosystem: "cargo", name: $name, kind: $kind, pinned: $pinned, pinned_ts: ($p.created_at | ts),
           latest: ($l.num // null), latest_ts: ($l.created_at | ts), yanked: ($p.yanked == true),
           repository: ($a.crate.repository // "")}
        | if $p == null then . + {error: "the pinned version is not in the crates.io answer"}
          elif .latest == null then . + {error: "the crates.io answer has no release"} else . end' "$f" 2>/dev/null \
        || lane_record_error cargo "$name" "$kind" "$pinned" "the crates.io answer is not readable"
    else lane_record_error cargo "$name" "$kind" "$pinned" "crates.io: $lane_error"; fi
  done <"$work/deps.cargo"
}
lane_actions() {
  local name sha tag r f rc resolved pdate tl obj typ
  while read -r name sha tag; do
    r="$(cut -d/ -f1-2 <<<"$name")"; f="$work/reg/gh.$(tr '/' '_' <<<"$name").$sha"
    if [[ "$tag" == - ]]; then lane_record_error github-actions "$name" action "$sha" "the pin has no version comment"; continue; fi
    gh_lane "repos/$r/releases?per_page=100" "$f.releases"; rc=$?
    [[ $rc -eq 4 ]] && { echo '[]' >"$f.releases"; rc=0; }
    [[ $rc -eq 0 ]] || { lane_record_error github-actions "$name" action "$tag" "GitHub releases of $r: $lane_error"; continue; }
    # The pin check: the tag of the comment must resolve to the pinned commit; an annotated tag is followed once.
    resolved=""
    gh_lane "repos/$r/git/ref/tags/$tag" "$f.ref"; rc=$?
    if [[ $rc -eq 0 ]]; then
      typ="$(jq -r '.object.type // empty' "$f.ref")"; obj="$(jq -r '.object.sha // empty' "$f.ref")"
      if [[ "$typ" == tag ]]; then
        gh_lane "repos/$r/git/tags/$obj" "$f.tag"; rc=$?
        [[ $rc -eq 0 ]] && resolved="$(jq -r '.object.sha // empty' "$f.tag")"
      else resolved="$obj"; fi
    fi
    [[ $rc -eq 0 || $rc -eq 4 ]] || { lane_record_error github-actions "$name" action "$tag" "the tag $tag of $r: $lane_error"; continue; }
    # The pinned date: the publish time of the release of the tag, else the committer date of the pinned commit.
    pdate="$(jq -r --arg t "$tag" '[.[] | select(.tag_name == $t and (.draft | not))] | first | .published_at // empty' "$f.releases")"
    if [[ -z "$pdate" ]]; then
      gh_lane "repos/$r/commits/$sha" "$f.commit"; rc=$?
      [[ $rc -eq 0 ]] && pdate="$(jq -r '.commit.committer.date // empty' "$f.commit")"
    fi
    # The latest with no release: the highest full version tag, with the date of its commit.
    tl='null'
    if [[ "$(jq "$JQ_LIB"'[.[] | select((.draft | not) and (.prerelease | not) and (.tag_name | fullsemver))] | length' "$f.releases")" -eq 0 ]]; then
      if gh_lane "repos/$r/tags?per_page=100" "$f.tags"; then
        obj="$(jq -r "$JQ_LIB"'[.[] | select(.name | fullsemver)] | max_by(.name | vkey) | "\(.name) \(.commit.sha)"' "$f.tags" 2>/dev/null)"
        if [[ -n "$obj" && "$obj" != "null null" ]] && gh_lane "repos/$r/commits/${obj#* }" "$f.tagcommit"; then
          tl="$(jq -c --arg t "${obj% *}" '{tag: $t, date: .commit.committer.date}' "$f.tagcommit")"
        fi
      fi
    fi
    jq -c --arg name "$name" --arg tag "$tag" --arg sha "$sha" --arg resolved "$resolved" --arg pdate "$pdate" --argjson tl "$tl" "$JQ_LIB"'
      . as $r | [$r[] | select((.draft | not) and (.prerelease | not) and (.tag_name | fullsemver))] as $rel
      | ($rel | max_by(.tag_name | vkey)) as $l
      | ([$r[] | select(.tag_name == $tag and (.draft | not))] | first) as $p
      | {ecosystem: "github-actions", name: $name, kind: "action", pinned: $tag, sha: $sha,
         pin_check: (if $resolved == $sha then "ok" else "mismatch" end), date_source: (if $p then "release" else "commit" end),
         pinned_ts: ($pdate | ts), latest: ($l.tag_name // $tl.tag // null), latest_ts: (($l.published_at // $tl.date // null) | ts),
         yanked: false, repository: ("https://github.com/" + ($name | split("/")[0:2] | join("/")))}
      | if .pinned_ts == null then . + {error: "no publish date for the tag \($tag)"}
        elif .latest == null then . + {error: "no release and no full version tag"} else . end' "$f.releases" 2>/dev/null \
      || lane_record_error github-actions "$name" action "$tag" "the GitHub answer is not readable"
  done <"$work/deps.actions"
}

scan() {
  local t0 t1
  t0="$(now_ms)"
  root_pkg="$(sed -n '/^\[package\]/,/^\[/ s/^name *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
  list_cargo >"$work/deps.cargo"; list_actions >"$work/deps.actions"
  # Two lanes, one per registry, in parallel; each keeps its own rate.
  lane_cargo >"$work/scan.cargo.jsonl" 2>"$work/lane.cargo.err" &
  local p1=$!
  lane_actions >"$work/scan.actions.jsonl" 2>"$work/lane.actions.err" &
  local p3=$!
  wait "$p1" "$p3"
  t1="$(now_ms)"
  cat "$work/scan.cargo.jsonl" "$work/scan.actions.jsonl" \
    | jq -s -c --argjson now "$now_epoch" --argjson fresh "$FRESH_DAYS" --argjson aged "$AGED_DAYS" "$JQ_LIB"'
      map(. + {pinned_date: (.pinned_ts | day), pinned_age_days: (.pinned_ts | age($now)), latest_date: (.latest_ts | day),
               latest_age_days: (.latest_ts | age($now))}
        | .behind = (if .error then false else ((.pinned | vkey) < (.latest | vkey)) end)
        | .class = (if .error then "unknown" elif .pin_check == "mismatch" then "stale" elif .yanked then "stale"
                    elif .latest_age_days > $aged then "aged" elif .pinned_age_days > $fresh and .behind then "stale" else "fresh" end))
      | sort_by([({"cargo": 0, "github-actions": 2}[.ecosystem]), .name, .pinned])' >"$work/scan.json"
  jq --argjson secs "$(( (t1 - t0 + 500) / 1000 ))" '{fresh: map(select(.class == "fresh")) | length, stale: map(select(.class == "stale")) | length,
      aged: map(select(.class == "aged")) | length, unknown: map(select(.class == "unknown")) | length,
      aged_behind: map(select(.class == "aged" and .behind)) | length, total: length, scan_secs: $secs}' "$work/scan.json" >"$work/summary.json"
  n_unknown="$(jq .unknown "$work/summary.json")"
  jq -r '.[] | select(.error) | "error: the lookup of \(.ecosystem) \(.name) failed: \(.error). The class is unknown and the dependency gets no claude -p run. Fix: check the network and the registry status, then re-run. Verify: deps.freshness.sh scan."' "$work/scan.json" | redact >&2
  if [[ $human -eq 1 ]]; then
    { echo "ECOSYSTEM NAME KIND PINNED PINNED_AGE LATEST LATEST_AGE BEHIND CLASS YANKED PIN_CHECK"
      jq -r '.[] | [.ecosystem, .name, .kind, .pinned, (.pinned_age_days // "-"), (.latest // "-"), (.latest_age_days // "-"),
        (if .behind then "yes" else "no" end), .class, (if .yanked then "yes" else "no" end), (.pin_check // "-")] | map(tostring) | join(" ")' "$work/scan.json"
    } | column -t | while IFS= read -r l; do say "$l"; done
    say "$(jq -r '"Freshness: \(.fresh) fresh, \(.stale) stale, \(.aged) aged (\(.aged_behind) behind), \(.unknown) unknown, \(.total) dependencies, scanned in \(.scan_secs) seconds."' "$work/summary.json")"
  else
    jq -r '.[] | "ecosystem: \(.ecosystem) name: \(.name) kind: \(.kind) pinned: \(.pinned) pinned_date: \(.pinned_date // "none") pinned_age_days: \(.pinned_age_days // "none") latest: \(.latest // "none") latest_date: \(.latest_date // "none") latest_age_days: \(.latest_age_days // "none") behind: \(if .behind then "yes" else "no" end) class: \(.class) yanked: \(if .yanked then "yes" else "no" end)\(if .ecosystem == "github-actions" then " pin_check: \(.pin_check // "none") date_source: \(.date_source // "none")" else "" end)"' "$work/scan.json" \
      | while IFS= read -r l; do say "$l"; done
    say "$(summary_line)"
  fi
}
scan_line_of() { jq -r --arg e "$1" --arg n "$2" '.[] | select(.ecosystem == $e and .name == $n) | "ecosystem: \(.ecosystem) name: \(.name) kind: \(.kind) pinned: \(.pinned) pinned_date: \(.pinned_date // "none") pinned_age_days: \(.pinned_age_days // "none") latest: \(.latest // "none") latest_date: \(.latest_date // "none") latest_age_days: \(.latest_age_days // "none") behind: \(if .behind then "yes" else "no" end) class: \(.class) yanked: \(if .yanked then "yes" else "no" end)"' "$work/scan.json" | head -n 1; }

scan
[[ "$act" == scan ]] && finish 0

# ---- the repository ---------------------------------------------------------------------------------------------------
get "repos/$repo" "$work/repo.json" || api_fail "repos/$repo"
visibility="$(jq -r 'if .private == true then "private" elif .private == false then "public" else "unknown" end' "$work/repo.json")"
if [[ $human -eq 1 ]]; then say "Repository $repo ($visibility), $mode, action $act."; else say "repo: $repo visibility: $visibility mode: $mode action: $act"; fi

# ---- the claude -p runs (shared by update and assess) -------------------------------------------------------------
# clean_env [--exec] [NAME=value...] -- command...: runs the command with only the allowlisted variables and the
# given pairs; --exec replaces the shell, so the process ID is the one of the command.
clean_env() {
  local args=() v ex=0
  [[ "$1" == --exec ]] && { ex=1; shift; }
  for v in $ENV_ALLOW; do [[ -n "${!v:-}" ]] && args+=("$v=${!v}"); done
  while [[ "$1" != -- ]]; do args+=("$1"); shift; done; shift
  if [[ $ex -eq 1 ]]; then exec env -i "${args[@]}" "$@"; else env -i "${args[@]}" "$@"; fi
}
# render <template> NAME=value...: replaces each {{name}} literally; the values reach awk through its environment.
render() {
  local tpl="$1" kv names=""; shift
  for kv in "$@"; do export "DEPS_F_${kv%%=*}=${kv#*=}"; names="$names ${kv%%=*}"; done
  DEPS_F_NAMES="$names" awk 'BEGIN { n = split(ENVIRON["DEPS_F_NAMES"], k, " ") }
    /^<!-- .* -->$/ { next }
    { out = ""; line = $0
      while ((p = index(line, "{{")) > 0) {
        q = index(substr(line, p + 2), "}}"); if (q == 0) break
        name = substr(line, p + 2, q - 1); val = "{{" name "}}"
        for (i = 1; i <= n; i++) if (k[i] == name) val = ENVIRON["DEPS_F_" name]
        out = out substr(line, 1, p - 1) val; line = substr(line, p + q + 3)
      }
      print out line }' "$tpl"
  for kv in "$@"; do unset "DEPS_F_${kv%%=*}"; done
}
# The model, the four token counts, the turns, the API-equivalent cost from the vendored price file (the base
# fields of cost.md on the totals per model), and total_cost_usd of the result, as one JSON object.
usage_json() {
  jq -c --slurpfile p "$ROOT/vendor/litellm/model_prices_and_context_window.json" '
    def price($m): ($p[0][$m] // $p[0]["anthropic/" + $m] // $p[0][$m | sub("-[0-9]{8}$"; "")] // null)
      | if . != null and .litellm_provider == "anthropic" then . else null end;
    def cost: [(.modelUsage // {}) | to_entries[] | price(.key) as $q
      | if $q == null then null else
        (.value.inputTokens // 0) * ($q.input_cost_per_token // 0) + (.value.outputTokens // 0) * ($q.output_cost_per_token // 0)
        + (.value.cacheReadInputTokens // 0) * ($q.cache_read_input_token_cost // 0)
        + (.value.cacheCreationInputTokens // 0) * ($q.cache_creation_input_token_cost // 0) end]
      | if length == 0 then 0 elif any(. == null) then null else add end;
    {model: ((.modelUsage // {}) | to_entries | sort_by(-(.value.costUSD // 0)) | (first | .key) // "unknown"),
     input_tokens: (.usage.input_tokens // 0), cache_creation_input_tokens: (.usage.cache_creation_input_tokens // 0),
     cache_read_input_tokens: (.usage.cache_read_input_tokens // 0), output_tokens: (.usage.output_tokens // 0),
     turns: (.num_turns // 0), cost_usd: (cost | if . == null then "unpriced" else (. * 1000000 | round / 1000000) end),
     total_cost_usd: (.total_cost_usd // 0)}' "$1" 2>/dev/null || echo '{"model": "unknown"}'
}
usage_text() { jq -r --arg s "$2" '"model: \(.model) input_tokens: \(.input_tokens // 0) cache_creation_input_tokens: \(.cache_creation_input_tokens // 0) cache_read_input_tokens: \(.cache_read_input_tokens // 0) output_tokens: \(.output_tokens // 0) turns: \(.turns // 0) run_secs: \($s) cost_usd: \(.cost_usd // 0) total_cost_usd: \(.total_cost_usd // 0)"' <<<"$1"; }
# The last JSON object of the result text that passes the filter: each line, then the whole text.
structured_of() {
  jq -c --arg f "$2" '.result // "" | ([split("\n")[] | fromjson? // empty] + [fromjson? // empty]
    + [scan("\\{(?:[^{}\\[\\]]|\\[[^\\]]*\\])*\\}") | fromjson? // empty]) | map(select(type == "object")) as $all
    | ($all | map(select(if $f == "update" then
        (.status == "updated" or .status == "breaks" or .status == "failed") and (.name | type) == "string"
        and (.from_version | type) == "string" and (.to_version | type) == "string" and (.also_raised | type) == "array"
        and (.files_changed | type) == "array" and (.code_changed | type) == "boolean" and (.verdict | type) == "string"
        and has("failing_stage") and has("first_error") and (.reason | type) == "string"
      else
        (.name | type) == "string" and (.symbols | type) == "array" and has("call_sites")
        and (.replacement_lines | type) == "number" and (.risks | type) == "array"
        and (.recommendation == "keep" or .recommendation == "replace" or .recommendation == "watch") and (.reason | type) == "string"
      end)) | last) // empty' "$1" 2>/dev/null
}
# run_claude <update|assess> <prompt file> <log>: one claude -p run with the limits of its kind; sets run_rc,
# run_out, run_usage (JSON), run_secs, run_reason (timeout, claude_auth, or empty), and adds its cost.
run_claude() {
  local kind="$1" prompt="$2" log="$3" marker="$work/claude.timeout" timeout s t0
  run_out="$work/claude.out"; rm -f "$marker" "$run_out"; run_reason=""
  claude_runs=$((claude_runs + 1)); t0="$(date +%s)"
  if [[ "$kind" == update ]]; then
    timeout=$claude_update_timeout
    ( clean_env --exec CLAUDE_CODE_OAUTH_TOKEN="$CLAUDE_CODE_OAUTH_TOKEN" BASH_DEFAULT_TIMEOUT_MS="$BASH_TIMEOUT_MS" BASH_MAX_TIMEOUT_MS="$BASH_TIMEOUT_MS" -- \
        claude -p --output-format json --model "$MODEL" --max-turns "$MAX_TURNS" --max-budget-usd "$RUN_BUDGET_USD" \
        --no-session-persistence \
        --allowedTools Read Edit Write Grep Glob "Bash(cargo:*)" \
          "Bash(git diff:*)" "Bash(git status:*)" "Bash(git log:*)" "Bash(.agents/scripts/pipeline.verdict.sh:*)" \
          "Bash(.agents/scripts/pipeline.format.sh:*)" "Bash(.agents/scripts/pipeline.lint.sh:*)" "Bash(.agents/scripts/pipeline.test.sh:*)" \
          "Bash(.agents/scripts/pipeline.web.sh:*)" "Bash(.agents/scripts/pipeline.image.sh:*)" \
        --disallowedTools WebFetch WebSearch "Bash(git commit:*)" "Bash(git push:*)" "Bash(gh:*)" "Bash(curl:*)" "Bash(env:*)" "Bash(printenv:*)" \
        <"$prompt" >"$run_out" 2>>"$log" ) &
  else
    timeout=$claude_assess_timeout
    ( clean_env --exec CLAUDE_CODE_OAUTH_TOKEN="$CLAUDE_CODE_OAUTH_TOKEN" -- \
        claude -p --output-format json --model "$MODEL" --max-turns "$MAX_TURNS" --max-budget-usd "$RUN_BUDGET_USD" \
        --no-session-persistence \
        --allowedTools Read Grep Glob \
        --disallowedTools Bash Edit Write NotebookEdit WebFetch WebSearch Task \
        <"$prompt" >"$run_out" 2>>"$log" ) &
  fi
  claude_pid=$!
  ( sleep "$timeout" & s=$!; trap 'kill $s 2>/dev/null; exit 0' TERM; wait $s; touch "$marker"; kill -TERM "$claude_pid" 2>/dev/null ) &
  local wd=$!
  wait "$claude_pid"; run_rc=$?; claude_pid=""
  kill -TERM "$wd" 2>/dev/null; wait "$wd" 2>/dev/null
  run_secs=$(( $(date +%s) - t0 ))
  jq -r '.result // empty' "$run_out" >>"$log" 2>/dev/null
  run_usage="$(usage_json "$run_out")"
  s="$(jq -r '.total_cost_usd // 0' "$run_out" 2>/dev/null || echo 0)"
  spent_usd="$(jq -nr --argjson a "$spent_usd" --argjson b "${s:-0}" '$a + $b' 2>/dev/null || echo "$spent_usd")"
  if [[ -e "$marker" ]]; then run_reason=timeout
  elif jq -e '(.api_error_status == 401) or ((.is_error == true) and ((.result // "") | test("authentication_error|invalid api key|oauth token|please run /login"; "i")))' "$run_out" >/dev/null 2>&1; then
    run_reason=claude_auth; auth_failed=1
  fi
}
over_budget() { jq -ne --argjson a "$spent_usd" --argjson c "$1" '$a >= $c' >/dev/null; }

# ---- issues ---------------------------------------------------------------------------------------------------------
ensure_label() {
  get "repos/$repo/labels/$1" "$work/label.json" && return 0
  write_issue --method POST "repos/$repo/labels" -f name="$1" -f color="$2" -f description="$3" >/dev/null \
    && action label "$1" done "made" || action label "$1" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
}

if [[ "$act" == update ]]; then
  # ---- (4) update: the verdict workflow, the open pull requests, and the breaking updates issue -------------------
  get "repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")" "$work/wf.json" || {
    grep -q 'HTTP 404' "$work/err" && die "the verdict workflow $VERDICT_WORKFLOW is not known on $repo, so no update can be judged; the run stops before any API write and any claude -p run. Fix: push $VERDICT_WORKFLOW to the default branch. Verify: gh api repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")." 5
    api_fail "repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW")"; }
  wf_state="$(jq -r '.state // "unknown"' "$work/wf.json")"
  [[ "$wf_state" == active ]] || die "the verdict workflow $VERDICT_WORKFLOW is $wf_state on $repo, not active, so no update can be judged; the run stops before any API write and any claude -p run. Fix: gh workflow enable verdict.yml. Verify: gh api repos/$repo/actions/workflows/$(basename "$VERDICT_WORKFLOW") --jq .state prints active." 5
  get "repos/$repo/pulls?state=open&per_page=100" "$work/pulls.json" || api_fail "repos/$repo/pulls"
  get "repos/$repo/issues?state=open&labels=$LABEL&per_page=100" "$work/issues.json" || api_fail "repos/$repo/issues"
  issue_number="$(jq -r --arg t "$BREAK_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .number // empty' "$work/issues.json")"
  issue_url="$(jq -r --arg t "$BREAK_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .html_url // empty' "$work/issues.json")"
  jq -r --arg t "$BREAK_TITLE" '[.[] | select(.title == $t and (.pull_request | not))] | first | .body // ""' "$work/issues.json" \
    | awk -v o="$HELD_OPEN" -v c="$HELD_CLOSE" '$0 == o {f = 1; next} f && $0 == c {f = 0} f && NF {print}' >"$work/held.old"
  nothing_open=0; merged_fix_sha=""; planned_fix_merge=0; planned_records=0; branch_ready=0; main_ref=HEAD
  git rev-parse --verify -q origin/main >/dev/null && main_ref=origin/main

  # The update items: stale cargo dependencies, and aged ones behind their latest; groups normal and build,
  # then dev; the oldest pinned version first. The scan has no npm lane, so no npm item reaches this list.
  jq -c '[.[] | select(.ecosystem == "cargo" and (.class == "stale" or (.class == "aged" and .behind)))
    | . + {group: (if .kind == "dev" then 2 else 1 end)}] | sort_by([.group, .pinned_ts]) | .[]' "$work/scan.json" >"$work/updates.jsonl"

  # held_reason <key> <latest>: the reason and the date of a held breaking item, or nothing.
  held_reason() {
    [[ $retry_failed -eq 1 ]] && return 0
    awk -v k="$1" -v l="latest:$2" '$1 == k && $2 == l {print $3, $4; exit}' "$work/held.old" | while read -r d r; do
      days="$(jq -nr --arg a "${d#date:}" --arg b "$today" '((($b | strptime("%Y-%m-%d") | mktime) - ($a | strptime("%Y-%m-%d") | mktime)) / 86400) | floor' 2>/dev/null || echo 99)"
      [[ "$days" -lt $HOLD_DAYS ]] && echo "${r#reason:} ${d#date:}"
    done
  }
  # record_failure <key> <latest or input> <reason> <package> <log file> <result json>: a failed item of the shared
  # steps or of an update run, for the issue and its machine block.
  record_failure() {
    echo "$1 latest:${2#*=} date:$today reason:$3" >>"$work/held.new"
    jq -nc --arg k "$1" --arg r "$3" --arg p "$4" --arg l "${5:-}" --arg res "${6:-}" '{key: $k, reason: $r, package: $p, log: $l, result: $res}' >>"$work/failures.jsonl"
  }
  # record_break <key> <status> <reason> <pinned> <latest> <check> <stage> <first error> <result json> <log>
  record_break() {
    echo "$1 latest:$5 date:$today reason:$3" >>"$work/held.new"
    jq -nc --arg k "$1" --arg s "$2" --arg r "$3" --arg pin "$4" --arg lat "$5" --arg c "$6" --arg st "$7" --arg fe "$8" --arg res "$9" --arg l "${10}" \
      '{key: $k, status: $s, reason: $r, pinned: $pin, latest: $lat, check: $c, stage: $st, first_error: $fe, result: $res, log: $l}' >>"$work/failures.jsonl"
  }

  # ---- the checks of the script ------------------------------------------------------------------------------------
  direct_names() { list_cargo | awk '{print "cargo:" $1}' | sort -u; }
  direct_pins() {
    root_pkg="$(sed -n '/^\[package\]/,/^\[/ s/^name *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
    list_cargo | while read -r n k; do echo "cargo:$n $(lock_version "$n" "$root_pkg")"; done
  }
  count_tests() { { grep -rhoE '#\[(tokio::)?test(\(|\])' src tests 2>/dev/null || true; } | wc -l | tr -d ' '; }
  run_verdict() { clean_env -- "$ROOT/.agents/scripts/pipeline.verdict.sh" >"$work/verdict.out" 2>&1 </dev/null; local rc=$?; cat "$work/verdict.out" >>"$1"; return $rc; }
  # first_error_of <stage>: the first error line of the stage log, else of the verdict output, redacted.
  first_error_of() {
    local l=""
    [[ -n "$1" && -f "$ROOT/.agents/logs/pipeline.$1.log" ]] && l="$(grep -m 1 -E '^error|error(\[[A-Z0-9]+\])?:|FAIL|panicked' "$ROOT/.agents/logs/pipeline.$1.log" 2>/dev/null)"
    [[ -n "$l" ]] || l="$(grep -m 1 -vE '^(stage|verdict):' "$work/verdict.out" 2>/dev/null | grep -m 1 -iE 'error|fail|panicked')"
    [[ -n "$l" ]] || l="$(grep -m 1 -E '^verdict:' "$work/verdict.out" 2>/dev/null)"
    redact <<<"$l" | head -c 300 | tr '\n' ' ' | sed 's/ *$//'
  }
  # smoke_check <log>: the binary check of release.md on one host with the debug build: --version, the line
  # listening, GET /healthz with 200 and {"status":"ok"}, and on SIGTERM the line "database closed, exit" with
  # exit code 0. Sets smoke_error.
  smoke_check() {
    local log="$1" bin="$ROOT/target/debug/loupe" d key k addr body rc
    smoke_error=""
    clean_env -- cargo build --locked >>"$log" 2>&1 </dev/null || { smoke_error="cargo build --locked failed"; return 1; }
    [[ -x "$bin" ]] || { smoke_error="no binary at target/debug/loupe"; return 1; }
    "$bin" --version >>"$log" 2>&1 </dev/null || { smoke_error="loupe --version failed"; return 1; }
    key="$("$bin" gen-key 2>>"$log" </dev/null | tr -d '\r\n')"
    [[ -n "$key" ]] || { smoke_error="loupe gen-key printed no key"; return 1; }
    d="$(mktemp -d "$work/smoke.XXXXXX")"
    ( clean_env --exec LOUPE_DATA_DIR="$d" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC LOUPE_LISTEN=127.0.0.1:0 DO_NOT_TRACK=1 -- \
      "$bin" serve >"$d/serve.out" 2>"$d/serve.err" </dev/null ) &
    serve_pid=$!
    k=0; addr=""
    while [[ $k -lt $((smoke_timeout * 4)) ]]; do
      addr="$(grep -m 1 'listening' "$d/serve.err" "$d/serve.out" 2>/dev/null | grep -oE '127\.0\.0\.1:[0-9]+' | head -n 1)"
      [[ -n "$addr" ]] && break
      kill -0 "$serve_pid" 2>/dev/null || break
      sleep_ms 250; k=$((k + 1))
    done
    if [[ -z "$addr" ]]; then smoke_error="loupe serve wrote no line listening"; kill -TERM "$serve_pid" 2>/dev/null; wait "$serve_pid" 2>/dev/null; serve_pid=""; cat "$d/serve.err" >>"$log"; return 1; fi
    body="$(curl -sS --max-time 10 -w '\n%{http_code}' "http://$addr/healthz" 2>>"$log")"
    if [[ "$(tail -n 1 <<<"$body")" != 200 || "$(sed '$d' <<<"$body" | tr -d ' \n')" != '{"status":"ok"}' ]]; then
      smoke_error="GET /healthz did not answer 200 with {\"status\":\"ok\"}"; kill -TERM "$serve_pid" 2>/dev/null; wait "$serve_pid" 2>/dev/null; serve_pid=""; return 1
    fi
    kill -TERM "$serve_pid" 2>/dev/null; wait "$serve_pid"; rc=$?; serve_pid=""
    cat "$d/serve.err" >>"$log"
    [[ $rc -eq 0 ]] || { smoke_error="loupe serve exited with code $rc after SIGTERM"; return 1; }
    grep -q 'database closed, exit' "$d/serve.err" "$d/serve.out" || { smoke_error="no line 'database closed, exit' after SIGTERM"; return 1; }
    return 0
  }

  # update_item <item json> <log>: one claude -p run and the checks; sets up_status (updated|breaks|failed),
  # up_reason, up_check, up_stage, up_error, up_result, up_to, up_raised (lines "ecosystem:name version").
  update_item() {
    local it="$1" log="$2" last_good eco name pinned latest kind manifest changed p code_changed=0 cargo_changed=0
    local tests_before deps_before pins_before type summary body cc
    eco="$(jq -r .ecosystem <<<"$it")"; name="$(jq -r .name <<<"$it")"; pinned="$(jq -r .pinned <<<"$it")"
    latest="$(jq -r .latest <<<"$it")"; kind="$(jq -r .kind <<<"$it")"
    manifest=Cargo.toml
    up_status=failed; up_reason=""; up_check=""; up_stage=""; up_error=""; up_result=""; up_to=""; up_raised=""
    last_good="$(git rev-parse HEAD)"
    tests_before="$(count_tests)"; direct_names >"$work/names.before"; direct_pins >"$work/pins.before"
    render "$ROOT/.agents/scripts/deps.freshness.update.prompt.md" ecosystem="$eco" name="$name" kind="$kind" \
      pinned_version="$pinned" latest_version="$latest" manifest_path="$manifest" >"$work/prompt.txt"
    run_claude update "$work/prompt.txt" "$log"
    up_reason="$run_reason"
    if [[ -z "$up_reason" ]]; then
      up_result="$(structured_of "$run_out" update)"
      [[ -n "$up_result" ]] || up_reason=invalid_result
    fi
    if [[ -z "$up_reason" ]]; then
      git add -A >/dev/null 2>&1
      changed="$(git diff --cached --name-only "$last_good")"
      [[ -n "$changed" ]] || up_reason=no_change
    fi
    # The guards: allowed paths, no credential, no removed dependency, no removed test.
    if [[ -z "$up_reason" ]]; then
      for p in $changed; do
        case " $ALWAYS_PATHS " in *" $p "*) cargo_changed=1; continue ;; esac
        case "$p" in src/*|tests/*) code_changed=1; cargo_changed=1; continue ;; esac
        up_reason=path_not_allowed; break
      done
    fi
    if [[ -z "$up_reason" ]] && git diff --cached "$last_good" | grep '^+' | grep -v '^+++' | has_credential; then up_reason=credential_in_diff; fi
    if [[ -z "$up_reason" ]]; then
      direct_names >"$work/names.after"
      [[ -z "$(comm -23 "$work/names.before" "$work/names.after")" ]] || up_reason=dependency_removed
    fi
    if [[ -z "$up_reason" ]] && [[ "$(count_tests)" -lt "$tests_before" ]]; then up_reason=tests_removed; fi
    if [[ -z "$up_reason" && $code_changed -eq 1 ]]; then
      # A code change is allowed only when the verdict fails after the version change alone (the autofix rule).
      git diff --cached --binary "$last_good" >"$work/full.patch"
      git reset -q --hard "$last_good"
      git apply --include=Cargo.toml --include=Cargo.lock "$work/full.patch" 2>>"$log"
      echo "verdict: the version change alone" >>"$log"
      if run_verdict "$log"; then up_reason=path_not_allowed; fi
      git reset -q --hard "$last_good"; git clean -fdq
      git apply "$work/full.patch" 2>>"$log" && git add -A >/dev/null 2>&1
    fi
    # The break checks: the verdict of the script, then the smoke check.
    if [[ -z "$up_reason" ]]; then
      echo "verdict: the update" >>"$log"
      if ! run_verdict "$log"; then
        up_status=breaks; up_check=verdict
        up_stage="$(awk '$1 == "stage:" && $4 == "failed" {print $2; exit}' "$work/verdict.out")"; up_error="$(first_error_of "$up_stage")"
      elif [[ $cargo_changed -eq 1 ]] && ! smoke_check "$log"; then
        up_status=breaks; up_check=smoke; up_stage=smoke; up_error="$(redact <<<"$smoke_error")"
      fi
    fi
    if [[ -z "$up_reason" && "$up_status" != breaks ]]; then
      direct_pins >"$work/pins.after"
      up_to="$(awk -v k="$eco:$name" '$1 == k {print $2}' "$work/pins.after")"
      [[ -n "$up_to" && "$up_to" != "$pinned" ]] || up_reason=no_change
    fi
    if [[ -z "$up_reason" && "$up_status" != breaks ]]; then
      up_raised="$(join <(sort "$work/pins.before") <(sort "$work/pins.after") | awk -v k="$eco:$name" '$1 != k && $2 != $3 {print $1, $3}')"
      type=fix; [[ $cargo_changed -eq 0 ]] && type=chore
      cc="$(jq -r '.code_changed' <<<"$up_result")"
      summary="$type(deps): bump $name to $up_to"
      body="pinned: $pinned, $(jq -r '.pinned_age_days' <<<"$it") days old
latest: $latest, $(jq -r '.latest_age_days' <<<"$it") days old"
      [[ -n "$up_raised" ]] && body="$body
$(awk '{n = $1; sub(/^[^:]*:/, "", n); print "also raised: " n " " $2}' <<<"$up_raised")"
      [[ $code_changed -eq 1 || "$cc" == true ]] && body="$body
code changed: yes"
      if git -c user.name="$COMMIT_NAME" -c user.email="$COMMIT_EMAIL" commit -q -m "$summary" -m "$body" >>"$log" 2>&1; then
        up_status=updated
      else up_reason=no_change; fi
    fi
    [[ -n "$up_reason" ]] && up_status=failed
    if [[ "$up_status" != updated ]]; then git reset -q --hard "$last_good"; git clean -fdq; fi
  }

  # ---- (5) the update runs ------------------------------------------------------------------------------------------
  raised=" "; planned_claude=0
  while IFS= read -r it; do
    eco="$(jq -r .ecosystem <<<"$it")"; name="$(jq -r .name <<<"$it")"; key="$eco:$name"
    pinned="$(jq -r .pinned <<<"$it")"; latest="$(jq -r .latest <<<"$it")"
    extra="pinned: $pinned latest: $latest pinned_age_days: $(jq -r .pinned_age_days <<<"$it") class: $(jq -r .class <<<"$it")"
    case "$raised" in *" $key "*) item "$key" update "$name" "$eco" raised also_raised "$extra"; continue ;; esac
    hr="$(held_reason "$key" "$latest")"
    if [[ -n "$hr" ]]; then
      item "$key" update "$name" "$eco" held "${hr% *}" "$extra since: ${hr#* }"
      awk -v k="$key" '$1 == k' "$work/held.old" >>"$work/held.new"; continue
    fi
    if [[ $auth_failed -eq 1 ]]; then item "$key" update "$name" "$eco" waiting claude_auth "$extra"; continue; fi
    elapsed_min=$(( ($(date +%s) - start_s) / 60 + job_elapsed_extra ))
    if [[ $((planned_claude + claude_runs)) -ge $UPDATE_RUN_CAP || $((UPDATE_JOB_MIN - elapsed_min)) -lt $MIN_REMAINING_MIN ]]; then
      item "$key" update "$name" "$eco" waiting run_cap "$extra"; continue; fi
    if over_budget "$UPDATE_JOB_BUDGET_USD"; then item "$key" update "$name" "$eco" waiting budget_cap "$extra"; continue; fi
    type=fix
    manifest=Cargo.toml
    if [[ $dry_run -eq 1 ]]; then
      if [[ $planned_claude -eq 0 ]]; then
        if git ls-remote --exit-code --heads origin "$branch" >/dev/null 2>&1; then action branch "$branch" planned "reuse the branch of an earlier run today"
        else action branch "$branch" planned "from the head of main"; fi
      fi
      planned_claude=$((planned_claude + 1))
      item "$key" update "$name" "$eco" planned "" "$extra plan: claude"
      action claude "$key" planned "ecosystem=$eco name=$name kind=$(jq -r .kind <<<"$it") pinned_version=$pinned latest_version=$latest manifest_path=$manifest model=$MODEL max_turns=$MAX_TURNS budget_usd=$RUN_BUDGET_USD timeout_min=$((claude_update_timeout / 60))"
      action commit "$key" planned "$type(deps): bump $name to $latest"
      [[ "$type" == fix ]] && planned_fix_merge=1
      continue
    fi
    prepare_branch "$branch"
    log="$work/item.$(tr ':/@' '___' <<<"$key").log"; : >"$log"
    update_item "$it" "$log"
    u="$(usage_text "$run_usage" "$run_secs")"
    jq -nc --arg k "$key" --arg s "$up_status" --arg r "$up_reason" --argjson u "$run_usage" --arg res "$up_result" \
      '{key: $k, status: $s, reason: $r, usage: $u, result: ($res | fromjson? // null)}' >>"$work/runs.jsonl"
    case "$up_status" in
      updated) item "$key" update "$name" "$eco" updated "" "$extra to: $up_to $u"
        while read -r rk rv; do [[ -n "$rk" ]] && raised="$raised$rk "; done <<<"$up_raised" ;;
      breaks) item "$key" update "$name" "$eco" breaks "" "$extra check: $up_check stage: ${up_stage:-none} $u"
        record_break "$key" breaks "" "$pinned" "$latest" "$up_check" "$up_stage" "$up_error" "$up_result" "$log" ;;
      *) item "$key" update "$name" "$eco" failed "$up_reason" "$extra $u"
        record_break "$key" failed "$up_reason" "$pinned" "$latest" "" "" "" "$up_result" "$log" ;;
    esac
  done <"$work/updates.jsonl"

  # The stale actions: no claude -p run; the Dependabot path when its pull request is open, else the issue.
  jq -c '.[] | select(.ecosystem == "github-actions" and .class == "stale")' "$work/scan.json" >"$work/stale_actions.jsonl"
  while IFS= read -r it; do
    name="$(jq -r .name <<<"$it")"; key="github-actions:$name"; repo_of="$(cut -d/ -f1-2 <<<"$name")"
    extra="pinned: $(jq -r .pinned <<<"$it") latest: $(jq -r .latest <<<"$it") pin_check: $(jq -r .pin_check <<<"$it")"
    pr="$(jq -r --arg n "$repo_of" '[.[] | select(.user.login == "dependabot[bot]" and (.head.ref | startswith("dependabot/github_actions/"))
      and (((.title // "") + " " + (.body // "")) | contains($n)))] | first | .number // empty' "$work/pulls.json")"
    if [[ -n "$pr" ]]; then item "$key" update "$name" github-actions waiting dependabot_path "$extra pull_request: $pr"
    else
      if [[ $dry_run -eq 1 ]]; then item "$key" update "$name" github-actions planned actions_no_pr "$extra plan: record in the issue, no claude -p run"; planned_records=$((planned_records + 1))
      else item "$key" update "$name" github-actions failed actions_no_pr "$extra"
        jq -nc --arg k "$key" --arg pin "$(jq -r .pinned <<<"$it")" --arg lat "$(jq -r .latest <<<"$it")" \
          '{key: $k, status: "failed", reason: "actions_no_pr", pinned: $pin, latest: $lat, check: "", stage: "", first_error: "", result: "", log: ""}' >>"$work/failures.jsonl"; fi
    fi
  done <"$work/stale_actions.jsonl"
  jq -r '.[] | select(.class == "unknown") | "\(.ecosystem):\(.name) \(.name) \(.ecosystem)"' "$work/scan.json" >"$work/unknown.txt"
  while read -r k n e; do item "$k" update "$n" "$e" unknown unknown "no class and no run"; done <"$work/unknown.txt"

  # ---- (6) push and merge of the freshness branch, then the release pull request step (deps.common.sh) ----------
  if [[ $dry_run -eq 1 ]]; then
    [[ $planned_claude -gt 0 ]] && plan_branch_merge "$branch"
  elif [[ $branch_ready -eq 1 ]] && [[ "$(git rev-list --count origin/main..HEAD)" -gt 0 ]]; then
    git log --reverse --format=%s origin/main..HEAD >"$work/alllines"
    branch_merge "$branch" "$work/alllines" freshness updates update "Scan: $(summary_line)"
  fi
  release_step

  # ---- (7) the breaking updates issue -------------------------------------------------------------------------------
  awk '!s[$1]++' "$work/held.new" >"$work/held.block"
  n_fail="$(grep -c . "$work/failures.jsonl" || true)"; n_block="$(grep -c . "$work/held.block" || true)"
  issue_body="The dependency freshness flow (docs/specification/dependencies.md, Freshness update) records here each update that breaks the app or fails a guard, and each stale action with no Dependabot pull request. A breaking item waits $HOLD_DAYS days, or until its latest version changes, before a new claude -p run; a run with --retry-failed ends the hold. The block below is read by the script.

$HELD_OPEN
$(cat "$work/held.block")
$HELD_CLOSE"
  comment_text() {
    echo "Run: ${run_url:-local run} on $today."
    echo
    echo "Scan: \`$(summary_line)\`"
    jq -c '.' "$work/failures.jsonl" | while IFS= read -r f; do
      echo
      jq -r '"### \(.key)\(if .package then ": \(.package)" else "" end)"' <<<"$f"
      jq -r 'if .pinned then "- Pinned: \(.pinned); latest: \(.latest)" else empty end' <<<"$f"
      jq -r 'if .status == "breaks" then "- Status: `breaks`\n- Failing check: `\(.check)`\n- Failing stage: `\(.stage // "none")`\n- First error: `\(.first_error)`" else "- Status: `failed`\n- Reason: `\(.reason)`" end' <<<"$f"
      [[ "$(jq -r .reason <<<"$f")" == claude_auth ]] && echo "- Fix: run \`claude setup-token\` and replace the secret CLAUDE_CODE_OAUTH_TOKEN (docs/specification/dependencies.md, Tokens)."
      [[ "$(jq -r .reason <<<"$f")" == actions_no_pr ]] && echo "- No claude -p run for a workflow file: Dependabot updates the pins of the actions; check .github/dependabot.yml."
      [[ -n "$(jq -r '.result // ""' <<<"$f")" ]] && echo "- Structured result: \`$(jq -r .result <<<"$f")\`"
      echo "- Run: ${run_url:-local run}"
    done
  } 
  if [[ $dry_run -eq 1 && $planned_records -gt 0 ]]; then
    action issue breaking-updates planned "${issue_number:+comment on #$issue_number}${issue_number:-open \"$BREAK_TITLE\" with the label $LABEL}; $planned_records items to record, and each update run that breaks or fails"
  elif [[ $dry_run -eq 1 ]]; then
    if [[ $planned_claude -gt 0 ]]; then action issue breaking-updates planned "open \"$BREAK_TITLE\" or comment on it for each update run that breaks or fails"
    elif [[ -n "$issue_number" && "$n_block" -eq 0 ]]; then action issue "#$issue_number" planned "comment with the run link and close"; fi
  elif [[ "$n_fail" -gt 0 ]]; then
    comment_text | redact >"$work/comment.md"
    ensure_label "$LABEL" "$LABEL_COLOR" "Pull requests that update a dependency file"
    if [[ -z "$issue_number" ]]; then
      if write_issue --method POST "repos/$repo/issues" -f title="$BREAK_TITLE" -f body="$issue_body" -f "labels[]=$LABEL" >"$work/issue.json"; then
        issue_number="$(jq -r '.number // empty' "$work/issue.json")"; action issue "#$issue_number" done "opened"
      else action issue breaking-updates failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
    else
      write_issue --method PATCH "repos/$repo/issues/$issue_number" -f body="$issue_body" >/dev/null && action issue "#$issue_number" done "block updated" \
        || action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
    fi
    [[ -n "$issue_number" ]] && { write_issue --method POST "repos/$repo/issues/$issue_number/comments" -F body=@"$work/comment.md" >/dev/null \
      && action comment "#$issue_number" done "$n_fail items" || action comment "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; }
  elif [[ -n "$issue_number" ]]; then
    if [[ "$n_block" -gt 0 ]]; then
      if ! cmp -s "$work/held.block" "$work/held.old"; then
        write_issue --method PATCH "repos/$repo/issues/$issue_number" -f body="$issue_body" >/dev/null && action issue "#$issue_number" done "block updated, $n_block held items" \
          || action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
      fi
    elif write_issue --method POST "repos/$repo/issues/$issue_number/comments" -f body="No breaking or failed update in this run. ${run_url:+Run: $run_url}" >/dev/null \
      && write_issue --method PATCH "repos/$repo/issues/$issue_number" -f state=closed >/dev/null; then action issue "#$issue_number" done "closed"
    else action issue "#$issue_number" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
  fi
  finish 0
fi

# ---- (8) assess: the zero-dep assessments of the aged dependencies -------------------------------------------------
# count_use <ecosystem> <name>: the fixed search of the use of a dependency. Prints one JSON object with
# import_lines, symbols (the distinct imported names), call_sites, and files; these counts are the authority.
count_use() {
  local files
  case "$1" in
    cargo) files="$( { find src tests -type f -name '*.rs' 2>/dev/null; [[ -f build.rs ]] && echo build.rs; } | sort)" ;;
    *) files="$(ls .github/workflows/*.yml .github/workflows/*.yaml 2>/dev/null | sort)" ;;
  esac
  # shellcheck disable=SC2086
  DEPS_ECO="$1" DEPS_NAME="$2" perl -e '
    use strict; my ($eco, $name) = ($ENV{DEPS_ECO}, $ENV{DEPS_NAME}); my (%sym, %files); my ($imports, $calls) = (0, 0);
    my $id = $name; $id =~ s/-/_/g; my $q = quotemeta($id); my $nq = quotemeta($name);
    for my $f (@ARGV) {
      open(my $fh, "<", $f) or next; local $/; my $t = <$fh>; close $fh; my $hit = 0;
      if ($eco eq "cargo") {
        my %local;
        while ($t =~ /\b(?:pub(?:\([a-z]+\))?\s+)?use\s+(?:::)?${q}(?:::([^;]*))?;|\bextern\s+crate\s+${q}\b/gs) {
          $imports++; $hit = 1; my $p = defined $1 ? $1 : "";
          for my $w ($p =~ /([A-Za-z_][A-Za-z0-9_]*)/g) { next if $w =~ /^(self|super|crate|as)$/; $sym{$w} = 1; $local{$w} = 1; } }
        (my $body = $t) =~ s/\b(?:pub(?:\([a-z]+\))?\s+)?use\s+(?:::)?${q}(?:::[^;]*)?;//gs;
        my @names = keys %local;
        for my $l (split /\n/, $body) {
          next if $l =~ /^\s*\/\//;
          if ($l =~ /\b${q}::/ || (@names && $l =~ /\b(?:@{[join "|", map { quotemeta } @names]})\b/)) { $calls++; $hit = 1;
            $sym{$1} = 1 while $l =~ /\b${q}::([A-Za-z_][A-Za-z0-9_]*)/g; } }
      } else {
        for my $l (split /\n/, $t) { if ($l =~ /uses:\s*["\x27]?$nq(?:\/[^@\s]*)?@/) { $imports++; $calls++; $hit = 1; } }
      }
      $files{$f} = 1 if $hit;
    }
    my @s = sort keys %sym; my $js = join(",", map { my $x = $_; $x =~ s/(["\\])/\\$1/g; "\"$x\"" } @s);
    printf "{\"import_lines\":%d,\"symbols\":[%s],\"call_sites\":%d,\"files\":%d}\n", $imports, $js, $calls, scalar(keys %files);
  ' $files
}
is_crypto() {
  local eco="$1" name="$2" r
  [[ "$eco" == cargo ]] || return 1
  case " $CRYPTO_CRATES " in *" $name "*) return 0 ;; esac
  r="$(jq -r --arg n "$name" '.[] | select(.ecosystem == "cargo" and .name == $n) | .repository // ""' "$work/scan.json" | head -n 1)"
  [[ "$r" =~ ^https?://github\.com/$CRYPTO_ORG/ ]]
}
counts_key() { jq -r '"\(.import_lines)/\(.symbols | length)/\(.call_sites)/\(.files)"' <<<"$1"; }

get "repos/$repo/issues?state=all&labels=$ZERO_LABEL&per_page=100" "$work/zissues.json" || api_fail "repos/$repo/issues"
[[ $dry_run -eq 0 ]] && ensure_label "$ZERO_LABEL" "$ZERO_LABEL_COLOR" "Zero-dep assessment of an aged dependency"
[[ $dry_run -eq 0 ]] && ensure_label "$LABEL" "$LABEL_COLOR" "Pull requests that update a dependency file"
jq -c '.[] | select(.class == "aged")' "$work/scan.json" >"$work/aged.jsonl"
planned_claude=0; : >"$work/aged.titles"
# issue_text <eco> <name> <counts json> <recommendation> <reason> <estimate> <risks> <symbols> <status>: the body of an assessment.
assessment_text() {
  local eco="$1" name="$2" counts="$3" rec="$4" reason="$5" est="$6" risks="$7" syms="$8"
  echo "Zero-dep assessment of the aged $eco dependency \`$name\` (docs/specification/dependencies.md, Zero-dep assessment)."
  echo
  echo "- Scan line: \`$(scan_line_of "$eco" "$name")\`"
  echo "- Counts of the fixed search: $(jq -r '"\(.import_lines) import lines, \(.symbols | length) symbols, \(.call_sites) call sites, \(.files) files"' <<<"$counts")"
  echo "- Symbols: ${syms:-none}"
  echo "- Estimate of own code: ${est:-not estimated}"
  echo "- Risks: ${risks:-none named}"
  echo "- Recommendation: **$rec**"
  echo "- Reason: $reason"
  echo "- Run: ${run_url:-local run} on $today"
  echo "- Scan: \`$(summary_line)\`"
  echo
  echo "$REPLACE_STATEMENT"
}
while IFS= read -r it; do
  eco="$(jq -r .ecosystem <<<"$it")"; name="$(jq -r .name <<<"$it")"; key="$eco:$name"; latest="$(jq -r .latest <<<"$it")"
  title="$ASSESS_TITLE_PREFIX$eco $name"; echo "$title" >>"$work/aged.titles"
  counts="$(count_use "$eco" "$name")"; ck="$(counts_key "$counts")"
  extra="latest: $latest latest_age_days: $(jq -r .latest_age_days <<<"$it") behind: $(jq -r 'if .behind then "yes" else "no" end' <<<"$it") counts: $ck"
  iss="$(jq -c --arg t "$title" '[.[] | select(.title == $t and (.pull_request | not))] | first // empty' "$work/zissues.json")"
  inum=""; istate=""; prev=""
  if [[ -n "$iss" ]]; then
    inum="$(jq -r .number <<<"$iss")"; istate="$(jq -r .state <<<"$iss")"
    prev="$(jq -r '.body // ""' <<<"$iss" | grep -F "$ASSESS_MARK" | tail -n 1)"
  fi
  plat="$(sed -nE 's/.* latest:([^ ]+).*/\1/p' <<<"$prev")"; pcounts="$(sed -nE 's/.* counts:([^ ]+).*/\1/p' <<<"$prev")"
  pclass="$(sed -nE 's/.* class:([^ ]+).*/\1/p' <<<"$prev")"; pdate="$(sed -nE 's/.* date:([0-9-]+).*/\1/p' <<<"$prev")"
  pdays=99999; [[ -n "$pdate" ]] && pdays="$(jq -nr --arg a "$pdate" --arg b "$today" '((($b | strptime("%Y-%m-%d") | mktime) - ($a | strptime("%Y-%m-%d") | mktime)) / 86400) | floor')"
  # The life of the issue: open one; or, for an open issue, a new run when the latest version or the counts
  # changed, or after the reassessment days; a closed issue stays closed until the latest version changes.
  issue_act=""
  if [[ -z "$inum" ]]; then issue_act=open
  elif [[ "$istate" == closed ]]; then [[ -n "$plat" && "$plat" != "$latest" ]] && issue_act=reopen
  elif [[ "$plat" != "$latest" || "$pcounts" != "$ck" || "$pclass" != aged ]]; then issue_act=comment
  elif [[ "$pdays" -ge $REASSESS_DAYS ]]; then issue_act=comment
  fi
  if [[ -z "$issue_act" ]]; then
    if [[ "$istate" == closed ]]; then item "$key" assess "$name" "$eco" closed "" "$extra issue: $inum plan: none, closed by the owner"
    else item "$key" assess "$name" "$eco" unchanged "" "$extra issue: $inum last: ${pdate:-none}"; fi
    continue
  fi
  crypto=0; is_crypto "$eco" "$name" && crypto=1
  if [[ $crypto -eq 0 ]]; then
    if [[ $auth_failed -eq 1 ]]; then item "$key" assess "$name" "$eco" waiting claude_auth "$extra"; continue; fi
    if [[ $((planned_claude + claude_runs)) -ge $ASSESS_RUN_CAP ]]; then item "$key" assess "$name" "$eco" waiting run_cap "$extra"; continue; fi
    if over_budget "$ASSESS_JOB_BUDGET_USD"; then item "$key" assess "$name" "$eco" waiting budget_cap "$extra"; continue; fi
  fi
  if [[ $dry_run -eq 1 ]]; then
    if [[ $crypto -eq 1 ]]; then item "$key" assess "$name" "$eco" planned cryptography "$extra recommendation: keep plan: no claude -p run"
    else planned_claude=$((planned_claude + 1)); item "$key" assess "$name" "$eco" planned "" "$extra plan: claude"
      action claude "$key" planned "fields=scan_line,counts ecosystem=$eco name=$name latest=$latest counts=$ck tools=Read,Grep,Glob model=$MODEL max_turns=$MAX_TURNS budget_usd=$RUN_BUDGET_USD timeout_min=$((claude_assess_timeout / 60))"; fi
    action issue "$key" planned "$issue_act \"$title\" with the labels $LABEL and $ZERO_LABEL${inum:+ (#$inum)}"
    continue
  fi
  rec=""; reason=""; est=""; risks=""; res=""; syms="$(jq -r '.symbols | join(", ")' <<<"$counts")"; u=""; fail=""
  if [[ $crypto -eq 1 ]]; then rec=keep; reason="$CRYPTO_REASON"
  else
    log="$work/assess.$(tr ':/@' '___' <<<"$key").log"; : >"$log"
    git status --porcelain --untracked-files=all >"$work/tree.before"; git diff HEAD >>"$work/tree.before"
    render "$ROOT/.agents/scripts/deps.freshness.assess.prompt.md" scan_line="$(scan_line_of "$eco" "$name")" ecosystem="$eco" name="$name" \
      import_lines="$(jq -r .import_lines <<<"$counts")" symbols="${syms:-none}" call_sites="$(jq -r .call_sites <<<"$counts")" \
      files="$(jq -r .files <<<"$counts")" >"$work/prompt.txt"
    run_claude assess "$work/prompt.txt" "$log"
    u="$(usage_text "$run_usage" "$run_secs")"
    git status --porcelain --untracked-files=all >"$work/tree.after"; git diff HEAD >>"$work/tree.after"
    fail="$run_reason"
    if ! cmp -s "$work/tree.before" "$work/tree.after"; then fail=tree_changed; git reset -q --hard HEAD; git clean -fdq; fi
    if [[ -z "$fail" ]]; then
      res="$(structured_of "$run_out" assess)"
      if [[ -z "$res" ]]; then fail=invalid_result
      else
        rec="$(jq -r .recommendation <<<"$res")"; reason="$(jq -r .reason <<<"$res" | redact)"
        est="$(jq -r '"\(.replacement_lines) lines"' <<<"$res")"; risks="$(jq -r '.risks | map(tostring) | join("; ")' <<<"$res" | redact)"
        if [[ "$rec" == replace ]] && jq -e --argjson m "$REPLACE_MAX_LINES" '.replacement_lines > $m' <<<"$res" >/dev/null; then
          rec=watch; reason="estimate_too_large: the estimate of $(jq -r .replacement_lines <<<"$res") lines is above $REPLACE_MAX_LINES lines. $reason"
        fi
      fi
    fi
    jq -nc --arg k "$key" --arg r "$fail" --arg rec "$rec" --argjson u "$run_usage" --arg res "${res:-}" \
      '{key: $k, reason: $r, recommendation: $rec, usage: $u, result: ($res | fromjson? // null)}' >>"$work/runs.jsonl"
  fi
  if [[ -n "$fail" ]]; then
    item "$key" assess "$name" "$eco" failed "$fail" "$extra $u"
    body="$(assessment_text "$eco" "$name" "$counts" "none" "the assessment run failed with the reason \`$fail\`; the next run tries again" "" "" "$syms" | redact)
$ASSESS_MARK latest:$latest counts:none class:aged date:none -->"
  else
    r2=""; [[ "$reason" == estimate_too_large* ]] && r2=estimate_too_large; [[ $crypto -eq 1 ]] && r2=cryptography
    item "$key" assess "$name" "$eco" assessed "$r2" "$extra recommendation: $rec${u:+ $u}"
    body="$(assessment_text "$eco" "$name" "$counts" "$rec" "$reason" "$est" "$risks" "$syms" | redact)
$ASSESS_MARK latest:$latest counts:$ck class:aged date:$today recommendation:$rec -->"
  fi
  case "$issue_act" in
    open)
      if write_issue --method POST "repos/$repo/issues" -f title="$title" -f body="$body" -f "labels[]=$LABEL" -f "labels[]=$ZERO_LABEL" >"$work/issue.json"; then
        action issue "#$(jq -r '.number // empty' "$work/issue.json")" done "opened: $title"
      else action issue "$key" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi ;;
    *)
      [[ "$issue_act" == reopen ]] && { write_issue --method PATCH "repos/$repo/issues/$inum" -f state=open >/dev/null || true; }
      if write_issue --method PATCH "repos/$repo/issues/$inum" -f body="$body" >/dev/null \
        && write_issue --method POST "repos/$repo/issues/$inum/comments" -f body="$(sed '$d' <<<"$body")" >/dev/null; then
        action comment "#$inum" done "$issue_act: new assessment"
      else action comment "#$inum" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi ;;
  esac
done <"$work/aged.jsonl"

# An open assessment issue of a dependency that is no longer aged, or no longer a dependency: comment and close.
jq -c --arg p "$ASSESS_TITLE_PREFIX" '.[] | select(.state == "open" and (.pull_request | not) and (.title | startswith($p)))' "$work/zissues.json" >"$work/zopen.jsonl"
while IFS= read -r iss; do
  title="$(jq -r .title <<<"$iss")"; inum="$(jq -r .number <<<"$iss")"
  grep -qxF "$title" "$work/aged.titles" && continue
  rest="${title#"$ASSESS_TITLE_PREFIX"}"; eco="${rest%% *}"; name="${rest#* }"
  line="$(scan_line_of "$eco" "$name")"
  if [[ -n "$line" ]]; then why="no longer aged: \`$line\`"; else why="no longer a direct dependency of the project"; fi
  if [[ $dry_run -eq 1 ]]; then item "$eco:$name" assess "$name" "$eco" planned "" "issue: $inum plan: comment and close"; action issue "#$inum" planned "comment ($why) and close"; continue; fi
  if write_issue --method POST "repos/$repo/issues/$inum/comments" -f body="The dependency is $why. ${run_url:+Run: $run_url}" >/dev/null \
    && write_issue --method PATCH "repos/$repo/issues/$inum" -f state=closed >/dev/null; then
    item "$eco:$name" assess "$name" "$eco" closed "" "issue: $inum"; action issue "#$inum" done "closed: $why"
  else action issue "#$inum" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
done <"$work/zopen.jsonl"
finish 0
