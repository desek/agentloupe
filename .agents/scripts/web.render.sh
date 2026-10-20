#!/usr/bin/env bash
# @agents-index Measures the dashboard in headless Chrome with N seeded days: draw_ms, headline_ratio, headline above the fold, remote requests.
#
# Usage: web.render.sh [--days N] [--human]
#   --days N   Published days to seed, ending yesterday. Default: 400.
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# The script builds the release binary, seeds a scratch store with days.seed.sh, starts loupe serve
# on a free port, and runs web.render.cdp.mjs with the pinned Node against Google Chrome. It stops
# the server and Chrome on exit. Full output: .agents/logs/web.render.log.
# Output (stdout, key: value): days, draw_ms, headline_ratio, headline_above_fold_360,
# headline_above_fold_1440, no_hscroll_360, remote_requests, status.
# Thresholds: draw_ms at most 500, headline_ratio at least 2.0, both folds true, remote_requests 0.
# Exit codes: 0 every threshold met, 1 a threshold missed or a run failed, 2 bad usage, 3 tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/web.render.log"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
days=400; human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --days) [[ $# -ge 2 && "$2" =~ ^[0-9]+$ && "$2" -le 400 ]] || { echo "error: --days needs a whole number from 0 to 400. Fix: pass --days 400. Verify: the script prints draw_ms." >&2; exit 2; }; days="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints draw_ms." >&2; exit 2 ;;
  esac
done
[[ -x "$CHROME" ]] || { echo "error: Google Chrome is not at $CHROME, so the page cannot be measured. Fix 1: install Google Chrome. Fix 2: set CHROME to the path of a Chrome binary. Verify: re-run and see draw_ms." >&2; exit 3; }
NODE=(node); command -v mise >/dev/null && NODE=(mise exec -- node)
for tool in cargo curl perl sqlite3; do
  command -v "$tool" >/dev/null || { echo "error: $tool is not installed. Fix: install $tool. Verify: re-run and see draw_ms." >&2; exit 3; }
done

mkdir -p "$(dirname "$LOG")"; : >"$LOG"
work="$(mktemp -d)"; pid=""
cleanup() { [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT
fail() { echo "error: $1. Fix: $2. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run web.render.sh. Log: .agents/logs/web.render.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 || fail "the release build failed" "read the log"
BIN="$ROOT/target/release/loupe"
key="$("$BIN" gen-key)"
LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" "$ROOT/.agents/scripts/days.seed.sh" --data-dir "$work/data" --days "$days" >>"$LOG" 2>&1 \
  || fail "the seed failed" "read the log"
port="$(perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport')"
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work/data" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC LOUPE_LISTEN="127.0.0.1:$port" "$BIN" serve) 2>>"$LOG" & pid=$!
for _ in $(seq 1 100); do curl -sf -o /dev/null "http://127.0.0.1:$port/healthz" && break; sleep 0.1; done
curl -sf -o /dev/null "http://127.0.0.1:$port/" || fail "loupe serve did not answer on port $port" "read the log"
json="$(cd "$ROOT" && "${NODE[@]}" .agents/scripts/web.render.cdp.mjs "$CHROME" "http://127.0.0.1:$port/" "$work/chrome" 2>>"$LOG")" \
  || fail "the browser measurement failed" "read the log; check that Chrome starts headless"
echo "$json" >>"$LOG"
read -r draw ratio f360 f1440 nh remote < <(echo "$json" | perl -MJSON::PP -e 'local $/; my $j = decode_json(<STDIN>); print join(" ", $j->{draw_ms}, $j->{headline_ratio}, map({ $_ ? "true" : "false" } @$j{qw(headline_above_fold_360 headline_above_fold_1440 no_hscroll_360)}), $j->{remote_requests}), "\n"')
status=passed
perl -e "exit(($draw >= 0 && $draw <= 500 && $ratio >= 2.0 && $remote == 0) ? 0 : 1)" || status=failed
[[ $f360 == true && $f1440 == true && $nh == true ]] || status=failed
if [[ $human -eq 1 ]]; then
  echo "With $days days: first full draw $draw ms, headline ratio $ratio, above the fold at 360 px $f360 and at 1440 px $f1440, no horizontal scroll at 360 px $nh, remote requests $remote: $status."
else
  echo "days: $days draw_ms: $draw headline_ratio: $ratio headline_above_fold_360: $f360 headline_above_fold_1440: $f1440 no_hscroll_360: $nh remote_requests: $remote status: $status"
fi
[[ $status == passed ]] || fail "a render threshold is missed" "compare each value with its threshold in the help text and change web/ to meet it"
