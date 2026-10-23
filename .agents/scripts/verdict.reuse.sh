#!/usr/bin/env bash
# @agents-index Hosted verdict reuse: lookup finds a full green verdict run of the identical git tree in the run records (read-only, fail closed); judge passes the verdict.
#
# Usage: verdict.reuse.sh lookup [--repo OWNER/NAME] [--commit SHA] [--event push|pull_request] [--base SHA]
#                                [--run-id ID] [--decision-file FILE] [--human]
#        verdict.reuse.sh judge --format R --lint R --test R --web R --image R [--decision hit|miss] [--human]
#   lookup           Decide whether the verdict of the checked-out commit can reuse an earlier full green run of
#                    the verdict workflow whose head commit has the same git tree hash. Only reads the GitHub API.
#   judge            Pass or fail the verdict from the results of the five stage jobs and the decision.
#   --repo R         The repository, OWNER/NAME. Default: the GitHub remote origin of the clone.
#   --commit SHA     The commit that the job checked out (40 hex). It must equal HEAD. Default: HEAD.
#   --event E        The event of the current run: push or pull_request. Default: push.
#   --base SHA       The base commit: for a push the first parent of the pushed commit, for a pull request the
#                    base commit of the pull request. Empty or absent: the first parent of HEAD from git.
#   --run-id ID      The id of the current run: it is never a source, and the window ends at its start.
#                    Absent: the window ends now.
#   --decision-file F  Append decision=, reason=, tree=, source_run=, source_commit= lines to F (for example
#                    the step output file of the platform) and exit 0 on a miss, so the step does not fail.
#   --format R, --lint R, --test R, --web R, --image R
#                    The result of each stage job: success, failure, cancelled, or skipped.
#   --decision D     The decision of lookup. Absent or empty counts as miss.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run        Print the inputs and the read calls that lookup would make, and call nothing.
#   -h, --help       Print this help.
#
# Environment: GH_TOKEN, the token for the read calls (in the workflow the job token with contents: read and
# actions: read). The script never prints it.
#
# Purpose: a squash merge makes a new commit over the tree of the pull request head, so the push run on main
# can reuse the proof of the pull request run. The key is the full git tree hash of HEAD, and only string
# equality counts. A source run is a run of .github/workflows/verdict.yml of this repository with the
# conclusion success, the event push or pull_request, a head repository that is this repository (no fork),
# a start within 72 hours before the current run, the same full tree_id of its head commit, the five stage
# jobs format, lint, test, web, and image each with the conclusion success (a run in which a reuse skipped a
# stage is never a source), for a pull_request run a head that contains the base commit (compare status
# ahead or identical, so its merge ref had the head tree and the same workflow file), and not the current
# run. Every doubt fails closed: a missing token, a failed, partial, or unknown API answer, a bad commit
# input, a HEAD that differs from the commit input, or a page limit gives miss with the reason
# lookup_failed and the cause. The lookup writes nothing on GitHub: it makes GET calls only, and keeps no
# cache, artifact, status, or store. The judge passes only when all five stage jobs succeeded, or when the
# decision is hit, lint succeeded, and format, test, web, and image were skipped.
#
# Output (stdout, key: value):
#   lookup: decision: hit|miss reason: match|novel|lookup_failed tree: <tree|unknown>
#     [source_run: <id> source_commit: <sha>] [cause: <text>]
#   judge: passed|failed case: full|reuse|none results: format=R lint=R test=R web=R image=R decision: D
# Exit codes: lookup with no decision file: 0 hit, 1 miss novel, 4 miss lookup_failed; with a decision file:
#   0 for each decision. judge: 0 passed, 1 failed. Both: 2 bad usage, 3 a required tool (git, gh, jq) missing.
# Reference: docs/specification/operations.md, section Hosted verdict reuse.
set -uo pipefail

WORKFLOW_PATH=".github/workflows/verdict.yml"
STAGES="format lint test web image"
WINDOW_SECS=$((72 * 3600))
PER_PAGE=100
MAX_PAGES=5

human=0; dry_run=0; mode=""; repo=""; commit=""; event="push"; base=""; run_id=""; decision_file=""
decision=""
r_format=""; r_lint=""; r_test=""; r_web=""; r_image=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }
need_value() { [[ $1 -ge 2 ]] || die "$2 needs a value. Fix: run --help for the flags. Verify: verdict.reuse.sh --help." 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    lookup|judge) mode="$1"; shift ;;
    --repo) need_value $# "$1"; repo="$2"; shift 2 ;;
    --commit) need_value $# "$1"; commit="$2"; shift 2 ;;
    --event) need_value $# "$1"; event="$2"; shift 2 ;;
    --base) need_value $# "$1"; base="$2"; shift 2 ;;
    --run-id) need_value $# "$1"; run_id="$2"; shift 2 ;;
    --decision-file) need_value $# "$1"; decision_file="$2"; shift 2 ;;
    --format) need_value $# "$1"; r_format="$2"; shift 2 ;;
    --lint) need_value $# "$1"; r_lint="$2"; shift 2 ;;
    --test) need_value $# "$1"; r_test="$2"; shift 2 ;;
    --web) need_value $# "$1"; r_web="$2"; shift 2 ;;
    --image) need_value $# "$1"; r_image="$2"; shift 2 ;;
    --decision) need_value $# "$1"; decision="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the modes and flags. Verify: verdict.reuse.sh --help." 2 ;;
  esac
done
[[ -n "$mode" ]] || die "a mode is required. Fix: pass lookup (before the stage jobs) or judge (in the job verdict). Reference: docs/specification/operations.md, section Hosted verdict reuse. Verify: verdict.reuse.sh --help." 2

# ---- judge -----------------------------------------------------------------------------------------------
if [[ "$mode" == judge ]]; then
  for v in "$r_format" "$r_lint" "$r_test" "$r_web" "$r_image"; do
    case "$v" in success|failure|cancelled|skipped) ;; *)
      die "each stage result must be success, failure, cancelled, or skipped, got '${v:-nothing}'. Fix: pass --format, --lint, --test, --web, and --image with the result of each job (needs.<job>.result). Verify: verdict.reuse.sh judge --format success --lint success --test success --web success --image success." 2 ;;
    esac
  done
  case "$decision" in hit|miss|"") ;; *) die "--decision must be hit, miss, or empty, got '$decision'. Fix: pass the output decision of the job reuse. Verify: verdict.reuse.sh --help." 2 ;; esac
  d="${decision:-miss}"
  case_name=none
  if [[ "$r_format $r_lint $r_test $r_web $r_image" == "success success success success success" ]]; then
    case_name=full
  elif [[ "$d" == hit && "$r_lint" == success && "$r_format $r_test $r_web $r_image" == "skipped skipped skipped skipped" ]]; then
    case_name=reuse
  fi
  status=passed; [[ "$case_name" == none ]] && status=failed
  if [[ $human -eq 1 ]]; then
    case "$case_name" in
      full) echo "The verdict passed: each of the five stage jobs succeeded." ;;
      reuse) echo "The verdict passed by reuse: lint succeeded, and the other four stages were proved by the source run." ;;
      *) echo "The verdict failed: format=$r_format lint=$r_lint test=$r_test web=$r_web image=$r_image with the decision $d. Fix: read the failed or skipped job." ;;
    esac
  else
    echo "judge: $status case: $case_name results: format=$r_format lint=$r_lint test=$r_test web=$r_web image=$r_image decision: $d"
  fi
  [[ "$status" == passed ]] && exit 0 || exit 1
fi

# ---- lookup ----------------------------------------------------------------------------------------------
tree="unknown"
# Prints the decision line, writes the decision file, and exits with the code of the decision.
decide() {
  local dec="$1" reason="$2" src_run="${3:-}" src_commit="${4:-}" cause="${5:-}" code
  if [[ $human -eq 1 ]]; then
    case "$reason" in
      match) echo "Reuse: the tree $tree has a full green verdict in run $src_run (commit $src_commit); the stages format, test, web, and image are skipped, and lint runs." ;;
      novel) echo "No reuse: no full green verdict run of the tree $tree; every stage runs." ;;
      *) echo "No reuse, the lookup failed closed: $cause. Every stage runs." ;;
    esac
  else
    echo "lookup: decision: $dec reason: $reason tree: $tree${src_run:+ source_run: $src_run source_commit: $src_commit}${cause:+ cause: $cause}"
  fi
  if [[ -n "$decision_file" ]]; then
    printf 'decision=%s\nreason=%s\ntree=%s\nsource_run=%s\nsource_commit=%s\n' "$dec" "$reason" "$tree" "$src_run" "$src_commit" >>"$decision_file" \
      || { echo "error: the decision file $decision_file cannot be written. Fix: pass a writable file. Verify: re-run." >&2; exit 2; }
  fi
  case "$reason" in
    match) code=0 ;;
    novel) code=1 ;;
    *) code=4 ;;
  esac
  [[ -n "$decision_file" ]] && code=0
  exit "$code"
}
lookup_failed() { decide miss lookup_failed "" "" "$1"; }
# A missing tool is exit 3 in both forms; with a decision file the miss is still written first.
missing_tool() {
  echo "error: $1 is not installed, so the lookup fails closed and every stage runs. Fix: install $1 (preinstalled on the GitHub runners). Verify: $1 --version." >&2
  if [[ -n "$decision_file" ]]; then
    printf 'decision=miss\nreason=lookup_failed\ntree=%s\nsource_run=\nsource_commit=\n' "$tree" >>"$decision_file" 2>/dev/null || true
  fi
  echo "lookup: decision: miss reason: lookup_failed tree: $tree cause: $1 missing"
  exit 3
}

case "$event" in push|pull_request) ;; *) die "--event must be push or pull_request, got '$event'. Fix: pass the event of the current run; a call from another workflow never runs the lookup. Verify: verdict.reuse.sh --help." 2 ;; esac
[[ -z "$run_id" || "$run_id" =~ ^[0-9]+$ ]] || die "--run-id must be digits, got '$run_id'. Fix: pass the id of the current run. Verify: verdict.reuse.sh --help." 2

command -v git >/dev/null || missing_tool git
command -v jq >/dev/null || missing_tool jq
command -v gh >/dev/null || missing_tool gh

# The tree that the decision names is the tree of the checkout, and the checkout must be the commit input.
head="$(git rev-parse -q --verify HEAD 2>/dev/null || true)"
[[ -n "$head" ]] || lookup_failed "git rev-parse HEAD failed; the job must run in the checkout"
[[ -n "$commit" ]] || commit="$head"
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || lookup_failed "the commit input '$commit' is not 40 lowercase hex"
[[ "$head" == "$commit" ]] || lookup_failed "HEAD $head differs from the commit input $commit"
tree="$(git rev-parse -q --verify "HEAD^{tree}" 2>/dev/null || true)"
[[ "$tree" =~ ^[0-9a-f]{40}$ ]] || { tree=unknown; lookup_failed "git rev-parse HEAD^{tree} gave no tree"; }
if [[ -z "$base" ]]; then
  base="$(git rev-parse -q --verify "HEAD^1" 2>/dev/null || true)"
  [[ -n "$base" ]] || lookup_failed "no base commit: pass --base, or fetch the first parent of HEAD (fetch-depth 2)"
fi
[[ "$base" =~ ^[0-9a-f]{40}$ ]] || lookup_failed "the base commit '$base' is not 40 lowercase hex"
if [[ -z "$repo" ]]; then
  repo="$(git remote get-url origin 2>/dev/null | sed -nE 's#^(https://github\.com/|git@github\.com:)([^/]+/[^/.]+)(\.git)?$#\2#p')"
fi
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || lookup_failed "the repository '${repo:-none}' is not OWNER/NAME; pass --repo"

if [[ $dry_run -eq 1 ]]; then
  echo "plan: repo $repo commit: $commit tree: $tree event: $event base: $base run_id: ${run_id:-none}"
  echo "plan: read GET repos/$repo/actions/runs/${run_id:-<none, window ends now>}"
  echo "plan: read GET repos/$repo/actions/workflows/verdict.yml/runs?status=success&per_page=$PER_PAGE&page=<1..$MAX_PAGES>"
  echo "plan: read GET repos/$repo/actions/runs/<candidate>/jobs?filter=latest&per_page=$PER_PAGE"
  echo "plan: read GET repos/$repo/compare/$base...<candidate head> (pull_request candidates only)"
  exit 0
fi
[[ -n "${GH_TOKEN:-}" ]] || lookup_failed "GH_TOKEN is not set"

work="$(mktemp -d "${TMPDIR:-/tmp}/verdict.reuse.XXXXXX")" || lookup_failed "mktemp failed"
trap 'rm -rf "$work"' EXIT
# One read call; the method is GET by default and never set. A failed call is a lookup failure.
api_get() { gh api -H "Accept: application/vnd.github+json" "$1" </dev/null >"$2" 2>"$work/err" && jq -e 'type == "object"' "$2" >/dev/null 2>&1; }
epoch() { jq -rn --arg t "$1" '$t | fromdateiso8601' 2>/dev/null; }

# The window ends at the start of the current run.
if [[ -n "$run_id" ]]; then
  api_get "repos/$repo/actions/runs/$run_id" "$work/current.json" || lookup_failed "the read of the current run $run_id failed: $(head -c 200 "$work/err" | tr '\n' ' ')"
  started="$(jq -r '.run_started_at // empty' "$work/current.json")"
  now="$(epoch "$started")"
  [[ "$now" =~ ^[0-9]+$ ]] || lookup_failed "the current run record has no valid run_started_at"
else
  now="$(date -u +%s)"
fi
since=$((now - WINDOW_SECS))

# All successful verdict runs, newest first, until a short page; a full last page is a page limit.
: >"$work/runs.jsonl"
page=1; complete=0
while [[ $page -le $MAX_PAGES ]]; do
  api_get "repos/$repo/actions/workflows/verdict.yml/runs?status=success&per_page=$PER_PAGE&page=$page" "$work/page.json" \
    || lookup_failed "the read of the run records (page $page) failed: $(head -c 200 "$work/err" | tr '\n' ' ')"
  jq -e '.workflow_runs | type == "array"' "$work/page.json" >/dev/null 2>&1 || lookup_failed "the run records of page $page have no workflow_runs list"
  jq -c '.workflow_runs[]' "$work/page.json" >>"$work/runs.jsonl"
  n="$(jq '.workflow_runs | length' "$work/page.json")"
  # A page that ends before the window ends the list too, since the runs come newest first.
  oldest="$(jq -r '[.workflow_runs[].run_started_at // empty] | min // empty' "$work/page.json")"
  if [[ "$n" -lt "$PER_PAGE" ]] || { [[ -n "$oldest" ]] && [[ "$(epoch "$oldest")" -lt "$since" ]]; }; then complete=1; break; fi
  page=$((page + 1))
done
[[ $complete -eq 1 ]] || lookup_failed "the page limit of $MAX_PAGES pages was reached before the end of the window"

# Each record needs the fields of the rules with the expected types; an unknown shape is a lookup failure.
bad="$(jq -r 'select((.id|type) != "number" or (.path|type) != "string" or (.event|type) != "string"
  or (.conclusion|type) != "string" or (.run_started_at|type) != "string" or (.head_sha|type) != "string"
  or (.head_commit.tree_id|type) != "string" or (.head_repository.full_name|type) != "string"
  or (.repository.full_name|type) != "string") | .id' "$work/runs.jsonl" | head -n 1)"
[[ -z "$bad" ]] || lookup_failed "the run record $bad lacks a field of the reuse rules"

# Rules 1 to 5 and 8 on the records; the newest candidate first.
jq -r --arg repo "$repo" --arg path "$WORKFLOW_PATH" --arg tree "$tree" --arg cur "${run_id:-0}" --argjson since "$since" --argjson now "$now" '
  select(.path == $path and .repository.full_name == $repo and .conclusion == "success")
  | select(.event == "push" or .event == "pull_request")
  | select(.head_repository.full_name == $repo)
  | select((.run_started_at | fromdateiso8601) as $t | $t >= $since and $t <= $now)
  | select(.head_commit.tree_id == $tree)
  | select((.id | tostring) != $cur)
  | [.id, .event, .head_sha] | @tsv' "$work/runs.jsonl" >"$work/candidates.tsv" 2>"$work/err" \
  || lookup_failed "the filter of the run records failed: $(head -c 200 "$work/err" | tr '\n' ' ')"

while IFS=$'\t' read -r cid cevent csha; do
  [[ -n "$cid" ]] || continue
  # Rule 6: the five stage jobs of the latest attempt, each success; a skipped stage is never a proof.
  api_get "repos/$repo/actions/runs/$cid/jobs?filter=latest&per_page=$PER_PAGE" "$work/jobs.json" \
    || lookup_failed "the read of the jobs of run $cid failed: $(head -c 200 "$work/err" | tr '\n' ' ')"
  jq -e '(.jobs | type == "array") and (.total_count | type == "number") and (.total_count == (.jobs | length))' "$work/jobs.json" >/dev/null 2>&1 \
    || lookup_failed "the jobs of run $cid are partial or of an unknown shape"
  ok=1
  for s in $STAGES; do
    c="$(jq -r --arg s "$s" '[.jobs[] | select(.name == $s) | .conclusion] | if length == 1 then .[0] // "none" else "count" end' "$work/jobs.json")"
    [[ "$c" == success ]] || { ok=0; break; }
  done
  [[ $ok -eq 1 ]] || continue
  # Rule 7: a pull_request run counts only when its head contains the base commit.
  if [[ "$cevent" == pull_request ]]; then
    api_get "repos/$repo/compare/$base...$csha" "$work/compare.json" \
      || lookup_failed "the compare of $base with the head $csha of run $cid failed: $(head -c 200 "$work/err" | tr '\n' ' ')"
    st="$(jq -r '.status // empty' "$work/compare.json")"
    case "$st" in
      ahead|identical) ;;
      behind|diverged) continue ;;
      *) lookup_failed "the compare of run $cid gave the unknown status '${st:-none}'" ;;
    esac
  fi
  decide hit match "$cid" "$csha"
done <"$work/candidates.tsv"
decide miss novel
