#!/usr/bin/env bash
# @agents-index Measures the added time to the first byte through the proxy against the fake upstream; prints p95_added_ms.
#
# Usage: proxy.latency.sh [--requests N] [--with-token] [--human]
#   --requests N   Server-sent events requests per path, direct and through the proxy. Default: 200.
#   --with-token   Create 100 proxy tokens before the start and send each proxied request with one.
#   --human        Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): requests, p95_direct_ms, p95_proxy_ms, p95_added_ms, threshold_ms, status, log.
# p95_added_ms is the 95th percentile time to the first byte through the proxy minus the one direct.
# The script builds the release binary and the fake_upstream example, starts both on free ports with
# a temporary data dir, and stops both on exit. Full output goes to .agents/logs/proxy.latency.log.
# Exit codes: 0 measured and at most the threshold, 1 measured above the threshold or a run failed,
# 2 bad usage, 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/proxy.latency.log"
THRESHOLD_MS=20
requests=200; human=0; with_token=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --requests)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --requests needs a positive whole number. Fix: pass --requests 200. Verify: the script prints p95_added_ms." >&2; exit 2; }
      requests="$2"; shift 2 ;;
    --with-token) with_token=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints p95_added_ms." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the latency cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see p95_added_ms." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run proxy.latency.sh and see p95_added_ms. Log: .agents/logs/proxy.latency.log" >&2; exit 1; }

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
prefix=""
if [[ $with_token -eq 1 ]]; then
  for i in $(seq 1 100); do
    tok="$(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work" "$BIN/loupe" token create --name "latency-$i" 2>>"$LOG" \
      | perl -ne 'print $1 if /"token":"([^"]+)"/')" || fail "loupe token create failed" "check the log"
    [[ $i -eq 50 ]] && prefix="/$tok"
  done
fi
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_LISTEN="127.0.0.1:$lport" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" \
  LOUPE_ACCOUNT_KEY="$key" LOUPE_UPSTREAMS_FILE="$work/upstreams.toml" "$BIN/loupe" serve) 2>>"$LOG" & pids+=($!)
wait_port "$lport" healthz || fail "loupe serve did not start on port $lport" "check the log for a configuration error"

# One time to the first byte per line, in seconds.
measure() {
  local url="$1" i
  for ((i = 0; i < requests; i++)); do
    curl -sf -o /dev/null -w '%{time_starttransfer}\n' "$url" || return 1
  done
}
p95() { sort -n | perl -e 'my @v=<STDIN>; chomp @v; my $i=int(0.95*@v+0.999)-1; $i=0 if $i<0; printf "%.2f", $v[$i]*1000'; }

direct="$(measure "http://127.0.0.1:$fport/sse?chunks=1&delay_ms=0" | p95)" || fail "a direct request failed" "check that the fake upstream runs"
proxy="$(measure "http://127.0.0.1:$lport$prefix/latency/fake/sse?chunks=1&delay_ms=0" | p95)" || fail "a proxied request failed" "check the log of loupe serve"
added="$(perl -e "printf '%.2f', $proxy - $direct")"
status=passed; perl -e "exit($added <= $THRESHOLD_MS ? 0 : 1)" || status=failed
echo "with_token=$with_token requests=$requests p95_direct_ms=$direct p95_proxy_ms=$proxy p95_added_ms=$added" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The proxy adds $added ms at the 95th percentile to the first byte ($proxy ms through the proxy, $direct ms direct, $requests requests each). Threshold: $THRESHOLD_MS ms. Result: $status."
else
  echo "with_token: $with_token requests: $requests p95_direct_ms: $direct p95_proxy_ms: $proxy p95_added_ms: $added threshold_ms: $THRESHOLD_MS status: $status log: .agents/logs/proxy.latency.log"
fi
[[ $status == passed ]] || fail "p95_added_ms $added is above $THRESHOLD_MS" "profile the forward path in src/proxy/forward.rs"
