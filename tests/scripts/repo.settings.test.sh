#!/bin/bash
# @agents-index Shell tests of repo.settings.sh with a fake gh: dry run, private stop, unavailable lines, idempotence, drift, in-place ruleset update, missing gh.
#
# Usage: repo.settings.test.sh. Each test runs the script against the repository o/r with a fake gh on PATH.
# The fake serves the state of a temp directory: one file per read endpoint with the output that the
# script's --jq would give, or a .http file with an error status; it records each call as
# "<method> <endpoint>" and writes nothing else. Prints one line per test:
# "test: <name> status: passed|failed". Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/.agents/scripts/repo.settings.sh"
failed=0

WANT_MAIN="enforcement=active; target=branch; include=~DEFAULT_BRANCH; exclude=; bypass=RepositoryRole:5:always; rules=deletion,non_fast_forward,pull_request,required_linear_history,required_status_checks; pull_request=0:squash; status_checks=false:verdict@15368"
WANT_TAGS="enforcement=active; target=tag; include=refs/tags/v*; exclude=; bypass=RepositoryRole:5:always; rules=creation,deletion,update; pull_request=; status_checks="
MERGE_ON="allow_squash_merge=true allow_merge_commit=false allow_rebase_merge=false delete_branch_on_merge=true allow_auto_merge=true squash_merge_commit_title=PR_TITLE squash_merge_commit_message=COMMIT_MESSAGES"
MERGE_OFF="allow_squash_merge=true allow_merge_commit=true allow_rebase_merge=true delete_branch_on_merge=false allow_auto_merge=false squash_merge_commit_title=COMMIT_OR_PR_TITLE squash_merge_commit_message=COMMIT_MESSAGES"

setup() {
  D="$(mktemp -d)"; mkdir -p "$D/bin" "$D/get"; : >"$D/calls"
  cat >"$D/bin/gh" <<'GH'
#!/bin/bash
# Fake gh: serves $FAKE_GH/get/<endpoint with / ? = & as _> and records each api call in $FAKE_GH/calls.
[ "${1:-}" = api ] || exit 0
shift
method=GET; endpoint=""
while [ $# -gt 0 ]; do
  case "$1" in
    --method|-X) method="$2"; shift 2 ;;
    --jq|-q|--input|-f|-F|-H) shift 2 ;;
    --silent|--paginate|-i) shift ;;
    *) [ -z "$endpoint" ] && endpoint="$1"; shift ;;
  esac
done
echo "$method $endpoint" >>"$FAKE_GH/calls"
[ "$method" = GET ] || exit 0
f="$FAKE_GH/get/$(echo "$endpoint" | tr '/?=&' '____')"
if [ -f "$f.http" ]; then echo "gh: fake error (HTTP $(cat "$f.http"))" >&2; exit 1; fi
[ -f "$f" ] && cat "$f"
exit 0
GH
  chmod +x "$D/bin/gh"
}
serve() { printf '%s\n' "$2" >"$D/get/$1"; }
fail_with() { printf '%s\n' "$2" >"$D/get/$1.http"; }
# A repository with each setting as given; $1 visibility, $2 merge words, $3 scanning status.
repo_state() {
  serve repos_o_r "admin=true
visibility=$1
merge=$2
scanning=secret_scanning=$3 secret_scanning_push_protection=$3"
}
# The public repository with every setting at the desired state.
desired() {
  repo_state public "$MERGE_ON" enabled
  serve repos_o_r_rulesets_includes_parents_false "1 main
2 release-tags"
  serve repos_o_r_rulesets_1 "$WANT_MAIN"
  serve repos_o_r_rulesets_2 "$WANT_TAGS"
  serve repos_o_r_actions_permissions "enabled=true allowed_actions=all sha_pinning_required=true"
  serve repos_o_r_actions_permissions_workflow "default_workflow_permissions=read can_approve_pull_request_reviews=false"
  : >"$D/get/repos_o_r_vulnerability-alerts"
  serve repos_o_r_private-vulnerability-reporting true
}
# The public repository with every setting off and no ruleset.
all_off() {
  repo_state public "$MERGE_OFF" disabled
  serve repos_o_r_rulesets_includes_parents_false ""
  serve repos_o_r_actions_permissions "enabled=true allowed_actions=all sha_pinning_required=false"
  serve repos_o_r_actions_permissions_workflow "default_workflow_permissions=read can_approve_pull_request_reviews=false"
  fail_with repos_o_r_vulnerability-alerts 404
  serve repos_o_r_private-vulnerability-reporting false
}
run() { PATH="$D/bin:$PATH" FAKE_GH="$D" /bin/bash "$SCRIPT" --repo o/r "$@"; }
writes() { grep -vc '^GET ' "$D/calls"; }
result() { if [ "$2" -eq 0 ]; then echo "test: $1 status: passed"; else echo "test: $1 status: failed"; failed=1; fi; rm -rf "$D"; }

setup; all_off; out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && [ "$(printf '%s\n' "$out" | grep -c '^setting: .* status: planned')" -eq 7 ] && [ "$(writes)" -eq 0 ] \
  && printf '%s\n' "$out" | tail -n 1 | grep -qx 'result: planned writes: 0 unavailable: 0 exit: 0'; result dry_run_writes_nothing $?

setup; all_off; repo_state private "$MERGE_OFF" none; err="$(run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 5 ] && echo "$err" | grep -q 'visibility private' && [ "$(writes)" -eq 0 ]; result private_apply_stops $?

setup; all_off; repo_state private "$MERGE_OFF" none; err="$(run --check 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 5 ] && [ "$(writes)" -eq 0 ]; result private_check_stops $?

setup; all_off; repo_state private "$MERGE_OFF" none
fail_with repos_o_r_rulesets_includes_parents_false 403; fail_with repos_o_r_private-vulnerability-reporting 404
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && printf '%s\n' "$out" | grep -q '^setting: main-ruleset status: unavailable detail: HTTP 403: the repository is private' \
  && printf '%s\n' "$out" | grep -q '^setting: tag-ruleset status: unavailable detail: HTTP 403' \
  && printf '%s\n' "$out" | grep -q '^setting: secret-scanning status: unavailable' \
  && printf '%s\n' "$out" | grep -q '^setting: private-vulnerability-reporting status: unavailable detail: HTTP 404' \
  && printf '%s\n' "$out" | grep -q '^setting: merge-settings status: planned' \
  && printf '%s\n' "$out" | grep -q '^setting: actions-policy status: planned' \
  && printf '%s\n' "$out" | tail -n 1 | grep -qx 'result: planned writes: 0 unavailable: 4 exit: 0' && [ "$(writes)" -eq 0 ]; result private_dry_run_unavailable $?

setup; all_off; out="$(run)"; rc=$?
[ $rc -eq 0 ] && [ "$(printf '%s\n' "$out" | grep -c '^setting: .* status: applied')" -eq 7 ] \
  && grep -qx 'POST repos/o/r/rulesets' "$D/calls" && grep -qx 'PATCH repos/o/r' "$D/calls" \
  && grep -qx 'PUT repos/o/r/actions/permissions' "$D/calls" && ! grep -q 'PUT repos/o/r/actions/permissions/workflow' "$D/calls" \
  && grep -qx 'PUT repos/o/r/vulnerability-alerts' "$D/calls" && grep -qx 'PUT repos/o/r/private-vulnerability-reporting' "$D/calls" \
  && ! grep -q '^DELETE' "$D/calls" && printf '%s\n' "$out" | tail -n 1 | grep -qx "result: applied writes: $(writes) unavailable: 0 exit: 0"; result apply_writes_each_difference $?

setup; desired; out="$(run)"; rc=$?; out2="$(run --check)"; rc2=$?
[ $rc -eq 0 ] && [ $rc2 -eq 0 ] && [ "$(printf '%s\n' "$out" | grep -c '^setting: .* status: unchanged$')" -eq 7 ] && [ "$(writes)" -eq 0 ] \
  && printf '%s\n' "$out" | tail -n 1 | grep -qx 'result: unchanged writes: 0 unavailable: 0 exit: 0'; result second_run_is_idempotent $?

setup; desired; repo_state public "${MERGE_ON/allow_rebase_merge=false/allow_rebase_merge=true}" enabled; out="$(run --check)"; rc=$?
[ $rc -eq 1 ] && [ "$(printf '%s\n' "$out" | grep -c 'status: drift')" -eq 1 ] \
  && printf '%s\n' "$out" | grep -qx 'setting: merge-settings status: drift detail: allow_rebase_merge true to false' && [ "$(writes)" -eq 0 ]; result check_reports_drift $?

setup; desired; serve repos_o_r_rulesets_1 "${WANT_MAIN/required_linear_history,/}"; out="$(run)"; rc=$?
[ $rc -eq 0 ] && grep -qx 'PUT repos/o/r/rulesets/1' "$D/calls" && ! grep -q '^POST' "$D/calls" && [ "$(writes)" -eq 1 ] \
  && printf '%s\n' "$out" | grep -q '^setting: main-ruleset status: applied detail: update rules of the ruleset main (1)'; result ruleset_updated_in_place $?

setup; rm "$D/bin/gh"; err="$(PATH="$D/bin" /bin/bash "$SCRIPT" --dry-run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 3 ] && echo "$err" | grep -q 'gh is not installed' && echo "$err" | grep -q 'Fix:' && echo "$err" | grep -q 'Verify:'; result missing_gh $?

setup; desired; serve repos_o_r "admin=false
visibility=public"; err="$(run --dry-run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 3 ] && echo "$err" | grep -q 'no admin permission' && [ "$(writes)" -eq 0 ]; result no_admin_stops $?

setup; out="$(/bin/bash "$SCRIPT" --help)"; rc=$?; err="$(/bin/bash "$SCRIPT" --dry-run --check 2>&1 >/dev/null)"; rc2=$?
[ $rc -eq 0 ] && echo "$out" | grep -q -- '--check' && echo "$out" | grep -q 'Exit codes' && [ $rc2 -eq 2 ]; result help_and_usage $?

exit $failed
