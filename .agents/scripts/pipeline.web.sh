#!/usr/bin/env bash
# @agents-index Pipeline entry point for the web stage: checks the Node pin, runs node --test on the page modules, checks the web/ size.
#
# Usage: pipeline.web.sh [--check-toolchain] [--human]
#   --check-toolchain  Only check that node --version equals the pin in .tool-versions.
#   --human            Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, node, log.
# The Node version comes from .tool-versions; this script states no version.
# Full tool output goes to .agents/logs/pipeline.web.log. No network; the tests use no npm package.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 node missing or not the pinned version.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.web.log"
human=0; only_toolchain=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check-toolchain) only_toolchain=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a stage line." >&2; exit 2 ;;
  esac
done

pin="$(awk '$1 == "nodejs" {print $2}' "$ROOT/.tool-versions" 2>/dev/null || true)"
if [[ -z "$pin" ]]; then
  echo "error: .tool-versions has no nodejs line, so the web stage has no Node pin. Fix: add the line 'nodejs 22.20.0' to .tool-versions. Reference: docs/specification/dashboard.md, section Toolchain and pipeline. Verify: pipeline.web.sh --check-toolchain exits 0." >&2
  exit 3
fi
# When mise is present, it selects the pinned version from .tool-versions, also in a shell whose PATH
# still names another version; else the node on PATH must match the pin.
NODE=(node)
command -v mise >/dev/null && NODE=(mise exec -- node)
have="$(cd "$ROOT" && "${NODE[@]}" --version 2>/dev/null || echo missing)"
if [[ "$have" != "v$pin" ]]; then
  echo "error: node is $have, but .tool-versions pins nodejs $pin, so the web stage cannot run. Fix 1: run 'mise install' in the repository, so mise selects nodejs $pin. Fix 2: install Node $pin by another means and put it first on PATH. Reference: docs/specification/operations.md, section Pipeline. Verify: pipeline.web.sh --check-toolchain exits 0." >&2
  exit 3
fi
if [[ $only_toolchain -eq 1 ]]; then
  if [[ $human -eq 1 ]]; then echo "node $have matches the pin."; else echo "stage: web-toolchain status: passed node: $have"; fi
  exit 0
fi

now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
mkdir -p "$(dirname "$LOG")"
start="$(now)"; status=passed
{
  (cd "$ROOT" && "${NODE[@]}" --test 'web/test/*.test.js') && "$ROOT/.agents/scripts/web.size.sh" && "$ROOT/.agents/scripts/fonts.vendor.sh" --check
} >"$LOG" 2>&1 || status=failed
seconds="$(perl -e "printf '%.1f', $(now) - $start")"
if [[ $human -eq 1 ]]; then
  echo "The web stage $status in $seconds seconds (node $have). Log: $LOG"
else
  echo "stage: web status: $status seconds: $seconds node: $have log: .agents/logs/pipeline.web.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the web stage failed. Fix: read the failing test or the size line in the log and correct web/. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.web.sh and see 'status: passed'." >&2
  exit 1
fi
