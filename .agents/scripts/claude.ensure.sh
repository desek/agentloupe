#!/bin/bash
# @agents-index Makes the Claude Code binary of the mise install runnable: runs the package postinstall when the native binary is missing, then checks claude --version.
#
# Usage: claude.ensure.sh [--dry-run] [--human]
#   --dry-run   Print what the script would do and exit 0 with no change.
#   --human     Print a friendly view. Also the default when stdout is a terminal.
#   -h, --help  Print this text.
#
# The hosted runner installs @anthropic-ai/claude-code through the npm backend of mise with the package
# scripts off, so the platform binary that the postinstall downloads is missing and claude stops with
# "claude native binary not installed". This script runs that postinstall once, from the install of mise.
# Output (stdout, key: value): claude: <version> status: ready|installed|planned. Exit codes: 0 ready,
# 2 bad usage, 3 the mise install of Claude Code is missing, 4 the postinstall ran but claude still fails.
set -euo pipefail
dry=0; human=0; [[ -t 1 ]] && human=1
usage() { sed -n '4,8p' "$0" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $2" >&2; exit "$1"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die 2 "unknown argument $1. Fix: see claude.ensure.sh --help." ;;
  esac
done
version() { claude --version 2>/dev/null | awk 'NR == 1 {print $1}'; }
out() { if [[ $human -eq 1 ]]; then echo "Claude Code $1 is $2."; else echo "claude: $1 status: $2"; fi; }
if v="$(version)" && [[ -n "$v" ]]; then out "$v" ready; exit 0; fi
dir="$(mise where npm:@anthropic-ai/claude-code 2>/dev/null || true)"
[[ -n "$dir" && -d "$dir" ]] || die 3 "the mise install of @anthropic-ai/claude-code is missing. Fix: run 'mise install npm:@anthropic-ai/claude-code' with the pin of .tool-versions. Reference: docs/specification/dependencies.md, section Claude Code pin. Verify: mise where npm:@anthropic-ai/claude-code prints a directory."
post="$(find "$dir" -path '*claude-code/install.cjs' -print -quit 2>/dev/null || true)"
[[ -n "$post" ]] || die 3 "the install of Claude Code under $dir has no install.cjs. Fix: run 'mise install npm:@anthropic-ai/claude-code' again. Verify: find $dir -name install.cjs prints one path."
if [[ $dry -eq 1 ]]; then out "$(basename "$dir")" planned; exit 0; fi
node "$post" >/dev/null 2>&1 || true
v="$(version)" && [[ -n "$v" ]] || die 4 "claude does not run after the postinstall of $post. Fix: read the output of 'node $post' and the network access of the runner. Reference: docs/specification/dependencies.md, section Claude Code pin. Verify: claude --version prints a version."
out "$v" installed
