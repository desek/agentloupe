#!/bin/bash
# @agents-index Shell tests of pi.session.sh: PI_CC_AUTH_EXT is required, a missing file is named, and the dry run uses the path.
#
# Usage: pi.session.test.sh. Each test runs with a fresh temp HOME and its own prompt file; no pi runs,
# because each test stops before the session or uses --dry-run. Prints one line per test:
# "test: <name> status: passed|failed". Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/.agents/scripts/pi.session.sh"
failed=0

setup() {
  H="$(mktemp -d)"; export HOME="$H"; unset PI_CC_AUTH_EXT
  mkdir -p "$H/.config"; printf 'lpt_test\n' > "$H/.config/agentloupe-token"
  printf 'say hi\n' > "$H/prompt"
}
result() { if [ "$2" -eq 0 ]; then echo "test: $1 status: passed"; else echo "test: $1 status: failed"; failed=1; fi; rm -rf "$H"; }
run() { bash "$SCRIPT" --name t --prompt-file "$H/prompt" --log "$H/pi.log" --dry-run; }

setup; err="$(run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 3 ] && echo "$err" | grep -q 'PI_CC_AUTH_EXT' && echo "$err" | grep -q 'Fix:' \
  && echo "$err" | grep -q 'Verify:'; result unset_variable $?

setup; err="$(PI_CC_AUTH_EXT=  run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 3 ] && echo "$err" | grep -q 'PI_CC_AUTH_EXT'; result empty_variable $?

setup; err="$(PI_CC_AUTH_EXT="$H/absent/index.ts" run 2>&1 >/dev/null)"; rc=$?
[ $rc -eq 3 ] && echo "$err" | grep -q 'PI_CC_AUTH_EXT' && echo "$err" | grep -q "$H/absent/index.ts"; result missing_file $?

setup; mkdir -p "$H/ext"; : > "$H/ext/index.ts"
out="$(PI_CC_AUTH_EXT="$H/ext/index.ts" run)"; rc=$?
[ $rc -eq 0 ] && printf '%s\n' "$out" | grep -q "^command: .* -e $H/ext/index.ts " \
  && printf '%s\n' "$out" | grep -qx "log: $H/pi.log"; result dry_run_uses_variable $?

exit $failed
