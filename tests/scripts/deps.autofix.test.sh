#!/bin/bash
# @agents-index Shell tests of deps.autofix.sh with a fake gh, claude, cargo, and corepack on recorded answers: empty, alert, Dependabot pull request, hold time, release step, issue, credentials.
#
# Usage: deps.autofix.test.sh [name...]. With names, only those tests run. Each test makes a scratch git repository with a local bare origin, copies
# deps.autofix.sh and its prompt template into it with a stand-in pipeline.verdict.sh and a small price file, and
# puts a fake gh, claude, cargo, and corepack first on PATH. The fake gh answers GET calls from the recorded
# files of tests/fixtures/deps/ and records every call; a call with another method is a write. The fake claude
# records its arguments, its environment, and its prompt, applies the edit of the test, and prints a recorded
# claude -p result. The script runs with --fixtures, so it polls with no delay and uses no network. Prints one
# line per test: "test: <name> status: passed|failed", with the output of a failed test.
# Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIX="$ROOT/tests/fixtures/deps"
REPO=o/r
DAY=2026-10-24
BRANCH="deps/autofix-$DAY"
# Test values only: the claude value matches a credential pattern, so a leak shows in every check.
T_GH=test-gh-token-value-0001
T_REL=test-release-token-value-0002
T_CC=sk-ant-oat01-test-claude-value-0003
failed=0
# want <name>: true when no name is given on the command line, or when the name is one of them.
want() { [ $# -gt 0 ] || return 1; [ ${#ONLY[@]} -eq 0 ] && return 0; local n; for n in "${ONLY[@]}"; do [ "$n" = "$1" ] && return 0; done; return 1; }
ONLY=("$@")

setup() {
  D="$(mktemp -d)"; mkdir -p "$D/bin" "$D/api" "$D/bodies" "$D/git"; : >"$D/calls"; echo "$DAY" >"$D/today"
  cat >"$D/bin/gh" <<GH
#!/bin/bash
# Fake gh: GET answers from files named by the endpoint; other calls are recorded with their body.
F="$D"
GH
  cat >>"$D/bin/gh" <<'GH'
case "${1:-}" in
  auth) exit 1 ;;
  pr) echo "PR $*" >>"$F/calls"; exit 0 ;;
  api) shift ;;
  *) echo "OTHER $*" >>"$F/calls"; exit 0 ;;
esac
method=""; endpoint=""; fields=0; body=""
while [ $# -gt 0 ]; do
  case "$1" in
    --method|-X) method="$2"; shift 2 ;;
    -f|--raw-field) fields=1; case "$2" in body=*) body="${2#body=}" ;; esac; shift 2 ;;
    -F|--field) fields=1; case "$2" in body=@*) body="$(cat "${2#body=@}")" ;; body=*) body="${2#body=}" ;; esac; shift 2 ;;
    --input) fields=1; shift 2 ;;
    -H|--header|--jq|-q) shift 2 ;;
    --paginate|--silent|-i) shift ;;
    *) [ -z "$endpoint" ] && endpoint="$1"; shift ;;
  esac
done
[ -n "$method" ] || { [ $fields -eq 1 ] && method=POST || method=GET; }
n="$(wc -l <"$F/calls" | tr -d ' ')"
echo "$method $endpoint" >>"$F/calls"
[ -n "$body" ] && printf '%s' "$body" >"$F/bodies/$n"
env >"$F/gh.env.$n"
if [ "$method" != GET ]; then
  case "$method $endpoint" in
    "PUT "*/merge) echo '{"sha": "3131313131313131313131313131313131313131", "merged": true}' ;;
    "POST "*/pulls) echo '{"number": 31, "html_url": "https://github.com/o/r/pull/31"}' ;;
    "POST "*/issues) echo '{"number": 41, "html_url": "https://github.com/o/r/issues/41"}' ;;
    *) echo '{}' ;;
  esac
  exit 0
fi
key() { echo "$1" | tr '/.?&=:' '______'; }
norm() { echo "$1" | sed -E 's/[0-9a-f]{40}/SHA/g'; }
full="$(key "$endpoint")"; path="$(key "${endpoint%%\?*}")"
for k in "$full" "$(norm "$full")" "$path" "$(norm "$path")"; do
  if [ -f "$F/api/$k.http" ]; then cat "$F/api/$k" 2>/dev/null; echo "gh: $(cat "$F/api/$k.msg" 2>/dev/null || echo error) (HTTP $(cat "$F/api/$k.http"))" >&2; exit 1; fi
  [ -f "$F/api/$k" ] && { cat "$F/api/$k"; exit 0; }
done
echo "gh: Not Found (HTTP 404)" >&2; exit 1
GH
  cat >"$D/bin/claude" <<CLAUDE
#!/bin/bash
# Fake claude -p: records, applies the edit of the test in the tree, prints the recorded result.
F="$D"
echo run >>"\$F/claude.calls"
printf '%s\n' "\$@" >"\$F/claude.args"
env >"\$F/claude.env"
cat >"\$F/claude.prompt"
[ -f "\$F/claude.sleep" ] && exec sleep "\$(cat "\$F/claude.sleep")"
[ -f "\$F/claude.edit" ] && bash "\$F/claude.edit"
cat "\$F/claude.out"
CLAUDE
  cat >"$D/bin/cargo" <<CARGO
#!/bin/bash
echo "cargo \$*" >>"$D/cargo.calls"; exit 0
CARGO
  cat >"$D/bin/corepack" <<COREPACK
#!/bin/bash
echo "corepack \$*" >>"$D/corepack.calls"; exit "\$(cat "$D/corepack_rc" 2>/dev/null || echo 0)"
COREPACK
  chmod +x "$D/bin/"*
  G="$D/git"; mkdir -p "$G/.agents/scripts" "$G/.github/workflows" "$G/vendor/litellm" "$G/src" "$G/prototype"
  cp "$ROOT/.agents/scripts/deps.autofix.sh" "$ROOT/.agents/scripts/deps.autofix.prompt.md" "$G/.agents/scripts/"
  cat >"$G/.agents/scripts/pipeline.verdict.sh" <<VERDICT
#!/bin/bash
# Stand-in verdict: records its environment, prints the log of the test, exits with the next code of the test.
n=\$(( \$(cat "$D/verdict.calls" 2>/dev/null | wc -l) + 1 )); echo call >>"$D/verdict.calls"
env >"$D/verdict.env.\$n"
[ -f "$D/verdict.log" ] && cat "$D/verdict.log"
rc=\$(sed -n "\${n}p" "$D/verdict_rcs" 2>/dev/null); exit "\${rc:-0}"
VERDICT
  chmod +x "$G/.agents/scripts/pipeline.verdict.sh"
  echo 'name: verdict' >"$G/.github/workflows/verdict.yml"
  echo '{"claude-sonnet-4-6": {"litellm_provider": "anthropic", "input_cost_per_token": 3e-06, "output_cost_per_token": 1.5e-05, "cache_read_input_token_cost": 3e-07, "cache_creation_input_token_cost": 3.75e-06}}' \
    >"$G/vendor/litellm/model_prices_and_context_window.json"
  printf '[package]\nname = "agent-loupe"\nversion = "0.1.0"\n' >"$G/Cargo.toml"
  printf '[[package]]\nname = "smallvec"\nversion = "1.13.2"\n' >"$G/Cargo.lock"
  echo 'pub fn f() {}' >"$G/src/lib.rs"
  echo '{"name": "p", "packageManager": "pnpm@12.9.0"}' >"$G/prototype/package.json"
  echo 'lockfileVersion: 9.0' >"$G/prototype/pnpm-lock.yaml"
  (cd "$G" && git init -q -b main && git config user.email t@example.com && git config user.name t && git add -A && git commit -qm init \
    && git init -q --bare "$D/origin.git" && git remote add origin "$D/origin.git" && git push -q origin main && git fetch -q origin) || exit 1
  serve "repos/$REPO" '{"private": false, "visibility": "public"}'
  serve "repos/$REPO/actions/workflows/verdict.yml" @workflow.verdict.json
  serve "repos/$REPO/dependabot/alerts" @alerts.empty.json
  serve "repos/$REPO/pulls" '[]'
  serve "repos/$REPO/issues" '[]'
  serve "repos/$REPO/labels/dependencies" '{"name": "dependencies"}'
  serve "repos/$REPO/commits/SHA/check-runs" @checks.success.json
  serve "repos/$REPO/compare/main...SHA" @compare.current.json
  serve "repos/$REPO/compare/SHA...SHA" @compare.current.json
  serve "repos/$REPO/actions/workflows/release-please.yml/runs" @runs.release_please.json
  serve "repos/$REPO/pulls?state=open&head=o:$BRANCH" '[]'
  cp "$FIX/claude.result.json" "$D/claude.out"
}
key() { echo "$1" | tr '/.?&=:' '______'; }
# serve <endpoint> <json | @fixture file>
serve() { case "$2" in @*) cp "$FIX/${2#@}" "$D/api/$(key "$1")" ;; *) printf '%s\n' "$2" >"$D/api/$(key "$1")" ;; esac; }
fail_api() { printf '%s\n' "$2" >"$D/api/$(key "$1").http"; [ -n "${3:-}" ] && printf '%s\n' "$3" >"$D/api/$(key "$1").msg"; }
alerts() { jq -c "[.[] | select($1)]" "$FIX/alerts.json" >"$D/api/$(key "repos/$REPO/dependabot/alerts")"; }
pulls() { jq -c "[.[] | select($1)]" "$FIX/pulls.json" >"$D/api/$(key "repos/$REPO/pulls")"; }
with_release() { jq -c --arg t "$1" --slurpfile r "$FIX/pulls.release.json" '. + [$r[0][0] | .title = ("chore(release): release " + $t)]' "$D/api/$(key "repos/$REPO/pulls")" >"$D/x" && mv "$D/x" "$D/api/$(key "repos/$REPO/pulls")"; }
edit() { printf '%s\n' "$1" >"$D/claude.edit"; }
# run [flags]: the script in the scratch tree with the test tokens; stdout in $out, stderr in $err, code in $rc.
run() {
  out="$(cd "$D/git" && PATH="$D/bin:$PATH" GH_TOKEN="${GHT-$T_GH}" RELEASE_TOKEN="${RELT-$T_REL}" CLAUDE_CODE_OAUTH_TOKEN="${CCT-$T_CC}" \
    /bin/bash .agents/scripts/deps.autofix.sh --repo "$REPO" --fixtures "$D" --run-url https://github.com/o/r/actions/runs/1 "$@" 2>"$D/stderr")"; rc=$?
  err="$(cat "$D/stderr")"
}
writes() { grep -vc '^GET ' "$D/calls"; }
claude_runs() { [ -f "$D/claude.calls" ] && wc -l <"$D/claude.calls" | tr -d ' ' || echo 0; }
remote_commits() { git -C "$D/origin.git" log --format=%s "main..$BRANCH" 2>/dev/null; }
no_value() { ! printf '%s\n%s\n' "$out" "$err" | grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC"; }
result() {
  if [ "$2" -eq 0 ]; then echo "test: $1 status: passed"; else
    echo "test: $1 status: failed"; echo "  rc: ${rc:-}"; echo "${out:-}" | sed 's/^/  out: /' | head -n 40
    echo "${err:-}" | sed 's/^/  err: /' | head -n 10; echo "  calls: $(tr '\n' ';' <"$D/calls")"; failed=1; fi
  rm -rf "$D"
}

# Nothing open: one zero count line, exit 0, no write, no claude -p run, no branch; in a dry run and in a run.
if want nothing_open_prints_zero_and_exits_0; then
setup
run --dry-run; ok=$rc; o1="$out"
run
[ $ok -eq 0 ] && [ $rc -eq 0 ] && echo "$o1" | grep -qx 'count: alerts: 0 pull_requests: 0' && echo "$out" | grep -qx 'count: alerts: 0 pull_requests: 0' \
  && [ "$(echo "$out" | grep -c '^item: ')" -eq 0 ] && [ "$(writes)" -eq 0 ] && [ "$(claude_runs)" -eq 0 ] && [ -z "$(remote_commits)" ] \
  && [ "$(git -C "$D/git" branch --show-current)" = main ]
result nothing_open_prints_zero_and_exits_0 $?
fi

# A dry run of one alert: the item fields, the branch, the prompt fields, the commit summary, the merge; no write.
if want dry_run_lists_actions_for_alert; then
setup; alerts '.number == 11'; head0="$(git -C "$D/git" rev-parse HEAD)"
run --dry-run
[ $rc -eq 0 ] && echo "$out" | grep -q '^item: alert:11 path: alert package: smallvec ecosystem: cargo status: planned number: 11 ghsa: GHSA-aaaa-bbbb-cccc severity: high range: >=1.0.0,<1.13.3 patched: 1.13.3 manifest: Cargo.lock plan: claude$' \
  && echo "$out" | grep -qx "action: branch target: $BRANCH status: planned detail: from the head of main" \
  && echo "$out" | grep -q '^action: claude target: alert:11 status: planned detail: package=smallvec ecosystem=cargo range=>=1.0.0,<1.13.3 patched=1.13.3 manifest=Cargo.lock advisory=GHSA-aaaa-bbbb-cccc model=sonnet max_turns=30 budget_usd=5 timeout_min=40$' \
  && echo "$out" | grep -qx 'action: commit target: alert:11 status: planned detail: fix(deps): bump smallvec to 1.13.3 (GHSA-aaaa-bbbb-cccc)' \
  && echo "$out" | grep -q "^action: merge target: $BRANCH status: planned" && echo "$out" | grep -q '^action: release target: release-please status: planned' \
  && ! echo "$out" | grep -q '^action: issue' && [ "$(writes)" -eq 0 ] && [ "$(claude_runs)" -eq 0 ] && [ "$(git -C "$D/git" rev-parse HEAD)" = "$head0" ] && [ -z "$(git -C "$D/git" status --porcelain)" ]
result dry_run_lists_actions_for_alert $?
fi

# An alert and a Dependabot pull request that moves the package to the patched version: one item, the pull request.
if want alert_with_pr_is_handled_once; then
setup; alerts '.number == 11'; pulls '.number == 21'; serve "repos/$REPO/pulls/21/files" @files.cargo.json
run --dry-run
[ $rc -eq 0 ] && [ "$(echo "$out" | grep -c '^item: ')" -eq 1 ] \
  && echo "$out" | grep -q '^item: pr:21 path: pull_request package: smallvec ecosystem: cargo status: planned number: 21 head: 212121212121 check: success behind: 0 target: 1.13.3 files: Cargo.lock ghsa: GHSA-aaaa-bbbb-cccc alerts: alert:11 plan: merge$' \
  && echo "$out" | grep -qx 'action: merge target: pr:21 status: planned detail: squash merge after a green check verdict of the head commit' \
  && ! echo "$out" | grep -q 'action: claude' && [ "$(writes)" -eq 0 ]
result alert_with_pr_is_handled_once $?
fi

# An alert with no patched version: no claude -p run; the autofix issue holds it with the reason; exit 1.
if want no_patched_version_starts_no_run; then
setup; alerts '.number == 13'
run --dry-run; ok=1
[ $rc -eq 0 ] && echo "$out" | grep -q '^item: alert:13 .* status: planned reason: no_patched_version' && [ "$(writes)" -eq 0 ] \
  && echo "$out" | grep -q '^action: issue target: autofix-issue status: planned detail: open "Dependency autofix: failed items" with the label dependencies' && ok=0
run
[ $ok -eq 0 ] && [ $rc -eq 1 ] && echo "$out" | grep -q '^item: alert:13 path: alert package: idna ecosystem: cargo status: failed reason: no_patched_version' \
  && [ "$(claude_runs)" -eq 0 ] && grep -qx "POST repos/$REPO/issues" "$D/calls" && grep -q 'no_patched_version' "$D/bodies/"* \
  && grep -l 'deps-autofix:held' "$D/bodies/"* | xargs grep -q "^alert:13 input:patched=none;main=[0-9a-f]* date:$DAY reason:no_patched_version$" \
  && [ -z "$(remote_commits)" ]
result no_patched_version_starts_no_run $?
fi

# The verdict workflow absent from the tree: exit 5 before any API call; not active on the repository: exit 5.
if want absent_verdict_workflow_exits_5; then
setup; alerts '.number == 11'; rm "$D/git/.github/workflows/verdict.yml"; (cd "$D/git" && git commit -qam 'no verdict')
run; ok=1
[ $rc -eq 5 ] && echo "$err" | grep -q '.github/workflows/verdict.yml' && echo "$err" | grep -q 'release governance' && [ ! -s "$D/calls" ] && ok=0
(cd "$D/git" && git checkout -q HEAD~1); serve "repos/$REPO/actions/workflows/verdict.yml" @workflow.verdict.disabled.json
run
[ $ok -eq 0 ] && [ $rc -eq 5 ] && echo "$err" | grep -q 'is disabled_manually on o/r, not active' && [ "$(writes)" -eq 0 ] && [ "$(claude_runs)" -eq 0 ]
result absent_verdict_workflow_exits_5 $?
fi

# A missing credential: exit 3, the name and never a value, before any API call.
if want missing_token_exits_3_without_value; then
setup; alerts '.number == 11'
CCT="" run; ok=1
[ $rc -eq 3 ] && echo "$err" | grep -q 'CLAUDE_CODE_OAUTH_TOKEN is empty' && no_value && [ ! -s "$D/calls" ] && ok=0
RELT="" run
[ $ok -eq 0 ] && [ $rc -eq 3 ] && echo "$err" | grep -q 'RELEASE_TOKEN is empty' && no_value && [ ! -s "$D/calls" ] && [ "$(claude_runs)" -eq 0 ]
result missing_token_exits_3_without_value $?
fi

# The Dependabot alerts off: an actionable error that names the setting, exit 4, no claude -p run.
if want alerts_off_names_the_setting; then
setup; fail_api "repos/$REPO/dependabot/alerts" 403 'Dependabot alerts are disabled for this repository.'
cp "$FIX/alerts.disabled.json" "$D/api/$(key "repos/$REPO/dependabot/alerts")"
run
[ $rc -eq 4 ] && echo "$err" | grep -q 'the Dependabot alerts are off' && echo "$err" | grep -q 'turn on the setting Dependabot alerts' && [ "$(writes)" -eq 0 ] && [ "$(claude_runs)" -eq 0 ]
result alerts_off_names_the_setting $?
fi

# One alert, fixed: one claude -p run with the limits and no GitHub token, the verdict of the script with no
# token, one fix(deps) commit by the script, the push, the pull request, its squash merge, the release pull
# request merge, and the close of the open autofix issue; the item line holds the tokens and the cost.
if want one_alert_fixed_merged_and_released; then
setup; alerts '.number == 11'; with_release 0.1.1
serve "repos/$REPO/issues" '[{"number": 41, "title": "Dependency autofix: failed items", "html_url": "https://github.com/o/r/issues/41", "body": "x\n<!-- deps-autofix:held\nalert:12 input:patched=0.25.0;main=abc date:2026-10-23 reason:verdict_failed\n-->"}]'
edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak'
run
args="$(cat "$D/claude.args" 2>/dev/null)"
[ $rc -eq 0 ] && echo "$out" | grep -q '^item: alert:11 path: alert package: smallvec ecosystem: cargo status: fixed .* model: claude-sonnet-4-6 input_tokens: 30 cache_creation_input_tokens: 20000 cache_read_input_tokens: 300000 output_tokens: 4000 turns: 14 cost_usd: 0.22509 total_cost_usd: 0.22509$' \
  && [ "$(claude_runs)" -eq 1 ] && echo "$args" | grep -qx -- '--max-turns' && echo "$args" | grep -qx 30 && echo "$args" | grep -qx -- '--max-budget-usd' \
  && echo "$args" | grep -qx sonnet && echo "$args" | grep -qxF 'Bash(git push:*)' && grep -q 'Package: smallvec' "$D/claude.prompt" \
  && grep -q 'Vulnerable version range: >= 1.0.0, < 1.13.3' "$D/claude.prompt" && grep -q 'Advisory identifier: GHSA-aaaa-bbbb-cccc' "$D/claude.prompt" \
  && [ "$(remote_commits)" = 'fix(deps): bump smallvec to 1.13.3 (GHSA-aaaa-bbbb-cccc)' ] \
  && [ "$(git -C "$D/origin.git" log -1 --format=%an "$BRANCH")" = 'github-actions[bot]' ] && [ "$(git -C "$D/origin.git" log --format=%s main | wc -l | tr -d ' ')" -eq 1 ] \
  && grep -qx "POST repos/$REPO/pulls" "$D/calls" && grep -qx "PUT repos/$REPO/pulls/31/merge" "$D/calls" && grep -qx "PUT repos/$REPO/pulls/2/merge" "$D/calls" \
  && echo "$out" | grep -q '^item: release:2 path: release package: 0.1.1 ecosystem: - status: merged' \
  && grep -qx "PATCH repos/$REPO/issues/41" "$D/calls" && echo "$out" | grep -qx 'action: issue target: #41 status: done detail: closed' \
  && echo "$out" | grep -q 'result: alerts: 1 pull_requests: 0 fixed: 1 merged: 2 failed: 0 held: 0 waiting: 0 claude_runs: 1 exit: 0'
result one_alert_fixed_merged_and_released $?
fi

# The credential rule on the same run: claude -p gets CLAUDE_CODE_OAUTH_TOKEN and no GitHub token; the verdict of
# the script gets no token; the writes that start a workflow use the release token; no value in the output,
# in git, or in the git configuration.
if want credentials_stay_out_of_claude_verdict_and_output; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak'
run
merge_env="$D/gh.env.$(grep -n "^PUT repos/$REPO/pulls/31/merge" "$D/calls" | cut -d: -f1 | awk '{print $1 - 1}')"
[ $rc -eq 0 ] && grep -qx "CLAUDE_CODE_OAUTH_TOKEN=$T_CC" "$D/claude.env" && ! grep -qE '^(GH_TOKEN|GITHUB_TOKEN|RELEASE_TOKEN)=' "$D/claude.env" \
  && ! grep -qF -e "$T_GH" -e "$T_REL" "$D/claude.env" && [ -s "$D/verdict.env.1" ] && ! grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC" "$D/verdict.env."* \
  && grep -qx "GH_TOKEN=$T_REL" "$merge_env" && no_value \
  && ! git -C "$D/origin.git" log -p --all | grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC" && ! grep -rqF -e "$T_GH" -e "$T_REL" -e "$T_CC" "$D/git/.git/config" "$D/git/Cargo.lock"
result credentials_stay_out_of_claude_verdict_and_output $?
fi

# A result with no structured result: failed with invalid_result, no commit, one issue with the label; exit 1.
if want invalid_result_counts_as_failed; then
setup; alerts '.number == 11'; cp "$FIX/claude.result.invalid.json" "$D/claude.out"; edit 'echo junk >>Cargo.lock'
run
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: alert:11 .* status: failed reason: invalid_result' && [ -z "$(remote_commits)" ] \
  && [ -z "$(git -C "$D/git" status --porcelain)" ] && ! grep -q junk "$D/git/Cargo.lock" \
  && grep -qx "POST repos/$REPO/issues" "$D/calls" && grep -l 'deps-autofix:held' "$D/bodies/"* >/dev/null && ! grep -q "PUT repos/$REPO/pulls" "$D/calls"
result invalid_result_counts_as_failed $?
fi

# A diff outside the allowed paths fails with path_not_allowed and leaves the branch at its last good commit.
if want path_outside_allowlist_fails; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak && echo "on: push" >.github/workflows/x.yml'
run
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: alert:11 .* status: failed reason: path_not_allowed' && [ ! -e "$D/git/.github/workflows/x.yml" ] \
  && [ -z "$(git -C "$D/git" status --porcelain)" ] && [ -z "$(remote_commits)" ] && [ ! -e "$D/verdict.calls" ]
result path_outside_allowlist_fails $?
fi

# A code change is allowed only when the verdict fails after the version change alone.
if want code_change_only_after_failed_version_verdict; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak && echo "pub fn g() {}" >>src/lib.rs'
printf '0\n' >"$D/verdict_rcs"; run; ok=1
[ $rc -eq 1 ] && echo "$out" | grep -q 'status: failed reason: path_not_allowed' && [ "$(wc -l <"$D/verdict.calls" | tr -d ' ')" -eq 1 ] && ok=0
rm -rf "$D/verdict.calls" "$D/claude.calls"; printf '1\n0\n' >"$D/verdict_rcs"; run
[ $ok -eq 0 ] && [ $rc -eq 0 ] && echo "$out" | grep -q 'status: fixed' && [ "$(wc -l <"$D/verdict.calls" | tr -d ' ')" -eq 2 ] \
  && git -C "$D/origin.git" show "$BRANCH" -- src/lib.rs | grep -q 'pub fn g'
result code_change_only_after_failed_version_verdict $?
fi

# A credential in the diff fails with credential_in_diff, and the matched text is written nowhere.
if want credential_in_diff_fails; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak && echo "# ghp_testvalue0000" >>Cargo.toml'
run
[ $rc -eq 1 ] && echo "$out" | grep -q 'status: failed reason: credential_in_diff' && ! printf '%s\n%s\n' "$out" "$err" | grep -q 'ghp_testvalue0000' \
  && ! cat "$D/bodies/"* 2>/dev/null | grep -q 'ghp_testvalue0000' && [ -z "$(remote_commits)" ] && [ ! -e "$D/verdict.calls" ]
result credential_in_diff_fails $?
fi

# The issue extract holds the last log lines with each credential pattern and each token value removed.
if want redaction_removes_credentials; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak'; cp "$FIX/item.log.txt" "$D/verdict.log"; echo 1 >"$D/verdict_rcs"
GHT=TEST-RUN-TOKEN-VALUE run
c="$(grep -l 'Last 200 lines' "$D/bodies/"* | head -n 1)"
[ $rc -eq 1 ] && echo "$out" | grep -q 'status: failed reason: verdict_failed' && [ -n "$c" ] && grep -q 'verdict: test failed' "$c" \
  && [ "$(grep -o '\[redacted\]' "$c" | wc -l | tr -d ' ')" -eq 6 ] \
  && ! grep -qE 'sk-ant-oat|ghp_t|github_pat_t|gho_t|ghs_t|TEST-RUN-TOKEN-VALUE' "$c" && grep -q 'smallvec' "$c" && grep -q 'Run: https://github.com/o/r/actions/runs/1' "$c"
result redaction_removes_credentials $?
fi

# A failed item is held: no new claude -p run within 7 days with the same input; a run after 7 days, a new input,
# and --retry-failed each end the hold.
if want hold_time_blocks_retry; then
setup; alerts '.number == 11'
m="$(git -C "$D/git" log -1 --format=%h origin/main -- Cargo.lock)"
issue() { serve "repos/$REPO/issues" "[{\"number\": 41, \"title\": \"Dependency autofix: failed items\", \"html_url\": \"https://github.com/o/r/issues/41\", \"body\": \"x\n<!-- deps-autofix:held\n$1\n-->\"}]"; }
issue "alert:11 input:patched=1.13.3;main=$m date:2026-10-20 reason:verdict_failed"; cp "$FIX/claude.result.invalid.json" "$D/claude.out"
run; r1=$rc; o1="$out"; c1="$(claude_runs)"; w1="$(writes)"
issue "alert:11 input:patched=1.13.3;main=$m date:2026-10-17 reason:verdict_failed"; run; c2="$(claude_runs)"
issue "alert:11 input:patched=1.13.2;main=$m date:2026-10-23 reason:verdict_failed"; run; c3="$(claude_runs)"
issue "alert:11 input:patched=1.13.3;main=$m date:2026-10-23 reason:verdict_failed"; run --retry-failed; c4="$(claude_runs)"
[ $r1 -eq 1 ] && echo "$o1" | grep -q '^item: alert:11 .* status: held reason: verdict_failed .* since: 2026-10-20$' && [ "$c1" -eq 0 ] && [ "$w1" -eq 0 ] \
  && [ "$c2" -eq 1 ] && [ "$c3" -eq 2 ] && [ "$c4" -eq 3 ]
result hold_time_blocks_retry $?
fi

# One green Dependabot pull request: a squash merge with the release token, no claude -p run, then the release
# pull request step; a release pull request of 1.0.0 is not merged and goes to the issue as release_needs_human.
if want dependabot_pr_merges_and_release_step; then
setup; pulls '.number == 21'; serve "repos/$REPO/pulls/21/files" @files.cargo.json; with_release 0.1.1
run; ok=1
[ $rc -eq 0 ] && echo "$out" | grep -q '^item: pr:21 path: pull_request package: smallvec ecosystem: cargo status: merged' && [ "$(claude_runs)" -eq 0 ] \
  && grep -qx "PUT repos/$REPO/pulls/21/merge" "$D/calls" && grep -qx "PUT repos/$REPO/pulls/2/merge" "$D/calls" && ok=0
rm -rf "$D/api/$(key "repos/$REPO/pulls")"; pulls '.number == 21'; with_release 1.0.0; : >"$D/calls"
run
[ $ok -eq 0 ] && [ $rc -eq 1 ] && echo "$out" | grep -q '^item: release:2 path: release package: 1.0.0 ecosystem: - status: failed reason: release_needs_human' \
  && ! grep -qx "PUT repos/$REPO/pulls/2/merge" "$D/calls" && grep -q 'release_needs_human' "$D/bodies/"*
result dependabot_pr_merges_and_release_step $?
fi

# On a private repository the fix pull request merges, but the release pull request stays with a person.
if want private_repo_release_stays_with_human; then
setup; serve "repos/$REPO" '{"private": true, "visibility": "private"}'; pulls '.number == 21'; serve "repos/$REPO/pulls/21/files" @files.cargo.json; with_release 0.1.1
run
[ $rc -eq 0 ] && grep -qx "PUT repos/$REPO/pulls/21/merge" "$D/calls" && ! grep -qx "PUT repos/$REPO/pulls/2/merge" "$D/calls" \
  && echo "$out" | grep -q '^item: release:2 path: release package: 0.1.1 ecosystem: - status: waiting reason: release_needs_human'
result private_repo_release_stays_with_human $?
fi

# A later run with nothing open merges a green release pull request whose changelog lists a fix(deps) change.
if want release_pr_with_deps_fix_merged_later; then
setup; serve "repos/$REPO/pulls" @pulls.release.json
run
[ $rc -eq 0 ] && echo "$out" | grep -qx 'count: alerts: 0 pull_requests: 0' && grep -qx "PUT repos/$REPO/pulls/2/merge" "$D/calls" && [ "$(writes)" -eq 1 ]
result release_pr_with_deps_fix_merged_later $?
fi

# A Dependabot pull request behind main gets the rebase command with the release token, then merges on its new head.
if want behind_pr_is_rebased_then_merged; then
setup; pulls '.number == 21'; serve "repos/$REPO/pulls/21/files" @files.cargo.json; serve "repos/$REPO/compare/main...2121212121212121212121212121212121212121" @compare.behind.json
serve "repos/$REPO/pulls/21" '{"number": 21, "head": {"sha": "2424242424242424242424242424242424242424"}}'
run
[ $rc -eq 0 ] && grep -qx "POST repos/$REPO/issues/21/comments" "$D/calls" && grep -q '@dependabot rebase' "$D/bodies/"* \
  && grep -qx "PUT repos/$REPO/pulls/21/merge" "$D/calls" && echo "$out" | grep -q 'status: merged'
result behind_pr_is_rebased_then_merged $?
fi

# A red cargo Dependabot pull request goes to one claude -p run; the fix closes it with a link to the autofix pull request.
if want red_dependabot_pr_goes_to_claude; then
setup; pulls '.number == 21'; serve "repos/$REPO/pulls/21/files" @files.cargo.json; serve "repos/$REPO/commits/2121212121212121212121212121212121212121/check-runs" @checks.failure.json
edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak'
run
[ $rc -eq 0 ] && [ "$(claude_runs)" -eq 1 ] && grep -q 'Dependabot pull request 21 whose check verdict failed' "$D/claude.prompt" \
  && [ "$(remote_commits)" = 'fix(deps): bump smallvec to 1.13.3' ] && grep -qx "PATCH repos/$REPO/pulls/21" "$D/calls" \
  && ! grep -qx "PUT repos/$REPO/pulls/21/merge" "$D/calls"
result red_dependabot_pr_goes_to_claude $?
fi

# A pin update that replaces a SHA with a tag is not merged (pin_not_sha); one that keeps a SHA merges.
if want actions_pr_sha_to_tag_refused; then
setup; pulls '.number == 22'; serve "repos/$REPO/pulls/22/files" @files.actions_tag.json
run; ok=1
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: pr:22 path: pull_request package: actions/checkout ecosystem: github-actions status: failed reason: pin_not_sha' \
  && ! grep -q "PUT repos/$REPO/pulls/22/merge" "$D/calls" && [ "$(claude_runs)" -eq 0 ] && ok=0
serve "repos/$REPO/pulls/22/files" @files.actions_sha.json; serve "repos/$REPO/issues" '[]'; : >"$D/calls"
run
[ $ok -eq 0 ] && [ $rc -eq 0 ] && grep -qx "PUT repos/$REPO/pulls/22/merge" "$D/calls" && ! grep -q "pulls/2/merge" "$D/calls"
result actions_pr_sha_to_tag_refused $?
fi

# An authentication error of claude -p: no more claude -p runs, the issue names claude setup-token, exit 3.
if want claude_auth_stops_runs_and_exits_3; then
setup; alerts '.number == 11 or .number == 12'; cp "$FIX/claude.result.auth.json" "$D/claude.out"
run
[ $rc -eq 3 ] && [ "$(claude_runs)" -eq 1 ] && echo "$out" | grep -q 'status: failed reason: claude_auth' && echo "$out" | grep -q 'status: waiting reason: claude_auth' \
  && grep -q 'claude setup-token' "$D/bodies/"*
result claude_auth_stops_runs_and_exits_3 $?
fi

# A claude -p run past the process timeout is ended and fails with the reason timeout.
if want claude_timeout_fails_item; then
setup; alerts '.number == 11'; echo 1 >"$D/claude_timeout_secs"; echo 20 >"$D/claude.sleep"
start=$SECONDS; run; took=$((SECONDS - start))
[ $rc -eq 1 ] && echo "$out" | grep -q 'status: failed reason: timeout' && [ "$took" -lt 15 ] && [ -z "$(remote_commits)" ]
result claude_timeout_fails_item $?
fi

# The interface: --help with the exit codes, an unknown flag and a raised turn limit exit 2, a lower turn limit
# reaches claude -p, and --human gives the friendly view.
if want interface_help_usage_and_turn_limit; then
setup; alerts '.number == 11'; edit 'sed -i.bak "s/1.13.2/1.13.3/" Cargo.lock && rm Cargo.lock.bak'
h="$(/bin/bash "$D/git/.agents/scripts/deps.autofix.sh" --help)"; run --bogus; r1=$rc; e1="$err"; run --max-turns 31; r2=$rc
run --max-turns 3; r3=$rc; a3="$(cat "$D/claude.args")"
echo "$h" | grep -q '^Exit codes: 0 ' && echo "$h" | grep -q '5 the verdict workflow is' && [ $r1 -eq 2 ] && echo "$e1" | grep -q 'unknown argument --bogus' \
  && [ $r2 -eq 2 ] && [ $r3 -eq 0 ] && echo "$a3" | grep -A1 -x -- '--max-turns' | grep -qx 3
result interface_help_usage_and_turn_limit $?
fi

# The workflow is a thin caller: daily at 04:17 UTC and on dispatch, never on a push or a pull request, one
# concurrency group that never cancels, the six permissions, one job with a timeout of 180 minutes, a checkout
# with no persisted credential, SHA-pinned actions, one call of the script, and the subscription token only in
# the step of the script; no model, branch name, label, or tool version.
if want workflow_is_thin_caller; then
D="$(mktemp -d)"; : >"$D/calls"; out=""; err=""; rc=0
W="$ROOT/.github/workflows/deps-autofix.yml"
perms="$(awk '/^permissions:/ {f = 1; next} f && /^[^ ]/ {f = 0} f && NF {gsub(/ /, ""); print}' "$W" | sort | tr '\n' ' ')"
step="$(awk '/^      - / {s = ""} {s = s $0 "\n"} /^        run: \.agents\/scripts\/deps\.autofix\.sh/ {printf "%s", s}' "$W")"
[ -f "$W" ] && grep -qx "    - cron: '17 4 \* \* \*'" "$W" && grep -qx '  workflow_dispatch:' "$W" && ! grep -qE '^  (push|pull_request)' "$W" \
  && grep -qx '  group: deps-autofix' "$W" && grep -qx '  cancel-in-progress: false' "$W" \
  && [ "$perms" = "actions:read checks:read contents:write issues:write pull-requests:write security-events:read " ] \
  && [ "$(grep -c 'runs-on:' "$W")" -eq 1 ] && grep -qx '    timeout-minutes: 180' "$W" && grep -qx '          persist-credentials: false' "$W" \
  && [ "$(grep -c 'run: .agents/scripts/deps.autofix.sh' "$W")" -eq 1 ] && [ "$(grep -c 'secrets.CLAUDE_CODE_OAUTH_TOKEN' "$W")" -eq 1 ] \
  && echo "$step" | grep -q 'secrets.CLAUDE_CODE_OAUTH_TOKEN' && echo "$step" | grep -q 'secrets.RELEASE_PLEASE_TOKEN' \
  && [ "$(grep -E '^ *(- )?uses:' "$W" | grep -vcE 'uses: [^@ ]+@[0-9a-f]{40} # v[0-9]')" -eq 0 ] \
  && ! sed -E 's/(^| )#.*$//' "$W" | grep -qiE 'sonnet|haiku|opus|deps/autofix|dependencies|--max|[0-9]+\.[0-9]+\.[0-9]+' \
  && ! grep -qE 'ghp_|github_pat_|gho_|ghs_|sk-ant-' "$W"
result workflow_is_thin_caller $?
fi

exit $failed
