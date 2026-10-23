#!/usr/bin/env bash
# @agents-index Runs the light mode scenario in headless Chrome against a scratch loupe serve with 400 seeded days and the admin token.
#
# Usage: web.scheme.scenario.sh [--human]
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# The script builds the release binary, seeds a scratch store with days.seed.sh, starts loupe serve on a free
# port with a random admin token, and runs web.scheme.scenario.mjs with the pinned Node against Google Chrome.
# It writes the four screenshots dashboard.light-mode.{light,dark}.{1440,360}.png into .agents/scenarios/ and
# stops the server and Chrome on exit. Full output, one line per check: .agents/logs/web.scheme.scenario.log.
# Output (stdout, key: value): checks, failed, status. Reference: .agents/scenarios/dashboard.light-mode.md.
# Exit codes: 0 every check passed, 1 a check failed or a run failed, 2 bad usage, 3 tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/web.scheme.scenario.log"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints checks." >&2; exit 2 ;;
  esac
done
[[ -x "$CHROME" ]] || { echo "error: Google Chrome is not at $CHROME. Fix 1: install Google Chrome. Fix 2: set CHROME to the path of a Chrome binary. Verify: re-run and see checks." >&2; exit 3; }
NODE=(node); command -v mise >/dev/null && NODE=(mise exec -- node)
for tool in cargo curl perl sqlite3 openssl; do
  command -v "$tool" >/dev/null || { echo "error: $tool is not installed. Fix: install $tool. Verify: re-run and see checks." >&2; exit 3; }
done

mkdir -p "$(dirname "$LOG")"; : >"$LOG"
work="$(mktemp -d)"; pid=""
cleanup() { [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT
fail() { echo "error: $1. Fix: $2. Reference: .agents/scenarios/dashboard.light-mode.md. Verify: re-run web.scheme.scenario.sh. Log: .agents/logs/web.scheme.scenario.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 || fail "the release build failed" "read the log"
BIN="$ROOT/target/release/loupe"
key="$("$BIN" gen-key)"
admin="$(openssl rand -hex 24)"
LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" "$ROOT/.agents/scripts/days.seed.sh" --data-dir "$work/data" --days 400 >>"$LOG" 2>&1 \
  || fail "the seed failed" "read the log"
port="$(perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport')"
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work/data" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_ADMIN_TOKEN="$admin" LOUPE_TZ=UTC LOUPE_LISTEN="127.0.0.1:$port" "$BIN" serve) 2>>"$LOG" & pid=$!
for _ in $(seq 1 100); do curl -sf -o /dev/null "http://127.0.0.1:$port/healthz" && break; sleep 0.1; done
curl -sf -o /dev/null "http://127.0.0.1:$port/" || fail "loupe serve did not answer on port $port" "read the log"
json="$(cd "$ROOT" && "${NODE[@]}" .agents/scripts/web.scheme.scenario.mjs "$CHROME" "http://127.0.0.1:$port/" "$work/chrome" "$admin" "$ROOT/.agents/scenarios" 2>>"$LOG")" \
  || fail "the browser run failed" "read the log; check that Chrome starts headless"
# One line per check in the log; the admin token is random per run and never printed.
echo "$json" | perl -MJSON::PP -e 'binmode STDOUT, ":utf8"; local $/; my $j = decode_json(<STDIN>); printf "%s %4d %s: %s\n", ($_->{ok} ? "ok  " : "FAIL"), $_->{width}, $_->{name}, $_->{detail} for @{$j->{checks}}' >>"$LOG"
read -r total failed < <(echo "$json" | perl -MJSON::PP -e 'local $/; my $j = decode_json(<STDIN>); my @c = @{$j->{checks}}; print scalar(@c), " ", scalar(grep { !$_->{ok} } @c), "\n"')
status=passed; [[ $failed -eq 0 && $total -gt 0 ]] || status=failed
if [[ $human -eq 1 ]]; then
  echo "The light mode scenario ran $total checks, $failed failed: $status. Log: $LOG"
else
  echo "checks: $total failed: $failed status: $status"
fi
[[ $status == passed ]] || fail "a scenario check failed" "read the FAIL lines in the log and change web/ to meet them"
