#!/usr/bin/env bash
# @agents-index Runs the pipeline stage entry points in the order format, lint, test, web, image, and prints the verdict.
#
# Usage: pipeline.verdict.sh [--human]
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout): one line per stage, "stage: <name> status: passed|failed seconds: <n>",
# then "verdict: passed|failed seconds: <total>". Every stage runs, so all failures show at once.
# This script only calls the stage entry points; it owns the list and the order of the stages.
# Exit codes: 0 every stage passed, 1 one or more stages failed, 2 bad usage.
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
STAGES=(format lint test web image)
human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a verdict line." >&2; exit 2 ;;
  esac
done

now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
start="$(now)"; failed=()
for stage in "${STAGES[@]}"; do
  t0="$(now)"
  if "$DIR/pipeline.$stage.sh" >/dev/null; then status=passed; else status=failed; failed+=("$stage"); fi
  seconds="$(perl -e "printf '%.1f', $(now) - $t0")"
  if [[ $human -eq 1 ]]; then echo "The $stage stage $status in $seconds seconds."
  else echo "stage: $stage status: $status seconds: $seconds"; fi
done
total="$(perl -e "printf '%.1f', $(now) - $start")"
verdict=passed; [[ ${#failed[@]} -gt 0 ]] && verdict=failed
if [[ $human -eq 1 ]]; then echo "The verdict is $verdict in $total seconds."
else echo "verdict: $verdict seconds: $total"; fi
if [[ $verdict == failed ]]; then
  echo "error: the stages ${failed[*]} failed. Fix: run each failed stage entry point alone, for example .agents/scripts/pipeline.${failed[0]}.sh, and read its log under .agents/logs/. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.verdict.sh and see 'verdict: passed'." >&2
  exit 1
fi
