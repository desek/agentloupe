#!/usr/bin/env bash
# @agents-index Times the load of the embedded price index and the median price of one call; prints both with thresholds.
#
# Usage: cost.bench.sh [--runs N] [--human]
#   --runs N    Calls to price for the median. Default: 10000.
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): entries, load_ms, load_threshold_ms, price_us_median,
# price_threshold_us, status, log. The script builds the cost_bench example in release mode and runs it.
# Full output: .agents/logs/cost.bench.log.
# Exit codes: 0 both values at most their thresholds; 1 a value above its threshold or the run failed;
# 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/cost.bench.log"
LOAD_MS_MAX=500
PRICE_US_MAX=50
runs=10000; human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --runs needs a positive whole number. Fix: pass --runs 10000. Verify: the script prints price_us_median." >&2; exit 2; }
      runs="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints price_us_median." >&2; exit 2 ;;
  esac
done

if ! command -v cargo >/dev/null; then
  echo "error: cargo is not installed, so the cost time cannot be measured. Fix: install rustup. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see price_us_median." >&2
  exit 3
fi

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
if ! (cd "$ROOT" && cargo build --locked --release --example cost_bench) >>"$LOG" 2>&1; then
  echo "error: the release build of the cost_bench example failed. Fix: read .agents/logs/cost.bench.log and correct the build. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run cost.bench.sh and see price_us_median." >&2
  exit 1
fi
out="$("$ROOT/target/release/examples/cost_bench" "$runs" 2>>"$LOG")"
echo "$out" >>"$LOG"
val() { sed -n "s/^$1: //p" <<<"$out"; }
entries="$(val entries)"; load_ms="$(val load_ms)"; price_us="$(val price_us_median)"
status=passed
perl -e "exit !($load_ms <= $LOAD_MS_MAX && $price_us <= $PRICE_US_MAX)" || status=failed

if [[ $human -eq 1 ]]; then
  echo "The index of $entries entries loaded in $load_ms ms (at most $LOAD_MS_MAX), and one call priced in $price_us µs median (at most $PRICE_US_MAX): $status."
else
  printf 'entries: %s\nload_ms: %s\nload_threshold_ms: %s\nprice_us_median: %s\nprice_threshold_us: %s\nstatus: %s\nlog: .agents/logs/cost.bench.log\n' \
    "$entries" "$load_ms" "$LOAD_MS_MAX" "$price_us" "$PRICE_US_MAX" "$status"
fi
if [[ $status == failed ]]; then
  echo "error: a cost time is above its threshold. Fix: profile src/price/index.rs for the load or src/price/compute.rs for the price. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run cost.bench.sh and see status: passed." >&2
  exit 1
fi
