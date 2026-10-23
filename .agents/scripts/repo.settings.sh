#!/usr/bin/env bash
# @agents-index Applies, plans, or checks the repository settings of desek/agentloupe with gh api: rulesets, merge settings, actions policy, security features.
#
# Usage: repo.settings.sh [--dry-run | --check] [--repo OWNER/NAME] [--human]
#   (no mode)        Apply: read each setting, write only a setting that differs from the desired state.
#   --dry-run        Read each setting, print the planned change, and write nothing. On a private repository,
#                    a setting that cannot be read there is reported as unavailable with the reason.
#   --check          Read each setting, print each difference as drift, and write nothing.
#   --repo OWNER/NAME  The repository. Default: desek/agentloupe. For a test against another repository.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   -h, --help       Print this help.
#
# Purpose: the repository settings of docs/specification/release.md (Repository settings). The desired state:
#   main-ruleset      One active branch ruleset "main" on the default branch: a pull request before a merge
#                     (0 approvals, squash only), the status check "verdict" of the GitHub Actions app (no
#                     up-to-date rule), a linear history, no force push, no deletion; the repository admin role
#                     bypasses always.
#   tag-ruleset       One active tag ruleset "release-tags" on refs/tags/v*: creation, update, and deletion are
#                     restricted; the repository admin role bypasses always, so the release token of the owner
#                     can make the tag of a release.
#   merge-settings    Squash merge on, merge commit off, rebase merge off, delete the head branch on merge on,
#                     auto-merge on; the squash commit title is the pull request title and its message the list
#                     of the commit messages.
#   actions-policy    Actions enabled, all actions allowed, actions pinned to a full-length commit SHA; the
#                     default workflow token read, and actions cannot approve a pull request.
#   secret-scanning   Secret scanning and push protection on.
#   dependabot-alerts On. The Dependabot security updates are never changed.
#   private-vulnerability-reporting  On; SECURITY.md names it as the one channel.
# The script reads and writes only through gh api with the gh login of the person who runs it (admin on the
# repository); it needs no secret of its own and never prints a token. It finds a ruleset by its name and
# updates it in place; it never makes a second ruleset with the same name and never deletes a ruleset. It
# never changes the visibility: on a private repository the apply mode and --check stop before any write
# (exit 5), because on the free plan the rulesets and the security features exist only on a public repository.
# No workflow calls this script. It runs with the bash 3.2 of macOS and gh; it needs no jq (gh evaluates
# its own --jq). Idempotent: a second run directly after a successful run makes zero writes.
#
# Output (stdout, key: value), one line per setting, then one result line:
#   setting: <name> status: unchanged|applied|planned|drift|unavailable [detail: ...]
#   result: <unchanged|applied|planned|drift|failed> writes: <n> unavailable: <n> exit: <code>
# Exit codes: 0 success, 1 --check found drift, 2 bad usage, 3 gh is missing, not logged in, or the login has
#   no admin permission on the repository, 4 an API call failed, 5 the repository is private (apply or --check).
# Reference: docs/specification/release.md, section Repository settings.
set -euo pipefail

# The one constant of the repository; --repo replaces it for a test.
REPO="desek/agentloupe"
# The app id of GitHub Actions, which reports the check runs of the verdict workflow.
ACTIONS_APP_ID=15368
# The id of the built-in repository role "admin" in a bypass list.
ADMIN_ROLE_ID=5
REF="Reference: docs/specification/release.md, section Repository settings."

mode=apply; human=0; repo="$REPO"
[ -t 1 ] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) [ "$mode" = apply ] || die "--dry-run and --check exclude each other. Fix: pass one mode. Verify: repo.settings.sh --dry-run prints one line per setting." 2
      mode=dry-run; shift ;;
    --check) [ "$mode" = apply ] || die "--dry-run and --check exclude each other. Fix: pass one mode. Verify: repo.settings.sh --check prints one line per setting." 2
      mode=check; shift ;;
    --repo) [ $# -ge 2 ] || die "--repo needs OWNER/NAME. Fix: pass --repo desek/agentloupe or omit it. Verify: re-run with --dry-run." 2
      repo="$2"; shift 2 ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the modes and flags. Verify: repo.settings.sh --dry-run prints one line per setting." 2 ;;
  esac
done
case "$repo" in */*) ;; *) die "--repo $repo is not of the form OWNER/NAME. Fix: pass for example --repo desek/agentloupe. Verify: re-run with --dry-run." 2 ;; esac

command -v gh >/dev/null 2>&1 || die "gh is not installed, so the settings cannot be read or written. Fix: install the GitHub CLI (brew install gh) and run gh auth login as an admin of $repo. $REF Verify: gh --version." 3

tmp="$(mktemp -d "${TMPDIR:-/tmp}/repo.settings.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

writes=0; unavailable=0; changed=0; failed=0
# A read: prints the --jq output of gh api, or fails with the error in $tmp/err. A read often runs in a
# command substitution, so its HTTP status comes from the error file (status), not from a variable.
get() {
  : >"$tmp/err"
  gh api "$@" 2>"$tmp/err"
}
# The HTTP status of the last failed read, from the "(HTTP nnn)" of the gh error, or "error".
status() {
  local code
  code="$(sed -n 's/.*(HTTP \([0-9][0-9]*\)).*/\1/p' "$tmp/err" | head -n 1)"
  if [ -z "$code" ] && grep -q 'gh auth login' "$tmp/err"; then code=401; fi
  echo "${code:-error}"
}
# A write: one gh api call with a write method; counts it and stops with exit 4 when it fails.
put() {
  local method="$1" endpoint="$2"; shift 2
  writes=$((writes + 1))
  if ! gh api --method "$method" "$endpoint" "$@" >/dev/null 2>"$tmp/err"; then
    die "the write $method $endpoint failed: $(tr '\n' ' ' <"$tmp/err"). Fix: check that the gh login is an admin of $repo and that the repository is public; run repo.settings.sh --dry-run to see the plan. $REF Verify: re-run repo.settings.sh; a successful run ends with exit 0." 4
  fi
}
emit() {
  local name="$1" status="$2" detail="${3:-}"
  if [ $human -eq 1 ]; then
    case "$status" in
      unchanged) echo "$name: already as desired." ;;
      applied) echo "$name: changed ($detail)." ;;
      planned) echo "$name: would change ($detail)." ;;
      drift) echo "$name: differs from the desired state ($detail)." ;;
      unavailable) echo "$name: cannot be read ($detail)." ;;
    esac
  else
    echo "setting: $name status: $status${detail:+ detail: $detail}"
  fi
}
# A setting that cannot be read: reported in every mode; outside --dry-run it fails the run with exit 4.
unavailable() {
  unavailable=$((unavailable + 1))
  [ "$mode" = dry-run ] || failed=1
  emit "$1" unavailable "$2"
}
# Compares a current and a desired value of one setting and acts by the mode; $3 is the change text and
# $4 the function that writes the setting.
settle() {
  local name="$1" detail="$3" writer="$4" current="$2" desired="$5"
  if [ "$current" = "$desired" ]; then emit "$name" unchanged; return; fi
  changed=$((changed + 1))
  case "$mode" in
    apply) "$writer"; emit "$name" applied "$detail" ;;
    dry-run) emit "$name" planned "$detail" ;;
    check) emit "$name" drift "$detail" ;;
  esac
}
# The change text of two lists of "key=value" words: each key whose value differs, as "key a to b".
diff_words() {
  awk -v cur="$1" -v want="$2" 'BEGIN {
    n = split(cur, c, " "); split(want, w, " "); out = ""
    for (i = 1; i <= n; i++) if (c[i] != w[i]) { split(c[i], a, "="); split(w[i], b, "="); out = out (out == "" ? "" : ", ") a[1] " " a[2] " to " b[2] }
    print out }'
}

# --- The repository: admin permission, visibility, merge settings, security_and_analysis -------------------
if ! repo_state="$(get "repos/$repo" --jq '"admin=\(.permissions.admin // false)",
    "visibility=\(.visibility)",
    "merge=allow_squash_merge=\(.allow_squash_merge) allow_merge_commit=\(.allow_merge_commit) allow_rebase_merge=\(.allow_rebase_merge) delete_branch_on_merge=\(.delete_branch_on_merge) allow_auto_merge=\(.allow_auto_merge) squash_merge_commit_title=\(.squash_merge_commit_title) squash_merge_commit_message=\(.squash_merge_commit_message)",
    "scanning=secret_scanning=\(.security_and_analysis.secret_scanning.status // "none") secret_scanning_push_protection=\(.security_and_analysis.secret_scanning_push_protection.status // "none")"')"; then
  http="$(status)"
  case "$http" in
    401|403|404) die "gh cannot read the repository $repo (HTTP $http): it does not exist, or the gh login has no access. Fix: run gh auth login as an admin of $repo, or pass the right --repo. $REF Verify: gh api repos/$repo --jq .permissions.admin prints true." 3 ;;
    *) die "the read of the repository $repo failed: $(tr '\n' ' ' <"$tmp/err"). Fix: check the network and gh auth status, then re-run. Verify: gh api repos/$repo exits 0." 4 ;;
  esac
fi
field() { echo "$repo_state" | sed -n "s/^$1=//p"; }
[ "$(field admin)" = true ] || die "the gh login has no admin permission on $repo, so it cannot read or write the settings. Fix: run gh auth login as an admin (the owner) of $repo. $REF Verify: gh api repos/$repo --jq .permissions.admin prints true." 3
visibility="$(field visibility)"
if [ "$visibility" != public ] && [ "$mode" != dry-run ]; then
  die "the repository $repo has the visibility $visibility; on the free plan the rulesets and the security features exist only on a public repository, so this run writes nothing. Fix: the owner makes the repository public first (this script never changes the visibility); repo.settings.sh --dry-run shows the plan now. $REF Verify: gh repo view $repo --json visibility prints PUBLIC." 5
fi
private_note="the repository is $visibility; on the free plan this setting exists only on a public repository"

# --- Rulesets -------------------------------------------------------------------------------------------------
# One normalized line per ruleset: the fields that this script sets, in a fixed order, so the comparison
# ignores the defaults that GitHub adds.
RULESET_JQ='[ "enforcement=\(.enforcement)", "target=\(.target)",
  "include=\(.conditions.ref_name.include // [] | sort | join(","))",
  "exclude=\(.conditions.ref_name.exclude // [] | sort | join(","))",
  "bypass=\([.bypass_actors[]? | "\(.actor_type):\(.actor_id):\(.bypass_mode)"] | sort | join(","))",
  "rules=\([.rules[]?.type] | sort | join(","))",
  "pull_request=\([.rules[]? | select(.type == "pull_request") | .parameters | "\(.required_approving_review_count):\(.allowed_merge_methods // [] | sort | join("+"))"] | join(","))",
  "status_checks=\([.rules[]? | select(.type == "required_status_checks") | .parameters | "\(.strict_required_status_checks_policy):\([.required_status_checks[] | "\(.context)@\(.integration_id // "any")"] | sort | join("+"))"] | join(","))"
] | join("; ")'
BYPASS="RepositoryRole:$ADMIN_ROLE_ID:always"
want_main="enforcement=active; target=branch; include=~DEFAULT_BRANCH; exclude=; bypass=$BYPASS; rules=deletion,non_fast_forward,pull_request,required_linear_history,required_status_checks; pull_request=0:squash; status_checks=false:verdict@$ACTIONS_APP_ID"
want_tags="enforcement=active; target=tag; include=refs/tags/v*; exclude=; bypass=$BYPASS; rules=creation,deletion,update; pull_request=; status_checks="
bypass_json="[{\"actor_id\": $ADMIN_ROLE_ID, \"actor_type\": \"RepositoryRole\", \"bypass_mode\": \"always\"}]"
cat >"$tmp/main.json" <<EOF
{
  "name": "main",
  "target": "branch",
  "enforcement": "active",
  "conditions": {"ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}},
  "bypass_actors": $bypass_json,
  "rules": [
    {"type": "deletion"},
    {"type": "non_fast_forward"},
    {"type": "required_linear_history"},
    {"type": "pull_request", "parameters": {
      "required_approving_review_count": 0,
      "dismiss_stale_reviews_on_push": false,
      "require_code_owner_review": false,
      "require_last_push_approval": false,
      "required_review_thread_resolution": false,
      "allowed_merge_methods": ["squash"]}},
    {"type": "required_status_checks", "parameters": {
      "strict_required_status_checks_policy": false,
      "do_not_enforce_on_create": false,
      "required_status_checks": [{"context": "verdict", "integration_id": $ACTIONS_APP_ID}]}}
  ]
}
EOF
cat >"$tmp/release-tags.json" <<EOF
{
  "name": "release-tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": {"ref_name": {"include": ["refs/tags/v*"], "exclude": []}},
  "bypass_actors": $bypass_json,
  "rules": [{"type": "creation"}, {"type": "update"}, {"type": "deletion"}]
}
EOF

rulesets=""; rulesets_ok=1
if ! rulesets="$(get "repos/$repo/rulesets?includes_parents=false" --paginate --jq '.[] | "\(.id) \(.name)"')"; then
  rulesets_ok=0
fi
ruleset_id=""
write_ruleset() {
  if [ -n "$ruleset_id" ]; then put PUT "repos/$repo/rulesets/$ruleset_id" --input "$tmp/$ruleset_name.json"
  else put POST "repos/$repo/rulesets" --input "$tmp/$ruleset_name.json"; fi
}
# The detail of a ruleset difference: the names of the fields that differ.
ruleset_detail() {
  awk -v cur="$1" -v want="$2" 'BEGIN {
    n = split(want, w, "; "); split(cur, c, "; "); out = ""
    for (i = 1; i <= n; i++) if (c[i] != w[i]) { split(w[i], k, "="); out = out (out == "" ? "" : ", ") k[1] }
    print "update " out }'
}
for pair in "main-ruleset main" "tag-ruleset release-tags"; do
  setting="${pair%% *}"; ruleset_name="${pair#* }"
  if [ "$ruleset_name" = main ]; then want="$want_main"; else want="$want_tags"; fi
  if [ $rulesets_ok -eq 0 ]; then
    if [ "$visibility" != public ]; then unavailable "$setting" "HTTP $(status): $private_note"
    else unavailable "$setting" "HTTP $(status): $(tr '\n' ' ' <"$tmp/err")"; fi
    continue
  fi
  # The first ruleset with this name; a second one with the same name is never made.
  ruleset_id="$(echo "$rulesets" | awk -v n="$ruleset_name" '$2 == n {print $1; exit}')"
  if [ -z "$ruleset_id" ]; then
    settle "$setting" "absent" "create the ruleset $ruleset_name" write_ruleset "$want"
    continue
  fi
  if ! current="$(get "repos/$repo/rulesets/$ruleset_id" --jq "$RULESET_JQ")"; then
    unavailable "$setting" "HTTP $(status): the ruleset $ruleset_name ($ruleset_id) cannot be read"
    continue
  fi
  settle "$setting" "$current" "$(ruleset_detail "$current" "$want") of the ruleset $ruleset_name ($ruleset_id)" write_ruleset "$want"
done

# --- Merge settings -------------------------------------------------------------------------------------------
want_merge="allow_squash_merge=true allow_merge_commit=false allow_rebase_merge=false delete_branch_on_merge=true allow_auto_merge=true squash_merge_commit_title=PR_TITLE squash_merge_commit_message=COMMIT_MESSAGES"
write_merge() {
  put PATCH "repos/$repo" -F allow_squash_merge=true -F allow_merge_commit=false -F allow_rebase_merge=false \
    -F delete_branch_on_merge=true -F allow_auto_merge=true \
    -f squash_merge_commit_title=PR_TITLE -f squash_merge_commit_message=COMMIT_MESSAGES
}
cur_merge="$(field merge)"
settle merge-settings "$cur_merge" "$(diff_words "$cur_merge" "$want_merge")" write_merge "$want_merge"

# --- Actions policy -------------------------------------------------------------------------------------------
want_actions="enabled=true allowed_actions=all sha_pinning_required=true"
want_token="default_workflow_permissions=read can_approve_pull_request_reviews=false"
if cur_actions="$(get "repos/$repo/actions/permissions" --jq '"enabled=\(.enabled) allowed_actions=\(.allowed_actions) sha_pinning_required=\(.sha_pinning_required // false)"')" \
   && cur_token="$(get "repos/$repo/actions/permissions/workflow" --jq '"default_workflow_permissions=\(.default_workflow_permissions) can_approve_pull_request_reviews=\(.can_approve_pull_request_reviews)"')"; then
  write_actions() {
    [ "$cur_actions" = "$want_actions" ] || put PUT "repos/$repo/actions/permissions" -F enabled=true -f allowed_actions=all -F sha_pinning_required=true
    [ "$cur_token" = "$want_token" ] || put PUT "repos/$repo/actions/permissions/workflow" -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
  }
  settle actions-policy "$cur_actions $cur_token" "$(diff_words "$cur_actions $cur_token" "$want_actions $want_token")" write_actions "$want_actions $want_token"
else
  unavailable actions-policy "HTTP $(status): $(tr '\n' ' ' <"$tmp/err")"
fi

# --- Secret scanning and push protection ----------------------------------------------------------------------
want_scanning="secret_scanning=enabled secret_scanning_push_protection=enabled"
cur_scanning="$(field scanning)"
write_scanning() {
  printf '%s\n' '{"security_and_analysis": {"secret_scanning": {"status": "enabled"}, "secret_scanning_push_protection": {"status": "enabled"}}}' >"$tmp/scanning.json"
  put PATCH "repos/$repo" --input "$tmp/scanning.json"
}
if [ "$cur_scanning" = "secret_scanning=none secret_scanning_push_protection=none" ]; then
  unavailable secret-scanning "security_and_analysis is null: $private_note"
else
  settle secret-scanning "$cur_scanning" "$(diff_words "$cur_scanning" "$want_scanning")" write_scanning "$want_scanning"
fi

# --- Dependabot alerts: 204 is on, 404 is off; the security updates are not touched ---------------------------
write_alerts() { put PUT "repos/$repo/vulnerability-alerts"; }
if get "repos/$repo/vulnerability-alerts" --silent; then cur_alerts=enabled
elif [ "$(status)" = 404 ]; then cur_alerts=disabled
else cur_alerts=""; fi
if [ -n "$cur_alerts" ]; then
  settle dependabot-alerts "$cur_alerts" "dependabot_alerts $cur_alerts to enabled" write_alerts enabled
else
  unavailable dependabot-alerts "HTTP $(status): $(tr '\n' ' ' <"$tmp/err")"
fi

# --- Private vulnerability reporting --------------------------------------------------------------------------
write_reporting() { put PUT "repos/$repo/private-vulnerability-reporting"; }
if cur_reporting="$(get "repos/$repo/private-vulnerability-reporting" --jq '.enabled')"; then
  settle private-vulnerability-reporting "$cur_reporting" "enabled $cur_reporting to true" write_reporting true
elif [ "$visibility" != public ]; then
  unavailable private-vulnerability-reporting "HTTP $(status): $private_note"
else
  unavailable private-vulnerability-reporting "HTTP $(status): $(tr '\n' ' ' <"$tmp/err")"
fi

# --- Result ---------------------------------------------------------------------------------------------------
code=0
if [ $failed -eq 1 ]; then result=failed; code=4
elif [ $changed -eq 0 ]; then result=unchanged
else case "$mode" in apply) result=applied ;; dry-run) result=planned ;; check) result=drift; code=1 ;; esac
fi
if [ $human -eq 1 ]; then
  echo "Result for $repo ($mode): $result, $writes writes, $unavailable settings unavailable."
else
  echo "result: $result writes: $writes unavailable: $unavailable exit: $code"
fi
if [ $code -eq 4 ]; then
  echo "error: $unavailable settings of $repo could not be read, so the run is incomplete. Fix: read the unavailable lines; check that the repository is public and that the gh login is an admin. $REF Verify: re-run repo.settings.sh --check and see exit 0." >&2
fi
exit $code
