#!/bin/bash
# @agents-index Shell tests of deps.freshness.sh with a fake curl, gh, claude, and cargo on recorded registry answers: classes, latest rule, rate, errors, update runs and guards, issues, assessments, workflow.
#
# Usage: deps.freshness.test.sh [name...]. With names, only those tests run. Each test makes a scratch git
# repository with a local bare origin: a Cargo.toml and Cargo.lock of chosen crates, a
# workflow with one action pin, the script, its shared helper, its two prompt templates, a stand-in
# pipeline.verdict.sh, and a small price file. A fake curl answers the registries from the recorded files of
# tests/fixtures/freshness/ and records the time, the User-Agent, and the URL of each request; it also answers
# GET /healthz of the smoke check. A fake gh answers GET calls from recorded files and records every call; a call
# with another method is a write. A fake claude records its arguments, its environment, and its prompt, applies
# the edit of the test, and prints a recorded claude -p result. A fake cargo puts a stand-in loupe binary in
# target/debug/. The script runs with --fixtures, so it uses no network. Prints one line per test:
# "test: <name> status: passed|failed", with the output of a failed test.
# Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIX="$ROOT/tests/fixtures/freshness"
DFIX="$ROOT/tests/fixtures/deps"
REPO=o/r
DAY=2026-10-24
BRANCH="deps/freshness-$DAY"
CHECKOUT_SHA=3d3c42e5aac5ba805825da76410c181273ba90b1
# Test values only: the claude value matches a credential pattern, so a leak shows in every check.
T_GH=test-gh-token-value-0001
T_REL=test-release-token-value-0002
T_CC=sk-ant-oat01-test-claude-value-0003
DEFAULT_CRATES="aes-gcm:normal:0.10.3 hex:normal:0.4.3 hmac:normal:0.12.1 reqwest:normal:0.12.28 rusqlite:normal:0.37.0 serde:normal:1.0.229 sha2:normal:0.10.9 subtle:normal:2.6.1 tokio:normal:1.54.1 tokio:dev:1.54.1 tokio-tungstenite:dev:0.29.0"
failed=0
want() { [ $# -gt 0 ] || return 1; [ ${#ONLY[@]} -eq 0 ] && return 0; local n; for n in "${ONLY[@]}"; do [ "$n" = "$1" ] && return 0; done; return 1; }
ONLY=("$@")

key() { echo "$1" | tr '/.?&=:' '______'; }
# serve <endpoint> <json | @fixture file of tests/fixtures/deps>
serve() { case "$2" in @*) cp "$DFIX/${2#@}" "$D/api/$(key "$1")" ;; *) printf '%s\n' "$2" >"$D/api/$(key "$1")" ;; esac; }
fail_api() { printf '%s\n' "$2" >"$D/api/$(key "$1").http"; }
# crates "<name:table:version> ...": writes Cargo.toml and Cargo.lock of the scratch repository and commits them.
crates() {
  local spec table name ver
  { printf '[package]\nname = "agent-loupe"\nversion = "0.1.0"\nedition = "2024"\n'
    for table in normal build dev; do
      local hdr=dependencies; [ "$table" = dev ] && hdr=dev-dependencies; [ "$table" = build ] && hdr=build-dependencies
      local lines=""; for spec in $1; do [ "$(echo "$spec" | cut -d: -f2)" = "$table" ] || continue
        name="$(echo "$spec" | cut -d: -f1)"; ver="$(echo "$spec" | cut -d: -f3)"; lines="$lines$name = \"$ver\"\n"; done
      [ -n "$lines" ] && printf '\n[%s]\n%b' "$hdr" "$lines"
    done; } >"$G/Cargo.toml"
  { printf 'version = 4\n\n[[package]]\nname = "agent-loupe"\nversion = "0.1.0"\ndependencies = [\n'
    for spec in $1; do echo "$spec" | cut -d: -f1; done | sort -u | while read -r name; do
      if [ -n "${2:-}" ] && echo " $2 " | grep -q " $name:"; then printf ' "%s %s",\n' "$name" "$(echo "$1" | tr ' ' '\n' | grep "^$name:" | head -n 1 | cut -d: -f3)"
      else printf ' "%s",\n' "$name"; fi; done
    printf ']\n'
    for spec in $1; do echo "$(echo "$spec" | cut -d: -f1) $(echo "$spec" | cut -d: -f3)"; done | sort -u | while read -r name ver; do
      printf '\n[[package]]\nname = "%s"\nversion = "%s"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n' "$name" "$ver"; done
    for spec in ${2:-}; do printf '\n[[package]]\nname = "%s"\nversion = "%s"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n' "${spec%%:*}" "${spec#*:}"; done
  } >"$G/Cargo.lock"
  (cd "$G" && git add -A && git commit -qm crates && git push -q origin main && git fetch -q origin)
}
setup() {
  D="$(mktemp -d)"; mkdir -p "$D/bin" "$D/api" "$D/bodies" "$D/reg/crates" "$D/git"; : >"$D/calls"; : >"$D/registry.calls"
  echo "${DAY}T10:00:00Z" >"$D/now"; echo 0 >"$D/registry_interval_ms"
  cp "$FIX"/crates/*.json "$D/reg/crates/"
  cat >"$D/bin/curl" <<CURL
#!/bin/bash
# Fake curl: records the time, the User-Agent, and the URL; answers the registries and /healthz from files.
F="$D"
CURL
  cat >>"$D/bin/curl" <<'CURL'
ua=""; out=""; fmt=""; url=""
while [ $# -gt 0 ]; do
  case "$1" in -A|--user-agent) ua="$2"; shift 2 ;; -o) out="$2"; shift 2 ;; -w) fmt="$2"; shift 2 ;;
    -H|--max-time) shift 2 ;; -s|-S|-sS|-f|-sf) shift ;; *) url="$1"; shift ;; esac
done
echo "$(perl -MTime::HiRes=time -e 'printf "%d", time * 1000') ua=[$ua] $url" >>"$F/registry.calls"
code=200; body=""
case "$url" in
  https://crates.io/api/v1/crates/*) f="$F/reg/crates/${url##*/}.json" ;;
  http://127.0.0.1:*/healthz) f="$F/healthz"; [ -f "$f" ] || { body='{"status":"ok"}'; f=""; } ;;
  *) echo "curl: (6) no network in the tests: $url" >&2; exit 6 ;;
esac
if [ -n "$f" ]; then
  if [ -f "$f.http" ]; then code="$(cat "$f.http")"; body='{"errors":[{"detail":"unavailable"}]}'
  elif [ -f "$f" ]; then body="$(cat "$f")"; else code=404; body='{"errors":[{"detail":"Not Found"}]}'; fi
fi
if [ -n "$out" ]; then printf '%s\n' "$body" >"$out"; else printf '%s' "$body"; fi
[ -n "$fmt" ] && printf "$(printf '%s' "$fmt" | sed "s/%{http_code}/$code/g")"
exit 0
CURL
  cat >"$D/bin/gh" <<GH
#!/bin/bash
# Fake gh: GET answers from files named by the endpoint; other calls are recorded with their body.
F="$D"
GH
  cat >>"$D/bin/gh" <<'GH'
case "${1:-}" in
  auth) exit 1 ;;
  pr) echo "PR $*" >>"$F/calls"; exit 0 ;;
  api) shift ;;
  *) echo "OTHER $*" >>"$F/calls"; exit 0 ;;
esac
method=""; endpoint=""; fields=0; body=""; title=""
while [ $# -gt 0 ]; do
  case "$1" in
    --method|-X) method="$2"; shift 2 ;;
    -f|--raw-field) fields=1; case "$2" in body=*) body="${2#body=}" ;; title=*) title="TITLE ${2#title=}" ;; esac; shift 2 ;;
    -F|--field) fields=1; case "$2" in body=@*) body="$(cat "${2#body=@}")" ;; body=*) body="${2#body=}" ;; esac; shift 2 ;;
    --input) fields=1; shift 2 ;;
    -H|--header|--jq|-q) shift 2 ;;
    --paginate|--silent|-i) shift ;;
    *) [ -z "$endpoint" ] && endpoint="$1"; shift ;;
  esac
done
[ -n "$method" ] || { [ $fields -eq 1 ] && method=POST || method=GET; }
n="$(wc -l <"$F/calls" | tr -d ' ')"
echo "$method $endpoint" >>"$F/calls"
echo "$(perl -MTime::HiRes=time -e 'printf "%d", time * 1000') $method $endpoint" >>"$F/gh.times"
[ -n "$title$body" ] && printf '%s%s' "${title:+$title
}" "$body" >"$F/bodies/$n"
env >"$F/gh.env.$n"
if [ "$method" != GET ]; then
  case "$method $endpoint" in
    "PUT "*/merge) echo '{"sha": "3131313131313131313131313131313131313131", "merged": true}' ;;
    "POST "*/pulls) echo '{"number": 31, "html_url": "https://github.com/o/r/pull/31"}' ;;
    "POST "*/issues) echo '{"number": 41, "html_url": "https://github.com/o/r/issues/41"}' ;;
    *) echo '{}' ;;
  esac
  exit 0
fi
key() { echo "$1" | tr '/.?&=:' '______'; }
norm() { echo "$1" | sed -E 's/[0-9a-f]{40}/SHA/g'; }
full="$(key "$endpoint")"; path="$(key "${endpoint%%\?*}")"
for k in "$full" "$(norm "$full")" "$path" "$(norm "$path")"; do
  if [ -f "$F/api/$k.http" ]; then echo "gh: error (HTTP $(cat "$F/api/$k.http"))" >&2; exit 1; fi
  [ -f "$F/api/$k" ] && { cat "$F/api/$k"; exit 0; }
done
echo "gh: Not Found (HTTP 404)" >&2; exit 1
GH
  cat >"$D/bin/claude" <<CLAUDE
#!/bin/bash
# Fake claude -p: records, applies the edit of the test for the named dependency, prints the recorded result.
F="$D"
cat >"\$F/claude.prompt"
name="\$(sed -n 's/^- Name: //p' "\$F/claude.prompt" | head -n 1)"
echo "\$name" >>"\$F/claude.calls"
printf '%s\n' "\$@" >"\$F/claude.args"
env >"\$F/claude.env"
cp "\$F/claude.prompt" "\$F/claude.prompt.\$name" 2>/dev/null
[ -f "\$F/claude.sleep" ] && exec sleep "\$(cat "\$F/claude.sleep")"
if [ -f "\$F/claude.edit.\$name" ]; then bash "\$F/claude.edit.\$name"; elif [ -f "\$F/claude.edit" ]; then bash "\$F/claude.edit"; fi
if [ -f "\$F/claude.out.\$name" ]; then cat "\$F/claude.out.\$name"; else cat "\$F/claude.out"; fi
CLAUDE
  cat >"$D/bin/cargo" <<CARGO
#!/bin/bash
echo "cargo \$*" >>"$D/cargo.calls"
if [ "\$1" = build ]; then mkdir -p target/debug && cp "$D/loupe.stub" target/debug/loupe && chmod +x target/debug/loupe; fi
exit 0
CARGO
  cat >"$D/loupe.stub" <<'STUB'
#!/bin/bash
# Stand-in loupe: the version, a key, and a serve that writes listening and closes on SIGTERM.
case "$1" in
  --version) echo "loupe 0.1.0" ;;
  gen-key) echo "dGVzdC1rZXktdmFsdWUtMDAwMDAwMDAwMDAwMDAwMDA=" ;;
  serve) trap 'echo "INFO database closed, exit" >&2; exit 0' TERM; echo "INFO listening listen=127.0.0.1:45999" >&2; while :; do sleep 0.1; done ;;
esac
STUB
  chmod +x "$D/bin/"*
  G="$D/git"; mkdir -p "$G/.agents/scripts" "$G/.github/workflows" "$G/vendor/litellm" "$G/src" "$G/tests"
  cp "$ROOT/.agents/scripts/deps.freshness.sh" "$ROOT/.agents/scripts/deps.common.sh" "$ROOT/.agents/scripts/deps.freshness.update.prompt.md" \
    "$ROOT/.agents/scripts/deps.freshness.assess.prompt.md" "$G/.agents/scripts/"
  cat >"$G/.agents/scripts/pipeline.verdict.sh" <<VERDICT
#!/bin/bash
# Stand-in verdict: records its environment, prints the log of the test, writes the stage log, exits with the next code.
n=\$(( \$(cat "$D/verdict.calls" 2>/dev/null | wc -l) + 1 )); echo call >>"$D/verdict.calls"
env >"$D/verdict.env.\$n"
[ -f "$D/verdict.log" ] && cat "$D/verdict.log"
[ -f "$D/stage.log" ] && mkdir -p .agents/logs && cp "$D/stage.log" .agents/logs/pipeline.test.log
rc=\$(sed -n "\${n}p" "$D/verdict_rcs" 2>/dev/null); exit "\${rc:-0}"
VERDICT
  chmod +x "$G/.agents/scripts/pipeline.verdict.sh"
  printf '.agents/logs/\n/target/\n' >"$G/.gitignore"
  echo 'name: verdict' >"$G/.github/workflows/verdict.yml"
  printf 'name: ci\njobs:\n  a:\n    steps:\n      - uses: actions/checkout@%s # v7.0.1\n  b:\n    uses: ./.github/workflows/verdict.yml\n' "$CHECKOUT_SHA" >"$G/.github/workflows/ci.yml"
  echo '{"claude-sonnet-4-6": {"litellm_provider": "anthropic", "input_cost_per_token": 3e-06, "output_cost_per_token": 1.5e-05, "cache_read_input_token_cost": 3e-07, "cache_creation_input_token_cost": 3.75e-06}}' \
    >"$G/vendor/litellm/model_prices_and_context_window.json"
  printf 'use hex::encode;\npub fn f(b: &[u8]) -> String { encode(b) }\npub fn g(b: &[u8]) -> String { hex::encode(b) }\n#[test]\nfn t1() {}\n#[test]\nfn t2() {}\n' >"$G/src/lib.rs"
  printf '#[tokio::test]\nasync fn t3() {}\n' >"$G/tests/a.rs"
  (cd "$G" && git init -q -b main && git config user.email t@example.com && git config user.name t && git add -A && git commit -qm init \
    && git init -q --bare "$D/origin.git" && git remote add origin "$D/origin.git" && git push -q origin main && git fetch -q origin) || exit 1
  crates "$DEFAULT_CRATES"
  serve "repos/$REPO" '{"private": false, "visibility": "public"}'
  serve "repos/$REPO/actions/workflows/verdict.yml" @workflow.verdict.json
  serve "repos/$REPO/pulls" '[]'
  serve "repos/$REPO/issues" '[]'
  serve "repos/$REPO/labels/dependencies" '{"name": "dependencies"}'
  serve "repos/$REPO/labels/zero-dep" '{"name": "zero-dep"}'
  serve "repos/$REPO/commits/SHA/check-runs" @checks.success.json
  serve "repos/$REPO/compare/SHA...SHA" @compare.current.json
  serve "repos/$REPO/actions/workflows/release-please.yml/runs" @runs.release_please.json
  serve "repos/$REPO/pulls?state=open&head=o:$BRANCH" '[]'
  cp "$FIX/github/checkout.releases.json" "$D/api/$(key "repos/actions/checkout/releases")"
  cp "$FIX/github/checkout.ref.v7.0.1.json" "$D/api/$(key "repos/actions/checkout/git/ref/tags/v7.0.1")"
  cp "$FIX/claude.update.json" "$D/claude.out"
}
# run <action> [flags]: the script in the scratch tree with the test tokens; stdout in $out, stderr in $err, code in $rc.
run() {
  out="$(cd "$D/git" && PATH="$D/bin:$PATH" GH_TOKEN="${GHT-$T_GH}" RELEASE_TOKEN="${RELT-$T_REL}" CLAUDE_CODE_OAUTH_TOKEN="${CCT-$T_CC}" \
    GITHUB_STEP_SUMMARY="$D/step_summary" /bin/bash .agents/scripts/deps.freshness.sh "$@" --repo "$REPO" --fixtures "$D" \
    --run-url https://github.com/o/r/actions/runs/1 2>"$D/stderr")"; rc=$?
  err="$(cat "$D/stderr")"
}
line() { echo "$out" | grep "^ecosystem: $1 name: $2 "; }
field() { echo "$1" | sed -nE "s/.* $2: ([^ ]+).*/\\1/p"; }
writes() { grep -vc '^GET ' "$D/calls"; }
claude_runs() { [ -f "$D/claude.calls" ] && wc -l <"$D/claude.calls" | tr -d ' ' || echo 0; }
remote_log() { git -C "$D/origin.git" log --format=%s "main..$BRANCH" 2>/dev/null; }
no_value() { ! cat "$@" 2>/dev/null | grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC"; }
edit_for() { printf '%s\n' "$2" >"$D/claude.edit.$1"; }
out_for() { jq -c --arg r "$2" '.result = $r' "$FIX/claude.update.json" >"$D/claude.out.$1"; }
bump() { echo "perl -pi -e 's/^$1 = \"[^\"]*\"/$1 = \"$2\"/' Cargo.toml && perl -0pi -e 's/name = \"$1\"\\nversion = \"[^\"]*\"/name = \"$1\"\\nversion = \"$3\"/' Cargo.lock"; }
result() {
  if [ "$2" -eq 0 ]; then echo "test: $1 status: passed"; else
    echo "test: $1 status: failed"; echo "  rc: ${rc:-}"; echo "${out:-}" | sed 's/^/  out: /' | head -n 60
    echo "${err:-}" | sed 's/^/  err: /' | head -n 10; echo "  calls: $(tr '\n' ';' <"$D/calls" 2>/dev/null)"; failed=1; fi
  rm -rf "$D"
}

# Each class from the stated rule: fresh, stale, aged, and aged with behind yes; the summary counts.
if want scan_classes; then
setup; run scan
[ $rc -eq 0 ] && [ "$(field "$(line cargo serde)" class)" = fresh ] && [ "$(field "$(line cargo rusqlite)" class)" = stale ] \
  && [ "$(field "$(line cargo hex)" class)" = aged ] && [ "$(field "$(line cargo hex)" behind)" = no ] \
  && [ "$(field "$(line cargo hmac)" class)" = aged ] && [ "$(field "$(line cargo hmac)" behind)" = yes ] \
  && line cargo aes-gcm | grep -q 'pinned: 0.10.3 pinned_date: 2023-09-21 pinned_age_days: 1128 latest: 0.11.1 latest_date: 2026-08-21 latest_age_days: 63 behind: yes class: stale yanked: no$' \
  && line cargo tokio-tungstenite | grep -q 'kind: dev .* class: stale' \
  && echo "$out" | grep -qx 'summary: fresh: 3 stale: 5 aged: 3 unknown: 0 aged_behind: 1 total: 11 scan_secs: [0-9]*' \
  && [ "$(writes)" -eq 0 ] && [ "$(claude_runs)" -eq 0 ] && [ -z "$(git -C "$D/git" status --porcelain)" ]
result scan_classes $?
fi

# The latest is the highest release, not the first entry: a pre-release first and an older release before it.
if want latest_is_highest_release; then
setup; run scan
l="$(line cargo rusqlite)"
[ $rc -eq 0 ] && [ "$(jq -r '.versions[0].num' "$FIX/crates/rusqlite.json")" = 0.41.0-beta.1 ] && [ "$(field "$l" latest)" = 0.40.2 ] \
  && [ "$(field "$l" latest_date)" = 2026-08-08 ] && [ "$(field "$l" latest_age_days)" = 76 ] && [ "$(field "$l" pinned_age_days)" = 475 ]
result latest_is_highest_release $?
fi

# A yanked version is not the latest: the next version is.
if want yanked_latest_skipped; then
setup; run scan
l="$(line cargo reqwest)"
[ $rc -eq 0 ] && [ "$(jq -r '.versions[0] | "\(.num) \(.yanked)"' "$FIX/crates/reqwest.json")" = "0.13.6 true" ] && [ "$(field "$l" latest)" = 0.13.5 ] \
  && [ "$(field "$l" class)" = stale ] && [ "$(field "$l" yanked)" = no ]
result yanked_latest_skipped $?
fi

# A yanked pinned version, 10 days old, is stale with yanked yes; a pre-release above it is not the latest.
if want yanked_pinned_is_stale; then
setup; crates "serde:normal:1.0.229 smallvec:normal:1.16.3"; run scan
l="$(line cargo smallvec)"
[ $rc -eq 0 ] && [ "$(field "$l" class)" = stale ] && [ "$(field "$l" yanked)" = yes ] && [ "$(field "$l" pinned_age_days)" = 10 ] && [ "$(field "$l" latest)" = 1.16.4 ]
result yanked_pinned_is_stale $?
fi

# A crate in two tables is one dependency; with two versions in the lock file, the root version is pinned.
if want crate_in_two_tables_counted_once; then
setup; crates "tokio:normal:1.54.1 tokio:dev:1.54.1 smallvec:normal:1.16.4" "smallvec:1.16.2"; run scan
[ $rc -eq 0 ] && [ "$(echo "$out" | grep -c '^ecosystem: cargo name: tokio ')" -eq 1 ] && [ "$(field "$(line cargo tokio)" kind)" = normal ] \
  && [ "$(field "$(line cargo smallvec)" pinned)" = 1.16.4 ] && [ "$(grep -c 'crates/tokio$' "$D/registry.calls")" -eq 1 ]
result crate_in_two_tables_counted_once $?
fi

# An action pin resolves to its tag and date; a draft, a pre-release, and a tag v9 are not the latest; a local
# workflow is not a dependency.
if want action_pin_resolved; then
setup; run scan; l="$(line github-actions actions/checkout)"
[ $rc -eq 0 ] && echo "$l" | grep -q 'kind: action pinned: v7.0.1 pinned_date: 2026-07-20 pinned_age_days: 95 latest: v7.0.1 .* class: fresh yanked: no pin_check: ok date_source: release$' \
  && [ "$(echo "$out" | grep -c '^ecosystem: github-actions ')" -eq 1 ] && ! echo "$out" | grep -q 'verdict.yml'
result action_pin_resolved $?
fi

# A tag that resolves to another SHA: pin_check mismatch and class stale.
if want action_pin_mismatch; then
setup; jq '.object.sha = "1111111111111111111111111111111111111111"' "$FIX/github/checkout.ref.v7.0.1.json" >"$D/api/$(key "repos/actions/checkout/git/ref/tags/v7.0.1")"
run scan; l="$(line github-actions actions/checkout)"
[ $rc -eq 0 ] && [ "$(field "$l" pin_check)" = mismatch ] && [ "$(field "$l" class)" = stale ]
result action_pin_mismatch $?
fi

# crates.io gets a User-Agent with the project; no registry gets two requests in one second.
if want user_agent_and_rate; then
setup; rm -f "$D/registry_interval_ms"; crates "hex:normal:0.4.3 serde:normal:1.0.229 subtle:normal:2.6.1"; run scan
ua_ok=1; grep 'crates.io' "$D/registry.calls" | grep -vqF 'ua=[agentloupe-deps-freshness (https://github.com/desek/agentloupe)]' && ua_ok=0
gaps="$(grep 'crates.io' "$D/registry.calls" | awk 'NR > 1 {print $1 - p} {p = $1}' | sort -n | head -n 1)"
ghgap="$(grep ' GET repos/actions/' "$D/gh.times" | awk 'NR > 1 {print $1 - p} {p = $1}' | sort -n | head -n 1)"
[ $rc -eq 0 ] && [ $ua_ok -eq 1 ] && [ "$(grep -c 'crates.io' "$D/registry.calls")" -eq 3 ] && [ -n "$gaps" ] && [ "$gaps" -ge 1000 ] && [ -n "$ghgap" ] && [ "$ghgap" -ge 1000 ]
result user_agent_and_rate $?
fi

# A registry that answers 503: at most 3 retries, class unknown, exit 6, no claude -p run.
if want registry_error_exit_6; then
setup; echo 503 >"$D/reg/crates/hex.json.http"; run update
[ $rc -eq 6 ] && [ "$(grep -c 'crates/hex$' "$D/registry.calls")" -eq 4 ] && [ "$(field "$(line cargo hex)" class)" = unknown ] \
  && echo "$err" | grep -q 'the lookup of cargo hex failed: crates.io: HTTP 503 after 3 retries' && ! grep -qx hex "$D/claude.calls" \
  && echo "$out" | grep -q '^item: cargo:hex path: update package: hex ecosystem: cargo status: unknown reason: unknown'
result registry_error_exit_6 $?
fi

# --json prints one JSON document with the fields of each line, and --report writes the same document.
if want json_output; then
setup; run scan --json --report "$D/report.json"
[ $rc -eq 0 ] && echo "$out" | jq -e '.dependencies | length == 11' >/dev/null \
  && echo "$out" | jq -e '.dependencies[] | select(.name == "hmac") | [.ecosystem, .name, .kind, .pinned, .pinned_date, .pinned_age_days, .latest, .latest_date, .latest_age_days, .behind, .class, .yanked] == ["cargo","hmac","normal","0.12.1","2022-02-17",1709,"0.13.0","2026-03-29",208,true,"aged",false]' >/dev/null \
  && echo "$out" | jq -e '.summary.aged == 3 and .summary.total == 11' >/dev/null && jq -e '.action == "scan"' "$D/report.json" >/dev/null
result json_output $?
fi

# The dry run plans and writes nothing: no claude, no cargo, no token, no write call, a clean tree.
if want dry_run_starts_no_claude; then
setup; rm "$D/bin/claude" "$D/bin/cargo"; head0="$(git -C "$D/git" rev-parse HEAD)"
CCT="" RELT="" run update --dry-run; o1="$out"; r1=$rc
CCT="" RELT="" run assess --dry-run
[ $r1 -eq 0 ] && [ $rc -eq 0 ] && echo "$o1" | grep -qx "action: branch target: $BRANCH status: planned detail: from the head of main" \
  && echo "$o1" | grep -q '^action: claude target: cargo:hmac status: planned detail: ecosystem=cargo name=hmac kind=normal pinned_version=0.12.1 latest_version=0.13.0 manifest_path=Cargo.toml model=sonnet max_turns=30 budget_usd=5 timeout_min=40$' \
  && echo "$o1" | grep -qx 'action: commit target: cargo:hmac status: planned detail: fix(deps): bump hmac to 0.13.0' \
  && echo "$o1" | grep -q "^action: merge target: $BRANCH status: planned" && echo "$o1" | grep -q '^action: issue target: breaking-updates status: planned' \
  && echo "$out" | grep -q '^item: cargo:subtle path: assess .* status: planned reason: cryptography .* recommendation: keep plan: no claude -p run$' \
  && echo "$out" | grep -q '^action: claude target: cargo:hex status: planned detail: fields=scan_line,counts .* tools=Read,Grep,Glob .* timeout_min=15$' \
  && echo "$out" | grep -q '^action: issue target: cargo:hex status: planned detail: open "Zero-dep assessment: cargo hex" with the labels dependencies and zero-dep$' \
  && [ "$(writes)" -eq 0 ] && [ ! -e "$D/claude.calls" ] && [ "$(git -C "$D/git" rev-parse HEAD)" = "$head0" ] && [ -z "$(git -C "$D/git" status --porcelain)" ] \
  && [ "$(git -C "$D/origin.git" branch | wc -l | tr -d ' ')" -eq 1 ]
result dry_run_starts_no_claude $?
fi

# The order of the runs (normal and build, then dev; the oldest pinned first) and the run cap of 4.
if want update_order_and_cap; then
setup
run update --dry-run
order="$(echo "$out" | sed -nE 's/^item: ([^ ]+) path: update .* status: (planned|waiting).*/\1/p' | tr '\n' ' ')"
[ $rc -eq 0 ] && [ "$order" = "cargo:hmac cargo:aes-gcm cargo:sha2 cargo:rusqlite cargo:reqwest cargo:tokio-tungstenite " ] \
  && [ "$(echo "$out" | grep -c 'status: planned pinned')" -eq 4 ] && [ "$(echo "$out" | grep -c 'status: waiting reason: run_cap')" -eq 2 ] \
  && echo "$out" | grep -q '^item: cargo:reqwest .* status: waiting reason: run_cap'
result update_order_and_cap $?
fi

# A green update: one commit by the script with the stated summary and body, the push, the pull request, the merge.
if want update_green_commits; then
setup; crates "rusqlite:normal:0.37.0 serde:normal:1.0.229"
edit_for rusqlite "$(bump rusqlite 0.40 0.40.2) && echo 'pub fn h() {}' >>src/lib.rs"; printf '1\n0\n' >"$D/verdict_rcs"
run update
msg="$(git -C "$D/origin.git" log -1 --format=%B "$BRANCH" 2>/dev/null)"
[ $rc -eq 0 ] && [ "$(remote_log)" = 'fix(deps): bump rusqlite to 0.40.2' ] && echo "$msg" | grep -qx 'pinned: 0.37.0, 475 days old' \
  && echo "$msg" | grep -qx 'latest: 0.40.2, 76 days old' && echo "$msg" | grep -qx 'code changed: yes' \
  && [ "$(git -C "$D/origin.git" log -1 --format=%an "$BRANCH")" = 'github-actions[bot]' ] && [ "$(claude_runs)" -eq 1 ] \
  && grep -qx 'Name: rusqlite' <(sed 's/^- //' "$D/claude.prompt") && grep -q 'Latest version: 0.40.2' "$D/claude.prompt" \
  && grep -qx -- '--max-turns' "$D/claude.args" && grep -qx 30 "$D/claude.args" && grep -qxF 'Bash(git push:*)' "$D/claude.args" \
  && grep -qx "CLAUDE_CODE_OAUTH_TOKEN=$T_CC" "$D/claude.env" && ! grep -qE '^(GH_TOKEN|GITHUB_TOKEN|RELEASE_TOKEN)=' "$D/claude.env" \
  && ! grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC" "$D/verdict.env."* && grep -q '^cargo build --locked' "$D/cargo.calls" \
  && grep -qx "POST repos/$REPO/pulls" "$D/calls" && grep -qx "PUT repos/$REPO/pulls/31/merge" "$D/calls" \
  && echo "$out" | grep -q '^item: cargo:rusqlite path: update .* status: updated .* to: 0.40.2 model: claude-sonnet-4-6 input_tokens: 30 .* cost_usd: 0.22509 total_cost_usd: 0.22509$'
result update_green_commits $?
fi

# A red verdict: the branch reset, the issue holds the stage test and the first error; exit 1.
if want update_red_records_stage; then
setup; crates "reqwest:normal:0.12.28 serde:normal:1.0.229"; cp "$FIX/claude.update.breaks.json" "$D/claude.out"
edit_for reqwest "$(bump reqwest 0.13 0.13.5)"; echo 1 >"$D/verdict_rcs"
printf 'stage: format status: passed seconds: 1\nstage: test status: failed seconds: 9\nverdict: failed seconds: 10\n' >"$D/verdict.log"
printf 'compiling\nerror[E0599]: no method named `json` found for struct `RequestBuilder`\n' >"$D/stage.log"
run update
c="$(grep -l 'Failing stage' "$D/bodies/"* 2>/dev/null | head -n 1)"
[ $rc -eq 1 ] && [ -z "$(remote_log)" ] && [ -z "$(git -C "$D/git" status --porcelain)" ] && grep -q 'reqwest = "0.12.28"' "$D/git/Cargo.toml" \
  && echo "$out" | grep -q '^item: cargo:reqwest path: update .* status: breaks .* check: verdict stage: test' \
  && grep -qx "POST repos/$REPO/issues" "$D/calls" && grep -q 'TITLE Dependency freshness: breaking updates' "$D/bodies/"* && [ -n "$c" ] \
  && grep -q 'Failing stage: `test`' "$c" && grep -q 'First error: `error\[E0599\]: no method named' "$c" && grep -q 'Pinned: 0.12.28; latest: 0.13.5' "$c" \
  && grep -q 'Failing check: `verdict`' "$c" && grep -q 'Structured result:' "$c" && grep -q 'Run: https://github.com/o/r/actions/runs/1' "$c" && grep -q 'Scan: `summary:' "$c" \
  && grep -l 'deps-freshness:held' "$D/bodies/"* | xargs grep -q "^cargo:reqwest latest:0.13.5 date:$DAY reason:$"
result update_red_records_stage $?
fi

# A passing verdict and a binary with no line listening: breaks with the failing check smoke.
if want smoke_failure_breaks; then
setup; crates "rusqlite:normal:0.37.0 serde:normal:1.0.229"; edit_for rusqlite "$(bump rusqlite 0.40 0.40.2)"; echo 1 >"$D/smoke_timeout_secs"
printf '#!/bin/bash\ncase "$1" in --version) echo loupe 0.1.0 ;; gen-key) echo a2V5 ;; serve) exec sleep 30 ;; esac\n' >"$D/loupe.stub"
run update
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: cargo:rusqlite path: update .* status: breaks .* check: smoke stage: smoke' && [ -z "$(remote_log)" ] \
  && grep -q 'loupe serve wrote no line listening' "$D/bodies/"*
result smoke_failure_breaks $?
fi

# A diff that removes one #[test] fails with tests_removed; no commit for the item.
if want tests_removed_fails; then
setup; crates "rusqlite:normal:0.37.0 serde:normal:1.0.229"
edit_for rusqlite "$(bump rusqlite 0.40 0.40.2) && perl -0pi -e 's/#\\[test\\]\\nfn t2\\(\\) \\{\\}\\n//' src/lib.rs"
run update
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: cargo:rusqlite path: update .* status: failed reason: tests_removed' && [ -z "$(remote_log)" ] \
  && grep -q 'fn t2' "$D/git/src/lib.rs" && [ ! -e "$D/verdict.calls" ]
result tests_removed_fails $?
fi

# A diff that removes a direct dependency from the manifest fails with dependency_removed.
if want dependency_removed_fails; then
setup; crates "sha2:normal:0.10.9 subtle:normal:2.6.1 serde:normal:1.0.229"
edit_for sha2 "$(bump sha2 0.11 0.11.1) && perl -ni -e 'print unless /^subtle = /' Cargo.toml"
run update
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: cargo:sha2 path: update .* status: failed reason: dependency_removed' && [ -z "$(remote_log)" ] \
  && grep -q '^subtle = ' "$D/git/Cargo.toml"
result dependency_removed_fails $?
fi

# A result with no structured result fails with invalid_result.
if want invalid_result_fails; then
setup; crates "rusqlite:normal:0.37.0 serde:normal:1.0.229"; cp "$FIX/claude.update.invalid.json" "$D/claude.out"; edit_for rusqlite "$(bump rusqlite 0.40 0.40.2)"
run update
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: cargo:rusqlite .* status: failed reason: invalid_result' && [ -z "$(remote_log)" ] && [ -z "$(git -C "$D/git" status --porcelain)" ]
result invalid_result_fails $?
fi

# An update of sha2 that also raises hmac: one run, hmac named in the commit body, no run for hmac.
if want also_raised_no_second_run; then
setup; crates "hmac:normal:0.12.1 sha2:normal:0.10.9 serde:normal:1.0.229"
# hmac comes first in the order, so its run raises sha2 too; sha2 then gets no run of its own.
edit_for hmac "$(bump hmac 0.13 0.13.0) && $(bump sha2 0.11 0.11.1)"
out_for hmac 'done {"status":"updated","name":"hmac","from_version":"0.12.1","to_version":"0.13.0","also_raised":[{"name":"sha2","version":"0.11.1"}],"files_changed":["Cargo.toml","Cargo.lock"],"code_changed":false,"verdict":"passed","failing_stage":null,"first_error":null,"reason":"digest 0.11"}'
run update
msg="$(git -C "$D/origin.git" log -1 --format=%B "$BRANCH" 2>/dev/null)"
[ $rc -eq 0 ] && [ "$(claude_runs)" -eq 1 ] && [ "$(cat "$D/claude.calls")" = hmac ] && echo "$msg" | grep -qx 'also raised: sha2 0.11.1' \
  && ! echo "$msg" | grep -q 'code changed' && echo "$out" | grep -q '^item: cargo:sha2 path: update .* status: raised reason: also_raised'
result also_raised_no_second_run $?
fi

# A stale action: no claude -p run; with no Dependabot pull request it goes into the issue as actions_no_pr; with
# one open, it carries dependabot_path.
if want stale_action_no_claude; then
setup; crates "serde:normal:1.0.229"; jq '.object.sha = "1111111111111111111111111111111111111111"' "$FIX/github/checkout.ref.v7.0.1.json" >"$D/api/$(key "repos/actions/checkout/git/ref/tags/v7.0.1")"
run update; o1="$out"; r1=$rc; c1="$(cat "$D/bodies/"* 2>/dev/null)"
serve "repos/$REPO/pulls" '[{"number": 22, "title": "ci(deps): bump actions/checkout from 7.0.1 to 7.0.2", "body": "Bumps actions/checkout", "user": {"login": "dependabot[bot]"}, "head": {"ref": "dependabot/github_actions/actions/checkout-7.0.2", "sha": "2222222222222222222222222222222222222222"}}]'
run update
[ $r1 -eq 1 ] && [ ! -e "$D/claude.calls" ] && echo "$o1" | grep -q '^item: github-actions:actions/checkout path: update .* status: failed reason: actions_no_pr' \
  && echo "$c1" | grep -q 'actions_no_pr' && [ $rc -eq 0 ] && echo "$out" | grep -q '^item: github-actions:actions/checkout .* status: waiting reason: dependabot_path .* pull_request: 22'
result stale_action_no_claude $?
fi

# A breaking item of 3 days ago with the same latest is held, with no claude -p run; --retry-failed runs it;
# a later run with no breaking item closes the open issue.
if want held_breaking_item; then
setup; crates "reqwest:normal:0.12.28 serde:normal:1.0.229"
serve "repos/$REPO/issues" '[{"number": 41, "title": "Dependency freshness: breaking updates", "html_url": "https://github.com/o/r/issues/41", "body": "x\n<!-- deps-freshness:held\ncargo:reqwest latest:0.13.5 date:2026-10-21 reason:\n-->"}]'
run update; r1=$rc; o1="$out"; c1="$(claude_runs)"; w1="$(writes)"
edit_for reqwest "$(bump reqwest 0.13 0.13.5)"; run update --retry-failed; c2="$(claude_runs)"
serve "repos/$REPO/issues" '[{"number": 41, "title": "Dependency freshness: breaking updates", "html_url": "https://github.com/o/r/issues/41", "body": "x\n<!-- deps-freshness:held\n-->"}]'
crates "serde:normal:1.0.229"; : >"$D/calls"; run update
[ $r1 -eq 1 ] && echo "$o1" | grep -q '^item: cargo:reqwest .* status: held .* since: 2026-10-21$' && [ "$c1" -eq 0 ] && [ "$w1" -eq 0 ] && [ "$c2" -eq 1 ] \
  && [ $rc -eq 0 ] && grep -qx "PATCH repos/$REPO/issues/41" "$D/calls" && echo "$out" | grep -qx 'action: issue target: #41 status: done detail: closed'
result held_breaking_item $?
fi

# The cryptography rule: subtle by name and an aged RustCrypto crate by its repository get keep with no run.
if want crypto_is_keep_without_claude; then
setup; crates "subtle:normal:2.6.1 dbl:normal:0.5.0 hmac:normal:0.12.1 serde:normal:1.0.229"
run assess
[ $rc -eq 0 ] && [ ! -e "$D/claude.calls" ] && [ "$(echo "$out" | grep -c 'status: assessed reason: cryptography .* recommendation: keep$')" -eq 3 ] \
  && [ "$(grep -l 'cryptography: the project does not write its own cryptography' "$D/bodies/"* | wc -l | tr -d ' ')" -eq 3 ] \
  && grep -q 'TITLE Zero-dep assessment: cargo dbl' "$D/bodies/"*
result crypto_is_keep_without_claude $?
fi

# The assessment issue: the title, both labels, the counts, the recommendation, and the statement; the label made.
if want assessment_issue_form; then
setup; crates "hex:normal:0.4.3 serde:normal:1.0.229"; rm "$D/api/$(key "repos/$REPO/labels/zero-dep")"; cp "$FIX/claude.assess.json" "$D/claude.out"
run assess
n="$(grep -nx "POST repos/$REPO/issues" "$D/calls" | cut -d: -f1)"; b="$D/bodies/$((n - 1))"
[ $rc -eq 0 ] && [ "$(claude_runs)" -eq 1 ] && grep -qx "POST repos/$REPO/labels" "$D/calls" && [ -n "$n" ] \
  && grep -q 'TITLE Zero-dep assessment: cargo hex' "$b" \
  && grep -q 'Counts of the fixed search: 1 import lines, 1 symbols, 2 call sites, 1 files' "$D/bodies/"* \
  && grep -q 'Recommendation: \*\*replace\*\*' "$D/bodies/"* && grep -q 'Estimate of own code: 40 lines' "$D/bodies/"* \
  && grep -q 'A replacement is a separate change request for this dependency. The owner approves it before any code changes.' "$D/bodies/"* \
  && grep -q "deps-freshness:assessment latest:0.4.3 counts:1/1/2/1 class:aged date:$DAY recommendation:replace" "$D/bodies/"* \
  && grep -qx Read "$D/claude.args" && grep -qx Glob "$D/claude.args" && ! grep -qx 'Bash(cargo:\*)' "$D/claude.args" && grep -q 'Ecosystem: cargo' "$D/claude.prompt"
result assessment_issue_form $?
fi

# A replace with an estimate of 800 lines becomes watch with estimate_too_large.
if want large_replace_becomes_watch; then
setup; crates "hex:normal:0.4.3 serde:normal:1.0.229"; cp "$FIX/claude.assess.large.json" "$D/claude.out"
run assess
[ $rc -eq 0 ] && echo "$out" | grep -q '^item: cargo:hex path: assess .* status: assessed reason: estimate_too_large .* recommendation: watch' \
  && grep -q 'Recommendation: \*\*watch\*\*' "$D/bodies/"* && grep -q 'estimate_too_large' "$D/bodies/"*
result large_replace_becomes_watch $?
fi

# An assessment run that writes a file fails with tree_changed, and the file is removed.
if want assessment_tree_changed_fails; then
setup; crates "hex:normal:0.4.3 serde:normal:1.0.229"; cp "$FIX/claude.assess.json" "$D/claude.out"; echo 'echo x >notes.txt' >"$D/claude.edit"
run assess
[ $rc -eq 1 ] && echo "$out" | grep -q '^item: cargo:hex path: assess .* status: failed reason: tree_changed' && [ ! -e "$D/git/notes.txt" ] \
  && grep -q 'tree_changed' "$D/bodies/"*
result assessment_tree_changed_fails $?
fi

# The life of an assessment issue: unchanged and open, no run and no write; closed by the owner with the same
# latest, no write; no longer aged, comment and close; 91 days old, a new run; changed counts, a new run.
if want assessment_issue_life; then
setup; crates "hex:normal:0.4.3 tokio:normal:1.54.1 serde:normal:1.0.229"; cp "$FIX/claude.assess.json" "$D/claude.out"
iss() { printf '{"number": %s, "title": "Zero-dep assessment: cargo %s", "state": "%s", "body": "x\\n<!-- deps-freshness:assessment latest:%s counts:%s class:aged date:%s recommendation:keep -->"}' "$@"; }
serve "repos/$REPO/issues" "[$(iss 51 hex open 0.4.3 1/1/2/1 2026-10-14), $(iss 52 tokio open 1.54.0 1/1/1/1 2026-10-14)]"
run assess; r1=$rc; o1="$out"; c1="$(claude_runs)"; w1="$(grep -v '^GET ' "$D/calls" | tr '\n' ';')"
serve "repos/$REPO/issues" "[$(iss 51 hex closed 0.4.3 9/9/9/9 2026-10-01)]"; : >"$D/calls"; run assess; w2="$(writes)"; c2="$(claude_runs)"
serve "repos/$REPO/issues" "[$(iss 51 hex open 0.4.3 1/1/2/1 2026-07-25)]"; : >"$D/calls"; run assess; c3="$(claude_runs)"; w3="$(grep -c "POST repos/$REPO/issues/51/comments" "$D/calls")"
serve "repos/$REPO/issues" "[$(iss 51 hex open 0.4.3 1/1/5/1 2026-10-20)]"; : >"$D/calls"; run assess; c4="$(claude_runs)"; w4="$(grep -c "POST repos/$REPO/issues/51/comments" "$D/calls")"
[ $r1 -eq 0 ] && [ "$c1" -eq 0 ] && echo "$o1" | grep -q '^item: cargo:hex path: assess .* status: unchanged' && [ "$w1" = "POST repos/$REPO/issues/52/comments;PATCH repos/$REPO/issues/52;" ] \
  && echo "$o1" | grep -q '^item: cargo:tokio path: assess .* status: closed' \
  && [ "$w2" -eq 0 ] && [ "$c2" -eq 0 ] && [ "$c3" -eq 1 ] && [ "$w3" -eq 1 ] && [ "$c4" -eq 2 ] && [ "$w4" -eq 1 ]
result assessment_issue_life $?
fi

# No credential in the output, the report, the issue text, the commit, or the step summary.
if want redaction_and_no_token; then
setup; crates "reqwest:normal:0.12.28 serde:normal:1.0.229"; cp "$FIX/claude.update.breaks.json" "$D/claude.out"
edit_for reqwest "$(bump reqwest 0.13 0.13.5)"; echo 1 >"$D/verdict_rcs"
printf 'stage: test status: failed seconds: 9\nverdict: failed seconds: 9\n' >"$D/verdict.log"
printf 'error: token ghp_testvalue0000 and %s and TEST-RUN-TOKEN-VALUE-0004 rejected\n' "$T_CC" >"$D/stage.log"
GHT=TEST-RUN-TOKEN-VALUE-0004 run update --report "$D/report.json"
[ $rc -eq 1 ] && grep -q 'First error: `error: token \[redacted\] and \[redacted\] and \[redacted\] rejected`' "$D/bodies/"* \
  && ! cat "$D/bodies/"* "$D/report.json" "$D/step_summary" | grep -qE 'ghp_t|sk-ant-oat|TEST-RUN-TOKEN-VALUE' \
  && ! printf '%s\n%s\n' "$out" "$err" | grep -qE 'ghp_t|sk-ant-oat|TEST-RUN-TOKEN-VALUE|test-release-token' \
  && ! git -C "$D/origin.git" log -p --all | grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC" && ! grep -rqF -e "$T_REL" "$D/git/.git/config" \
  && grep -q 'Dependency freshness: update' "$D/step_summary"
result redaction_and_no_token $?
fi

# The interface: --help with the actions and the exit codes 0 to 6; an unknown flag, no action, and a missing
# credential each give an actionable error and their exit code, and never a value.
if want interface_help_and_usage; then
setup
h="$(/bin/bash "$D/git/.agents/scripts/deps.freshness.sh" --help)"; run scan --bogus; r1=$rc; e1="$err"
out="$(cd "$D/git" && /bin/bash .agents/scripts/deps.freshness.sh 2>&1)"; r2=$?
CCT="" run update; r3=$rc; e3="$err"; RELT="" run update; r4=$rc; e4="$err"
echo "$h" | grep -q '^  scan ' && echo "$h" | grep -q '^  update ' && echo "$h" | grep -q '^  assess ' && echo "$h" | grep -q '^Exit codes: 0 ' \
  && echo "$h" | grep -q '6 a registry lookup failed' && echo "$h" | grep -q -- '--fixtures DIR' && [ $r1 -eq 2 ] && echo "$e1" | grep -q 'unknown argument --bogus. Fix:' \
  && [ $r2 -eq 2 ] && [ $r3 -eq 3 ] && echo "$e3" | grep -q 'CLAUDE_CODE_OAUTH_TOKEN is empty' && [ $r4 -eq 3 ] && echo "$e4" | grep -q 'RELEASE_TOKEN is empty' \
  && ! printf '%s\n%s\n' "$e3" "$e4" | grep -qF -e "$T_GH" -e "$T_REL" -e "$T_CC" && [ ! -e "$D/claude.calls" ]
result interface_help_and_usage $?
fi

# The thresholds 90 and 180 are named constants with their reasons in the script, and no other script, prompt
# template, or workflow file states them; the freshness flow keeps the run limits and the env allowlist of the autofix.
if want thresholds_and_limits; then
D="$(mktemp -d)"; : >"$D/calls"; out=""; err=""; rc=0; S="$ROOT/.agents/scripts/deps.freshness.sh"; A="$ROOT/.agents/scripts/deps.autofix.sh"
v() { sed -n "s/^$1=\\(.*\\)$/\\1/p" "$2" | head -n 1; }
others="$(ls "$ROOT"/.agents/scripts/deps.*.sh "$ROOT"/.agents/scripts/deps.*.prompt.md "$ROOT"/.github/workflows/*.yml | grep -v 'deps.freshness.sh$')"
grep -B1 -x 'FRESH_DAYS=90' "$S" | grep -q '^# D1' && grep -B2 -x 'AGED_DAYS=180' "$S" | grep -q '^# D2' \
  && ! cat $others | grep -v 'timeout-minutes:' | grep -qE '(^|[^0-9A-Za-z.])(90|180)([^0-9A-Za-z.]|$)' \
  && [ "$(v MODEL "$S")" = "$(v MODEL "$A")" ] && [ "$(v MAX_TURNS "$S")" = "$(v MAX_TURNS "$A")" ] && [ "$(v RUN_BUDGET_USD "$S")" = "$(v RUN_BUDGET_USD "$A")" ] \
  && [ "$(v UPDATE_TIMEOUT_SECS "$S")" = "$(v CLAUDE_TIMEOUT_SECS "$A")" ] && [ "$(v BASH_TIMEOUT_MS "$S")" = "$(v BASH_TIMEOUT_MS "$A")" ] \
  && [ "$(v ENV_ALLOW "$S")" = "$(v ENV_ALLOW "$A")" ] && [ "$(v ALWAYS_PATHS "$S")" = "$(v ALWAYS_PATHS "$A")" ] && [ "$(v MERGE_MODE "$S")" = "$(v MERGE_MODE "$A")" ] \
  && [ "$(v WAIT_CAP_SECS "$S")" = "$(v WAIT_CAP_SECS "$A")" ] && [ "$(v UPDATE_JOB_BUDGET_USD "$S")" = "$(v WORKFLOW_BUDGET_USD "$A")" ] \
  && [ "$(grep -A6 'if \[\[ "$kind" == update \]\]; then' "$S" | grep -o -- '--allowedTools.*' | head -n 1)" = "$(grep -o -- '--allowedTools.*' "$A" | head -n 1)" ] \
  && [ "$(grep -A9 'if \[\[ "$kind" == update \]\]; then' "$S" | grep -o -- '--disallowedTools.*' | head -n 1)" = "$(grep -o -- '--disallowedTools.*' "$A" | head -n 1)" ]
result thresholds_and_limits $?
fi

# The weekly workflow is a thin caller: Tuesday 07:43 UTC and dispatch with dry_run, the concurrency group of the
# autofix with no cancel, the jobs update and assess with timeouts 330 and 180, their permissions, one call of the
# script per job, claude.ensure.sh, a checkout with no persisted credential, SHA-pinned actions, one upload of the
# report; the tokens only in the step of the script; no model, branch, label, threshold, or tool version.
if want workflow_is_thin_caller; then
D="$(mktemp -d)"; : >"$D/calls"; out=""; err=""; rc=0
W="$ROOT/.github/workflows/deps-freshness.yml"
job() { awk -v j="  $1:" '$0 == j {f = 1; print; next} f && /^  [a-z]/ {f = 0} f' "$W"; }
perms() { job "$1" | awk '/^    permissions:/ {f = 1; next} f && /^    [a-z]/ {f = 0} f && NF {gsub(/ /, ""); print}' | sort | tr '\n' ' '; }
step() { job "$1" | awk '/^      - / {s = ""} {s = s $0 "\n"} /run: \.agents\/scripts\/deps\.freshness\.sh/ {printf "%s", s}'; }
[ -f "$W" ] && grep -qx "    - cron: '43 7 \* \* 2'" "$W" && grep -qx '  workflow_dispatch:' "$W" && grep -qx '      dry_run:' "$W" && ! grep -qE '^  (push|pull_request)' "$W" \
  && grep -qx '  group: deps-autofix' "$W" && grep -qx '  cancel-in-progress: false' "$W" && grep -qx 'permissions: {}' "$W" \
  && [ "$(perms update)" = "actions:read checks:read contents:write issues:write pull-requests:write security-events:read " ] \
  && [ "$(perms assess)" = "contents:read issues:write " ] \
  && job update | grep -qx '    timeout-minutes: 330' && job assess | grep -qx '    timeout-minutes: 180' \
  && [ "$(job update | grep -c 'run: .agents/scripts/deps.freshness.sh update')" -eq 1 ] && [ "$(job assess | grep -c 'run: .agents/scripts/deps.freshness.sh assess')" -eq 1 ] \
  && [ "$(grep -c 'run: .agents/scripts/claude.ensure.sh' "$W")" -eq 2 ] && [ "$(grep -c 'persist-credentials: false' "$W")" -eq 2 ] \
  && [ "$(grep -c 'run: .agents/scripts/runner.space.sh' "$W")" -eq 1 ] \
  && [ "$(grep -c 'uses: actions/upload-artifact@' "$W")" -eq 2 ] && [ "$(grep -c 'retention-days: 30' "$W")" -eq 2 ] \
  && [ "$(grep -c 'secrets.CLAUDE_CODE_OAUTH_TOKEN' "$W")" -eq 2 ] && [ "$(grep -c 'secrets.RELEASE_PLEASE_TOKEN' "$W")" -eq 1 ] \
  && step update | grep -q 'secrets.CLAUDE_CODE_OAUTH_TOKEN' && step update | grep -q 'secrets.RELEASE_PLEASE_TOKEN' \
  && step assess | grep -q 'secrets.CLAUDE_CODE_OAUTH_TOKEN' && ! step assess | grep -q 'RELEASE_PLEASE_TOKEN' \
  && [ "$(grep -E '^ *(- )?uses:' "$W" | grep -vcE 'uses: [^@ ]+@[0-9a-f]{40} # v[0-9]')" -eq 0 ] \
  && ! sed -E 's/(^| )#.*$//' "$W" | grep -qiE 'sonnet|haiku|opus|deps/freshness|zero-dep|dependencies|--max|(^|[^0-9])(90|180) |[0-9]+\.[0-9]+\.[0-9]+' \
  && ! grep -qE 'ghp_|github_pat_|gho_|ghs_|sk-ant-' "$W"
result workflow_is_thin_caller $?
fi

exit $failed
