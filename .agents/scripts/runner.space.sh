#!/bin/bash
# @agents-index Frees disk space on a GitHub-hosted runner by removing preinstalled software the project never uses, and prints the free space before and after.
#
# Usage: runner.space.sh [--dry-run] [--human]
#   --dry-run   Print the directories the script would remove and the free space, and remove nothing.
#   --human     Print a friendly view. Also the default when stdout is a terminal.
#   -h, --help  Print this text.
#
# A hosted runner has about 14 GB free. One job of the dependency autofix builds the crate for the
# verdict and then again for each Claude Code run with a changed lock file, and a run of 2026-10-24
# ended with "No space left on device". The preinstalled .NET, Android, Haskell, and CodeQL trees
# that this project never uses take about 20 GB, so the script removes them on a hosted runner only.
# Off a hosted runner (no GITHUB_ACTIONS or no /home/runner) it removes nothing and reports skipped.
# Output (stdout, key: value): free_before_gib, free_after_gib, removed (count), status: freed|skipped|planned.
# Exit codes: 0 always after a report, 2 bad usage.
set -euo pipefail
dry=0; human=0; [[ -t 1 ]] && human=1
usage() { sed -n '4,8p' "$0" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $2" >&2; exit "$1"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die 2 "unknown argument $1. Fix: see runner.space.sh --help." ;;
  esac
done
TARGETS=(/usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/.ghcup /opt/hostedtoolcache/CodeQL /usr/share/swift)
free_gib() { df -Pk / | awk 'NR == 2 {printf "%.1f", $4 / 1048576}'; }
report() { if [[ $human -eq 1 ]]; then echo "Free space $1 GiB before, $2 GiB after; $3 directories removed; $4."; else echo "free_before_gib: $1 free_after_gib: $2 removed: $3 status: $4"; fi; }
before="$(free_gib)"
if [[ "${GITHUB_ACTIONS:-}" != true || ! -d /home/runner ]]; then report "$before" "$before" 0 skipped; exit 0; fi
n=0
for t in "${TARGETS[@]}"; do
  [[ -e "$t" ]] || continue
  if [[ $dry -eq 1 ]]; then echo "plan: remove $t"; n=$((n + 1)); continue; fi
  sudo rm -rf "$t" && n=$((n + 1))
done
[[ $dry -eq 0 ]] && sudo apt-get clean >/dev/null 2>&1 || true
if [[ $dry -eq 1 ]]; then report "$before" "$before" "$n" planned; else report "$before" "$(free_gib)" "$n" freed; fi
