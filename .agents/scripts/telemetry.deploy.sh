#!/usr/bin/env bash
# @agents-index Tests and deploys the telemetry endpoint: the Worker agentloupe-telemetry and its D1 database, through the pinned wrangler, with a dry run.
#
# Usage: telemetry.deploy.sh [--dry-run] [--test] [--human]
#   --dry-run   Run the pin check, the Worker test, the custom domain check, the login check, and wrangler
#               deploy --dry-run; print each step that a real run does; change nothing on the Cloudflare
#               account or in the repository.
#   --test      Run the pin check and the Worker test alone. Needs no login and no network to the account.
#   --human     Print a friendly view for a person. Also the default when stdout is a terminal.
#   -h, --help  Print this help.
#
# Purpose: the one way to put the telemetry endpoint of docs/specification/telemetry.md on the owner's
# account. Only the owner runs it, on the owner's machine, with the owner's consent; no workflow and no
# pipeline script runs it. A real run does, in this order: the pin check (wrangler --version equals the
# npm:wrangler line of .tool-versions), the Worker test under telemetry/test/, the custom domain check (one
# routes entry with custom_domain = true under the zone agentloupe.dev, and workers_dev = false in
# telemetry/wrangler.toml), the login check (wrangler whoami), the D1 database agentloupe-telemetry (made
# only when the account has none of that name; its ID goes into telemetry/wrangler.toml, which the owner
# commits), the remote migration, and the deploy, which must list the custom domain. It ends with the host,
# which is the custom domain telemetry.agentloupe.dev, and the command that sets the repository variable
# LOUPE_TELEMETRY_HOST, which the owner runs. It is idempotent: a second run makes no second database,
# applies no migration again, and prints the same host. It stores no Cloudflare credential and never sets
# the repository variable.
#
# Output (stdout, key: value), one line per step, then one summary line:
#   step: <pin|test|domain|login|database|migration|deploy> status: passed|planned|skipped [detail: ...]
#   result: done|planned|tested host: <host or none> set_variable: gh variable set LOUPE_TELEMETRY_HOST --body <host>
# Full wrangler and test output goes to .agents/logs/telemetry.deploy.log.
# Exit codes: 0 done, 1 a step failed, 2 bad usage, 3 wrangler missing or not the pinned version,
#   4 wrangler not logged in, 5 telemetry/wrangler.toml has no single custom domain on agentloupe.dev or
#   leaves workers_dev on.
# Reference: docs/specification/telemetry.md, section Deploy script.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/telemetry"
CONFIG="$DIR/wrangler.toml"
NAME="agentloupe-telemetry"
REPO="desek/agentloupe"
LOG="$ROOT/.agents/logs/telemetry.deploy.log"
PLACEHOLDER="00000000-0000-0000-0000-000000000000"
ZONE="agentloupe.dev"
human=0; dry_run=0; test_only=0
[[ -t 1 ]] && human=1
export WRANGLER_SEND_METRICS=false NO_COLOR=1 CI=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }
step() {
  if [[ $human -eq 1 ]]; then echo "The $1 step ${2}${3:+: $3}."; else echo "step: $1 status: $2${3:+ detail: $3}"; fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=1; shift ;;
    --test) test_only=1; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the flags. Verify: telemetry.deploy.sh --test prints step lines." 2 ;;
  esac
done
[[ $dry_run -eq 1 && $test_only -eq 1 ]] && fail "--dry-run and --test do not combine. Fix: pass one of them. Verify: re-run." 2

cd "$ROOT"
mkdir -p "$(dirname "$LOG")"
: >"$LOG"

# 1. The pin: wrangler and node through mise, which reads .tool-versions; a plain wrangler on PATH otherwise.
pin="$(awk '$1 == "npm:wrangler" {print $2}' .tool-versions 2>/dev/null || true)"
[[ -n "$pin" ]] || fail ".tool-versions has no npm:wrangler line, so the script has no wrangler pin. Fix: add the line 'npm:wrangler 4.100.0' to .tool-versions, then run mise install. Reference: docs/specification/telemetry.md, section Deploy script. Verify: telemetry.deploy.sh --test prints step: pin status: passed." 3
if command -v mise >/dev/null; then W=(mise exec -- wrangler); N=(mise exec -- node); else W=(wrangler); N=(node); fi
have="$("${W[@]}" --version 2>>"$LOG" | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1 || true)"
if [[ "$have" != "$pin" ]]; then
  fail "wrangler is ${have:-missing}, but .tool-versions pins npm:wrangler $pin. Fix: run 'mise install npm:wrangler@$pin' in the repository (or 'npm install -g wrangler@$pin' without mise). Pin file: .tool-versions. Verify: 'mise exec -- wrangler --version' prints $pin." 3
fi
step pin passed "wrangler $have"

# 2. The Worker test, local only.
if ! WRANGLER="${W[*]}" "${N[@]}" --test "$DIR/test/worker.test.mjs" >>"$LOG" 2>&1; then
  tail -n 30 "$LOG" >&2
  fail "the Worker test failed, so nothing is deployed. Fix: read .agents/logs/telemetry.deploy.log and correct telemetry/src/index.js or the test. Verify: telemetry.deploy.sh --test exits 0." 1
fi
cases="$(sed -n 's/^# pass \([0-9]*\)$/\1/p' "$LOG" | tail -n 1)"
step test passed "${cases:-0} Worker cases passed"
if [[ $test_only -eq 1 ]]; then
  if [[ $human -eq 1 ]]; then echo "The Worker test passed; nothing was deployed."; else echo "result: tested host: none"; fi
  exit 0
fi

# 3. The custom domain: the one routes entry with custom_domain = true, a host under the zone, and workers_dev off.
domains="$(grep -Eo 'pattern *= *"[^"]+" *, *custom_domain *= *true' "$CONFIG" | sed -E 's/^pattern *= *"([^"]+)".*/\1/' || true)"
host="$(head -n 1 <<<"$domains")"
if [[ -z "$host" || "$(grep -c . <<<"$domains")" -ne 1 || "$host" != *".$ZONE" ]]; then
  fail "telemetry/wrangler.toml does not hold exactly one custom domain under $ZONE (found: ${domains:-none}). Fix: set routes = [{ pattern = \"telemetry.$ZONE\", custom_domain = true }] in telemetry/wrangler.toml. Reference: docs/specification/telemetry.md, section Telemetry endpoint. Verify: telemetry.deploy.sh --dry-run prints step: domain status: passed detail: $host." 5
fi
grep -Eq '^workers_dev *= *false$' "$CONFIG" || fail "telemetry/wrangler.toml leaves workers_dev on, so the old workers.dev host would still answer next to the custom domain $host. Fix: set workers_dev = false in telemetry/wrangler.toml. Verify: telemetry.deploy.sh --dry-run prints step: domain status: passed." 5
step domain passed "$host"

# 4. The login; the output (an e-mail address and the account) is not printed.
if ! "${W[@]}" whoami --json >/dev/null 2>>"$LOG"; then
  fail "wrangler is not logged in to a Cloudflare account. Fix: run 'mise exec -- wrangler login' as the owner of the account. Verify: 'mise exec -- wrangler whoami' says that you are logged in." 4
fi
step login passed "wrangler is logged in"

# 5. The database: one of the name, made only when absent.
list="$("${W[@]}" d1 list --json 2>>"$LOG")" || fail "wrangler d1 list failed. Fix: read .agents/logs/telemetry.deploy.log; check the network and the login. Verify: 'mise exec -- wrangler d1 list' lists the databases." 1
db_id="$(printf '%s' "$list" | "${N[@]}" -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const m=JSON.parse(s).filter(d=>d.name===process.argv[1]);console.log(m.length===1?m[0].uuid:m.length)})' "$NAME")"
configured="$(sed -n 's/^database_id = "\(.*\)"$/\1/p' "$CONFIG")"
case "$db_id" in
  0) db_state=absent ;;
  *-*) db_state=present ;;
  *) fail "the account has $db_id databases of the name $NAME, so the script cannot pick one. Fix: delete the extra ones in the Cloudflare dashboard (D1) after a check of their rows. Verify: 'mise exec -- wrangler d1 list' lists one $NAME." 1 ;;
esac

if [[ $dry_run -eq 1 ]]; then
  if [[ $db_state == absent ]]; then step database planned "would create $NAME and write its ID into telemetry/wrangler.toml"
  else step database planned "exists, ID ${db_id}; telemetry/wrangler.toml $([[ $configured == "$db_id" ]] && echo holds it || echo would get it)"; fi
  step migration planned "would apply the pending migrations of telemetry/migrations to the remote $NAME"
  out="$(mktemp -d)"
  if ! (cd "$DIR" && "${W[@]}" deploy --dry-run --outdir "$out" --config "$CONFIG") >>"$LOG" 2>&1; then
    rm -rf "$out"; tail -n 20 "$LOG" >&2
    fail "wrangler deploy --dry-run failed. Fix: read .agents/logs/telemetry.deploy.log and correct telemetry/wrangler.toml or telemetry/src/index.js. Verify: telemetry.deploy.sh --dry-run exits 0." 1
  fi
  rm -rf "$out"
  step deploy planned "wrangler deploy --dry-run passed; a real run deploys $NAME to the custom domain $host"
  if [[ $human -eq 1 ]]; then echo "The dry run passed; nothing changed on the account or in the repository."; else echo "result: planned host: none"; fi
  exit 0
fi

if [[ $db_state == absent ]]; then
  "${W[@]}" d1 create "$NAME" >>"$LOG" 2>&1 || fail "wrangler d1 create $NAME failed. Fix: read .agents/logs/telemetry.deploy.log; check the D1 limits of the free plan. Verify: 'mise exec -- wrangler d1 list' lists $NAME." 1
  list="$("${W[@]}" d1 list --json 2>>"$LOG")"
  db_id="$(printf '%s' "$list" | "${N[@]}" -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const m=JSON.parse(s).filter(d=>d.name===process.argv[1]);console.log(m.length===1?m[0].uuid:"")})' "$NAME")"
  [[ -n "$db_id" ]] || fail "the new database $NAME has no ID in wrangler d1 list. Fix: run the script again; it finds the database and does not make a second one. Verify: 'mise exec -- wrangler d1 list' lists one $NAME." 1
  step database passed "created, ID $db_id"
else
  step database passed "exists, ID $db_id; none created"
fi
if [[ "$configured" != "$db_id" ]]; then
  sed -i.bak "s/^database_id = \".*\"$/database_id = \"$db_id\"/" "$CONFIG" && rm -f "$CONFIG.bak"
  step database passed "telemetry/wrangler.toml now holds the ID; commit the change"
fi

# 6. The remote migration; wrangler keeps the list of applied migrations in the database.
mig="$(cd "$DIR" && "${W[@]}" d1 migrations apply "$NAME" --remote --config "$CONFIG" 2>&1)" || {
  printf '%s\n' "$mig" >>"$LOG"; tail -n 20 "$LOG" >&2
  fail "the remote migration of $NAME failed. Fix: read .agents/logs/telemetry.deploy.log and correct the file under telemetry/migrations/. Verify: 'mise exec -- wrangler d1 migrations list $NAME --remote' lists no pending migration." 1
}
printf '%s\n' "$mig" >>"$LOG"
if grep -q "No migrations to apply" <<<"$mig"; then applied=0; else applied="$(grep -cE '[0-9]{4}_[a-z0-9_]+\.sql.*✅' <<<"$mig" || true)"; fi
step migration passed "$applied applied"

# 7. The deploy; the host is the custom domain, which wrangler lists as "<host> (custom domain)".
dep="$(cd "$DIR" && "${W[@]}" deploy --config "$CONFIG" 2>&1)" || {
  printf '%s\n' "$dep" >>"$LOG"; tail -n 20 "$LOG" >&2
  fail "wrangler deploy of the custom domain $host failed. Fix: read .agents/logs/telemetry.deploy.log; the zone $ZONE must be on the logged-in account, and $host must have no DNS record that another Worker or origin holds (remove it in the Cloudflare dashboard, DNS of $ZONE). Verify: re-run telemetry.deploy.sh." 1
}
printf '%s\n' "$dep" >>"$LOG"
grep -Fq "$host (custom domain)" <<<"$dep" || fail "wrangler deploy did not list the custom domain $host. Fix: read .agents/logs/telemetry.deploy.log; check the routes entry in telemetry/wrangler.toml and that the zone $ZONE is on the logged-in account (Cloudflare dashboard, Workers, $NAME, Settings, Domains and Routes). Verify: re-run telemetry.deploy.sh; it prints step: deploy status: passed detail: deployed $NAME to $host." 1
step deploy passed "deployed $NAME to $host"
if [[ $human -eq 1 ]]; then
  echo "Deployed to $host. Set the repository variable once: gh variable set LOUPE_TELEMETRY_HOST --body $host --repo $REPO"
else
  echo "result: done host: $host set_variable: gh variable set LOUPE_TELEMETRY_HOST --body $host --repo $REPO"
fi
