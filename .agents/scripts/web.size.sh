#!/usr/bin/env bash
# @agents-index Measures web/ in KiB without and with the fonts, the page code without web/test/, and web/private/, against the budgets of 100, 400, and 200 KiB.
#
# Usage: web.size.sh [--human]
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): code_kib (every file of web/ outside web/assets/fonts/ and web/private/),
# page_kib (the code_kib files minus web/test/: only the files that the browser loads; no budget of its own),
# total_kib (every file of web/), private_kib (every file of web/private/), the three thresholds, and status. Reads only the files on disk; no network.
# Exit codes: 0 both within the budget, 1 a budget exceeded, 2 bad usage.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CODE_MAX=100; TOTAL_MAX=400; PRIVATE_MAX=200; human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints code_kib." >&2; exit 2 ;;
  esac
done

bytes() { find "$@" -type f -exec stat -f%z {} + 2>/dev/null | awk '{s+=$1} END {print s+0}'; }
total="$(bytes "$ROOT/web")"
fonts="$(bytes "$ROOT/web/assets/fonts")"
private="$(bytes "$ROOT/web/private")"
tests="$(bytes "$ROOT/web/test")"
code_kib="$(perl -e "printf '%.1f', ($total - $fonts - $private) / 1024")"
page_kib="$(perl -e "printf '%.1f', ($total - $fonts - $private - $tests) / 1024")"
private_kib="$(perl -e "printf '%.1f', $private / 1024")"
total_kib="$(perl -e "printf '%.1f', $total / 1024")"
status=passed
perl -e "exit(($code_kib <= $CODE_MAX && $total_kib <= $TOTAL_MAX && $private_kib <= $PRIVATE_MAX) ? 0 : 1)" || status=failed
if [[ $human -eq 1 ]]; then
  echo "web/ is $code_kib KiB without the fonts (budget $CODE_MAX), $page_kib KiB of it page code without web/test/, and $total_kib KiB with them (budget $TOTAL_MAX); web/private/ is $private_kib KiB (budget $PRIVATE_MAX): $status."
else
  echo "code_kib: $code_kib page_kib: $page_kib total_kib: $total_kib code_max_kib: $CODE_MAX total_max_kib: $TOTAL_MAX private_kib: $private_kib private_max_kib: $PRIVATE_MAX status: $status"
fi
if [[ $status == failed ]]; then
  echo "error: web/ is over its size budget. Fix 1: remove unused CSS or JavaScript. Fix 2: vendor fewer font files in fonts.vendor.sh. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run web.size.sh and see status: passed." >&2
  exit 1
fi
