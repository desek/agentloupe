#!/usr/bin/env bash
# @agents-index Runs the public instance scenario in an isolated compose project: isolation, read-only mount, routes, live delay, mirror lag, guard.
#
# Usage: public.scenario.sh --dir DIR [--agent-call] [--keep] [--human]
#   --dir DIR      A scratch directory for the env files, the token, and the logs. Required.
#   --agent-call   Also send one real Claude Code call (claude -p, Haiku) through the proxy and measure the live delay.
#   --keep         Leave the project loupe-scn running after the checks. Default: down -v at the end.
#   --human        Print a friendly view for a person. Also the default when stdout is a terminal.
#
# The project name is loupe-scn, on 127.0.0.1:18787 (role full) and 127.0.0.1:18788 (role public), with the
# image tag agent-loupe:scn and fresh volumes, so the loupe service of compose.yaml and its data stay untouched.
# Output (stdout, key: value per check): check, status (passed or failed), value. Last line: checks, failed, status.
# Full output: <DIR>/public.scenario.log. Exit codes: 0 every check passed, 1 a check failed, 2 bad usage, 3 tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
dir=""; agent_call=0; keep=0; human=0
[[ -t 1 ]] && human=1
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) [[ $# -ge 2 ]] || { echo "error: --dir needs a path. Fix: pass --dir /tmp/scn. Verify: the script prints checks." >&2; exit 2; }; dir="$2"; shift 2 ;;
    --agent-call) agent_call=1; shift ;;
    --keep) keep=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints checks." >&2; exit 2 ;;
  esac
done
[[ -n "$dir" ]] || { echo "error: --dir is required. Fix: pass --dir with a scratch directory. Verify: the script prints checks." >&2; exit 2; }
for t in docker curl node sqlite3 openssl python3; do
  command -v "$t" >/dev/null || { echo "error: $t is missing. Fix: install $t. Verify: command -v $t prints a path." >&2; exit 3; }
done
mkdir -p "$dir"; dir="$(cd "$dir" && pwd)"; log="$dir/public.scenario.log"; : > "$log"
umask 077

# Scratch configuration: generated keys for the role full, and no secret for the role public.
cat > "$dir/scn.env" <<ENV
LOUPE_DATA_KEY=$(openssl rand -base64 32)
LOUPE_ACCOUNT_KEY=$(openssl rand -base64 32)
LOUPE_TZ=Europe/Stockholm
LOUPE_SITE_TITLE=Scenario
ENV
printf 'LOUPE_TZ=Europe/Stockholm\nLOUPE_SITE_TITLE=Scenario\n' > "$dir/scn.public.env"
cat > "$dir/scn.override.yaml" <<YAML
services:
  loupe:
    image: agent-loupe:scn
    ports: !override
      - "127.0.0.1:18787:8787"
    env_file: !override
      - $dir/scn.env
  loupe-public:
    image: agent-loupe:scn
    ports: !override
      - "127.0.0.1:18788:8787"
    env_file: !override
      - path: $dir/scn.public.env
        required: false
YAML
dc() { (cd "$ROOT" && docker compose -p loupe-scn -f compose.yaml -f "$dir/scn.override.yaml" "$@"); }
now_ms() { python3 -c 'import time;print(int(time.time()*1000))'; }

checks=0; failed=0
report() { # name, passed (0 or 1), value
  checks=$((checks+1)); local s=passed; [[ "$2" == 1 ]] || { s=failed; failed=$((failed+1)); }
  if [[ $human -eq 1 ]]; then printf '%-28s %-7s %s\n' "$1" "$s" "$3"; else echo "check: $1 status: $s value: $3"; fi
  echo "check: $1 status: $s value: $3" >> "$log"
}
cleanup() { [[ $keep -eq 1 ]] || dc down -v >>"$log" 2>&1 || true; }
trap cleanup EXIT

dc down -v >>"$log" 2>&1 || true
dc build loupe >>"$log" 2>&1
t0=$(now_ms); dc up -d >>"$log" 2>&1
for _ in $(seq 1 60); do s=$(dc ps --format '{{.Service}}={{.Health}}' | tr '\n' ' '); [[ "$s" == *"loupe=healthy"* && "$s" == *"loupe-public=healthy"* ]] && break; sleep 1; done
report both_healthy "$([[ "$s" == *"loupe=healthy"* && "$s" == *"loupe-public=healthy"* ]] && echo 1 || echo 0)" "$(( $(now_ms) - t0 )) ms"

pub="loupe-scn-loupe-public-1"
files=$(docker exec "$pub" bash -c 'ls -A /data | tr "\n" " "; printf "|"; ls -A /data/public | tr "\n" " "')
report public_files "$([[ "$files" == "logs public |public.db " ]] && echo 1 || echo 0)" "$files"
secrets=$(docker exec "$pub" bash -c 'env | cut -d= -f1' | grep -cE 'KEY|TOKEN|SECRET' || true)
report public_env_no_secret "$([[ "$secrets" == 0 ]] && echo 1 || echo 0)" "$secrets secret names"
docker exec "$pub" bash -c 'touch /data/public/x' >>"$log" 2>&1 && ro=0 || ro=1
report public_mount_read_only "$ro" "touch under /data/public refused"
docker cp -q "$pub:/data/public/public.db" "$dir/public.copy.db"
tables=$(sqlite3 "$dir/public.copy.db" "select group_concat(name) from sqlite_schema where type='table'")
report mirror_tables "$([[ "$tables" == "days,live_state" ]] && echo 1 || echo 0)" "$tables"
h1=$(curl -s http://127.0.0.1:18787/healthz); h2=$(curl -s http://127.0.0.1:18788/healthz)
report healthz_same "$([[ -n "$h1" && "$h1" == "$h2" ]] && echo 1 || echo 0)" "$h2"
first=$(dc logs loupe-public 2>&1 | grep -m1 'loupe serve starting' || true)
report role_line "$([[ "$first" == *'"role":"public"'* ]] && echo 1 || echo 0)" "role public in the first line"

dc exec -T loupe loupe token create --name scn 2>/dev/null | grep -o 'lpt_[A-Za-z0-9_-]*' | head -1 > "$dir/token"
token=$(cat "$dir/token")
if [[ $agent_call -eq 1 ]]; then
  cat > "$dir/live.watch.mjs" <<'JS'
const ports = process.argv.slice(2).map(Number);
for (const port of ports) {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/live`);
  ws.onmessage = (m) => { const f = JSON.parse(m.data); if (f.kind !== 'heartbeat') console.log(JSON.stringify({ t: Date.now(), port, kind: f.kind })); };
}
setTimeout(() => process.exit(0), 40000);
JS
  node "$dir/live.watch.mjs" 18787 18788 > "$dir/live.frames.jsonl" 2>&1 &
  watch=$!; sleep 2
  env -u CLAUDECODE ANTHROPIC_BASE_URL="http://127.0.0.1:18787/$token/claude-code/anthropic" claude -p "say hi" --model claude-haiku-5-5 < /dev/null >>"$log" 2>&1 || true
  for _ in $(seq 1 30); do grep -q '"port":18788,"kind":"calls.batch"' "$dir/live.frames.jsonl" && break; sleep 1; done
  kill "$watch" 2>/dev/null || true
  d=$(python3 - "$dir/live.frames.jsonl" <<'PY'
import json, sys
f = [json.loads(l) for l in open(sys.argv[1]) if l.startswith('{')]
a = next((x['t'] for x in f if x['port'] == 18787 and x['kind'] == 'call.stored'), None)
b = next((x['t'] for x in f if x['port'] == 18788 and x['kind'] == 'calls.batch'), None)
print(b - a if a and b else -1)
PY
)
  report live_delay "$([[ "$d" -ge 0 && "$d" -le 2100 ]] && echo 1 || echo 0)" "$d ms (at most 2100)"
  dc exec -T loupe loupe aggregate --date today >>"$log" 2>&1
  day=$(TZ=Europe/Stockholm date +%Y-%m-%d); t0=$(now_ms); body=""
  for _ in $(seq 1 70); do body=$(curl -s "http://127.0.0.1:18788/v1/days/$day"); [[ "$body" == *'"calls"'* ]] && break; sleep 0.1; done
  lag=$(( $(now_ms) - t0 ))
  report mirror_day_lag "$([[ "$body" == *'"calls"'* && $lag -le 5000 ]] && echo 1 || echo 0)" "$lag ms (at most 5000)"
  same=$([[ "$(curl -s "http://127.0.0.1:18787/v1/days/$day")" == "$body" ]] && echo 1 || echo 0)
  report day_body_same "$same" "/v1/days/$day on both ports"
fi

fake="lpt_$(printf 'a%.0s' $(seq 1 43))"; bad=0; n=0
probe() { local code; code=$(curl -s -o "$dir/body.txt" -w '%{http_code}' -X "$1" "http://127.0.0.1:18788$2"); n=$((n+1))
  if [[ "$code" != 404 ]] || grep -q lpt_ "$dir/body.txt" || ! grep -q not_found "$dir/body.txt"; then bad=$((bad+1)); echo "404 probe failed: $1 $2 $code" >> "$log"; fi; }
for p in /private/ /private/login /private/api/v1/report /private/api/v1/sessions /private/api/providers /private/api/config /live/x /v1/other /healthz/x /unknown /a/b/c /claude-code/anthropic/v1/messages /pi/openai/v1/responses "/$fake/v1/days" "/$fake/claude-code/anthropic/v1/messages"; do probe GET "$p"; done
for p in /private/login /private/logout /claude-code/anthropic/v1/messages /pi/openai/v1/responses "/$fake/claude-code/anthropic/v1/messages" "/$token/claude-code/anthropic/v1/messages"; do probe POST "$p"; done
report not_found_list "$([[ $bad == 0 ]] && echo 1 || echo 0)" "$n entries, $bad failures, 0 bodies with lpt_ required"

dc stop loupe >>"$log" 2>&1
sleep 2; a=$(docker cp -q "$pub:/data/public/public.db" - | shasum)
ok=1; for p in / /v1/days /healthz; do [[ "$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:18788$p")" == 200 ]] || ok=0; done
b=$(docker cp -q "$pub:/data/public/public.db" - | shasum)
report serves_with_full_stopped "$ok" "/, /v1/days, /healthz answer 200"
report public_writes_nothing "$([[ "$a" == "$b" ]] && echo 1 || echo 0)" "mirror bytes unchanged after reads"

dc run --rm --no-deps -e LOUPE_DATA_KEY=c2NlbmFyaW8tb25seS1ub3QtYS1yZWFsLWtleS0wMDA= loupe-public > "$dir/guard.out" 2>&1 && rc=0 || rc=$?
leak=$(grep -c c2NlbmFyaW8 "$dir/guard.out" || true)
report secret_guard "$([[ $rc == 2 && $leak == 0 ]] && grep -q LOUPE_DATA_KEY "$dir/guard.out" && echo 1 || echo 0)" "exit $rc, value in output $leak times"

st=passed; [[ $failed -eq 0 ]] || st=failed
echo "checks: $checks failed: $failed status: $st"
[[ $failed -eq 0 ]]
