#!/bin/bash
# @agents-index Prints the export lines that point one agent (Claude Code or Pi) at the proxy for one shell, never touching global settings.
#
# Usage: eval "$(agent.connect.sh --agent claude-code|pi [--token-file PATH] [--base URL] [--dry-run] [--check])"
#   --agent       claude-code or pi. Required.
#   --token-file  A file that holds the proxy token. Else the variable LOUPE_PROXY_TOKEN.
#                 The token is never an argument value, because the shell history and the process list keep arguments.
#   --base        The base of the proxy. Default: http://127.0.0.1:8787.
#   --dry-run     Print the planned file path and content with the token masked as lpt_…, and write nothing.
#   --check       Send one GET <base>/healthz first, and stop if the container does not answer.
#   -h, --help    Print this text.
#
# Output (stdout): shell lines to eval; a hint line starts with '#'. Errors go to stderr.
# Pi: writes ${XDG_STATE_HOME:-$HOME/.local/state}/agentloupe/pi-agent/models.json (dir 0700, file 0600)
# with only the baseUrl of the anthropic provider. No credential file is written or read.
# Exit codes: 0 success, 2 bad usage, 3 no token, 4 proxy not reachable, 5 write failed.
set -eu

DEFAULT_BASE="http://127.0.0.1:8787"
agent=""; token_file=""; base="$DEFAULT_BASE"; dry_run=0; check=0

usage() { sed -n '4,16p' "$0" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $2" >&2; exit "$1"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --agent) [ $# -ge 2 ] || die 2 "--agent needs a value. Fix: pass --agent claude-code or --agent pi."; agent="$2"; shift 2 ;;
    --token-file) [ $# -ge 2 ] || die 2 "--token-file needs a path. Fix: pass --token-file PATH."; token_file="$2"; shift 2 ;;
    --base) [ $# -ge 2 ] || die 2 "--base needs a URL. Fix: pass --base $DEFAULT_BASE."; base="${2%/}"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    --check) check=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die 2 "unknown argument $1. Fix: run --help for the flags." ;;
  esac
done

case "$agent" in
  claude-code|pi) ;;
  "") die 2 "missing --agent. Fix: pass --agent claude-code or --agent pi. Verify: an export line prints." ;;
  *) die 2 "unknown agent '$agent'. Fix: pass --agent claude-code or --agent pi." ;;
esac

if [ -n "$token_file" ]; then
  [ -r "$token_file" ] || die 3 "cannot read the token file $token_file. Fix: make a token with 'docker compose exec loupe loupe token create --name NAME' and save it in that file. Verify: 'docker compose exec loupe loupe token list' shows the name."
  token="$(tr -d ' \t\r\n' < "$token_file")"
else
  token="${LOUPE_PROXY_TOKEN:-}"
fi
if [ -z "$token" ]; then
  die 3 "no proxy token. Fix (a): pass --token-file PATH with a file that holds the token. Fix (b): export LOUPE_PROXY_TOKEN. Make a token with 'docker compose exec loupe loupe token create --name NAME'. Verify: 'docker compose exec loupe loupe token list' shows the name."
fi

if [ "$check" -eq 1 ]; then
  if ! curl -sf -m 5 -o /dev/null "$base/healthz"; then
    die 4 "the proxy at $base does not answer on /healthz. Fix (a): start the container with 'docker compose up -d'. Fix (b): pass --base with the right address. Verify: 'curl -s $base/healthz' returns {\"status\":\"ok\"}."
  fi
fi

# shown is the token as printed; the dry run masks it so it never reaches a log.
if [ "$dry_run" -eq 1 ]; then shown="lpt_…"; else shown="$token"; fi

if [ "$agent" = "claude-code" ]; then
  [ "$dry_run" -eq 1 ] && echo "# dry-run: no file is written"
  echo "export ANTHROPIC_BASE_URL=$base/$shown/claude-code/anthropic"
  echo "# hint: claude -p 'Reply with the word ok.' --model claude-haiku-5-5"
  exit 0
fi

dir="${XDG_STATE_HOME:-$HOME/.local/state}/agentloupe/pi-agent"
content="{\"providers\":{\"anthropic\":{\"baseUrl\":\"$base/$shown/pi/anthropic\"}}}"
if [ "$dry_run" -eq 1 ]; then
  echo "# dry-run: would write $dir/models.json"
  echo "# content: $content"
  echo "export PI_CODING_AGENT_DIR=$dir"
  exit 0
fi
( umask 077; mkdir -p "$dir" ) || die 5 "cannot make $dir. Fix: check the permissions of the parent directory. Verify: re-run and check that $dir exists."
chmod 700 "$dir"
# A unique temporary name: two shells can connect at the same time, and each must keep its own file until its rename.
tmp="$dir/.models.json.$$.tmp"
( umask 077; printf '%s\n' "$content" > "$tmp" ) || die 5 "cannot write $dir/models.json. Fix: check the free space and the permissions of $dir. Verify: re-run and check the file."
chmod 600 "$tmp"
mv -f "$tmp" "$dir/models.json"
echo "export PI_CODING_AGENT_DIR=$dir"
echo "# hint: pi -p --provider anthropic --model claude-haiku-5-5 'Reply with the word ok.' (credential: ANTHROPIC_OAUTH_TOKEN or a credential extension)"
