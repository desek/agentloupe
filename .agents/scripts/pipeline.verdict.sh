#!/usr/bin/env bash
# @agents-index Runs the pipeline stage entry points in the order format, lint, test, web, image, and prints the verdict.
#
# Usage: pipeline.verdict.sh [--human] [--no-cache]
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#   --no-cache  Run every stage; still write the fingerprints of the stages that passed.
#
# Output (stdout): one line per stage, "stage: <name> status: passed|failed|cached seconds: <n>",
# then "verdict: passed|failed seconds: <total>". Every stage that is not cached runs, so all failures show at once.
# A stage is cached only if its fingerprint equals the last passed one (pipeline.fingerprint.sh decides);
# a cached stage counts as passed and writes its fingerprint again. Any error in the fingerprint or the cache runs the stage in full.
# This script only calls the stage entry points and pipeline.fingerprint.sh; it owns the list and the order of the stages.
# Exit codes: 0 every stage passed, 1 one or more stages failed, 2 bad usage.
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
STAGES=(format lint test web image)
human=0; no_cache=()
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    --no-cache) no_cache=(--no-cache); shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints a verdict line." >&2; exit 2 ;;
  esac
done

now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
start="$(now)"; failed=(); passed=()
plan="$("$DIR/pipeline.fingerprint.sh" plan ${no_cache[@]+"${no_cache[@]}"} "${STAGES[@]}")" ||
  echo "diagnostic: pipeline.fingerprint.sh failed, so every stage runs in full." >&2
key="$(awk '$1 == "key:" {print $2}' <<<"$plan")"
for stage in "${STAGES[@]}"; do
  fp="$(awk -v s="$stage" '$1 == "stage:" && $2 == s {print $4}' <<<"$plan")"
  action="$(awk -v s="$stage" '$1 == "stage:" && $2 == s {print $6}' <<<"$plan")"
  t0="$(now)"
  # A cached stage writes its fingerprint again: a whole-tree hit proves each stage fingerprint passed, so the
  # cache then holds the fingerprints of this tree and not of an earlier, reverted tree.
  if [[ "$action" == cached ]]; then status=cached; [[ -n "$fp" && "$fp" != none ]] && passed+=("$stage=$fp")
  elif "$DIR/pipeline.$stage.sh" >/dev/null; then status=passed; [[ -n "$fp" && "$fp" != none ]] && passed+=("$stage=$fp")
  else status=failed; failed+=("$stage"); fi
  seconds="$(perl -e "printf '%.1f', $(now) - $t0")"
  if [[ $human -eq 1 ]]; then echo "The $stage stage $status in $seconds seconds."
  else echo "stage: $stage status: $status seconds: $seconds"; fi
done
# Only passed stages write; the tree key is written only when every stage passed or was cached.
tree_flag=(); [[ ${#failed[@]} -eq 0 ]] && tree_flag=(--tree)
"$DIR/pipeline.fingerprint.sh" record --key "${key:-none}" ${tree_flag[@]+"${tree_flag[@]}"} ${passed[@]+"${passed[@]}"} ||
  echo "diagnostic: the verdict cache write failed; the next run runs the stages in full." >&2
total="$(perl -e "printf '%.1f', $(now) - $start")"
verdict=passed; [[ ${#failed[@]} -gt 0 ]] && verdict=failed
if [[ $human -eq 1 ]]; then echo "The verdict is $verdict in $total seconds."
else echo "verdict: $verdict seconds: $total"; fi
if [[ $verdict == failed ]]; then
  echo "error: the stages ${failed[*]} failed. Fix: run each failed stage entry point alone, for example .agents/scripts/pipeline.${failed[0]}.sh, and read its log under .agents/logs/. Reference: docs/specification/operations.md, section Pipeline. Verify: re-run pipeline.verdict.sh and see 'verdict: passed'." >&2
  exit 1
fi
