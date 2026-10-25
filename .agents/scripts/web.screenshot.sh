#!/usr/bin/env bash
# @agents-index Makes the README screenshot of the dashboard from seeded demo data: release build, seed, serve, headless Chrome, one PNG.
#
# Usage: web.screenshot.sh [--out PATH] [--days N] [--scheme dark|light] [--width W] [--height H] [--dry-run] [--human]
#   --out PATH   The PNG file. Default: docs/images/dashboard.png.
#   --days N     Published demo days, ending yesterday. Default: 400.
#   --scheme S   The emulated color scheme. Default: dark.
#   --width W    The viewport width in CSS pixels. Default: 1280.
#   --height H   The clipped height in CSS pixels. Default: 1030, the end of the second row of cards. The PNG has twice each size.
#   --dry-run    Print the plan and change nothing.
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#
# The data comes from days.seed.sh, never from a real data directory, so the image shows no data of a user.
# Output (stdout, key: value): out, bytes, scheme, status. Full output: .agents/logs/web.screenshot.log.
# Exit codes: 0 the PNG is written, 1 a step failed, 2 bad usage, 3 a tool is missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/web.screenshot.log"
out="$ROOT/docs/images/dashboard.png"; days=400; scheme=dark; width=1280; height=1030; dry=0; human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --days) days="$2"; shift 2 ;;
    --scheme) scheme="$2"; shift 2 ;;
    --width) width="$2"; shift 2 ;;
    --height) height="$2"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints out." >&2; exit 2 ;;
  esac
done
[[ "$scheme" == dark || "$scheme" == light ]] || { echo "error: --scheme must be dark or light, not $scheme. Fix: pass --scheme dark. Verify: the script prints scheme: dark." >&2; exit 2; }
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[[ -x "$CHROME" ]] || { echo "error: Google Chrome is not at $CHROME. Fix 1: install Google Chrome. Fix 2: set CHROME to a Chrome binary. Verify: re-run and see status: written." >&2; exit 3; }
for tool in cargo curl perl sqlite3; do
  command -v "$tool" >/dev/null || { echo "error: $tool is not installed. Fix: install $tool. Verify: re-run and see status: written." >&2; exit 3; }
done
NODE=(node); command -v mise >/dev/null && NODE=(mise exec -- node)
if [[ $dry -eq 1 ]]; then echo "out: $out days: $days scheme: $scheme width: $width height: $height status: dry_run"; exit 0; fi

mkdir -p "$(dirname "$LOG")" "$(dirname "$out")"; : >"$LOG"
work="$(mktemp -d)"; pid=""
cleanup() { [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT
fail() { echo "error: $1. Fix: $2. Verify: re-run web.screenshot.sh and see status: written. Log: .agents/logs/web.screenshot.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 || fail "the release build failed" "read the log"
BIN="$ROOT/target/release/loupe"
key="$("$BIN" gen-key)"
LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" "$ROOT/.agents/scripts/days.seed.sh" --data-dir "$work/data" --days "$days" >>"$LOG" 2>&1 \
  || fail "the seed failed" "read the log"
port="$(perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport')"
(cd "$ROOT" && env -i PATH="$PATH" LOUPE_DATA_DIR="$work/data" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_TZ=UTC LOUPE_LISTEN="127.0.0.1:$port" "$BIN" serve) 2>>"$LOG" & pid=$!
for _ in $(seq 1 100); do curl -sf -o /dev/null "http://127.0.0.1:$port/healthz" && break; sleep 0.1; done
curl -sf -o /dev/null "http://127.0.0.1:$port/" || fail "loupe serve did not answer on port $port" "read the log"
(cd "$ROOT" && "${NODE[@]}" .agents/scripts/web.screenshot.cdp.mjs "$CHROME" "http://127.0.0.1:$port/" "$work/chrome" "$out" --scheme "$scheme" --width "$width" --height "$height") >>"$LOG" 2>&1 \
  || fail "the screenshot failed" "read the log"
bytes=$(wc -c < "$out" | tr -d ' ')
if [[ $human -eq 1 ]]; then echo "Wrote $out ($bytes bytes, $scheme)"; else echo "out: $out bytes: $bytes scheme: $scheme status: written"; fi
