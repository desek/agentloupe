#!/usr/bin/env bash
# @agents-index Dependency check of the lint stage: cargo deny advisories, licenses, bans, and sources against deny.toml, with the lock file locked.
#
# Usage: deps.deny.sh [--human] [--dry-run]
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run    Compare the license lists, print the commands, and fetch and check nothing.
#   -h, --help   Print this help.
#
# Purpose: the dependency check of docs/specification/operations.md (Dependency check). pipeline.lint.sh
# calls it after clippy, so the verdict keeps five stages. It runs, in this order:
#   1. The license lists: the accepted list of about.toml must equal the allow list of deny.toml, because
#      the third-party notices and the check must judge the same licenses. A license in one list only fails.
#   2. The fetch of the advisory database (cargo deny fetch db). This fetch is the one network use of the
#      lint stage. A failed fetch fails the check (fail closed), with an error that names the fetch.
#   3. cargo fetch --locked, a no-op when clippy fetched the crates before, then
#      cargo deny --offline --locked check advisories licenses bans sources, which reads the fetched
#      database and the local crates and makes no second fetch.
# deny.toml states the policy; the cargo-deny version comes from .tool-versions, and this script states none.
# Idempotent: it writes only its log and the advisory database cache of cargo deny.
#
# Output (stdout, key: value): check: deps status: passed|failed seconds: <s> tool: <cargo-deny version> log: <log>
#   With --dry-run: plan lines (plan: licenses, plan: fetch, plan: check).
# Full tool output goes to .agents/logs/deps.deny.log.
# Exit codes: 0 passed (or planned), 1 a check failed or the license lists differ, 2 bad usage,
#   3 cargo or cargo-deny is missing, 4 the advisory database could not be fetched.
# Reference: docs/specification/operations.md, section Dependency check.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/deps.deny.log"
REF="Reference: docs/specification/operations.md, section Dependency check."
human=0; dry_run=0
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the flags. Verify: deps.deny.sh --dry-run prints plan lines." 2 ;;
  esac
done

cd "$ROOT"
command -v cargo >/dev/null || fail "cargo is not installed, so the dependency check cannot run. Fix: install Rust with rustup. Verify: cargo --version." 3
if ! tool="$(cargo deny --version 2>/dev/null)"; then
  fail "cargo-deny is not installed, so the dependency check cannot run. Fix: mise install (it reads the cargo-deny pin of .tool-versions), or install the pinned version of .tool-versions by hand. $REF Verify: cargo deny --version prints the pinned version." 3
fi

# The strings of one TOML array, one per line: the array starts at the line "<key> = [" (within the
# table <table>, or at the top level when <table> is empty) and ends at the line "]".
toml_list() {
  awk -v table="$2" -v key="$3" '
    /^\[/ { cur = $0; gsub(/^\[|\].*$/, "", cur); next }
    !inside && cur == table && $0 ~ "^" key " *= *\\[" { inside = 1; sub(/^[^[]*\[/, "") }
    inside {
      line = $0; sub(/#.*/, "", line)
      while (match(line, /"[^"]*"/)) { print substr(line, RSTART + 1, RLENGTH - 2); line = substr(line, RSTART + RLENGTH) }
      if ($0 ~ /\]/) exit
    }' "$1" | LC_ALL=C sort -u
}
[[ -f deny.toml && -f about.toml ]] || fail "deny.toml or about.toml is missing at the root. Fix: restore them from git (git checkout -- deny.toml about.toml). $REF Verify: ls deny.toml about.toml." 1
deny_list="$(toml_list deny.toml licenses allow)"
about_list="$(toml_list about.toml "" accepted)"
[[ -n "$deny_list" ]] || fail "deny.toml has no allow list in [licenses]. Fix: restore the allow list from git. $REF Verify: grep -A3 '^allow' deny.toml." 1
only_deny="$(LC_ALL=C comm -23 <(echo "$deny_list") <(echo "$about_list") | paste -sd ',' - | sed 's/,/, /g')"
only_about="$(LC_ALL=C comm -13 <(echo "$deny_list") <(echo "$about_list") | paste -sd ',' - | sed 's/,/, /g')"
if [[ -n "$only_deny$only_about" ]]; then
  fail "the license lists differ: only in the allow list of deny.toml: ${only_deny:-none}; only in the accepted list of about.toml: ${only_about:-none}. Fix: make the two lists equal; add a license to both only when it is permissive. $REF Verify: re-run deps.deny.sh." 1
fi
count="$(echo "$deny_list" | wc -l | tr -d ' ')"

fetch=(cargo deny --color never fetch db)
crates=(cargo fetch --locked)
check=(cargo deny --color never --offline --locked check advisories licenses bans sources)
if [[ $dry_run -eq 1 ]]; then
  if [[ $human -eq 1 ]]; then
    echo "The license lists agree ($count licenses). Would run '${fetch[*]}', '${crates[*]}', then '${check[*]}' with $tool. Nothing was fetched or checked."
  else
    echo "plan: licenses status: equal count: $count"
    echo "plan: fetch command: ${fetch[*]}"
    echo "plan: crates command: ${crates[*]}"
    echo "plan: check command: ${check[*]} tool: $tool"
  fi
  exit 0
fi

now() { perl -MTime::HiRes=time -e 'printf "%.1f\n", time'; }
mkdir -p "$(dirname "$LOG")"
start="$(now)"
echo "\$ ${fetch[*]}" >"$LOG"
if ! "${fetch[@]}" >>"$LOG" 2>&1; then
  tail -n 20 "$LOG" >&2
  fail "the fetch of the advisory database failed, so the advisories cannot be checked and the check fails closed. Fix: check the network to github.com (the RustSec advisory database), then run again; an outage passes on a later run. $REF Verify: cargo deny fetch db exits 0." 4
fi
echo "\$ ${crates[*]}" >>"$LOG"
if ! "${crates[@]}" >>"$LOG" 2>&1; then
  tail -n 20 "$LOG" >&2
  fail "cargo fetch --locked failed, so the crates of Cargo.lock are not local. Fix: check the network to crates.io and that Cargo.lock is current (cargo metadata --locked). $REF Verify: cargo fetch --locked exits 0." 1
fi
echo "\$ ${check[*]}" >>"$LOG"
if "${check[@]}" >>"$LOG" 2>&1; then status=passed; else status=failed; fi
seconds="$(perl -e "printf '%.1f', $(now) - $start")"

if [[ $human -eq 1 ]]; then
  echo "The dependency check $status in $seconds seconds ($tool). Log: $LOG"
else
  echo "check: deps status: $status seconds: $seconds tool: $tool log: .agents/logs/deps.deny.log"
fi
if [[ $status == failed ]]; then
  # Each finding with its crate: the title and advisory ID of an advisory, the crate and the license
  # expression of a refused license (from the path of its Cargo.toml); no path of this machine.
  awk '/^error\[/ {title = $0; print; next}
       title ~ /^error\[rejected\]/ && /registry\/src\/.*\/Cargo\.toml/ {n = $0; sub(/\/Cargo\.toml.*/, "", n); sub(/.*\//, "", n); crate = n; next}
       title ~ /^error\[rejected\]/ && crate != "" && /license *=/ {l = $0; sub(/.*license *= *"/, "", l); sub(/".*/, "", l); print "  crate: " crate " license: " l; crate = ""; next}
       /^ *├ ID: / && title != "" {sub(/^ *├ /, "  "); print}' "$LOG" | head -n 40 >&2 || true
  echo "error: the dependency check failed. Fix: read each error of the log: update a crate with a vulnerability (cargo update -p <crate> --precise <fixed version>); for a license, add it to both lists only when it is permissive; when no patched version exists, add an ignore entry with the advisory, the reason, and a review date to deny.toml. $REF Verify: re-run deps.deny.sh and see 'status: passed'." >&2
  exit 1
fi
