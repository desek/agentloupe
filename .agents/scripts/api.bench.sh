#!/usr/bin/env bash
# @agents-index Times GET /v1/days over N stored published days with R requests from the host; prints p95_ms.
#
# Usage: api.bench.sh [--days N] [--requests R] [--human]
#   --days N       Published days in the store, ending 2026-10-10, each with a full record. Default: 400.
#   --requests R   Requests of GET /v1/days over the whole range. Default: 200.
#   --human        Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): days, requests, p95_ms, threshold_ms, status, log.
# The script builds the release binary, migrates a temporary store, builds one full record with
# loupe aggregate, copies it to N days with sqlite3, starts loupe serve on a free port, and times
# each request with curl. Full output: .agents/logs/api.bench.log.
# Exit codes: 0 measured and at most the threshold; 1 above the threshold or a run failed;
# 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/api.bench.log"
THRESHOLD_MS=50
LAST=2026-10-10
days=400; requests=200; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --days)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ && "$2" -le 400 ]] || { echo "error: --days needs a whole number from 1 to 400. Fix: pass --days 400. Verify: the script prints p95_ms." >&2; exit 2; }
      days="$2"; shift 2 ;;
    --requests)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --requests needs a positive whole number. Fix: pass --requests 200. Verify: the script prints p95_ms." >&2; exit 2; }
      requests="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints p95_ms." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl sqlite3; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the API latency cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see p95_ms." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run api.bench.sh and see p95_ms. Log: .agents/logs/api.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 \
  || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release/loupe"
free_port() { perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport'; }
wait_port() { for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$1/$2" && return 0; sleep 0.1; done; return 1; }
key="$("$BIN" gen-key)"
envs=(PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC LOUPE_SUBSCRIPTION_CLAUDE_MAX=200)
(cd "$ROOT" && env -i "${envs[@]}" "$BIN" aggregate --date 2001-01-01) >>"$LOG" 2>&1 \
  || fail "loupe aggregate could not migrate the store" "read the log"
# One full record: 3 agents and 4 models, then the same record text on each day.
sqlite3 "$work/loupe.db" >>"$LOG" 2>&1 <<SQL || fail "the seed of the calls failed" "read the log"
INSERT INTO sessions (id, agent_label, key_hash, key_source, segment, started_at, last_call_at, closed_at, state, call_count)
VALUES (1, 'claude-code', 'k1', 'header', 1, '${LAST}T01:00:00.000Z', '${LAST}T01:00:00.000Z', '${LAST}T02:00:00.000Z', 'closed', 1);
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 12)
INSERT INTO calls (exchange_id, call_index, started_at, agent_label, upstream, session_id, model, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit)
SELECT 'e' || i, 0, '${LAST}T01:00:00.000Z', 'agent-' || (i % 3), 'anthropic', 1, 'model-' || (i % 4), 200, 21000, 1250000, 12200, 0, 48000, '1.234567', 'c' FROM n;
SQL
(cd "$ROOT" && env -i "${envs[@]}" "$BIN" aggregate --date "$LAST") >>"$LOG" 2>&1 || fail "loupe aggregate failed" "read the log"
sqlite3 "$work/loupe.db" >>"$LOG" 2>&1 <<SQL || fail "the copy of the record to $days days failed" "read the log"
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < $days)
INSERT OR IGNORE INTO days (day, record, content_hash, revision, built_at, state, published_at)
SELECT date('$LAST', '-' || i || ' days'), (SELECT record FROM days WHERE day = '$LAST'), 'h', 1, '${LAST}T01:00:00.000Z', 'published', '${LAST}T01:00:00.000Z' FROM n;
DELETE FROM days WHERE day < date('$LAST', '-$((days - 1)) days');
SQL
stored="$(sqlite3 "$work/loupe.db" "SELECT count(*) FROM days WHERE state = 'published'")"
[[ "$stored" -eq "$days" ]] || fail "only $stored of $days days are stored" "read the log"

lport="$(free_port)"
(cd "$ROOT" && env -i "${envs[@]}" LOUPE_LISTEN="127.0.0.1:$lport" "$BIN" serve) 2>>"$LOG" & pids+=($!)
wait_port "$lport" healthz || fail "loupe serve did not start on port $lport" "check the log for a configuration error"
from="$(sqlite3 "$work/loupe.db" "SELECT min(day) FROM days")"
url="http://127.0.0.1:$lport/v1/days?from=$from&to=$LAST"
n="$(curl -sf "$url" | perl -MJSON::PP -e 'local $/; print scalar @{decode_json(<STDIN>)->{days}}')" \
  || fail "GET /v1/days failed" "check the log of loupe serve"
[[ "$n" -eq "$days" ]] || fail "the response holds $n days, not $days" "check the range rule in src/api/query.rs"
times="$work/times"
for ((i = 0; i < requests; i++)); do
  curl -sf -o /dev/null -w '%{time_total}\n' "$url" >>"$times" || fail "a request failed" "check the log of loupe serve"
done
p95="$(sort -n "$times" | perl -e 'my @t = <STDIN>; my $i = int(0.95 * @t + 0.999) - 1; printf "%.2f", $t[$i] * 1000')"
status=passed; perl -e "exit($p95 <= $THRESHOLD_MS ? 0 : 1)" || status=failed
echo "days=$days requests=$requests p95_ms=$p95 status=$status" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "GET /v1/days over $days days: p95 $p95 ms in $requests requests. Threshold: $THRESHOLD_MS ms. Result: $status."
else
  echo "days: $days requests: $requests p95_ms: $p95 threshold_ms: $THRESHOLD_MS status: $status log: .agents/logs/api.bench.log"
fi
[[ $status != failed ]] || fail "p95_ms $p95 is above $THRESHOLD_MS" "profile the list route in src/api/days.list.rs"
