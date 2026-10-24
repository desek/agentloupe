#!/usr/bin/env bash
# @agents-index Shared steps of deps.autofix.sh and deps.freshness.sh: GitHub reads and writes, redaction, the branch, the push, the pull request, the merge rule, and the release pull request step.
#
# Usage: source "$ROOT/.agents/scripts/deps.common.sh" from deps.autofix.sh or deps.freshness.sh. It is not a
# command: run directly, it prints this text and exits 2 (with --help, exit 0).
#
# Purpose: one place for the steps that the daily autofix and the weekly freshness flow share
# (docs/specification/dependencies.md, "Autofix branch and merge", "Merge rule", "Release pull request step",
# "Credentials and logs"). The steps take the branch name and the file of the commit lines as inputs, so each
# flow keeps its own branch, title, and item lines, and the autofix keeps its behaviour.
#
# Contract with the calling script. Before it calls a function, the caller sets these variables:
#   repo owner work dry_run release_token run_url visibility nothing_open ROOT
#   POLL_SECS WAIT_CAP_SECS WAIT_CAP_MIN MERGE_MODE RELEASE_PLEASE_WORKFLOW
#   merged_fix_sha planned_fix_merge planned_records branch_ready
# and defines these functions: item, action, record_failure, api_fail, finish, clean_env.
# It keeps the open pull requests in "$work/pulls.json" and the Dependabot pull requests to close in
# "$work/closeprs" (one number per line, may be empty).
# Functions: get, get_release, write_release, write_issue, git_auth, sleep_poll, polls, check_state,
#   wait_check, token_values, redact, has_credential, prepare_branch, title_of, plan_branch_merge,
#   branch_merge, release_step.
# Exit codes (run directly): 0 with --help, 2 otherwise.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"
  [[ "${1:-}" == -h || "${1:-}" == --help ]] && exit 0
  echo "error: deps.common.sh is a helper, not a command. Fix: source it from deps.autofix.sh or deps.freshness.sh. Verify: deps.autofix.sh --help." >&2
  exit 2
fi

# ---- redaction and the credential check -----------------------------------------------------------------------
token_values() { printf '%s\n%s\n%s\n%s\n' "${CLAUDE_CODE_OAUTH_TOKEN:-}" "$release_token" "${GH_TOKEN:-}" "${GITHUB_TOKEN:-}"; }
# Removes each credential pattern and the value of each token of the run; the values reach awk only through
# its environment, never through its arguments.
redact() {
  DEPS_REDACT_VALUES="$(token_values)" awk 'BEGIN { n = split(ENVIRON["DEPS_REDACT_VALUES"], v, "\n") }
    { line = $0
      for (i = 1; i <= n; i++) if (length(v[i]) >= 8) while ((p = index(line, v[i])) > 0) line = substr(line, 1, p - 1) "[redacted]" substr(line, p + length(v[i]))
      gsub(/(sk-ant-|ghp_|github_pat_|gho_|ghs_)[A-Za-z0-9_-]*/, "[redacted]", line); print line }'
}
# Exit 0 when the text on stdin holds a credential pattern or a token value; prints nothing.
has_credential() {
  DEPS_REDACT_VALUES="$(token_values)" awk 'BEGIN { n = split(ENVIRON["DEPS_REDACT_VALUES"], v, "\n"); hit = 0 }
    { for (i = 1; i <= n; i++) if (length(v[i]) >= 8 && index($0, v[i]) > 0) hit = 1
      if ($0 ~ /sk-ant-|ghp_|github_pat_|gho_|ghs_/) hit = 1 }
    END { exit hit ? 0 : 1 }'
}

# ---- GitHub calls ---------------------------------------------------------------------------------------------
# Reads use GET only. The release token reads the alerts; the workflow token reads the rest.
get() { gh api -H "Accept: application/vnd.github+json" "$1" </dev/null >"$2" 2>"$work/err"; }
get_release() {
  if [[ -n "$release_token" ]]; then GH_TOKEN="$release_token" gh api -H "Accept: application/vnd.github+json" "$1" </dev/null >"$2" 2>"$work/err"
  else get "$1" "$2"; fi
}
# Writes: never in a dry run. "release" writes start workflows or Dependabot; "issue" writes use the workflow token.
write_release() { [[ $dry_run -eq 0 ]] || return 1; GH_TOKEN="$release_token" gh api -H "Accept: application/vnd.github+json" "$@" </dev/null 2>"$work/err"; }
write_issue() { [[ $dry_run -eq 0 ]] || return 1; gh api -H "Accept: application/vnd.github+json" "$@" </dev/null 2>"$work/err"; }
# git with the release token for one fetch or push, from the environment of a credential helper.
git_auth() {
  GIT_PUSH_TOKEN="$release_token" git -c credential.helper= \
    -c 'credential.helper=!f() { test "$1" = get && echo username=x-access-token && echo "password=$GIT_PUSH_TOKEN"; }; f' "$@"
}
sleep_poll() { [[ $POLL_SECS -gt 0 ]] && sleep "$POLL_SECS"; return 0; }
# polls [cap]: the number of polls within the cap (default: the wait cap of a check).
polls() { local cap="${1:-$WAIT_CAP_SECS}"; if [[ $POLL_SECS -gt 0 ]]; then echo $((cap / POLL_SECS)); else echo "$cap"; fi; }
# check_state <sha>: success, failure, pending, or none, from the latest check run named verdict.
check_state() {
  get "repos/$repo/commits/$1/check-runs?check_name=verdict" "$work/checks.json" || { echo none; return; }
  jq -r '[.check_runs[]? | select(.name == "verdict")] | sort_by(.started_at // "") | last
    | if . == null then "none" elif .status != "completed" then "pending" elif .conclusion == "success" then "success"
      elif .conclusion == "skipped" or .conclusion == "neutral" then "none" else "failure" end' "$work/checks.json" 2>/dev/null || echo none
}
# wait_check <sha>: polls the check verdict up to the wait cap; prints the last state.
wait_check() {
  local s n=0 max; max="$(polls)"
  while :; do
    s="$(check_state "$1")"
    [[ "$s" == success || "$s" == failure ]] && { echo "$s"; return; }
    n=$((n + 1)); [[ $n -ge $max ]] && { echo "$s"; return; }
    sleep_poll
  done
}

# ---- the branch -------------------------------------------------------------------------------------------------
# prepare_branch <branch>: fetches main, checks out the branch of an earlier run of the same day or makes it from
# main, and builds the crate once so the later verdicts reuse the target directory.
prepare_branch() {
  local br="$1"
  [[ $branch_ready -eq 1 ]] && return 0
  git_auth fetch -q origin main >/dev/null 2>"$work/err" || { echo "error: git fetch of main failed: $(redact <"$work/err" | head -c 200 | tr '\n' ' '). Fix: check the release token and the network. Verify: git fetch origin main." >&2; finish 4; }
  if git_auth fetch -q origin "refs/heads/$br:refs/remotes/origin/$br" >/dev/null 2>&1; then
    git checkout -q -B "$br" "origin/$br"; action branch "$br" done "reused from an earlier run today"
  else
    git checkout -q -B "$br" origin/main; action branch "$br" done "from main"
  fi
  main_ref=origin/main
  echo "build: cargo build --locked" >>"$work/build.log"
  clean_env -- cargo build --locked >>"$work/build.log" 2>&1 </dev/null || true
  branch_ready=1
}

# ---- push, pull request, and merge of the branch -------------------------------------------------------------
# title_of <lines file>: the pull request title from the commit summaries: one change keeps its summary, more
# changes become "<type>(deps): update <n> dependencies"; the type is fix when one line is fix(deps).
title_of() {
  local n type=chore; n="$(grep -c . "$1")"; grep -q '^fix(deps)' "$1" && type=fix
  if [[ "$n" -eq 1 ]]; then sed -E 's/^[a-z]+\(deps\): (bump [^ ]+ to [^ ]+).*/'"$type"'(deps): \1/' "$1"; else echo "$type(deps): update $n dependencies"; fi
}
# plan_branch_merge <branch>: the dry run lines of the pull request and of its merge.
plan_branch_merge() {
  action pull_request "$1" planned "open or update one pull request to main"
  action merge "$1" planned "squash merge after a green check verdict of the head commit, wait up to $WAIT_CAP_MIN minutes"
}
# branch_merge <branch> <lines file> <path> <unit> <noun> [trailer]: pushes the branch with the release token, opens or
# updates its one pull request to main, closes the Dependabot pull requests of "$work/closeprs" with a link, and
# squash merges after a green check verdict of the head commit (the merge rule). The body is one line for each
# commit, then the trailer, then the run link. The item line counts "<n>-<unit>", the push line "<n> <noun>
# commits". Sets merged_fix_sha when a fix(deps) change merged.
branch_merge() {
  local br="$1" lines="$2" path="$3" unit="$4" noun="$5" trailer="${6:-}" title body head state k max n pr="" url="" count
  count="$(grep -c . "$lines")"
  title="$(title_of "$lines")"
  body="$(awk 'NF {print; print ""}' "$lines")
${trailer}
${run_url:+Run: $run_url}"
  if git_auth push -q origin "HEAD:refs/heads/$br" >/dev/null 2>"$work/err"; then action push "$br" done "$count $noun commits"
  else echo "error: the push of $br failed: $(redact <"$work/err" | head -c 200 | tr '\n' ' '). Fix: check that the release token has contents write. Verify: git ls-remote origin $br." >&2; finish 4; fi
  get "repos/$repo/pulls?state=open&head=$owner:$br" "$work/own.json" || api_fail "repos/$repo/pulls?head=$owner:$br"
  pr="$(jq -r 'first | .number // empty' "$work/own.json")"
  if [[ -z "$pr" ]]; then
    write_release --method POST "repos/$repo/pulls" -f title="$title" -f head="$br" -f base=main -f body="$body" >"$work/newpr.json" || api_fail "repos/$repo/pulls"
    pr="$(jq -r '.number // empty' "$work/newpr.json")"; url="$(jq -r '.html_url // empty' "$work/newpr.json")"
    action pull_request "#$pr" done "opened: $title"
  else
    url="$(jq -r 'first | .html_url // empty' "$work/own.json")"
    write_release --method PATCH "repos/$repo/pulls/$pr" -f title="$title" -f body="$body" >/dev/null || api_fail "repos/$repo/pulls/$pr"
    action pull_request "#$pr" done "updated: $title"
  fi
  # A Dependabot pull request whose update the branch carries closes with a link.
  for n in $(sort -u "$work/closeprs" 2>/dev/null); do
    if write_release --method POST "repos/$repo/issues/$n/comments" -f body="The dependency autofix carries this update in ${url:-#$pr}." >/dev/null \
      && write_release --method PATCH "repos/$repo/pulls/$n" -f state=closed >/dev/null; then action comment "pr:$n" done "closed with a link to #$pr"
    else action comment "pr:$n" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; fi
  done
  head="$(git rev-parse HEAD)"
  if [[ "$MERGE_MODE" == auto ]]; then
    GH_TOKEN="$release_token" gh pr merge "$pr" --repo "$repo" --auto --squash --subject "$title" --body "$(awk 'NF {print; print ""}' "$lines")" </dev/null >/dev/null 2>"$work/err" \
      && action merge "#$pr" done "auto-merge with squash enabled" || action merge "#$pr" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"
    k=0; max="$(polls)"; state=open
    while [[ $k -lt $max ]]; do
      get "repos/$repo/pulls/$pr" "$work/pr.json" && [[ "$(jq -r '.merged // false' "$work/pr.json")" == true ]] && { state=merged; break; }
      k=$((k + 1)); sleep_poll
    done
    if [[ "$state" == merged ]]; then item "branch:$br" "$path" "$count-$unit" mixed merged "" "pull_request: $pr"
      grep -q '^fix(deps)' "$lines" && merged_fix_sha="$(jq -r '.merge_commit_sha // empty' "$work/pr.json")"
    else item "branch:$br" "$path" "$count-$unit" mixed waiting release_pending "pull_request: $pr"; fi
  else
    state="$(wait_check "$head")"
    if [[ "$state" == success ]]; then
      if write_release --method PUT "repos/$repo/pulls/$pr/merge" -f merge_method=squash -f sha="$head" -f commit_title="$title (#$pr)" \
        -f commit_message="$(awk 'NF {print; print ""}' "$lines")" >"$work/merge.json"; then
        action merge "#$pr" done "squash after a green check verdict"
        item "branch:$br" "$path" "$count-$unit" mixed merged "" "pull_request: $pr"
        grep -q '^fix(deps)' "$lines" && merged_fix_sha="$(jq -r '.sha // empty' "$work/merge.json")"
      else action merge "#$pr" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; item "branch:$br" "$path" "$count-$unit" mixed waiting release_pending "pull_request: $pr"; fi
    elif [[ "$state" == failure ]]; then
      item "branch:$br" "$path" "$count-$unit" mixed failed verdict_failed "pull_request: $pr"
      record_failure "branch:$br" "head=$(echo "$head" | cut -c1-12)" verdict_failed "$br" "" ""
    else item "branch:$br" "$path" "$count-$unit" mixed waiting release_pending "pull_request: $pr check: $state"; fi
  fi
}

# ---- the release pull request step ----------------------------------------------------------------------------
# release_step: merges the one open release pull request after a green check verdict when a fix(deps) change
# merged in this run or is in its changelog; never 1.0.0 or a new major version, and never while the repository
# is private (docs/specification/dependencies.md, Release pull request step).
release_step() {
  local rel n title version head cur_major cur
  [[ -n "$merged_fix_sha" && $dry_run -eq 0 ]] && {
    local k=0 max done_run=0; max="$(polls)"
    while [[ $k -lt $max ]]; do
      get "repos/$repo/actions/workflows/$RELEASE_PLEASE_WORKFLOW/runs?head_sha=$merged_fix_sha" "$work/rp.json" \
        && [[ "$(jq -r '[.workflow_runs[]? | select(.status == "completed")] | length' "$work/rp.json")" -gt 0 ]] && { done_run=1; break; }
      k=$((k + 1)); sleep_poll
    done
    [[ $done_run -eq 1 ]] || { item "release:none" release release-please - waiting release_pending "wait: release-please run"; return; }
    get "repos/$repo/pulls?state=open&per_page=100" "$work/pulls.json" || api_fail "repos/$repo/pulls"
  }
  jq -c '[.[] | select(.title | test("^chore\\(release\\): release [0-9]+\\.[0-9]+\\.[0-9]+$"))]' "$work/pulls.json" >"$work/rel.json"
  case "$(jq length "$work/rel.json")" in
    0) if [[ -n "$merged_fix_sha" ]]; then item "release:none" release release-please - waiting release_pending "wait: release pull request"
       elif [[ $dry_run -eq 1 && $planned_fix_merge -eq 1 ]]; then action release release-please planned "wait for the release-please run, then squash merge the release pull request after a green check verdict"; fi
       return ;;
    1) ;;
    *) item "release:many" release release-please - waiting release_pending "more than one release pull request"; return ;;
  esac
  rel="$(jq -c '.[0]' "$work/rel.json")"; n="$(jq -r .number <<<"$rel")"; title="$(jq -r .title <<<"$rel")"
  version="${title##* }"; head="$(jq -r .head.sha <<<"$rel")"
  # The step runs for a fix(deps) merged in this run, or a release pull request whose changelog lists one.
  local lists_deps=0
  jq -r '.body // ""' <<<"$rel" | awk '/^### /{s = ($0 == "### Bug Fixes")} s && /\*\*deps:\*\*/ {f = 1} END {exit f ? 0 : 1}' && lists_deps=1
  if [[ -z "$merged_fix_sha" && $lists_deps -eq 0 && $planned_fix_merge -eq 0 ]]; then return; fi
  cur="$(sed -n '/^\[package\]/,/^\[/ s/^version *= *"\([^"]*\)".*/\1/p' "$ROOT/Cargo.toml" | head -n 1)"; cur_major="${cur%%.*}"
  if [[ "$version" == 1.0.0 || "${version%%.*}" -gt "${cur_major:-0}" ]]; then
    if [[ $dry_run -eq 1 ]]; then item "release:$n" release "$version" - planned release_needs_human "plan: no merge, record in the issue"; planned_records=$((planned_records + 1))
    elif [[ $nothing_open -eq 1 ]]; then item "release:$n" release "$version" - waiting release_needs_human "no merge; a human merges it"
    else item "release:$n" release "$version" - failed release_needs_human; record_failure "release:$n" "version=$version" release_needs_human "$version" "" ""; fi
    return
  fi
  # A private repository keeps the release with a person: the release workflow stops on a private repository,
  # so a merge would make a tag with no assets.
  if [[ "$visibility" != public ]]; then
    if [[ $dry_run -eq 1 ]]; then item "release:$n" release "$version" - planned release_needs_human "plan: no merge, the repository is $visibility"; planned_records=$((planned_records + 1))
    else item "release:$n" release "$version" - waiting release_needs_human "no merge; the repository is $visibility and the release workflow stops"; fi
    return
  fi
  if [[ $dry_run -eq 1 ]]; then
    item "release:$n" release "$version" - planned "" "check: $(check_state "$head") plan: merge"
    action release "#$n" planned "squash merge $title after a green check verdict of the head commit"
    return
  fi
  if [[ -n "$merged_fix_sha" ]]; then
    get "repos/$repo/compare/$merged_fix_sha...$head" "$work/relcmp.json" || api_fail "repos/$repo/compare"
    case "$(jq -r .status "$work/relcmp.json")" in ahead|identical) ;; *) item "release:$n" release "$version" - waiting release_pending "base: no merge commit"; return ;; esac
  fi
  case "$(wait_check "$head")" in
    success)
      if write_release --method PUT "repos/$repo/pulls/$n/merge" -f merge_method=squash -f sha="$head" -f commit_title="$title (#$n)" >/dev/null; then
        action release "#$n" done "squash merged $title"; item "release:$n" release "$version" - merged
      else action release "#$n" failed "$(redact <"$work/err" | head -c 200 | tr '\n' ' ')"; item "release:$n" release "$version" - waiting release_pending; fi ;;
    *) item "release:$n" release "$version" - waiting release_pending "check: not green before the cap" ;;
  esac
}
