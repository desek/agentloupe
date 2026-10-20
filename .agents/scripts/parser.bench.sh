#!/usr/bin/env bash
# @agents-index Times loupe parse --once over N mixed JSON and SSE exchanges made through the proxy; prints parse_secs.
#
# Usage: parser.bench.sh [--exchanges N] [--human]
#   --exchanges N   Exchanges to make through the proxy, half JSON and half SSE. Default: 1000.
#   --human         Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): exchanges, parse_secs, threshold_secs, status, log.
# The script builds the release binary and the fake_upstream example, starts both on free ports with
# a temporary data dir, makes the exchanges, stops loupe serve, then times loupe parse --once.
# status is not_implemented while loupe has no parse subcommand. Full output: .agents/logs/parser.bench.log.
# Exit codes: 0 measured and at most the threshold, or not_implemented; 1 above the threshold or a
# run failed; 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/parser.bench.log"
THRESHOLD_SECS=10
exchanges=1000; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --exchanges)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --exchanges needs a positive whole number. Fix: pass --exchanges 1000. Verify: the script prints parse_secs." >&2; exit 2; }
      exchanges="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints parse_secs." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl sqlite3; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the parse time cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see parse_secs." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run parser.bench.sh and see parse_secs. Log: .agents/logs/parser.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe --example fake_upstream) >>"$LOG" 2>&1 \
  || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release"
free_port() { perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport'; }
wait_port() { for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$1/$2" && return 0; sleep 0.1; done; return 1; }

fport="$(free_port)"; lport="$(free_port)"
"$BIN/examples/fake_upstream" "$fport" >>"$LOG" 2>&1 & pids+=($!)
wait_port "$fport" json || fail "the fake upstream did not start on port $fport" "check the log for a bind error"
printf 'fake = "http://127.0.0.1:%s"\n' "$fport" >"$work/upstreams.toml"
key="$("$BIN/loupe" gen-key)"
envs=(PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_UPSTREAMS_FILE="$work/upstreams.toml" LOUPE_PARSE_INTERVAL_SECS=86400)
(cd "$ROOT" && env -i "${envs[@]}" LOUPE_LISTEN="127.0.0.1:$lport" "$BIN/loupe" serve) 2>>"$LOG" & serve_pid=$!; pids+=($serve_pid)
wait_port "$lport" healthz || fail "loupe serve did not start on port $lport" "check the log for a configuration error"

url="http://127.0.0.1:$lport/bench/fake/v1/messages"
for ((i = 0; i < exchanges; i++)); do
  stream=false; (( i % 2 )) && stream=true
  curl -sf -o /dev/null -H 'content-type: application/json' -H "x-session-id: bench-$((i % 50))" \
    -d "{\"model\":\"claude-haiku-5-5\",\"stream\":$stream,\"messages\":[{\"role\":\"user\",\"content\":\"hi $((i % 50))\"}]}" "$url" \
    || fail "a proxied request failed" "check the log of loupe serve"
done
kill -TERM "$serve_pid"; wait "$serve_pid" 2>/dev/null || true
rows="$(sqlite3 "$work/loupe.db" 'SELECT count(*) FROM exchanges')"
[[ "$rows" -eq "$exchanges" ]] || fail "only $rows of $exchanges exchanges are stored" "check the capture in the log"

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
if ! "$BIN/loupe" parse --help >/dev/null 2>&1; then
  status=not_implemented; secs=none
else
  t0="$(now)"
  (cd "$ROOT" && env -i "${envs[@]}" "$BIN/loupe" parse --once) >>"$LOG" 2>&1 || fail "loupe parse --once failed" "read the log"
  secs="$(perl -e "printf '%.2f', $(now) - $t0")"
  status=passed; perl -e "exit($secs <= $THRESHOLD_SECS ? 0 : 1)" || status=failed
fi
echo "exchanges=$exchanges parse_secs=$secs status=$status" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The parse of $exchanges exchanges took $secs seconds. Threshold: $THRESHOLD_SECS seconds. Result: $status."
else
  echo "exchanges: $exchanges parse_secs: $secs threshold_secs: $THRESHOLD_SECS status: $status log: .agents/logs/parser.bench.log"
fi
[[ $status != failed ]] || fail "parse_secs $secs is above $THRESHOLD_SECS" "profile the parser worker in src/parse/worker.rs"
