#!/bin/bash
# @agents-index Runs one Pi session through the loupe proxy with a prompt from a file, stdin closed, output to a log; prints the log path and the exit code.
#
# Usage: pi.session.sh --name NAME --prompt-file FILE [--model ID] [--skill DIR]... [--log FILE] [--dry-run]
#   --name         The session name that Pi records. Required.
#   --prompt-file  The file that holds the prompt. Required. A prompt is never an argument value.
#   --model        The model ID. Default: claude-opus-5-5.
#   --skill        A skill directory to load; can repeat.
#   --log          The log file. Default: .agents/logs/pi.<name>.log.
#   --dry-run      Print the pi command with the prompt file name and exit 0.
#   -h, --help     Print this text.
#
# Environment:
#   PI_CC_AUTH_EXT  Required, also with --dry-run. The path of the entry file of the extension
#                   pi-cc-auth, for example <clone of pi-cc-auth>/src/index.ts. The script holds no
#                   default path, so it names no directory of a machine.
#
# The proxy token comes from the file ~/.config/agentloupe-token through agent.connect.sh; the
# Pi credential comes from the extension pi-cc-auth, which reads the Claude Code login from the
# keychain. Standard input is /dev/null, because `pi -p` waits for the end of an open stdin.
# Output (stdout, key: value): log, exit_code; with --dry-run: command, log. Exit codes: 0 pi
# exited 0 (or the dry run printed the command), 2 bad usage, 3 PI_CC_AUTH_EXT is unset, empty,
# or names no file, or the token file or the proxy is not reachable, else the exit code of pi.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TOKEN_FILE="$HOME/.config/agentloupe-token"
name=""; prompt=""; model="claude-opus-5-5"; log=""; dry=0; skills=()
usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $2" >&2; exit "$1"; }
while [ $# -gt 0 ]; do
  case "$1" in
    --name) name="$2"; shift 2 ;;
    --prompt-file) prompt="$2"; shift 2 ;;
    --model) model="$2"; shift 2 ;;
    --skill) skills+=(--skill "$2"); shift 2 ;;
    --log) log="$2"; shift 2 ;;
    --dry-run) dry=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die 2 "unknown argument $1. Fix: see pi.session.sh --help." ;;
  esac
done
[ -n "$name" ] || die 2 "--name is missing. Fix: pass --name NAME."
[ -n "$prompt" ] && [ -f "$prompt" ] || die 2 "--prompt-file is missing or not a file. Fix: pass --prompt-file FILE."
# The extension path comes only from the environment, so the tracked script names no home directory.
EXT="${PI_CC_AUTH_EXT:-}"
[ -n "$EXT" ] || die 3 "PI_CC_AUTH_EXT is unset or empty, so Pi has no credential extension. Fix: clone pi-cc-auth and export PI_CC_AUTH_EXT=<clone>/src/index.ts, the path of its entry file. Verify: ls \"\$PI_CC_AUTH_EXT\" lists the file."
[ -f "$EXT" ] || die 3 "PI_CC_AUTH_EXT names the path $EXT, which is not a file. Fix: export PI_CC_AUTH_EXT=<clone of pi-cc-auth>/src/index.ts with a path that exists. Verify: ls \"\$PI_CC_AUTH_EXT\" lists the file."
[ -f "$TOKEN_FILE" ] || die 3 "the token file $TOKEN_FILE is missing. Fix: see docs/guide/connect-agents.md, section Connect the agents. Verify: ls $TOKEN_FILE."
log="${log:-$ROOT/.agents/logs/pi.$name.log}"
mkdir -p "$(dirname "$log")"
cmd=(pi -p --name "$name" --no-mcp --no-extensions -e "$EXT" ${skills[@]+"${skills[@]}"} --provider anthropic --model "$model" --thinking medium)
if [ "$dry" = 1 ]; then echo "command: ${cmd[*]} <prompt of $prompt>"; echo "log: $log"; exit 0; fi
eval "$("$ROOT/.agents/scripts/agent.connect.sh" --agent pi --token-file "$TOKEN_FILE" --check)" || die 3 "the proxy is not reachable. Fix: docker compose up -d. Verify: curl -sf http://127.0.0.1:8787/healthz."
cd "$ROOT"
set +e
"${cmd[@]}" "$(cat "$prompt")" > "$log" 2>&1 < /dev/null
code=$?
set -e
echo "log: $log"
echo "exit_code: $code"
exit "$code"
