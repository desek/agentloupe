#!/usr/bin/env bash
# @agents-index Times loupe backfill over N synthetic Claude Code log entries in a temp tree; prints backfill_secs.
#
# Usage: backfill.bench.sh [--entries N] [--dry-run] [--human]
#   --entries N   Synthetic log entries to write, half user lines and half assistant lines with usage, over 10 files. Default: 10000.
#   --dry-run     Time loupe backfill --dry-run, which writes nothing to the database.
#   --human       Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): entries, calls_added, backfill_secs, threshold_secs, status, log.
# The script builds the release binary, writes made-up log files to a temp dir (never a real log),
# and times one backfill run against a temp data dir. Full output: .agents/logs/backfill.bench.log.
# Exit codes: 0 measured and at most the threshold; 1 above the threshold or the run failed; 2 bad usage; 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/backfill.bench.log"
THRESHOLD_SECS=15
entries=10000; dry=(); human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --entries)
      [[ $# -ge 2 && "$2" =~ ^[1-9][0-9]*$ ]] || { echo "error: --entries needs a positive whole number. Fix: pass --entries 10000. Verify: the script prints backfill_secs." >&2; exit 2; }
      entries="$2"; shift 2 ;;
    --dry-run) dry=(--dry-run); shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints backfill_secs." >&2; exit 2 ;;
  esac
done

for tool in cargo perl; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the backfill time cannot be measured. Fix: install $tool (rustup for cargo). Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see backfill_secs." >&2
    exit 3
  fi
done

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run backfill.bench.sh and see backfill_secs. Log: .agents/logs/backfill.bench.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 || fail "the release build failed" "read the log and correct the build"

mkdir -p "$work/logs"
# Ten files of one made-up session each; each pair is a user line and an assistant line with usage.
perl -e '
  my ($n, $dir) = @ARGV; my $per = int(($n + 9) / 10);
  for my $f (0..9) {
    mkdir "$dir/logs/proj$f" or die; open my $o, ">", "$dir/logs/proj$f/s$f.jsonl" or die;
    for my $i (0..$per-1) {
      my $t = sprintf("2026-02-%02dT%02d:%02d:%02d.000Z", 1 + $f, int($i/3600)%24, int($i/60)%60, $i%60);
      if ($i % 2 == 0) { print $o qq({"type":"user","sessionId":"bench-$f","timestamp":"$t","message":{"role":"user","content":"step $i"}}\n); }
      else { print $o qq({"type":"assistant","sessionId":"bench-$f","timestamp":"$t","message":{"id":"m$f-$i","role":"assistant","model":"claude-haiku-5-5","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":5,"output_tokens":20}}}\n); }
    }
  }' "$entries" "$work" 2>>"$LOG"
mkdir -p "$work/data"
key="AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="
t0="$(perl -MTime::HiRes=time -e 'printf "%.3f", time')"
out="$(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work/data" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" \
  "$ROOT/target/release/loupe" backfill --source claude-code --path "$work/logs" "${dry[@]}" 2>>"$LOG")" || fail "loupe backfill failed" "read the log for the actionable error"
secs="$(perl -MTime::HiRes=time -e "printf '%.2f', time - $t0")"
echo "$out" >>"$LOG"
added="$(echo "$out" | awk -F': ' '$1=="calls_added"{print $2}')"
status=passed; perl -e "exit($secs <= $THRESHOLD_SECS ? 0 : 1)" || status=failed
if [[ $human -eq 1 ]]; then
  echo "The backfill of $entries entries added $added calls in $secs seconds (threshold $THRESHOLD_SECS): $status."
else
  printf 'entries: %s\ncalls_added: %s\nbackfill_secs: %s\nthreshold_secs: %s\nstatus: %s\nlog: .agents/logs/backfill.bench.log\n' "$entries" "$added" "$secs" "$THRESHOLD_SECS" "$status"
fi
[[ $status == passed ]]
