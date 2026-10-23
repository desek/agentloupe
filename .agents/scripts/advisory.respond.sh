#!/usr/bin/env bash
# @agents-index Advisory response for published releases: cargo deny advisories on the Cargo.lock of each tag; warning note, issue, draft advisory, and latest only forward; never deletes.
#
# Usage: advisory.respond.sh [--repo OWNER/NAME] [--deny-config FILE] [--run-url URL] [--human] [--dry-run]
#   --repo R           The repository, OWNER/NAME. Default: the GitHub remote origin of the clone.
#   --deny-config F    The cargo deny policy for every release. Default: deny.toml of the checkout (the default
#                      branch in the workflow), so an ignore entry of today also applies to the past releases.
#                      A file whose [advisories] db-path names a scratch database is the seeded dry run.
#   --run-url URL      The link of the workflow run, for the comment that closes an issue. Default: none.
#   --human            Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run          List and check the releases, and print each planned action; no write to the GitHub API,
#                      the registry, or git.
#   -h, --help         Print this help.
#
# Environment: GH_TOKEN, the token of gh for the reads and the release notes, the issues, and the label
# (contents: write, issues: write in the workflow). ADVISORY_TOKEN, the token for the draft repository security
# advisory, needed by a run that writes on a public repository. The draft advisory follows the public
# repository: on a private repository that action reports blocked and needs no token. No token is printed.
#
# Purpose: one place for the response of docs/specification/release.md (Advisory response). It lists the
# published releases with a tag v<major>.<minor>.<patch> (no draft, no pre-release), fetches the advisory
# database one time (cargo deny fetch db), exports the tree of each tag with git archive into a temporary
# directory, and runs cargo deny check advisories there offline against the Cargo.lock of the tag. Only the
# kind vulnerability is a hit. Each release carries one hidden marker line in its notes with the advisory ids
# that are handled (id, or id@fixed version); a hit whose id is in the marker gives no new action. For a new
# hit: one issue with the label security per advisory id (open or comment), a warning note at the top of the
# notes of each affected release with the marker in the same write (the earlier text stays below), and one
# draft advisory per id. A fix release is a published release newer than each affected release whose own check
# has no hit for that id; then the notes, the issue, and the draft name it, latest moves forward to the newest
# such release with release.image.sh promote, and the issue closes when latest points to it and the draft
# names it. The script never deletes or replaces a release, an asset, a tag, an image tag, or a package
# version, never uploads an asset, never changes the draft or pre-release state, and moves only latest.
#
# Output (stdout, key: value):
#   repo: <owner/name> visibility: public|private releases: <n>
#   release: <tag> status: clean|hit|marked advisories: <ids|none>
#   action: draft|note|issue|label|promote|close target: <id or tag> status: planned|done|failed|blocked|unchanged
#     [detail: <text>]
#   result: releases: <n> hits: <n> planned: <n> done: <n> failed: <n> blocked: <n> exit: <code>
# Exit codes: 0 no new hit, or each action done, planned, or blocked; 1 one or more actions failed; 2 bad usage;
#   3 a tool or a credential is missing; 4 a GitHub API call failed; 5 the advisory database could not be
#   fetched or read, or a release could not be checked (no clean result is claimed).
# Reference: docs/specification/release.md, section Advisory response.
set -uo pipefail

MARKER_PREFIX="<!-- advisory-response:"
NOTE_OPEN="<!-- advisory-note"
NOTE_CLOSE="<!-- /advisory-note -->"
LABEL=security
LABEL_COLOR=b60205

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
human=0; dry_run=0; repo=""; deny_config=""; run_url=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }
need_value() { [[ $1 -ge 2 ]] || die "$2 needs a value. Fix: run --help for the flags. Verify: advisory.respond.sh --help." 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) need_value $# "$1"; repo="$2"; shift 2 ;;
    --deny-config) need_value $# "$1"; deny_config="$2"; shift 2 ;;
    --run-url) need_value $# "$1"; run_url="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the flags. Verify: advisory.respond.sh --dry-run prints release lines." 2 ;;
  esac
done

for t in git gh jq cargo tar; do
  command -v "$t" >/dev/null || die "$t is not installed, so the advisory response cannot run. Fix: install $t (gh and jq are on the GitHub runners; cargo with rustup). Verify: $t --version." 3
done
cargo deny --version >/dev/null 2>&1 || die "cargo-deny is not installed, so the releases cannot be checked. Fix: mise install (it reads the cargo-deny pin of .tool-versions). Verify: cargo deny --version." 3
[[ -n "${GH_TOKEN:-}" ]] || gh auth token >/dev/null 2>&1 || die "GH_TOKEN is not set and gh is not logged in, so the releases cannot be read. Fix: pass the job token as GH_TOKEN, or run gh auth login. Verify: gh api user." 3

cd "$ROOT" || die "cannot enter $ROOT. Fix: run the script from its checkout. Verify: re-run." 1
[[ -n "$deny_config" ]] || deny_config="$ROOT/deny.toml"
[[ -f "$deny_config" ]] || die "the policy file $deny_config does not exist. Fix: pass --deny-config <file>, or run from a checkout that has deny.toml. Verify: ls $deny_config." 2
deny_config="$(cd "$(dirname "$deny_config")" && pwd)/$(basename "$deny_config")"
if [[ -z "$repo" ]]; then
  repo="$(git remote get-url origin 2>/dev/null | sed -nE 's#^(https://github\.com/|git@github\.com:)([^/]+/[^/.]+)(\.git)?$#\2#p')"
fi
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "the repository '${repo:-none}' is not OWNER/NAME. Fix: pass --repo OWNER/NAME. Verify: re-run with --dry-run." 2

work="$(mktemp -d "${TMPDIR:-/tmp}/advisory.respond.XXXXXX")" || die "mktemp failed. Fix: check TMPDIR. Verify: re-run." 1
trap 'rm -rf "$work"' EXIT

planned=0; done_n=0; failed=0; blocked=0; hits_n=0
finish() {
  local code="$1"
  [[ "$code" -eq 0 && $failed -gt 0 ]] && code=1
  if [[ $human -eq 1 ]]; then
    echo "Advisory response: $nrel releases, $hits_n new hits; $planned planned, $done_n done, $failed failed, $blocked blocked (exit $code)."
  else
    echo "result: releases: $nrel hits: $hits_n planned: $planned done: $done_n failed: $failed blocked: $blocked exit: $code"
  fi
  exit "$code"
}
nrel=0
api_fail() { echo "error: the GitHub API call $1 failed: $(head -c 300 "$work/err" | tr '\n' ' '). Fix: check the token permissions and the network, then re-run. Verify: gh api $1." >&2; finish 4; }
# Read calls: GET only. Write calls: never in a dry run.
get() { gh api -H "Accept: application/vnd.github+json" "$1" </dev/null >"$2" 2>"$work/err"; }
report() {
  local kind="$1" target="$2" status="$3" detail="${4:-}"
  case "$status" in planned) planned=$((planned + 1)) ;; done|unchanged) done_n=$((done_n + 1)) ;; failed) failed=$((failed + 1)) ;; blocked) blocked=$((blocked + 1)) ;; esac
  if [[ $human -eq 1 ]]; then echo "$kind $target: $status${detail:+ ($detail)}"; else
    echo "action: $kind target: $target status: $status${detail:+ detail: $detail}"; fi
}

# ---- the repository and its releases ------------------------------------------------------------------------
get "repos/$repo" "$work/repo.json" || api_fail "repos/$repo"
visibility="$(jq -r 'if .private == true then "private" elif .private == false then "public" else "unknown" end' "$work/repo.json")"
[[ "$visibility" != unknown ]] || { echo "the repository record has no private field" >"$work/err"; api_fail "repos/$repo"; }
# A write run on a public repository needs the draft advisory token before any write.
if [[ $dry_run -eq 0 && "$visibility" == public && -z "${ADVISORY_TOKEN:-}" ]]; then
  die "ADVISORY_TOKEN is not set, so the draft advisory cannot be written and the run stops before any write. Fix: give the script step the secret with only the permission Repository security advisories (read and write) on $repo, or run with --dry-run. Reference: docs/specification/release.md, section Advisory response. Verify: advisory.respond.sh --dry-run." 3
fi

: >"$work/releases.jsonl"
page=1
while :; do
  get "repos/$repo/releases?per_page=100&page=$page" "$work/page.json" || api_fail "repos/$repo/releases"
  jq -e 'type == "array"' "$work/page.json" >/dev/null 2>&1 || { echo "the releases answer is not a list" >"$work/err"; api_fail "repos/$repo/releases"; }
  jq -c '.[] | select(.draft == false and .prerelease == false and (.tag_name | test("^v(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$")))
    | {id, tag: .tag_name, version: (.tag_name | ltrimstr("v")), body: (.body // ""), url: .html_url}' "$work/page.json" >>"$work/releases.jsonl"
  [[ "$(jq length "$work/page.json")" -lt 100 ]] && break
  page=$((page + 1))
done
# Oldest version first; the version order is numeric per part.
jq -s -c 'sort_by(.version | split(".") | map(tonumber)) | .[]' "$work/releases.jsonl" >"$work/sorted.jsonl"
nrel="$(wc -l <"$work/sorted.jsonl" | tr -d ' ')"
if [[ $human -eq 1 ]]; then echo "Repository $repo ($visibility): $nrel published releases."; else
  echo "repo: $repo visibility: $visibility releases: $nrel"; fi
[[ "$nrel" -gt 0 ]] || finish 0

# ---- one fetch of the database, then one offline check per release ----------------------------------------
if ! cargo deny --color never --config "$deny_config" fetch db >"$work/fetch.log" 2>&1; then
  echo "error: the fetch of the advisory database failed, so no release is checked and no clean result is claimed: $(tail -n 3 "$work/fetch.log" | tr '\n' ' '). Fix: check the network to github.com and the db settings of $deny_config, then re-run. Verify: cargo deny --config $deny_config fetch db exits 0." >&2
  finish 5
fi
: >"$work/hits.tsv"
while IFS= read -r rel; do
  tag="$(jq -r .tag <<<"$rel")"
  dir="$work/tree/$tag"; mkdir -p "$dir"
  if ! git archive --format=tar "refs/tags/$tag" 2>"$work/err" | tar -x -C "$dir" 2>>"$work/err"; then
    echo "error: git archive of $tag failed: $(head -c 200 "$work/err" | tr '\n' ' '). Fix: check out with all tags (fetch-depth 0), or git fetch --tags. Verify: git rev-parse $tag." >&2
    finish 5
  fi
  (cd "$dir" && cargo deny --format json --color never --offline --locked --config "$deny_config" check advisories) >/dev/null 2>"$work/deny.$tag.json"
  if ! jq -e 'select(.type == "summary")' "$work/deny.$tag.json" >/dev/null 2>&1; then
    echo "error: cargo deny could not check $tag, so no clean result is claimed: $(grep -v '^{' "$work/deny.$tag.json" | tail -n 3 | tr '\n' ' '). Fix: run cargo deny check advisories in the tree of $tag and read the error. Verify: re-run." >&2
    finish 5
  fi
  # One line per vulnerability: tag, id, crate, crate version, url, aliases, cvss.
  jq -r --arg tag "$tag" 'select(.type == "diagnostic" and .fields.code == "vulnerability" and .fields.advisory != null)
    | .fields as $f | [$tag, $f.advisory.id, $f.advisory.package, ([$f.graphs[]?.Krate.version] | first // "unknown"),
      ($f.advisory.url // ("https://rustsec.org/advisories/" + $f.advisory.id + ".html")), (($f.advisory.aliases // []) | join(",")),
      ($f.advisory.cvss // "")] | @tsv' "$work/deny.$tag.json" | sort -u >>"$work/hits.tsv"
  rm -rf "$dir"
done <"$work/sorted.jsonl"

# The marker of a release: the handled ids, each as id or id@fixed version.
marker_of() { jq -r .body <<<"$1" | grep -o "${MARKER_PREFIX}[^>]*-->" | head -n 1 | sed "s/^$MARKER_PREFIX//; s/-->\$//" | tr ' ' '\n' | grep -v '^$' || true; }
ver_gt() { [[ "$1" != "$2" && "$(printf '%s\n%s\n' "$1" "$2" | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1)" == "$1" ]]; }

# ---- per release status -----------------------------------------------------------------------------------
declare -a new_pairs=()
while IFS= read -r rel; do
  tag="$(jq -r .tag <<<"$rel")"
  ids="$(awk -F'\t' -v t="$tag" '$1 == t {print $2}' "$work/hits.tsv" | sort -u)"
  marked="$(marker_of "$rel" | sed 's/@.*//')"
  status=clean
  if [[ -n "$ids" ]]; then
    status=marked
    for id in $ids; do
      if ! grep -qx "$id" <<<"$marked"; then status=hit; new_pairs+=("$tag $id"); hits_n=$((hits_n + 1)); fi
    done
  fi
  list="$(echo $ids | tr ' ' ',')"
  if [[ $human -eq 1 ]]; then echo "$tag: $status${list:+ ($list)}"; else echo "release: $tag status: $status advisories: ${list:-none}"; fi
done <"$work/sorted.jsonl"

# ---- per advisory id: affected releases, fix release, and the target of latest ------------------------------
all_ids="$(cut -f2 "$work/hits.tsv" | sort -u)"
for id in $all_ids; do
  row="$(awk -F'\t' -v i="$id" '$2 == i {print; exit}' "$work/hits.tsv")"
  crate="$(cut -f3 <<<"$row")"; crate_version="$(cut -f4 <<<"$row")"; adv_url="$(cut -f5 <<<"$row")"
  aliases="$(cut -f6 <<<"$row")"; cvss="$(cut -f7 <<<"$row")"
  affected="$(awk -F'\t' -v i="$id" '$2 == i {print $1}' "$work/hits.tsv" | sed 's/^v//' | sort -t. -k1,1n -k2,2n -k3,3n | uniq)"
  newest_affected="$(tail -n 1 <<<"$affected")"
  new_tags="$(printf '%s\n' "${new_pairs[@]:-}" | awk -v i="$id" '$2 == i {print $1}')"
  # Fix releases: newer than each affected release, and no hit for this id.
  fixes=""
  while IFS= read -r rel; do
    v="$(jq -r .version <<<"$rel")"
    ver_gt "$v" "$newest_affected" || continue
    grep -qx "$v" <<<"$affected" && continue
    fixes="$fixes$v"$'\n'
  done <"$work/sorted.jsonl"
  fixed="$(grep -v '^$' <<<"$fixes" | head -n 1)"; newest_fix="$(grep -v '^$' <<<"$fixes" | tail -n 1)"
  # Releases whose marker does not yet name the fixed version for this id get their note again.
  stale_tags=""
  if [[ -n "$fixed" ]]; then
    while IFS= read -r rel; do
      v="$(jq -r .version <<<"$rel")"; grep -qx "$v" <<<"$affected" || continue
      grep -qx "$id@$fixed" <<<"$(marker_of "$rel")" || stale_tags="$stale_tags v$v"
    done <"$work/sorted.jsonl"
  fi
  [[ -z "$new_tags" && -z "$stale_tags" && -z "$fixed" ]] && continue
  affected_list="$(echo $affected | sed 's/ /, /g')"
  title="Security: $id in $crate affects published releases"

  # Label and issue: one open issue per id.
  issue_url=""; issue_number=""
  get "repos/$repo/issues?state=open&labels=$LABEL&per_page=100" "$work/issues.json" || api_fail "repos/$repo/issues"
  issue_number="$(jq -r --arg t "$title" '[.[] | select(.title == $t and (.pull_request | not))] | first | .number // empty' "$work/issues.json")"
  issue_url="$(jq -r --arg t "$title" '[.[] | select(.title == $t and (.pull_request | not))] | first | .html_url // empty' "$work/issues.json")"
  fix_path="The fix path: the daily dependency autofix workflow, when it exists, makes a fix(deps) commit and a patch release. Else the manual fix steps of docs/specification/release.md (Advisory response, Manual fix steps): a fix(deps) commit that updates $crate, a squash merge with a green verdict, and the merge of the release pull request."
  issue_body="The advisory $id (${aliases:-no alias}) affects the crate \`$crate\` $crate_version in the Cargo.lock of the published releases $affected_list.

- Upstream advisory: $adv_url
- Affected versions of agentloupe: $affected_list
- Fixed version: ${fixed:-none yet}
- Draft security advisory: kept by the advisory response; a human publishes it.

$fix_path"
  if [[ -n "$new_tags" || -n "$stale_tags" ]]; then
    if [[ $dry_run -eq 1 ]]; then
      get "repos/$repo/labels/$LABEL" "$work/label.json" || report label "$LABEL" planned "create the label"
      if [[ -z "$issue_number" ]]; then report issue "$id" planned "open '$title' with the label $LABEL"; else report issue "$id" planned "comment on #$issue_number"; fi
    else
      if ! get "repos/$repo/labels/$LABEL" "$work/label.json"; then
        if gh api --method POST "repos/$repo/labels" -f name="$LABEL" -f color="$LABEL_COLOR" -f description="A security advisory affects a published release" </dev/null >/dev/null 2>"$work/err"; then
          report label "$LABEL" "done" "created"
        else report label "$LABEL" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
      fi
      if [[ -z "$issue_number" ]]; then
        if gh api --method POST "repos/$repo/issues" -f title="$title" -f body="$issue_body" -f "labels[]=$LABEL" </dev/null >"$work/issue.json" 2>"$work/err"; then
          issue_number="$(jq -r '.number // empty' "$work/issue.json")"; issue_url="$(jq -r '.html_url // empty' "$work/issue.json")"
          report issue "$id" "done" "opened #$issue_number"
        else report issue "$id" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
      else
        if gh api --method POST "repos/$repo/issues/$issue_number/comments" -f body="$issue_body" </dev/null >/dev/null 2>"$work/err"; then
          report issue "$id" "done" "commented on #$issue_number"
        else report issue "$id" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
      fi
    fi
  fi

  # Notes: each affected release with a new hit or a note that does not yet name the fixed version.
  for tag in $(echo $new_tags $stale_tags | tr ' ' '\n' | sort -u); do
    rel="$(jq -c --arg t "$tag" 'select(.tag == $t)' "$work/sorted.jsonl")"
    old_body="$(jq -r .body <<<"$rel")"
    # The managed head (notes and marker) is rebuilt; the text below the marker stays as it is.
    if grep -q "^$MARKER_PREFIX" <<<"$old_body"; then
      head_part="$(awk -v m="$MARKER_PREFIX" 'index($0, m) == 1 {exit} {print}' <<<"$old_body")"
      rest="$(awk -v m="$MARKER_PREFIX" 'found {print} index($0, m) == 1 {found = 1}' <<<"$old_body")"
    else
      head_part=""; rest="$old_body"
    fi
    entry="$id"; [[ -n "$fixed" ]] && entry="$id@$fixed"
    ids_line="$( (marker_of "$rel" | grep -vE "^$id(@|\$)"; echo "$entry") | sort -u | tr '\n' ' ' | sed 's/ $//')"
    kept="$(awk -v o="$NOTE_OPEN $id -->" -v c="$NOTE_CLOSE" '$0 == o {skip = 1} !skip {print} skip && $0 == c {skip = 0}' <<<"$head_part")"
    note="$NOTE_OPEN $id -->
> [!WARNING]
> Security advisory $id: the crate \`$crate\` $crate_version in this release has a known vulnerability ($adv_url). ${fixed:+Fixed in $fixed: upgrade to $fixed or later. }Tracking issue: ${issue_url:-the issue with the label $LABEL}.
$NOTE_CLOSE"
    new_body="$note
${kept:+$kept
}$MARKER_PREFIX $ids_line -->
$rest"
    printf '%s' "$new_body" >"$work/body.$tag"
    if [[ $dry_run -eq 1 ]]; then
      report note "$tag" planned "warning for $id${fixed:+, fixed in $fixed}; marker: $ids_line; earlier text kept ($(printf '%s' "$rest" | wc -c | tr -d ' ') bytes)"
    else
      rid="$(jq -r .id <<<"$rel")"
      if gh api --method PATCH "repos/$repo/releases/$rid" -f body="$new_body" </dev/null >/dev/null 2>"$work/err"; then
        report note "$tag" "done" "warning for $id; marker: $ids_line"
      else report note "$tag" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
    fi
  done

  # The draft advisory follows the public repository. On a public repository the marker gets id@fixed in the
  # run that writes the draft with the fixed version, so a marker with id@fixed on each affected release
  # means that the draft names it.
  draft_named=0
  if [[ -n "$fixed" && "$visibility" == public && -z "$stale_tags" && -z "$new_tags" ]]; then draft_named=1; fi
  if [[ -n "$new_tags" || -n "$stale_tags" ]]; then
    if [[ "$visibility" != public ]]; then
      report draft "$id" blocked "the repository is private; the draft security advisory follows the public repository"
    elif [[ $dry_run -eq 1 ]]; then
      report draft "$id" planned "draft for $id (${aliases:-no alias}), crate $crate $crate_version, affected $affected_list${fixed:+, fixed $fixed}; not published, no CVE request"
    else
      summary="$id in $crate affects published releases of agentloupe"
      range="$(echo $affected | sed 's/^/= /; s/ /, = /g')"
      jq -n --arg s "$summary" --arg d "$issue_body" --arg r "$range" --arg p "${fixed:-}" --arg c "$cvss" \
        '{summary: $s, description: $d, vulnerabilities: [{package: {ecosystem: "other", name: "agentloupe"}, vulnerable_version_range: $r, patched_versions: (if $p == "" then null else $p end)}]}
         + (if $c == "" then {severity: "medium"} else {cvss_vector_string: $c} end)' >"$work/draft.json"
      existing="$(GH_TOKEN="$ADVISORY_TOKEN" gh api "repos/$repo/security-advisories?state=draft&per_page=100" </dev/null 2>"$work/err" \
        | jq -r --arg s "$summary" '[.[] | select(.summary == $s)] | first | .ghsa_id // empty' 2>/dev/null || true)"
      if [[ -n "$existing" ]]; then
        GH_TOKEN="$ADVISORY_TOKEN" gh api --method PATCH "repos/$repo/security-advisories/$existing" --input "$work/draft.json" >/dev/null 2>"$work/err" \
          && { report draft "$id" "done" "updated $existing"; draft_named=1; } || report draft "$id" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"
      else
        GH_TOKEN="$ADVISORY_TOKEN" gh api --method POST "repos/$repo/security-advisories" --input "$work/draft.json" >/dev/null 2>"$work/err" \
          && { report draft "$id" "done" "created"; draft_named=1; } || report draft "$id" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"
      fi
    fi
  fi

  # latest: only forward, only to the newest fix release.
  if [[ -n "$newest_fix" ]]; then
    current="$("$ROOT/.agents/scripts/release.image.sh" latest 2>/dev/null </dev/null | sed -n 's/.* version: \([^ ]*\).*/\1/p')"
    if [[ "$current" == "$newest_fix" ]]; then
      report promote "v$newest_fix" unchanged "latest already points to $newest_fix"
    elif [[ "$current" != none && -n "$current" ]] && ver_gt "$current" "$newest_fix"; then
      report promote "v$newest_fix" unchanged "latest points to $current, newer than $newest_fix; no backward move"
    elif [[ $dry_run -eq 1 ]]; then
      report promote "v$newest_fix" planned "latest from ${current:-none} to $newest_fix"
    elif "$ROOT/.agents/scripts/release.image.sh" promote --tag "v$newest_fix" </dev/null >"$work/promote.out" 2>"$work/err"; then
      report promote "v$newest_fix" "done" "$(tail -n 1 "$work/promote.out")"; current="$newest_fix"
    else report promote "v$newest_fix" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
    # Close the issue when the fix release exists, latest points to it, and the draft names it.
    if [[ $dry_run -eq 0 && "$current" == "$newest_fix" && $draft_named -eq 1 && -n "$issue_number" ]]; then
      if gh api --method POST "repos/$repo/issues/$issue_number/comments" -f body="The fix release $fixed exists, latest points to $newest_fix, and the draft advisory names it. ${run_url:+Run: $run_url}" </dev/null >/dev/null 2>"$work/err" \
        && gh api --method PATCH "repos/$repo/issues/$issue_number" -f state=closed </dev/null >/dev/null 2>>"$work/err"; then
        report close "#$issue_number" "done" "fixed in $fixed"
      else report close "#$issue_number" failed "$(head -c 200 "$work/err" | tr '\n' ' ')"; fi
    fi
  fi
done
finish 0
