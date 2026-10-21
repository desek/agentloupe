#!/usr/bin/env bash
# @agents-index Pipeline entry point for the image stage: builds the container image from the Dockerfile; --dry-run prints the build command only.
#
# Usage: pipeline.image.sh [--human] [--dry-run] [--inputs] [--toolchain]
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --inputs     Print the paths that the stage reads, one per line, and exit 0 with no run.
#   --toolchain  Print the Docker server version and the local digest of each FROM image, and exit 0 with no run.
#   --dry-run    Print the build command and do not run it.
#
# Output (stdout, key: value): stage, status (passed or failed), seconds, toolchain, log.
# The toolchain version comes from rust-toolchain.toml; this script states no version.
# Full tool output goes to .agents/logs/pipeline.image.log.
# Exit codes: 0 passed, 1 stage failed, 2 bad usage, 3 required tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/pipeline.image.log"
human=0; dry_run=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

# The declared inputs: the Dockerfile, .dockerignore, every COPY source of the Dockerfile, and this script.
inputs() { printf '%s\n' Dockerfile .dockerignore rust-toolchain.toml Cargo.toml Cargo.lock build.rs web vendor/litellm src migrations tests examples config upstreams.toml .agents/scripts/pipeline.image.sh; }
toolchain() {
  docker version --format 'docker-server {{.Server.Version}}'
  local img id
  for img in $(awk 'toupper($1) == "FROM" {print $2}' "$ROOT/Dockerfile"); do
    # An absent base image fails here, so the verdict runs the stage in full; no pull, no network.
    id="$(docker image inspect --format '{{.Id}}' "$img")" || return 1
    printf 'base %s %s\n' "$img" "$id"
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    --inputs) inputs; exit 0 ;;
    --toolchain) toolchain; exit 0 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a stage line." >&2; exit 2 ;;
  esac
done

for tool in docker; do
  if ! command -v "$tool" >/dev/null; then
    echo "error: $tool is not installed, so the image stage cannot run. Fix: install it (rustup for cargo, Docker Desktop for docker). Reference: docs/specification/operations.md, section Pipeline. Verify: re-run and see 'status: passed'." >&2
    exit 3
  fi
done

toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
cmd=(docker build -t agent-loupe:local .)
mkdir -p "$(dirname "$LOG")"
if [[ $dry_run -eq 1 ]]; then echo "stage: image status: dry-run command: ${cmd[*]}"; exit 0; fi
start="$(now)"
if (cd "$ROOT" && "${cmd[@]}") >"$LOG" 2>&1; then status=passed; else status=failed; fi
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The image stage $status in $seconds seconds (toolchain $toolchain). Log: $LOG"
else
  echo "stage: image status: $status seconds: $seconds toolchain: $toolchain log: .agents/logs/pipeline.image.log"
fi
if [[ $status == failed ]]; then
  tail -n 30 "$LOG" >&2
  echo "error: the image stage failed. Fix: read the build log; check that Docker runs and that the build stage can reach crates.io. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.image.sh and see 'status: passed'." >&2
  exit 1
fi
