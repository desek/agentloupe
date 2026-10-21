#!/bin/bash
# @agents-index Shell tests of agent.connect.sh: output, Pi file modes, idempotence, errors, dry run, and isolation from global agent settings.
#
# Usage: agent.connect.test.sh. Each test runs with a fresh temp HOME. Prints one line per test:
# "test: <name> status: passed|failed". Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/.agents/scripts/agent.connect.sh"
failed=0

setup() {
  H="$(mktemp -d)"; export HOME="$H"; unset XDG_STATE_HOME LOUPE_PROXY_TOKEN
  printf 'lpt_test\n' > "$H/token"
}
result() { if [ "$2" -eq 0 ]; then echo "test: $1 status: passed"; else echo "test: $1 status: failed"; failed=1; fi; rm -rf "$H"; }
mode() { stat -f '%Lp' "$1" 2>/dev/null || stat -c '%a' "$1"; }
# snap lists every path under the global agent dirs with its mode and checksum.
snap() { (cd "$H" && find .claude .pi | sort | while read -r f; do echo "$f $(mode "$f") $( [ -f "$f" ] && cksum < "$f")"; done); }
PIDIR=".local/state/agentloupe/pi-agent"

setup; out="$(bash "$SCRIPT" --agent claude-code --token-file "$H/token")"; rc=$?
n="$(printf '%s\n' "$out" | grep -c '^export ANTHROPIC_BASE_URL=http://127.0.0.1:8787/lpt_test/claude-code/anthropic$')"
[ $rc -eq 0 ] && [ "$n" -eq 1 ] && [ "$(printf '%s\n' "$out" | grep -c '^export')" -eq 1 ]; result claude_code_prints_base_url $?

setup; out="$(bash "$SCRIPT" --agent pi --token-file "$H/token")"; rc=$?
[ $rc -eq 0 ] && [ "$(cat "$H/$PIDIR/models.json")" = '{"providers":{"anthropic":{"baseUrl":"http://127.0.0.1:8787/lpt_test/pi/anthropic"}}}' ] \
  && [ "$(mode "$H/$PIDIR/models.json")" = 600 ] && [ "$(mode "$H/$PIDIR")" = 700 ] \
  && printf '%s\n' "$out" | grep -qx "export PI_CODING_AGENT_DIR=$H/$PIDIR"; result pi_writes_models_json $?

setup; o1="$(bash "$SCRIPT" --agent pi --token-file "$H/token")"; b1="$(cat "$H/$PIDIR/models.json")"
o2="$(bash "$SCRIPT" --agent pi --token-file "$H/token")"; b2="$(cat "$H/$PIDIR/models.json")"
[ "$o1" = "$o2" ] && [ "$b1" = "$b2" ] && [ "$(ls -A "$H/$PIDIR")" = models.json ]; result pi_is_idempotent $?

setup; err="$(bash "$SCRIPT" --agent pi 2>&1 >/dev/null)"; rc=$?
[ $rc -ne 0 ] && echo "$err" | grep -q -- '--token-file' && echo "$err" | grep -q 'loupe token create' \
  && echo "$err" | grep -q 'loupe token list' && [ ! -e "$H/$PIDIR" ]; result missing_token_is_actionable $?

setup; out="$(LOUPE_PROXY_TOKEN=lpt_secretvalue bash "$SCRIPT" --dry-run --agent pi)"; rc=$?
[ $rc -eq 0 ] && [ ! -e "$H/.local" ] && echo "$out" | grep -q 'lpt_…' && ! echo "$out" | grep -q 'lpt_secretvalue'; result dry_run_writes_nothing_and_masks $?

setup; mkdir -p "$H/.claude" "$H/.pi/agent"; echo '{}' > "$H/.claude/settings.json"; echo '{}' > "$H/.pi/agent/models.json"
before="$(snap)"
bash "$SCRIPT" --agent pi --token-file "$H/token" >/dev/null && bash "$SCRIPT" --agent claude-code --token-file "$H/token" >/dev/null
after="$(snap)"
[ "$before" = "$after" ]; result never_touches_global_dirs $?

setup; mkdir -p "$H/.pi/agent"; echo '{"anthropic":{"key":"sk-ant-fake"}}' > "$H/.pi/agent/auth.json"
out="$(bash "$SCRIPT" --agent pi --token-file "$H/token")"
[ ! -e "$H/$PIDIR/auth.json" ] && [ -z "$(find "$H/$PIDIR" -type l)" ] && ! grep -rq 'sk-ant' "$H/$PIDIR" \
  && ! echo "$out" | grep -q 'sk-ant'; result never_reads_pi_auth $?

setup; err="$(bash "$SCRIPT" --check --agent claude-code --token-file "$H/token" --base http://127.0.0.1:1 2>&1 >/dev/null)"; rc=$?
[ $rc -ne 0 ] && echo "$err" | grep -q 'docker compose up -d'; result check_fails_when_down $?

exit $failed
