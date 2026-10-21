#!/usr/bin/env bash
# @agents-index Pipeline entry point for the format stage: checks that the Rust sources are formatted (cargo fmt --check).
#
# Usage: pipeline.format.sh [--human] [--inputs] [--toolchain]
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --inputs     Print the paths that the stage reads, one per line, and exit 0 with no run.
#   --toolchain  Print the versions of rustc and cargo, and exit 0 with no run.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, toolchain, log.
# The toolchain version comes from rust-toolchain.toml; this script states no version.
# Full tool output goes to .agents/logs/pipeline.format.log.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.format.log"
human=0; dry_run=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

# The declared inputs: a change outside them cannot change the result of this stage.
inputs() { printf '%s\n' src tests Cargo.toml Cargo.lock build.rs migrations vendor rust-toolchain.toml examples .cargo rustfmt.toml .rustfmt.toml clippy.toml .clippy.toml .agents/scripts/pipeline.format.sh; }
toolchain() {
  (cd "$ROOT" && rustc -V && cargo -V)
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    --inputs) inputs; exit 0 ;;
    --toolchain) toolchain; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a stage line." >&2; exit 2 ;;
  esac
done

for tool in cargo; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the format stage cannot run. Fix: install it (rustup for cargo, Docker Desktop for docker). Reference: docs/specification/operations.md, section Pipeline. Verify: re-run and see 'status: passed'." >&2
    exit 3
  fi
done

toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
cmd=(cargo fmt --check)
mkdir -p "$(dirname "$LOG")"

start="$(now)"
if (cd "$ROOT" && "${cmd[@]}") >"$LOG" 2>&1; then status=passed; else status=failed; fi
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The format stage $status in $seconds seconds (toolchain $toolchain). Log: $LOG"
else
  echo "stage: format status: $status seconds: $seconds toolchain: $toolchain log: .agents/logs/pipeline.format.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the format stage failed. Fix: run 'cargo fmt' and commit the result. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.format.sh and see 'status: passed'." >&2
  exit 1
fi
