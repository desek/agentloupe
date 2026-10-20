#!/usr/bin/env bash
# @agents-index Seeds N published day records into a scratch data dir for the dashboard scenarios and the render measurement.
#
# Usage: days.seed.sh --data-dir DIR [--days N] [--last YYYY-MM-DD] [--github full|none|partial] [--dry-run] [--human]
#   --data-dir DIR   Scratch data dir; the script migrates its store with loupe aggregate. Required.
#   --days N         Published days, ending on --last, each with a record. Default: 400. 0 seeds none.
#   --last DAY       The newest seeded day. Default: yesterday in UTC.
#   --github MODE    full: PR and line counts on each day; none: all four null; partial: null on odd days. Default: full.
#   --dry-run        Print what the script would seed; change nothing.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Needs LOUPE_DATA_KEY and LOUPE_ACCOUNT_KEY in the environment, or makes a key with loupe gen-key.
# Uses the release binary target/release/loupe when present, else target/debug/loupe.
# Output (stdout, key: value): days, first, last, github, data_dir, status.
# Exit codes: 0 seeded, 1 seed failed, 2 bad usage, 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
dir=""; days=400; last="$(date -u -v-1d +%Y-%m-%d 2>/dev/null || date -u -d yesterday +%Y-%m-%d)"; github=full; dry=0; human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
bad() { echo "error: $1. Fix: $2. Verify: the script prints status: passed." >&2; exit 2; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --data-dir) [[ $# -ge 2 ]] || bad "--data-dir needs a value" "pass --data-dir /tmp/loupe-seed"; dir="$2"; shift 2 ;;
    --days) [[ $# -ge 2 && "$2" =~ ^[0-9]+$ && "$2" -le 400 ]] || bad "--days needs a whole number from 0 to 400" "pass --days 400"; days="$2"; shift 2 ;;
    --last) [[ $# -ge 2 && "$2" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || bad "--last needs a date YYYY-MM-DD" "pass --last 2026-10-19"; last="$2"; shift 2 ;;
    --github) [[ $# -ge 2 && "$2" =~ ^(full|none|partial)$ ]] || bad "--github needs full, none, or partial" "pass --github full"; github="$2"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) bad "unknown argument $1" "run --help for the flags" ;;
  esac
done
[[ -n "$dir" ]] || bad "--data-dir is missing" "pass --data-dir with a scratch directory"
command -v sqlite3 >/dev/null || { echo "error: sqlite3 is not installed, so the days cannot be seeded. Fix: install sqlite3. Verify: re-run and see status: passed." >&2; exit 3; }
BIN="$ROOT/target/release/loupe"; [[ -x "$BIN" ]] || BIN="$ROOT/target/debug/loupe"
[[ -x "$BIN" ]] || { echo "error: no loupe binary in target/. Fix: run cargo build --release --bin loupe. Verify: re-run and see status: passed." >&2; exit 3; }

out() { if [[ $human -eq 1 ]]; then echo "$2"; else echo "$1"; fi; }
if [[ $dry -eq 1 ]]; then
  out "days: $days last: $last github: $github data_dir: $dir status: planned" "Would seed $days days ending $last into $dir (GitHub $github)."
  exit 0
fi
mkdir -p "$dir"
key="${LOUPE_DATA_KEY:-$("$BIN" gen-key)}"
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$dir" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="${LOUPE_ACCOUNT_KEY:-$key}" LOUPE_TZ=UTC "$BIN" aggregate --date 2001-01-01) >/dev/null 2>&1 \
  || { echo "error: loupe aggregate could not migrate the store in $dir. Fix: check that the directory is writable and holds no store of another version. Verify: re-run and see status: passed." >&2; exit 1; }
case "$github" in
  full) gh="i % 4, i % 3, 40 + (i * 37) % 900, 10 + (i * 13) % 300" ;;
  none) gh="NULL, NULL, NULL, NULL" ;;
  partial) gh="CASE WHEN i % 2 = 0 THEN i % 4 END, CASE WHEN i % 2 = 0 THEN i % 3 END, CASE WHEN i % 2 = 0 THEN 40 + (i * 37) % 900 END, CASE WHEN i % 2 = 0 THEN 10 + (i * 13) % 300 END" ;;
esac
if [[ $days -gt 0 ]]; then
sqlite3 "$dir/loupe.db" <<SQL || { echo "error: the insert of the days failed. Fix: read the sqlite3 message above. Verify: re-run and see status: passed." >&2; exit 1; }
DELETE FROM days;
WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < $days - 1),
v(i, day, input_total, output, po, pm, la, lr) AS (
  SELECT i, date('$last', '-' || i || ' days'), 200000 + (i * 7919) % 1800000, 4000 + (i * 613) % 60000, $gh FROM n)
INSERT INTO days (day, record, content_hash, revision, built_at, state, published_at)
SELECT day, json_object(
  'day', day, 'input_total', input_total, 'input', input_total / 20, 'cache_read', input_total - input_total / 20 - 1000,
  'cache_write', 1000, 'output', output, 'calls', 20 + i % 50, 'sessions', 1 + i % 5,
  'cost_usd', round((input_total * 0.0000004 + output * 0.000015), 4), 'unpriced_calls', CASE WHEN i % 30 = 0 THEN 1 ELSE 0 END,
  'subscription_cost_usd', 6.5753, 'prs_opened', po, 'prs_merged', pm, 'lines_added', la, 'lines_removed', lr,
  'summary', CASE WHEN i % 9 = 0 THEN NULL ELSE 'Worked on the seeded task number ' || i || '.' END,
  'detail', json_object('by_agent', json_object(), 'by_model', json_object())),
  'h' || i, 1, '${last}T01:00:00.000Z', 'published', '${last}T01:00:00.000Z' FROM v;
SQL
fi
n="$(sqlite3 "$dir/loupe.db" "SELECT count(*) FROM days WHERE state = 'published'")"
[[ "$n" -eq "$days" ]] || { echo "error: $n of $days days are stored. Fix: read the sqlite3 output. Verify: re-run and see status: passed." >&2; exit 1; }
first="$(sqlite3 "$dir/loupe.db" "SELECT coalesce(min(day), '') FROM days")"
out "days: $days first: $first last: $last github: $github data_dir: $dir status: passed" "Seeded $days days from $first to $last into $dir."
