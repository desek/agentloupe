#!/bin/bash
# @agents-index Shell tests of advisory.respond.sh with a fake gh, cargo deny, and docker: dry run, marker, kinds, latest forward only, no delete, database failure, notes kept.
#
# Usage: advisory.respond.test.sh. Each test makes a scratch git repository with one tag per release (the
# Cargo.toml version equals the tag), copies advisory.respond.sh and release.image.sh into it, and puts a fake
# gh, a fake cargo (only cargo deny), and a fake docker first on PATH. The cargo deny answers are recorded from
# a run of cargo deny 0.20.2 against a scratch copy of the advisory database with one seeded vulnerability
# (RUSTSEC-2099-0001 for hex 0.4.3) and one seeded unmaintained notice, in tests/fixtures/advisory.respond/.
# The fakes record each call; a gh call with a method other than GET, an asset upload, a DELETE, or a docker
# write is a write. Prints one line per test: "test: <name> status: passed|failed".
# Exit codes: 0 all passed, 1 a test failed.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIX="$ROOT/tests/fixtures/advisory.respond"
REPO=o/r
ID=RUSTSEC-2099-0001
failed=0

setup() {
  D="$(mktemp -d)"; mkdir -p "$D/bin" "$D/api" "$D/deny" "$D/git/.agents/scripts" "$D/bodies"; : >"$D/calls"; : >"$D/docker"
  cp "$ROOT/.agents/scripts/advisory.respond.sh" "$ROOT/.agents/scripts/release.image.sh" "$D/git/.agents/scripts/"
  cat >"$D/bin/gh" <<'GH'
#!/bin/bash
# Fake gh api: GET answers from files; every other call is recorded with its method, and a release body is kept.
[ "${1:-}" = api ] || { [ "${1:-}" = auth ] && exit 0; echo "$*" >>"$FAKE/calls"; exit 0; }
shift
method=""; endpoint=""; fields=0; body=""
while [ $# -gt 0 ]; do
  case "$1" in
    --method|-X) method="$2"; shift 2 ;;
    -f|-F|--field|--raw-field) fields=1; case "$2" in body=*) body="${2#body=}" ;; esac; shift 2 ;;
    --input) fields=1; shift 2 ;;
    -H|--header|--jq|-q) shift 2 ;;
    --paginate|--silent|-i) shift ;;
    *) [ -z "$endpoint" ] && endpoint="$1"; shift ;;
  esac
done
[ -n "$method" ] || { [ $fields -eq 1 ] && method=POST || method=GET; }
echo "$method $endpoint" >>"$FAKE/calls"
case "$method $endpoint" in
  "PATCH "*releases/*) printf '%s' "$body" >"$FAKE/bodies/${endpoint##*/}" ;;
  "POST "*/issues) echo '{"number": 7, "html_url": "https://github.com/o/r/issues/7"}'; exit 0 ;;
esac
[ "$method" = GET ] || { echo '{}'; exit 0; }
path="${endpoint%%\?*}"
case "$endpoint" in *[?\&]page=*) page="${endpoint##*[?&]page=}"; path="${path}_page${page%%&*}" ;; esac
f="$FAKE/api/$(echo "$path" | tr '/.' '__')"
if [ -f "$f.http" ]; then echo "gh: fake error (HTTP $(cat "$f.http"))" >&2; exit 1; fi
[ -f "$f" ] || { echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
cat "$f"
GH
  cat >"$D/bin/cargo" <<'CARGO'
#!/bin/bash
# Fake cargo deny: --version, fetch db (exit code from fetch_rc), and check advisories with the recorded
# diagnostics of the release version of the current directory.
[ "${1:-}" = deny ] || exit 3
case " $* " in
  *" --version "*) echo "cargo-deny 0.20.2"; exit 0 ;;
  *" fetch "*) echo "fetch $*" >>"$FAKE/deny_calls"; exit "$(cat "$FAKE/fetch_rc" 2>/dev/null || echo 0)" ;;
  *" check "*)
    v="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
    echo "check $v" >>"$FAKE/deny_calls"
    n=0
    if [ -s "$FAKE/deny/$v" ]; then cat "$FAKE/deny/$v" >&2; n="$(wc -l <"$FAKE/deny/$v" | tr -d ' ')"; fi
    echo "{\"fields\":{\"advisories\":{\"errors\":$n,\"helps\":0,\"notes\":0,\"warnings\":0}},\"type\":\"summary\"}" >&2
    [ "$n" -eq 0 ] ;;
esac
CARGO
  cat >"$D/bin/docker" <<'DOCKER'
#!/bin/bash
# Fake docker buildx imagetools: latest is the version in FAKE/latest; the digest of a version is its digits
# padded with zeros; create moves latest to the version of the digest and is recorded.
echo "$*" >>"$FAKE/docker"
all="$*"
dig() { local x; x="$(echo "$1" | tr -d '.')"; printf 'sha256:%s%s' "$(printf '0%.0s' $(seq 1 $((64 - ${#x}))))" "$x"; }
case "$*" in
  "buildx imagetools inspect --raw "*:latest) [ -s "$FAKE/latest" ] || exit 1
    echo "{\"annotations\":{\"org.opencontainers.image.version\":\"$(cat "$FAKE/latest")\"}}" ;;
  "buildx imagetools inspect "*:latest) [ -s "$FAKE/latest" ] || exit 1; echo "Digest: $(dig "$(cat "$FAKE/latest")")" ;;
  "buildx imagetools inspect "*) v="${all##*:}"; echo "$(dig "$v") $v" >>"$FAKE/digests"; echo "Digest: $(dig "$v")" ;;
  "buildx imagetools create --tag "*:latest*) d="${all##*@}"; awk -v d="$d" '$1 == d {print $2; exit}' "$FAKE/digests" >"$FAKE/latest" ;;
  *) exit 1 ;;
esac
DOCKER
  chmod +x "$D/bin/gh" "$D/bin/cargo" "$D/bin/docker"
  (cd "$D/git" && git init -q && git config user.email t@example.com && git config user.name t && echo '[advisories]' >deny.toml) || exit 1
  serve "repos/$REPO" '{"private": false, "visibility": "public"}'
  serve "repos/$REPO/issues" '[]'
  serve "repos/$REPO/labels/security" '{"name": "security"}'
}
key() { echo "$1" | tr '/.' '__'; }
serve() { printf '%s\n' "$2" >"$D/api/$(key "$1")"; }
# release <version>: a commit whose Cargo.toml holds the version, tagged v<version>.
release() { (cd "$D/git" && printf '[package]\nname = "agent-loupe"\nversion = "%s"\n' "$1" >Cargo.toml && echo lock >Cargo.lock \
  && git add -A && git commit -qm "release $1" && git tag "v$1") || exit 1; }
# releases <json objects>: the releases list of the API, newest first as GitHub gives it.
releases() { serve "repos/$REPO/releases_page1" "$(jq -cs '.' <<<"$*")"; }
rel() { jq -cn --arg v "$1" --arg b "${2:-}" --argjson d "${3:-false}" --argjson p "${4:-false}" \
  '{id: ($v | gsub("\\."; "") | tonumber), tag_name: ("v" + $v), draft: $d, prerelease: $p, body: $b, html_url: ("https://github.com/o/r/releases/tag/v" + $v)}'; }
hit() { cp "$FIX/deny.$2.jsonl" "$D/deny/$1"; }
run() { (cd "$D/git" && PATH="$D/bin:$PATH" FAKE="$D" GH_TOKEN=fake /bin/bash .agents/scripts/advisory.respond.sh --repo "$REPO" "$@"); }
# Writes: any gh call that is not a GET, and any docker call that creates or removes.
writes() { echo $(( $(grep -vc '^GET ' "$D/calls") + $(grep -c 'create\|rm\|delete' "$D/docker") )); }
deletes() { echo $(( $(grep -ciE '^DELETE |assets|upload' "$D/calls") + $(grep -cE ' rm | rmi |delete' "$D/docker") )); }
actions() { echo "$out" | grep -c '^action: '; }
result() {
  if [ "$2" -eq 0 ] && [ "$(deletes)" -eq 0 ]; then echo "test: $1 status: passed"; else
    echo "test: $1 status: failed"; echo "  out: ${out:-}" | head -n 20; echo "  calls: $(tr '\n' ';' <"$D/calls")"; failed=1; fi
  rm -rf "$D"
}

# A seeded hit in a dry run: one planned draft, note, and issue (and the label), no promote, and no write.
setup; release 0.1.0; releases "$(rel 0.1.0 'Notes N')"; hit 0.1.0 vulnerability; serve "repos/$REPO/labels/security" ''; rm "$D/api/$(key "repos/$REPO/labels/security")"
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx "release: v0.1.0 status: hit advisories: $ID" \
  && [ "$(echo "$out" | grep -c "^action: draft target: $ID status: planned")" -eq 1 ] \
  && [ "$(echo "$out" | grep -c "^action: note target: v0.1.0 status: planned")" -eq 1 ] \
  && [ "$(echo "$out" | grep -c "^action: issue target: $ID status: planned")" -eq 1 ] \
  && [ "$(echo "$out" | grep -c "^action: label target: security status: planned")" -eq 1 ] \
  && ! echo "$out" | grep -q "action: promote" && [ "$(writes)" -eq 0 ] && [ "$(grep -c '^fetch' "$D/deny_calls")" -eq 1 ]
result dry_run_plans_and_writes_nothing $?

# A marker that holds the id: status marked, no action.
setup; release 0.1.0; releases "$(rel 0.1.0 "<!-- advisory-response: $ID -->
Notes N")"; hit 0.1.0 vulnerability
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx "release: v0.1.0 status: marked advisories: $ID" && [ "$(actions)" -eq 0 ] && [ "$(writes)" -eq 0 ]
result marked_release_gives_no_action $?

# A marker with another id: actions for the new id only, and the marker keeps both ids.
setup; release 0.1.0; releases "$(rel 0.1.0 "<!-- advisory-response: RUSTSEC-2099-0009 -->
Notes N")"; hit 0.1.0 vulnerability
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx "release: v0.1.0 status: hit advisories: $ID" \
  && echo "$out" | grep -q "action: note target: v0.1.0 status: planned detail: warning for $ID; marker: $ID RUSTSEC-2099-0009;" \
  && ! echo "$out" | grep -q "target: RUSTSEC-2099-0009" && [ "$(writes)" -eq 0 ]
result new_advisory_on_marked_release $?

# An unmaintained notice is no hit.
setup; release 0.1.0; releases "$(rel 0.1.0 'Notes N')"; hit 0.1.0 unmaintained
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx "release: v0.1.0 status: clean advisories: none" && [ "$(actions)" -eq 0 ]
result unmaintained_is_not_a_hit $?

# 0.1.0 affected, 0.1.1 clean, latest at 0.1.0: a write run moves latest forward to 0.1.1 and names the fix.
setup; release 0.1.0; release 0.1.1; releases "$(rel 0.1.1 'Notes B')" "$(rel 0.1.0 'Notes A')"; hit 0.1.0 vulnerability; echo 0.1.0 >"$D/latest"
out="$(cd "$D/git" && PATH="$D/bin:$PATH" FAKE="$D" GH_TOKEN=fake ADVISORY_TOKEN=fake /bin/bash .agents/scripts/advisory.respond.sh --repo "$REPO")"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -q "^action: promote target: v0.1.1 status: done" && [ "$(cat "$D/latest")" = 0.1.1 ] \
  && [ "$(grep -c 'imagetools create' "$D/docker")" -eq 1 ] && grep -q "Fixed in 0.1.1" "$D/bodies/10" \
  && grep -q "<!-- advisory-response: $ID@0.1.1 -->" "$D/bodies/10" && [ ! -e "$D/bodies/11" ] \
  && grep -q "^POST repos/$REPO/security-advisories" "$D/calls"
result latest_moves_only_forward_to_fix $?

# 0.1.1 affected, 0.1.0 clean, latest at 0.1.1: no fix release, so no move.
setup; release 0.1.0; release 0.1.1; releases "$(rel 0.1.1 'Notes B')" "$(rel 0.1.0 'Notes A')"; hit 0.1.1 vulnerability; echo 0.1.1 >"$D/latest"
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && ! echo "$out" | grep -q "action: promote" && [ "$(cat "$D/latest")" = 0.1.1 ] && echo "$out" | grep -qx "release: v0.1.0 status: clean advisories: none"
result no_backward_move $?

# A failed fetch of the database: exit 5 and no clean line.
setup; release 0.1.0; releases "$(rel 0.1.0 'Notes N')"; echo 1 >"$D/fetch_rc"
out="$(run --dry-run 2>/dev/null)"; rc=$?
[ $rc -eq 5 ] && ! echo "$out" | grep -q "status: clean" && ! grep -q '^check' "$D/deny_calls"
result db_fetch_failure_exits_5 $?

# A write run on a private repository: the note is the warning, the marker, then the earlier text unchanged;
# the draft is blocked; a second run on the written notes is marked and acts no more.
setup; release 0.1.0; serve "repos/$REPO" '{"private": true, "visibility": "private"}'
N="## What's Changed
* feat: first release

**Full Changelog**: https://github.com/o/r/commits/v0.1.0"
releases "$(rel 0.1.0 "$N")"; hit 0.1.0 vulnerability
out="$(run)"; rc=$?
body="$(cat "$D/bodies/10" 2>/dev/null)"
first="$(printf '%s\n' "$body" | head -n 1)"; rest="$(printf '%s\n' "$body" | awk 'f {print} /^<!-- advisory-response:/ {f = 1}')"
ok1=1; [ $rc -eq 0 ] && [ "$first" = "<!-- advisory-note $ID -->" ] && [ "$rest" = "$N" ] \
  && echo "$out" | grep -q "^action: draft target: $ID status: blocked" && echo "$out" | grep -q "^action: note target: v0.1.0 status: done" \
  && ! grep -q "security-advisories" "$D/calls" && ok1=0
releases "$(rel 0.1.0 "$body")"; : >"$D/calls"
out="$(run)"; rc=$?
[ $ok1 -eq 0 ] && [ $rc -eq 0 ] && echo "$out" | grep -qx "release: v0.1.0 status: marked advisories: $ID" && [ "$(actions)" -eq 0 ] && [ "$(writes)" -eq 0 ]
result notes_kept_below_warning $?

# A draft release, a pre-release, and a tag of another form are not checked.
setup; release 0.1.0; releases "$(rel 0.1.0 'N')" "$(rel 0.2.0 'D' true)" "$(rel 0.1.9 'P' false true)" \
  '{"id": 9, "tag_name": "v1.0", "draft": false, "prerelease": false, "body": "", "html_url": "x"}'
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && [ "$(echo "$out" | grep -c '^release: ')" -eq 1 ] && echo "$out" | grep -q '^repo: o/r visibility: public releases: 1$'
result skips_draft_and_prerelease $?

# A write run on a public repository with no draft advisory token stops before any write with exit 3.
setup; release 0.1.0; releases "$(rel 0.1.0 'N')"; hit 0.1.0 vulnerability
out="$(run 2>&1)"; rc=$?
[ $rc -eq 3 ] && echo "$out" | grep -q "ADVISORY_TOKEN is not set" && [ "$(writes)" -eq 0 ]
result write_without_token_exits_3 $?

# No release: nothing to check, no database fetch.
setup; releases
out="$(run --dry-run)"; rc=$?
[ $rc -eq 0 ] && echo "$out" | grep -qx 'result: releases: 0 hits: 0 planned: 0 done: 0 failed: 0 blocked: 0 exit: 0' && [ ! -e "$D/deny_calls" ]
result no_release_no_fetch $?

exit $failed
