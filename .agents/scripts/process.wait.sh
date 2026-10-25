#!/usr/bin/env bash
# @agents-index Waits until no process matches a pattern, without matching its own shell or its callers, then prints the wait result.
#
# Usage: process.wait.sh --pattern REGEX [--timeout SECONDS] [--interval SECONDS] [--dry-run]
#   --pattern   An extended regular expression for the full command line, as pgrep -f reads it. Required.
#   --timeout   Stop the wait after this many seconds. Default: 600.
#   --interval  Seconds between two checks. Default: 2.
#   --dry-run   Print the processes that match now, and do not wait.
#
# A loop such as `until ! pgrep -f "cargo test"; do sleep 5; done` never ends: its own shell
# has "cargo test" on its command line, so pgrep always finds it. This script removes its own
# process and each parent process from the matches before it decides.
# Output (stdout, key: value): pattern, status (done, timeout, or dry_run), waited_secs, matches (pids).
# Exit codes: 0 no match left, 1 timeout with a match left, 2 bad usage.
set -euo pipefail

pattern=""; timeout=600; interval=2; dry_run=0
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --pattern) [[ $# -ge 2 ]] || { echo "error: --pattern needs a value. Fix: pass --pattern 'cargo test'. Verify: the script prints status." >&2; exit 2; }; pattern="$2"; shift 2 ;;
    --timeout) timeout="$2"; shift 2 ;;
    --interval) interval="$2"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints status." >&2; exit 2 ;;
  esac
done
[[ -n "$pattern" ]] || { echo "error: --pattern is required. Fix: pass --pattern REGEX, for example --pattern 'pipeline.verdict.sh'. Verify: the script prints status." >&2; exit 2; }

# The pids of this script and of each parent up to pid 1; a match among them is the caller, not the work.
own=" $$ "
p=$$
while [[ "$p" -gt 1 ]]; do
  p=$(ps -o ppid= -p "$p" | tr -d ' ')
  [[ -n "$p" ]] || break
  own+="$p "
done

matches() {
  local out=""
  for pid in $(pgrep -f -- "$pattern" || true); do
    [[ "$own" == *" $pid "* ]] && continue
    # A sibling of the caller, such as the pgrep of a command substitution, ends at once; skip pids that are gone.
    kill -0 "$pid" 2>/dev/null || continue
    # A zombie has ended; only its parent has not collected it yet.
    [[ "$(ps -o stat= -p "$pid" 2>/dev/null)" == Z* ]] && continue
    out+="$pid,"
  done
  echo "${out%,}"
}

start=$SECONDS
if [[ $dry_run -eq 1 ]]; then
  echo "pattern: $pattern status: dry_run waited_secs: 0 matches: $(matches)"
  exit 0
fi
while :; do
  m=$(matches)
  if [[ -z "$m" ]]; then
    echo "pattern: $pattern status: done waited_secs: $((SECONDS - start)) matches: none"
    exit 0
  fi
  if (( SECONDS - start >= timeout )); then
    echo "pattern: $pattern status: timeout waited_secs: $((SECONDS - start)) matches: $m"
    echo "error: processes that match '$pattern' still run after ${timeout}s: $m. Fix 1: wait longer with --timeout. Fix 2: read the command of each pid with ps -o command= -p PID. Verify: process.wait.sh --pattern '$pattern' --dry-run prints matches: (empty)." >&2
    exit 1
  fi
  sleep "$interval"
done
