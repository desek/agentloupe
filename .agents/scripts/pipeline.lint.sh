#!/usr/bin/env bash
# @agents-index Pipeline entry point for the lint stage: runs cargo clippy on every target with warnings as errors, then the dependency check deps.deny.sh.
#
# Usage: pipeline.lint.sh [--human] [--inputs] [--toolchain]
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --inputs     Print the paths that the stage reads, one per line, and exit 0 with no run.
#   --toolchain  Print the versions of rustc, cargo, and cargo-deny, and exit 0 with no run.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, toolchain, log.
# The stage runs clippy, then .agents/scripts/deps.deny.sh (cargo deny against deny.toml), and prints one
# stage line; the status is failed when either fails. The dependency check fetches the advisory database,
# the one network use of this stage (docs/specification/operations.md, section Dependency check).
# The toolchain version comes from rust-toolchain.toml and the cargo-deny version from .tool-versions;
# this script states no version.
# Full tool output of both steps goes to .agents/logs/pipeline.lint.log.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 required tool missing.
set -euo pipefail
# The verdict builds with no telemetry host, so a value in the shell of a developer never changes it.
unset LOUPE_TELEMETRY_HOST LOUPE_BUILD_COMMIT

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.lint.log"
human=0; dry_run=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

# The declared inputs: a change outside them cannot change the result of this stage. web is declared
# because build.rs reads web/ when clippy builds the crate; deny.toml, about.toml, and deps.deny.sh are
# declared because the dependency check reads them.
inputs() { printf '%s\n' src tests Cargo.toml Cargo.lock build.rs migrations vendor rust-toolchain.toml examples .cargo rustfmt.toml .rustfmt.toml clippy.toml .clippy.toml .agents/scripts/pipeline.lint.sh web deny.toml about.toml .agents/scripts/deps.deny.sh; }
# A missing cargo-deny prints "cargo-deny absent", so the fingerprint differs from an installed one and
# a run of the stage stops with exit 3; the flag itself still works on a machine with no cargo-deny.
toolchain() {
  (cd "$ROOT" && rustc -V && cargo -V && { cargo deny --version 2>/dev/null || echo "cargo-deny absent"; })
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
    echo "error: $tool is not installed, so the lint stage cannot run. Fix: install it (rustup for cargo, Docker Desktop for docker). Reference: docs/specification/operations.md, section Pipeline. Verify: re-run and see 'status: passed'." >&2
    exit 3
  fi
done
if ! (cd "$ROOT" && cargo deny --version) >/dev/null 2>&1; then
  echo "error: cargo-deny is not installed, so the dependency check of the lint stage cannot run. Fix: mise install (it reads the cargo-deny pin of .tool-versions). Reference: docs/specification/operations.md, section Dependency check. Verify: cargo deny --version prints the pinned version." >&2
  exit 3
fi

toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
cmd=(cargo clippy --locked --all-targets -- -D warnings)
mkdir -p "$(dirname "$LOG")"

start="$(now)"
# Both steps always run, so one run of the stage shows every clippy finding and every dependency finding.
status=passed
if ! (cd "$ROOT" && "${cmd[@]}") >"$LOG" 2>&1; then status=failed; fi
(
  echo "\$ .agents/scripts/deps.deny.sh"
  if ! "$ROOT/.agents/scripts/deps.deny.sh" 2>&1; then deny_failed=1; else deny_failed=0; fi
  cat "$ROOT/.agents/logs/deps.deny.log" 2>/dev/null || true
  exit "$deny_failed"
) >>"$LOG" 2>&1 || status=failed
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The lint stage $status in $seconds seconds (toolchain $toolchain). Log: $LOG"
else
  echo "stage: lint status: $status seconds: $seconds toolchain: $toolchain log: .agents/logs/pipeline.lint.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the lint stage failed. Fix: correct each clippy finding in the log, or add a justified allow on the item; for a dependency finding (the lines after '\$ .agents/scripts/deps.deny.sh'), follow the fix that deps.deny.sh printed. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.lint.sh and see 'status: passed'." >&2
  exit 1
fi
