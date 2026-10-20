#!/usr/bin/env bash
# @agents-index Times loupe aggregate over one day with N synthetic calls in a temporary store; prints build_secs.
#
# Usage: days.bench.sh [--calls N] [--human]
#   --calls N   Synthetic calls on the day, over 50 segments, 3 agents, and 4 models. Default: 10000.
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): calls, build_secs, threshold_secs, status, log.
# The script builds the release binary, migrates a temporary store, seeds the calls with sqlite3,
# then times loupe aggregate --date 2026-10-10 (the build and the write of the day).
# Full output: .agents/logs/days.bench.log.
# Exit codes: 0 measured and at most the threshold; 1 above the threshold or a run failed;
# 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/days.bench.log"
THRESHOLD_SECS=2
DAY=2026-10-10
calls=10000; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --calls)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --calls needs a positive whole number. Fix: pass --calls 10000. Verify: the script prints build_secs." >&2; exit 2; }
      calls="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints build_secs." >&2; exit 2 ;;
  esac
done

for tool in cargo perl sqlite3; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the build time cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see build_secs." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run days.bench.sh and see build_secs. Log: .agents/logs/days.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 \
  || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release/loupe"
key="$("$BIN" gen-key)"
envs=(PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC)
# A dry run of an empty day would not migrate; this run migrates the empty store.
(cd "$ROOT" && env -i "${envs[@]}" "$BIN" aggregate --date 2001-01-01) >>"$LOG" 2>&1 \
  || fail "loupe aggregate could not migrate the store" "read the log"
sqlite3 "$work/loupe.db" >>"$LOG" 2>&1 <<SQL || fail "the seed of the synthetic calls failed" "read the log"
WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < 50)
INSERT INTO sessions (id, agent_label, key_hash, key_source, segment, started_at, last_call_at, closed_at, state, call_count)
SELECT i, 'agent-' || (i % 3), 'k' || i, 'header', 1, '${DAY}T00:00:00.000Z', '${DAY}T23:00:00.000Z', '${DAY}T23:59:00.000Z', 'closed', 1 FROM s;
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < $calls)
INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, session_id, model, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit)
SELECT 'e' || i, 0, '${DAY}T' || printf('%02d', i % 24) || ':00:00.000Z', 'agent-' || ((i % 50 + 1) % 3), 'anthropic', i % 50 + 1,
       'model-' || (i % 4), 200, 1000, 20000, 300, 0, 500, CASE WHEN i % 10 = 0 THEN NULL ELSE '0.012345678901' END, 'c' FROM n;
SQL

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
t0="$(now)"
(cd "$ROOT" && env -i "${envs[@]}" "$BIN" aggregate --date "$DAY") >>"$LOG" 2>&1 || fail "loupe aggregate failed" "read the log"
secs="$(perl -e "printf '%.2f', $(now) - $t0")"
got="$(sqlite3 "$work/loupe.db" "SELECT json_extract(record, '\$.calls') FROM days WHERE day = '$DAY'")"
[[ "$got" == "$calls" ]] || fail "the record holds $got calls, not $calls" "check build_day in src/day/build.rs"
status=passed; perl -e "exit($secs <= $THRESHOLD_SECS ? 0 : 1)" || status=failed
echo "calls=$calls build_secs=$secs status=$status" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The build of a day with $calls calls took $secs seconds. Threshold: $THRESHOLD_SECS seconds. Result: $status."
else
  echo "calls: $calls build_secs: $secs threshold_secs: $THRESHOLD_SECS status: $status log: .agents/logs/days.bench.log"
fi
[[ $status != failed ]] || fail "build_secs $secs is above $THRESHOLD_SECS" "profile build_day in src/day/build.rs"
