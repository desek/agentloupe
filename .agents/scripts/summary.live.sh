#!/usr/bin/env bash
# @agents-index Runs a real claude -p session through the proxy, closes it, summarizes it live, then measures the raw-body retention.
#
# Usage: summary.live.sh [--human]
#   --human    Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): status, line_words, line_safe, calls, bodies_left, calls_unchanged,
# sessions_unchanged, claude_version, log.
# The script builds the release binary, starts loupe serve on a free port with a temporary data dir
# and LOUPE_IDLE_SECS=60, runs claude -p --model claude-haiku-5-5 through the proxy with a proxy token,
# waits for the idle close, stops serve, runs loupe summarize --once with LOUPE_SUMMARY_OAUTH_TOKEN
# from .env at the repository root, then runs it again with LOUPE_RAW_RETENTION_HOURS=0.
# The token is read only into the environment of loupe summarize; it is never printed or logged.
# Full output: .agents/logs/summary.live.log.
# Exit codes: 0 status done and line safe and no body left; 1 a check or a run failed; 2 bad usage;
# 3 required tool missing; 4 no LOUPE_SUMMARY_OAUTH_TOKEN in .env.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/summary.live.log"
human=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints status." >&2; exit 2 ;;
  esac
done

for tool in cargo curl perl sqlite3 claude; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the live summary cannot run. Fix: install $tool. Reference: docs/specification/operations.md, section Measurable targets. Verify: re-run and see status." >&2
    exit 3
  fi
done
if ! grep -q '^LOUPE_SUMMARY_OAUTH_TOKEN=.' "$ROOT/.env" 2>/dev/null; then
  echo "error: .env at the repository root has no LOUPE_SUMMARY_OAUTH_TOKEN. Fix 1: run claude setup-token and put the value in .env as LOUPE_SUMMARY_OAUTH_TOKEN=<value>. Fix 2: put LOUPE_SUMMARY_API_KEY in .env instead. Verify: re-run and see status: done." >&2
  exit 4
fi

mkdir -p "$(dirname "$LOG")"
: >"$LOG"
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

fail() { echo "error: $1. Fix: $2. Reference: docs/specification/summaries.md. Verify: re-run summary.live.sh and see status: done. Log: .agents/logs/summary.live.log" >&2; exit 1; }

(cd "$ROOT" && cargo build --locked --release --bin loupe) >>"$LOG" 2>&1 || fail "the release build failed" "read the log and correct the build"
BIN="$ROOT/target/release/loupe"
free_port() { perl -MIO::Socket::INET -e 'my $s=IO::Socket::INET->new(Listen=>1,LocalAddr=>"127.0.0.1:0") or die; print $s->sockport'; }
wait_port() { for _ in $(seq 1 100); do curl -s -o /dev/null "http://127.0.0.1:$1/healthz" && return 0; sleep 0.1; done; return 1; }

printf '[anthropic]\nbase_url = "https://api.anthropic.com"\nlitellm_provider = "anthropic"\n' >"$work/upstreams.toml"
key="$("$BIN" gen-key)"
envs=(PATH="$PATH" LOUPE_DATA_DIR="$work" LOUPE_DATA_KEY="$key" LOUPE_ACCOUNT_KEY="$key" LOUPE_UPSTREAMS_FILE="$work/upstreams.toml")
token="$(cd "$ROOT" && env -i "${envs[@]}" "$BIN" token create --name live | perl -MJSON::PP -e 'print decode_json(join "", <STDIN>)->{token}')"
port="$(free_port)"
(cd "$ROOT" && env -i "${envs[@]}" LOUPE_LISTEN="127.0.0.1:$port" LOUPE_IDLE_SECS=60 LOUPE_SUMMARY_INTERVAL_SECS=86400 "$BIN" serve) 2>>"$LOG" & serve_pid=$!; pids+=($serve_pid)
wait_port "$port" || fail "loupe serve did not start on port $port" "check the log for a configuration error"

claude_version="$(claude --version | awk '{print $1}')"
env -u CLAUDECODE ANTHROPIC_BASE_URL="http://127.0.0.1:$port/$token/claude-code/anthropic" \
  claude -p "Explain in two sentences what a binary search does." --model claude-haiku-5-5 >>"$LOG" 2>&1 \
  || fail "claude -p through the proxy failed" "check that Claude Code is logged in"

db="$work/loupe.db"
for _ in $(seq 1 60); do
  open="$(sqlite3 "$db" "SELECT count(*) FROM sessions WHERE state = 'open'" 2>/dev/null || echo 1)"
  total="$(sqlite3 "$db" "SELECT count(*) FROM sessions" 2>/dev/null || echo 0)"
  [[ "$total" -gt 0 && "$open" -eq 0 ]] && break
  sleep 5
done
[[ "$total" -gt 0 && "$open" -eq 0 ]] || fail "the session did not close in 300 seconds" "check the parser lines in the log"
kill -TERM "$serve_pid"; wait "$serve_pid" 2>/dev/null || true

calls_before="$(sqlite3 "$db" "SELECT count(*), sum(input + output) FROM calls")"
sessions_before="$(sqlite3 "$db" "SELECT group_concat(id || state || call_count) FROM sessions")"
(
  cd "$ROOT"
  set -a; . "$ROOT/.env"; set +a
  env -i "${envs[@]}" LOUPE_SUMMARY_OAUTH_TOKEN="$LOUPE_SUMMARY_OAUTH_TOKEN" "$BIN" summarize --once
  env -i "${envs[@]}" LOUPE_SUMMARY_OAUTH_TOKEN="$LOUPE_SUMMARY_OAUTH_TOKEN" LOUPE_RAW_RETENTION_HOURS=0 "$BIN" summarize --once
) >>"$LOG" 2>&1 || fail "loupe summarize --once failed" "read the log"

status="$(sqlite3 "$db" "SELECT coalesce(ss.status, 'none') || ':' || coalesce(ss.last_error_kind, '') FROM sessions s LEFT JOIN session_summaries ss ON ss.session_id = s.id ORDER BY s.started_at DESC LIMIT 1")"
kind="${status#*:}"; status="${status%%:*}"
failed="$(sqlite3 "$db" "SELECT count(*) FROM session_summaries WHERE status = 'failed'")"
line="$(sqlite3 "$db" "SELECT coalesce(line, '') FROM sessions s LEFT JOIN session_summaries ss ON ss.session_id = s.id ORDER BY s.started_at DESC LIMIT 1")"
words="$(wc -w <<<"$line" | tr -d ' ')"
safe=yes; [[ "$line" =~ [0-9/@] || "$words" -gt 12 ]] && safe=no
calls="$(sqlite3 "$db" "SELECT count(*) FROM calls")"
left="$(sqlite3 "$db" "SELECT count(*) FROM exchanges WHERE bodies_deleted_at IS NULL AND parse_outcome = 'call'")"
calls_same=false; [[ "$(sqlite3 "$db" "SELECT count(*), sum(input + output) FROM calls")" == "$calls_before" ]] && calls_same=true
sessions_same=false; [[ "$(sqlite3 "$db" "SELECT group_concat(id || state || call_count) FROM sessions")" == "$sessions_before" ]] && sessions_same=true
echo "status=$status error_kind=$kind failed=$failed words=$words safe=$safe calls=$calls bodies_left=$left claude=$claude_version" >>"$LOG"

if [[ $human -eq 1 ]]; then
  echo "The newest segment has the status $status ($words words, safe: $safe). $calls calls; $left exchanges with a call keep bodies. Claude Code $claude_version."
else
  echo "status: $status error_kind: ${kind:-none} failed: $failed line_words: $words line_safe: $safe calls: $calls bodies_left: $left calls_unchanged: $calls_same sessions_unchanged: $sessions_same claude_version: $claude_version log: .agents/logs/summary.live.log"
fi
[[ $status == done && $safe == yes && $failed -eq 0 && $left -eq 0 && $calls_same == true && $sessions_same == true ]] \
  || fail "the live check did not pass (status $status, error kind ${kind:-none})" "read the error kind; for unauthorized or bad_request check the OAuth header set in src/summary/headers.rs"
