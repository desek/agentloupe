#!/usr/bin/env bash
# @agents-index Times GET /private/api/v1/report over 400 days with 100 000 seeded calls and the bearer admin token; prints time_total.
#
# Usage: private.bench.sh [--calls N] [--human]
#   --calls N   Calls to seed, spread over the 400 days that end today in UTC. Default: 100000.
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): calls, days, rows, time_total, threshold, status, log.
# The script builds the release binary, migrates a temporary store, seeds 4 proxy tokens, 10 000
# sessions, and N calls with sqlite3, starts loupe serve with a temporary admin token on a free port,
# and times one warm report request with curl. Full output: .agents/logs/private.bench.log.
# Exit codes: 0 measured and below the threshold; 1 at or above the threshold or a run failed;
# 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/private.bench.log"
THRESHOLD=1.0
calls=100000; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --calls)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --calls needs a positive whole number. Fix: pass --calls 100000. Verify: the script prints time_total." >&2; exit 2; }
      calls="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints time_total." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl sqlite3; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the report time cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see time_total." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run private.bench.sh and see time_total. Log: .agents/logs/private.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 \
  || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release/loupe"
free_port() { perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport'; }
wait_port() { for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$1/$2" && return 0; sleep 0.1; done; return 1; }
key="$("$BIN" gen-key)"
# A temporary admin token of 43 characters; it lives only in this process and the child.
admin="$("$BIN" token generate)"
envs=(PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC)
(cd "$ROOT" && env -i "${envs[@]}" "$BIN" aggregate --date 2001-01-01) >>"$LOG" 2>&1 \
  || fail "loupe aggregate could not migrate the store" "read the log"
sqlite3 "$work/loupe.db" >>"$LOG" 2>&1 <<SQL || fail "the seed of the calls failed" "read the log"
PRAGMA foreign_keys = OFF;
INSERT INTO proxy_tokens (id, name, token_hash, tags, created_at) VALUES
 (1, 'laptop', 'h1', '{"owner":"alice"}', '2025-01-01T00:00:00.000Z'),
 (2, 'desktop', 'h2', '{"owner":"bob"}', '2025-01-01T00:00:00.000Z'),
 (3, 'ci', 'h3', '{}', '2025-01-01T00:00:00.000Z'),
 (4, 'server', 'h4', '{"owner":"alice","env":"prod"}', '2025-01-01T00:00:00.000Z');
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 10000)
INSERT INTO sessions (id, agent_label, proxy_token_id, key_hash, key_source, segment, started_at, last_call_at, state, call_count)
SELECT i, 'agent-' || (i % 3), CASE WHEN i % 5 = 0 THEN NULL ELSE i % 4 + 1 END, 'k' || i, 'header', 1,
 strftime('%Y-%m-%dT%H:%M:%f', 'now', '-' || (i % 400) || ' days') || 'Z', strftime('%Y-%m-%dT%H:%M:%f', 'now', '-' || (i % 400) || ' days') || 'Z', 'closed', 10 FROM n;
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < $calls)
INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, model, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit)
SELECT 'e' || i, 0, s.started_at, s.agent_label, 'anthropic', s.proxy_token_id, s.id, 'model', 200, 2100, 125000, 1220, 0, 4800,
 CASE WHEN i % 50 = 0 THEN NULL ELSE '0.123456' END, 'c'
FROM n JOIN sessions s ON s.id = (i % 10000) + 1;
SQL
seeded="$(sqlite3 "$work/loupe.db" "SELECT count(*) FROM calls")"
[[ "$seeded" -eq "$calls" ]] || fail "only $seeded of $calls calls are stored" "read the log"

lport="$(free_port)"
(cd "$ROOT" && env -i "${envs[@]}" LOUPE_ADMIN_TOKEN="$admin" LOUPE_LISTEN="127.0.0.1:$lport" "$BIN" serve) 2>>"$LOG" & pids+=($!)
wait_port "$lport" healthz || fail "loupe serve did not start on port $lport" "check the log for a configuration error"
from="$(perl -MPOSIX=strftime -e 'print strftime("%Y-%m-%d", gmtime(time - 399 * 86400))')"
url="http://127.0.0.1:$lport/private/api/v1/report?group=token&from=$from"
# The first request warms the page cache of SQLite; the second is the measured one.
rows="$(curl -sf -H "Authorization: Bearer $admin" "$url" | perl -MJSON::PP -e 'local $/; print scalar @{decode_json(<STDIN>)->{rows}}')" \
  || fail "the report request failed" "check the log of loupe serve"
[[ "$rows" -ge 1 ]] || fail "the report has no rows" "check the seed in this script"
t="$(curl -sf -o /dev/null -w '%{time_total}' -H "Authorization: Bearer $admin" "$url")" || fail "the report request failed" "check the log of loupe serve"
status=passed; perl -e "exit($t < $THRESHOLD ? 0 : 1)" || status=failed
echo "calls=$calls rows=$rows time_total=$t status=$status" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The report over 400 days and $calls calls took $t s. Threshold: below $THRESHOLD s. Result: $status."
else
  echo "calls: $calls days: 400 rows: $rows time_total: $t threshold: $THRESHOLD status: $status log: .agents/logs/private.bench.log"
fi
[[ $status != failed ]] || fail "time_total $t is not below $THRESHOLD" "profile src/private/report.rs"
