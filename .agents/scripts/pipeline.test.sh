#!/usr/bin/env bash
# @agents-index Pipeline entry point for the test stage: runs every unit and integration test (cargo test).
#
# Usage: pipeline.test.sh [--human]
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, toolchain, log.
# The toolchain version comes from rust-toolchain.toml; this script states no version.
# Full tool output goes to .agents/logs/pipeline.test.log.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.test.log"
human=0; dry_run=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a stage line." >&2; exit 2 ;;
  esac
done

for tool in cargo; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the test stage cannot run. Fix: install it (rustup for cargo, Docker Desktop for docker). Reference: docs/specification/operations.md, section Pipeline. Verify: re-run and see 'status: passed'." >&2
    exit 3
  fi
done

toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
cmd=(cargo test --locked)
mkdir -p "$(dirname "$LOG")"

start="$(now)"
if (cd "$ROOT" && "${cmd[@]}") >"$LOG" 2>&1; then status=passed; else status=failed; fi
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The test stage $status in $seconds seconds (toolchain $toolchain). Log: $LOG"
else
  echo "stage: test status: $status seconds: $seconds toolchain: $toolchain log: .agents/logs/pipeline.test.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the test stage failed. Fix: read the failing test in the log and correct the code or the test. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.test.sh and see 'status: passed'." >&2
  exit 1
fi
