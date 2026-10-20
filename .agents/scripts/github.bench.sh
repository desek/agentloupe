#!/usr/bin/env bash
# @agents-index Times loupe aggregate --date with one GitHub token against the fake GitHub with N merged pull requests; prints fetch_secs.
#
# Usage: github.bench.sh [--merged N] [--human]
#   --merged N   Merged pull requests of the day in the fake GitHub. Default: 50.
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): merged, fetch_secs, threshold_secs, status, log.
# The script builds the release binary and the fake_github example, starts the fake on a free port,
# then times one loupe aggregate --date, which fetches the day for the label BENCH and builds it.
# It never calls api.github.com. Full output: .agents/logs/github.bench.log.
# Exit codes: 0 measured and at most the threshold; 1 above the threshold or a run failed;
# 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/github.bench.log"
THRESHOLD_SECS=10
DAY=2026-10-18
merged=50; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --merged)
      [[ $# -ge 2 && "$2" =~ ^[0-9]+$ ]] || { echo "error: --merged needs a whole number. Fix: pass --merged 50. Verify: the script prints fetch_secs." >&2; exit 2; }
      merged="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints fetch_secs." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl sqlite3; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the fetch time cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see fetch_secs." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run github.bench.sh and see fetch_secs. Log: .agents/logs/github.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe --example fake_github) >>"$LOG" 2>&1 \
  || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release"
free_port() { perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport'; }
port="$(free_port)"
"$BIN/examples/fake_github" "$port" "$merged" "$DAY" bench-token >>"$LOG" 2>&1 & pids+=($!)
for _ in $(seq 1 100); do curl -s -o /dev/null -H 'authorization: Bearer bench-token' "http://127.0.0.1:$port/user" && break; sleep 0.1; done
curl -sf -o /dev/null -H 'authorization: Bearer bench-token' "http://127.0.0.1:$port/user" || fail "the fake GitHub did not start on port $port" "check the log for a bind error"

key="$("$BIN/loupe" gen-key)"
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
t0="$(now)"
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC \
  LOUPE_GITHUB_TOKEN_BENCH=bench-token LOUPE_GITHUB_API_URL="http://127.0.0.1:$port" "$BIN/loupe" aggregate --date "$DAY") >>"$LOG" 2>&1 \
  || fail "loupe aggregate failed" "read the log"
secs="$(perl -e "printf '%.2f', $(now) - $t0")"
got="$(sqlite3 "$work/loupe.db" "SELECT count(*) FROM github_prs WHERE merged_day = '$DAY'")"
[[ "$got" -eq "$merged" ]] || fail "only $got of $merged merged pull requests are stored" "check the fetch warn line in the log"
status=passed; perl -e "exit($secs <= $THRESHOLD_SECS ? 0 : 1)" || status=failed
echo "merged=$merged fetch_secs=$secs status=$status" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The fetch of $merged merged pull requests took $secs seconds. Threshold: $THRESHOLD_SECS seconds. Result: $status."
else
  echo "merged: $merged fetch_secs: $secs threshold_secs: $THRESHOLD_SECS status: $status log: .agents/logs/github.bench.log"
fi
[[ $status != failed ]] || fail "fetch_secs $secs is above $THRESHOLD_SECS" "profile the fetch in src/github/fetch.rs"
