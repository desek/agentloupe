#!/bin/bash
# @agents-index Shell tests of verdict.reuse.sh with a fake gh on recorded run records: hit, each soundness rule, fail closed, read-only, decision file, judge table.
#
# Usage: verdict.reuse.test.sh. Each test makes a scratch git repository (a base commit and a head commit),
# and serves the run records of tests/fixtures/verdict.reuse/ through a fake gh on PATH. The fixtures are
# recorded from the run records of desek/agentloupe on 2026-10-23 (the pull_request run 40028067522 and the
# push run 40029927658 of one tree, and the run 40031187766 as the current run), trimmed to the fields that
# the reuse rules read. Each test replaces the recorded tree hash with the tree of its scratch head, and
# changes one field for its rule. The fake records each call as "<method> <endpoint>" and writes nothing.
# Prints one line per test: "test: <name> status: passed|failed".
# Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/.agents/scripts/verdict.reuse.sh"
FIX="$ROOT/tests/fixtures/verdict.reuse"
REPO=desek/agentloupe
RECORDED_TREE=31da684888d7054e9acc7deb18e0ffa681f733a3
CURRENT=40031187766
PR_RUN=40028067522
PUSH_RUN=40029927658
failed=0

setup() {
  D="$(mktemp -d)"; mkdir -p "$D/bin" "$D/api" "$D/git"; : >"$D/calls"
  cat >"$D/bin/gh" <<'GH'
#!/bin/bash
# Fake gh api: records "<method> <endpoint>"; a field flag with no method makes gh POST, so it records POST.
[ "${1:-}" = api ] || exit 0
shift
method=""; endpoint=""; fields=0
while [ $# -gt 0 ]; do
  case "$1" in
    --method|-X) method="$2"; shift 2 ;;
    -f|-F|--field|--raw-field|--input) fields=1; shift 2 ;;
    -H|--header|--jq|-q|--template|-t) shift 2 ;;
    --paginate|--silent|-i|--include) shift ;;
    *) [ -z "$endpoint" ] && endpoint="$1"; shift ;;
  esac
done
[ -n "$method" ] || { [ $fields -eq 1 ] && method=POST || method=GET; }
echo "$method $endpoint" >>"$FAKE_GH/calls"
[ "$method" = GET ] || exit 0
path="${endpoint%%\?*}"
case "$endpoint" in *[?\&]page=*) page="${endpoint##*[?&]page=}"; page="${page%%&*}"; path="${path}_page$page" ;; esac
f="$FAKE_GH/api/$(echo "$path" | tr '/.' '__')"
if [ -f "$f.http" ]; then echo "gh: fake error (HTTP $(cat "$f.http"))" >&2; exit 1; fi
[ -f "$f" ] || { echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
cat "$f"
GH
  chmod +x "$D/bin/gh"
  # The scratch clone: a base commit and a head commit on top of it.
  (cd "$D/git" && git init -q && git config user.email t@example.com && git config user.name t \
    && echo base >f && git add f && git commit -qm base && echo head >f && git commit -qam head) || exit 1
  HEAD_SHA="$(git -C "$D/git" rev-parse HEAD)"; BASE_SHA="$(git -C "$D/git" rev-parse HEAD^)"
  TREE="$(git -C "$D/git" rev-parse 'HEAD^{tree}')"
}
key() { echo "repos/$REPO/$1" | tr '/.' '__'; }
# serve <endpoint path> <json>: the answer of a read call.
serve() { printf '%s\n' "$2" >"$D/api/$(key "$1")"; }
fail_with() { printf '%s\n' "$2" >"$D/api/$(key "$1").http"; }
# A run record from a recorded fixture with the scratch tree, then the jq edit of the test.
record() { sed "s/$RECORDED_TREE/$TREE/" "$FIX/run.$1.json" | jq -c "${2:-.}"; }
# The runs list page 1 with the given records.
runs() { serve "actions/workflows/verdict.yml/runs_page1" "$(jq -cs '{total_count: length, workflow_runs: .}' <<<"$*")"; }
jobs() { serve "actions/runs/$1/jobs" "$(jq -c "${3:-.}" "$FIX/jobs.$2.json")"; }
compare() { serve "compare/$BASE_SHA...$1" "{\"status\":\"$2\"}"; }
# The default world: the current push run, one recorded pull_request run and one push run of the tree.
world() {
  serve "actions/runs/$CURRENT" "$(jq -c . "$FIX/run.current.json")"
  runs "$(record pull_request)" "$(record push)"
  jobs "$PR_RUN" full_green; jobs "$PUSH_RUN" full_green
  compare "$(jq -r .head_sha "$FIX/run.pull_request.json")" ahead
}
run() { (cd "$D/git" && PATH="$D/bin:$PATH" FAKE_GH="$D" GH_TOKEN="${TOKEN-fake}" /bin/bash "$SCRIPT" lookup --repo "$REPO" \
  --commit "${COMMIT:-$HEAD_SHA}" --event push --base "$BASE_SHA" --run-id "$CURRENT" "$@"); }
writes() { grep -vc '^GET ' "$D/calls"; }
result() {
  if [ "$2" -eq 0 ] && [ "$(writes)" -eq 0 ]; then echo "test: $1 status: passed"; else
    echo "test: $1 status: failed"; echo "  out: ${out:-}"; echo "  calls: $(tr '\n' ';' <"$D/calls")"; failed=1; fi
  rm -rf "$D"
}

# A full green pull_request run of the same tree whose head contains the base: hit, the newest source.
setup; world; out="$(run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx "lookup: decision: hit reason: match tree: $TREE source_run: $PR_RUN source_commit: bc4c4869d8c9831734070d13ca9cb09ee84895ad"
result hit_on_equal_tree $?

# A push run alone is a source with no compare.
setup; world; runs "$(record push)"; out="$(run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -q "source_run: $PUSH_RUN" && ! grep -q compare "$D/calls"
result push_run_is_a_source $?

# A different tree: miss novel, exit 1 with no decision file.
setup; world; runs "$(record pull_request '.head_commit.tree_id = "0000000000000000000000000000000000000000"')"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -qx "lookup: decision: miss reason: novel tree: $TREE"
result miss_on_other_tree $?

# A prefix of the tree is no match: only the full 40 characters count.
setup; world; runs "$(record push '.head_commit.tree_id |= .[0:12]')"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result tree_prefix_is_no_match $?

# A run in which a reuse skipped four stages is never a source.
setup; world; runs "$(record push)"
jobs "$PUSH_RUN" full_green '.jobs |= map(if .name == "lint" or .name == "verdict" then . else .conclusion = "skipped" end)'
out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result reused_run_is_not_a_source $?

# A run with a failed stage (recorded) is no source even when its record says success.
setup; world; runs "$(record push)"; jobs "$PUSH_RUN" test_failed; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result failed_stage_is_not_a_source $?

# The window of 72 hours before the current run holds: 73 hours is out.
setup; world; runs "$(record push '.run_started_at = "2026-10-20T21:25:58Z"')"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result old_run_is_not_a_source $?

# A run from a fork does not count.
setup; world; runs "$(record pull_request '.head_repository.full_name = "someone/agentloupe"')"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result fork_run_is_not_a_source $?

# A run of another event does not count.
setup; world; runs "$(record push '.event = "workflow_dispatch"')"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result other_event_is_not_a_source $?

# A pull_request run whose head is behind or diverged from the base does not count.
setup; world; runs "$(record pull_request)"; compare bc4c4869d8c9831734070d13ca9cb09ee84895ad behind; out1="$(run)"; rc1=$?
compare bc4c4869d8c9831734070d13ca9cb09ee84895ad diverged; out="$(run)"; rc=$?
[ $rc1 -eq 1 ] && [ $rc -eq 1 ] && echo "$out1" | grep -q "reason: novel" && echo "$out" | grep -q "reason: novel"
result pr_run_behind_base_is_not_a_source $?

# The current run is never its own source.
setup; world; runs "$(record push ".id = $CURRENT")"; out="$(run)"; rc=$?
[ $rc -eq 1 ] && echo "$out" | grep -q "reason: novel"
result current_run_is_not_a_source $?

# A failed read of the run records fails closed with the cause, exit 4.
setup; world; fail_with "actions/workflows/verdict.yml/runs_page1" 502; out="$(run)"; rc=$?
[ $rc -eq 4 ] && echo "$out" | grep -q "decision: miss reason: lookup_failed tree: $TREE cause: the read of the run records (page 1) failed"
result api_error_fails_closed $?

# A failed compare and a record with no tree_id each fail closed.
setup; world; runs "$(record pull_request)"; fail_with "compare/$BASE_SHA...bc4c4869d8c9831734070d13ca9cb09ee84895ad" 500; out1="$(run)"; rc1=$?
runs "$(record push 'del(.head_commit.tree_id)')"; out="$(run)"; rc=$?
[ $rc1 -eq 4 ] && [ $rc -eq 4 ] && echo "$out1" | grep -q "lookup_failed" && echo "$out" | grep -q "lacks a field"
result unknown_answer_fails_closed $?

# Partial jobs (a count above the list) fail closed.
setup; world; runs "$(record push)"; jobs "$PUSH_RUN" full_green '.total_count = 7'; out="$(run)"; rc=$?
[ $rc -eq 4 ] && echo "$out" | grep -q "partial"
result partial_jobs_fail_closed $?

# A full first page with no end of the window is a page limit: fail closed.
setup; world
for p in 1 2 3 4 5; do serve "actions/workflows/verdict.yml/runs_page$p" "$(jq -c '{total_count: 500, workflow_runs: [range(100) as $i | . + {id: (1000 + $i)}]}' <<<"$(record push '.head_commit.tree_id = "1111111111111111111111111111111111111111"')")"; done
out="$(run)"; rc=$?
[ $rc -eq 4 ] && echo "$out" | grep -q "page limit"
result page_limit_fails_closed $?

# HEAD must equal the commit input, and the token must be set.
setup; world; out="$(COMMIT="$BASE_SHA" run)"; rc=$?
[ $rc -eq 4 ] && echo "$out" | grep -q "cause: HEAD $HEAD_SHA differs from the commit input $BASE_SHA"
result head_mismatch_fails_closed $?
setup; world; out="$(TOKEN="" run)"; rc=$?
[ $rc -eq 4 ] && echo "$out" | grep -q "cause: GH_TOKEN is not set" && [ ! -s "$D/calls" ]
result missing_token_fails_closed $?

# With a decision file, a miss exits 0 and the file holds the decision; a hit writes the source.
setup; world; runs "$(record push '.head_commit.tree_id = "2222222222222222222222222222222222222222"')"
out="$(run --decision-file "$D/out1")"; rc1=$?
fail_with "actions/workflows/verdict.yml/runs_page1" 500; out="$(run --decision-file "$D/out2")"; rc2=$?
rm -f "$D/api/"*.http; runs "$(record push)"; out="$(run --decision-file "$D/out3")"; rc3=$?
[ $rc1 -eq 0 ] && [ $rc2 -eq 0 ] && [ $rc3 -eq 0 ] && grep -qx decision=miss "$D/out1" && grep -qx reason=novel "$D/out1" \
  && grep -qx reason=lookup_failed "$D/out2" && grep -qx decision=hit "$D/out3" && grep -qx "source_run=$PUSH_RUN" "$D/out3" \
  && grep -qx "tree=$TREE" "$D/out3"
result decision_file_exits_0 $?

# A missing tool is exit 3 in both forms, and the decision file still says miss.
setup; world; mkdir "$D/nojq"; for t in git gh bash dirname mktemp; do ln -s "$(PATH="$D/bin:$PATH" command -v $t)" "$D/nojq/$t"; done
out="$(cd "$D/git" && PATH="$D/nojq" FAKE_GH="$D" GH_TOKEN=fake /bin/bash "$SCRIPT" lookup --repo "$REPO" --decision-file "$D/out" 2>/dev/null)"; rc=$?
[ $rc -eq 3 ] && grep -qx decision=miss "$D/out" && grep -qx reason=lookup_failed "$D/out"
result missing_tool_exits_3 $?

# Every call of every case above was a GET: result() fails a test with any other method. This case checks
# the calls of one full hit, which reads the current run, the list, the jobs, and the compare.
setup; world; out="$(run)"; rc=$?
[ $rc -eq 0 ] && [ "$(grep -c '^GET ' "$D/calls")" -eq 4 ] && [ "$(wc -l <"$D/calls" | tr -d ' ')" -eq 4 ]
result lookup_is_read_only $?

# The judge passes only for five successes, or for a hit with lint success and four skips; each of the
# 4^5 x 3 combinations runs, eight at a time, and prints "bad" when its exit code differs from the rule.
judge_case() {
  /bin/bash "$SCRIPT" judge --format "$1" --lint "$2" --test "$3" --web "$4" --image "$5" --decision "$6" >/dev/null 2>&1
  [ $? -eq "$7" ] || echo "bad: $*"
}
export -f judge_case; export SCRIPT
for f in success failure cancelled skipped; do for l in success failure cancelled skipped; do for t in success failure cancelled skipped; do
for w in success failure cancelled skipped; do for i in success failure cancelled skipped; do for d in hit miss none; do
  want=1
  [ "$f$l$t$w$i" = successsuccesssuccesssuccesssuccess ] && want=0
  [ "$d" = hit ] && [ "$l" = success ] && [ "$f$t$w$i" = skippedskippedskippedskipped ] && want=0
  echo "$f $l $t $w $i $d $want"
done; done; done; done; done; done >"${TMPDIR:-/tmp}/judge.cases.$$"
n="$(wc -l <"${TMPDIR:-/tmp}/judge.cases.$$" | tr -d ' ')"
bad="$(sed 's/ none / "" /' "${TMPDIR:-/tmp}/judge.cases.$$" | xargs -P 8 -L 1 /bin/bash -c 'judge_case "$@"' _)"
rm -f "${TMPDIR:-/tmp}/judge.cases.$$"
/bin/bash "$SCRIPT" judge --format skipped --lint success --test skipped --web skipped --image skipped >/dev/null 2>&1; absent=$?
[ -z "$bad" ] && [ "$n" -eq 3072 ] && [ $absent -eq 1 ]
D="$(mktemp -d)"; : >"$D/calls"; out="$bad"; result judge_truth_table $?

exit $failed
