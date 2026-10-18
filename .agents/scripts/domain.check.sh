#!/usr/bin/env bash
# @agents-index Checks if names are registered under .dev and .app through the Google Registry RDAP service.
#
# Usage: domain.check.sh [--tlds dev,app] [--free-only] [--delay SECONDS] [--dry-run] NAME [NAME ...]
#        echo "name1 name2" | domain.check.sh [--tlds dev,app]
#
# Output: a tab-separated table (name, then one column per TLD) on stdout.
#   Values: free (RDAP 404), taken (RDAP 200), error:<http code> (lookup failed).
#   "free" means not registered. A reserved or premium name can still be free here and cost more.
# Exit codes: 0 all lookups answered, 2 bad usage, 3 at least one lookup failed.
set -euo pipefail

RDAP="https://pubapi.registry.google/rdap/domain"
tlds="dev,app"; free_only=0; dry_run=0; delay=1; names=()

usage() { sed -n '4,11p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tlds) tlds="${2:?--tlds needs a value, for example --tlds dev,app}"; shift 2 ;;
    --free-only) free_only=1; shift ;;
    --delay) delay="${2:?--delay needs seconds, for example --delay 1}"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "error: unknown flag $1. Fix: run --help for the flags." >&2; exit 2 ;;
    *) names+=("$1"); shift ;;
  esac
done
if [[ ${#names[@]} -eq 0 && ! -t 0 ]]; then read -r -a names <<<"$(tr '\n' ' ')"; fi
if [[ ${#names[@]} -eq 0 ]]; then
  echo "error: no names given. Fix: pass names as arguments or on stdin. Verify: one row per name on stdout." >&2
  exit 2
fi

IFS=',' read -r -a tld_list <<<"$tlds"
printf "name"; for t in "${tld_list[@]}"; do printf "\t.%s" "$t"; done; printf "\n"

failed=0
for n in "${names[@]}"; do
  n="$(echo "$n" | tr '[:upper:]' '[:lower:]')"
  row="$n"; any_free=0
  for t in "${tld_list[@]}"; do
    if [[ $dry_run -eq 1 ]]; then row+=$'\t'"dry-run:$RDAP/$n.$t"; continue; fi
    # The registry rate-limits with HTTP 429, so pause between requests and back off on 429.
    for attempt in 1 2 3 4 5; do
      sleep "$delay"
      code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$RDAP/$n.$t" || echo 000)"
      [[ "$code" == "429" ]] || break
      sleep $(( attempt * 5 ))
    done
    case "$code" in
      404) row+=$'\tfree'; any_free=1 ;;
      200) row+=$'\ttaken' ;;
      *) row+=$'\t'"error:$code"; failed=1
         echo "error: lookup for $n.$t returned HTTP $code. Fix: re-run later (rate limit or network). Verify: the cell shows free or taken." >&2 ;;
    esac
  done
  if [[ $free_only -eq 0 || $any_free -eq 1 ]]; then printf "%s\n" "$row"; fi
done
exit $(( failed ? 3 : 0 ))
