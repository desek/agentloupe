#!/usr/bin/env bash
# @agents-index Measures the skipped rate, request count, and request characters of the session texts at several conversation thresholds.
#
# Usage: summary.threshold.sh --from YYYY-MM-DD --to YYYY-MM-DD [--values 200,400,800] [--source claude-code] [--human]
#   --from, --to   The backfill window, days in LOUPE_TZ. Required.
#   --values       Comma-separated values of LOUPE_SUMMARY_MIN_CONVERSATION_CHARS. Default 200,400,800.
#   --source       The backfill source. Default claude-code.
#   --human        Print a friendly view. Also the default when stdout is a terminal.
#
# For each value, the script runs the release binary of loupe backfill on a new scratch data dir, with
# CLAUDE_CODE_OAUTH_TOKEN from .env, through summary.recorder.py, which forwards to the Anthropic API
# and records only the kind and the character count of each request. The runs are parallel.
# Output (stdout, one line per value, key=value fields): value, day_rows, day_skipped, day_pending,
# skipped_pct, pairs, full_rows, requests, line_requests, full_requests, day_requests, chars_max,
# chars_median (of the full and day requests).
# Full output: .agents/logs/summary.threshold.log. The scratch dirs are removed at the end.
# Exit codes: 0 measured; 1 a run failed; 2 bad usage; 3 a required tool is missing; 4 no token in .env.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/summary.threshold.log"
human=0
[[ -t 1 ]] && human=1
from="" to="" values="200,400,800" source="claude-code"

usage() { awk 'NR>=3 && /^#/ {sub(/^# ?/, ""); print; next} NR>=3 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from) from="${2:-}"; shift 2 ;;
    --to) to="${2:-}"; shift 2 ;;
    --values) values="${2:-}"; shift 2 ;;
    --source) source="${2:-}"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints value= lines." >&2; exit 2 ;;
  esac
done
if [[ -z "$from" || -z "$to" ]]; then
  echo "error: --from and --to are required. Fix: pass --from 2026-10-20 --to 2026-10-21. Verify: the script prints value= lines." >&2
  exit 2
fi
for tool in cargo python3 sqlite3; do
  command -v "$tool" >/dev/null || { echo "error: $tool is not installed. Fix: install $tool. Verify: re-run the script." >&2; exit 3; }
done
grep -q '^CLAUDE_CODE_OAUTH_TOKEN=.' "$ROOT/.env" 2>/dev/null || {
  echo "error: .env has no CLAUDE_CODE_OAUTH_TOKEN. Fix: run claude setup-token and put the value in .env. Verify: re-run the script." >&2; exit 4; }

mkdir -p "$(dirname "$LOG")"
: > "$LOG"
(cd "$ROOT" && cargo build --release >>"$LOG" 2>&1)
work="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$work"' EXIT

free_port() { python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }

run_one() {
  local v="$1" dir="$work/v$1" port
  mkdir -p "$dir"
  port="$(free_port)"
  python3 -I "$ROOT/.agents/scripts/summary.recorder.py" "$port" "$dir/requests.tsv" &
  local rec=$!
  sleep 1
  (
    set -a; . "$ROOT/.env"; set +a
    unset ANTHROPIC_API_KEY OPENAI_API_KEY LOUPE_SUMMARY_API_KEY LOUPE_SUMMARY_OAUTH_TOKEN LOUPE_SUMMARY_BASE_URL
    LOUPE_DATA_DIR="$dir" ANTHROPIC_BASE_URL="http://127.0.0.1:$port" LOUPE_SUMMARY_MIN_CONVERSATION_CHARS="$v" \
      "$ROOT/target/release/loupe" backfill --source "$source" --from "$from" --to "$to"
  ) >"$dir/out.txt" 2>>"$LOG" || { kill "$rec"; echo "error: the backfill run at $v failed; see $LOG" >&2; return 1; }
  kill "$rec" 2>/dev/null || true
  touch "$dir/requests.tsv"
}

IFS=',' read -r -a list <<<"$values"
pids=()
for v in "${list[@]}"; do run_one "$v" & pids+=($!); done
status=0
for p in "${pids[@]}"; do wait "$p" || status=1; done
[[ $status -eq 0 ]] || exit 1

for v in "${list[@]}"; do
  dir="$work/v$v"; db="$dir/loupe.db"
  q() { sqlite3 "$db" "$1"; }
  day_rows=$(q "SELECT count(*) FROM session_day_summaries")
  skipped=$(q "SELECT count(*) FROM session_day_summaries WHERE status = 'skipped'")
  pending=$(q "SELECT count(*) FROM session_day_summaries WHERE status = 'pending'")
  full_rows=$(q "SELECT count(*) FROM session_full_summaries")
  pairs=$(python3 -I - "$db" "${LOUPE_TZ:-Europe/Stockholm}" <<'PY'
import sqlite3, sys, datetime, zoneinfo
tz = zoneinfo.ZoneInfo(sys.argv[2])
rows = sqlite3.connect(sys.argv[1]).execute("SELECT s.key_hash, c.started_at FROM calls c JOIN sessions s ON s.id = c.session_id")
day = lambda t: datetime.datetime.fromisoformat(t.replace("Z", "+00:00")).astimezone(tz).date()
print(len({(k, day(t)) for k, t in rows}))
PY
)
  stats=$(python3 -I - "$dir/requests.tsv" <<'PY'
import statistics, sys
rows = [l.split("\t") for l in open(sys.argv[1]) if l.strip()]
n = lambda k: sum(1 for r in rows if r[0] == k)
chars = sorted(int(r[1]) for r in rows if r[0] in ("full", "day"))
mx = max(chars) if chars else 0
md = int(statistics.median(chars)) if chars else 0
print(f"requests={len(rows)} line_requests={n('line')} full_requests={n('full')} day_requests={n('day')} chars_max={mx} chars_median={md}")
PY
)
  pct=$(python3 -I -c "print(round(100*$skipped/$day_rows,1) if $day_rows else 0)")
  line="value=$v day_rows=$day_rows day_skipped=$skipped day_pending=$pending skipped_pct=$pct pairs=$pairs full_rows=$full_rows $stats"
  echo "$line" >>"$LOG"
  if [[ $human -eq 1 ]]; then
    echo "At $v characters: $skipped of $day_rows session day summaries skipped ($pct percent). $stats"
  else
    echo "$line"
  fi
done
