#!/usr/bin/env bash
# @agents-index Pipeline entry point for the test stage: runs every unit and integration test with cargo nextest at the pinned minimum version.
#
# Usage: pipeline.test.sh [--human] [--inputs] [--toolchain]
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --inputs     Print the paths that the stage reads, one per line, and exit 0 with no run.
#   --toolchain  Print the versions of rustc, cargo, and cargo-nextest, and exit 0 with no run.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, toolchain, log.
# The Rust version comes from rust-toolchain.toml and the minimum cargo-nextest version from the
# cargo-nextest line of .tool-versions; this script states no version. There is no fallback to cargo test,
# so the measured loop stays one runner. Doc tests run with cargo test --doc only if src has a doc code block,
# because nextest does not run doc tests.
# Full tool output goes to .agents/logs/pipeline.test.log.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 required tool missing or cargo-nextest below the pin.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.test.log"
human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

# The declared inputs: a change outside them cannot change the result of this stage. web is declared
# because build.rs embeds web/ into the crate and tests read web/; Dockerfile because a test reads its COPY lines; a test checks that every repository
# path that a test reads is in this list.
inputs() {
  printf '%s\n' src tests Cargo.toml Cargo.lock build.rs migrations vendor rust-toolchain.toml examples \
    .cargo rustfmt.toml .rustfmt.toml clippy.toml .clippy.toml .agents/scripts/pipeline.test.sh \
    .tool-versions .config/nextest.toml upstreams.toml config .agents/scripts \
    docs/specification/accounts.md web Dockerfile
}
toolchain() { (cd "$ROOT" && rustc -V && cargo -V && cargo nextest --version | head -n 1); }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    --inputs) inputs; exit 0 ;;
    --toolchain) toolchain; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a stage line." >&2; exit 2 ;;
  esac
done

if ! command -v cargo >/dev/null; then
  echo "error: cargo is not installed, so the test stage cannot run. Fix: install it with rustup. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run and see 'status: passed'." >&2
  exit 3
fi
pin="$(awk '$1 == "cargo-nextest" {print $2}' "$ROOT/.tool-versions" 2>/dev/null || true)"
if [[ -z "$pin" ]]; then
  echo "error: .tool-versions has no cargo-nextest line, so the test stage has no runner pin. Fix: add the line 'cargo-nextest <version>' to .tool-versions. Reference: docs/specification/operations.md, section Test runner. Verify: pipeline.test.sh --toolchain prints a cargo-nextest line." >&2
  exit 3
fi
have="$(cd "$ROOT" && cargo nextest --version 2>/dev/null | awk 'NR==1 {print $2}' || true)"
if [[ -z "$have" || "$(printf '%s\n%s\n' "$pin" "$have" | sort -V | head -n 1)" != "$pin" ]]; then
  echo "error: cargo-nextest is ${have:-missing}, but .tool-versions pins at least cargo-nextest $pin, so the test stage cannot run. Fix 1: run 'cargo install cargo-nextest --locked --version $pin'. Fix 2: run 'mise use cargo:cargo-nextest@$pin' in the repository. Pin file: .tool-versions. Reference: docs/specification/operations.md, section Test runner. Verify: 'cargo nextest --version' prints $pin or later, then pipeline.test.sh prints 'status: passed'." >&2
  exit 3
fi

toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
mkdir -p "$(dirname "$LOG")"

run() {
  cargo nextest run --locked --no-fail-fast
  if grep -rqE '^[[:space:]]*//[/!].*```' src; then cargo test --locked --doc; fi
}

start="$(now)"
if (cd "$ROOT" && echo "run: cargo nextest run --locked --no-fail-fast" && run) >"$LOG" 2>&1; then status=passed; else status=failed; fi
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The test stage $status in $seconds seconds (toolchain $toolchain, cargo-nextest $have). Log: $LOG"
else
  echo "stage: test status: $status seconds: $seconds toolchain: $toolchain runner: cargo-nextest $have log: .agents/logs/pipeline.test.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the test stage failed. Fix: read the failing test in the log and correct the code or the test. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.test.sh and see 'status: passed'." >&2
  exit 1
fi
