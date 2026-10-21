#!/usr/bin/env bash
# @agents-index Makes the tree and stage fingerprints and reads and writes the verdict cache that pipeline.verdict.sh uses.
#
# Usage:
#   pipeline.fingerprint.sh tree                      Print the tree fingerprint of the working tree.
#   pipeline.fingerprint.sh stage <name>              Print the stage fingerprint of one stage.
#   pipeline.fingerprint.sh plan [--no-cache] <stage>...
#       Print "key: <hash>|none", then one line per stage "stage: <name> fingerprint: <hash>|none action: cached|run".
#   pipeline.fingerprint.sh record --key <hash>|none [--tree] [<stage>=<fingerprint>]...
#       Write the passed stage fingerprints, and with --tree also the tree key, to the cache.
#
# The stage scripts are the files pipeline.<stage>.sh beside this script; the repository root is two levels up.
# The cache is .agents/logs/pipeline.verdict.cache (git-ignored), at most 200 tree entries, written atomically.
# Fail closed: any error in a fingerprint or in the cache read gives action run, and one diagnostic line
# per cause on stderr. Only a positive match gives action cached. A timing line goes to stderr on plan.
# Exit codes: 0 done (also when a stage must run in full), 1 tree or stage fingerprint failed, 2 bad usage.
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIR/../.." && pwd)"
CACHE="$ROOT/.agents/logs/pipeline.verdict.cache"
HEADER="pipeline-verdict-cache 1"
MAX_TREES=200

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
diag() { echo "diagnostic: $1. The affected stages run in full. Reference: docs/specification/operations.md, section Fingerprints and the verdict cache." >&2; }

# The tree hash of HEAD plus every tracked change and every untracked file that is not ignored. A temporary
# index keeps the real index, the working tree, and the stash unchanged. The copy of the real index only
# carries its stat data, so unchanged files are not hashed again; read-tree -m resets its content to HEAD.
tree_fingerprint() {
  command -v git >/dev/null || { diag "git is missing"; return 1; }
  local tmp real
  tmp="$(mktemp "${TMPDIR:-/tmp}/verdict.index.XXXXXX")" || { diag "the temporary index failed"; return 1; }
  real="$(git -C "$ROOT" rev-parse --absolute-git-dir 2>/dev/null)/index"
  if [[ -f "$real" ]]; then cp "$real" "$tmp"; else rm -f "$tmp"; fi
  if ! (cd "$ROOT" && export GIT_INDEX_FILE="$tmp" && git read-tree -m HEAD 2>/dev/null && git add -A 2>/dev/null && git write-tree); then
    rm -f "$tmp"; diag "the temporary index failed"; return 1
  fi
  rm -f "$tmp"
}

# The hash of the sorted "<path> <object id>" entries of the declared inputs of a stage, the absent declared
# paths, the name of the stage, and its --toolchain output.
stage_fingerprint() {
  local stage="$1" tree="$2" toolchain="$3" script="$DIR/pipeline.$1.sh" inputs
  inputs="$(bash "$script" --inputs 2>/dev/null)" || { diag "pipeline.$stage.sh --inputs failed"; return 1; }
  local listing entries
  listing="$(git -C "$ROOT" ls-tree -r "$tree")" || { diag "the listing of tree $tree failed"; return 1; }
  entries="$(INPUTS="$inputs" awk '
      BEGIN { n = split(ENVIRON["INPUTS"], inp, "\n") }
      { tab = index($0, "\t"); p = substr($0, tab + 1); split(substr($0, 1, tab - 1), m, " ")
        for (i = 1; i <= n; i++) if (p == inp[i] || index(p, inp[i] "/") == 1) { print p " " m[3]; seen[i] = 1; break } }
      END { for (i = 1; i <= n; i++) if (!seen[i]) print "absent " inp[i] }' <<<"$listing" | LC_ALL=C sort)" ||
    { diag "the input entries of stage $stage failed"; return 1; }
  printf 'stage %s\n%s\ntoolchain\n%s\n' "$stage" "$entries" "$toolchain" | git -C "$ROOT" hash-object --stdin ||
    { diag "the hash of stage $stage failed"; return 1; }
}

stage_toolchain() { bash "$DIR/pipeline.$1.sh" --toolchain 2>/dev/null || { diag "pipeline.$1.sh --toolchain failed"; return 1; }; }

# Prints the cache content when it is present and valid; else a diagnostic and exit 1.
read_cache() {
  [[ -e "$CACHE" ]] || { diag "the cache is absent"; return 1; }
  [[ -r "$CACHE" ]] || { diag "the cache is unreadable"; return 1; }
  local text
  text="$(cat "$CACHE" 2>/dev/null)" || { diag "the cache is unreadable"; return 1; }
  if [[ "$(head -n 1 <<<"$text")" != "$HEADER" ]] ||
     tail -n +2 <<<"$text" | grep -qvE '^(stage [a-z]+ [0-9a-f]{40,64}|tree [0-9a-f]{40,64})$'; then
    diag "the cache is garbled"; return 1
  fi
  printf '%s\n' "$text"
}

now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }

plan() {
  local no_cache=0 start tree="" key="" cache="" ok=1 stage i
  [[ "${1:-}" == "--no-cache" ]] && { no_cache=1; shift; }
  [[ $# -gt 0 ]] || { echo "error: plan needs one or more stage names. Fix: pass the stages, for example 'plan format lint'. Verify: one line per stage." >&2; exit 2; }
  start="$(now)"
  local stages=("$@") fps=() tcs=()
  # The --toolchain calls start a process each (mise, docker), so they run in parallel to keep plan under 1 s.
  local tcdir; tcdir="$(mktemp -d "${TMPDIR:-/tmp}/verdict.toolchain.XXXXXX")" || { diag "the temporary directory failed"; tcdir=""; ok=0; }
  if [[ -n "$tcdir" ]]; then
    for i in "${!stages[@]}"; do (stage_toolchain "${stages[$i]}" >"$tcdir/$i" || rm -f "$tcdir/$i") & done
  fi
  tree="$(tree_fingerprint)" || { tree=""; ok=0; }
  wait
  for i in "${!stages[@]}"; do
    stage="${stages[$i]}"; fps[$i]=none; tcs[$i]=""
    if [[ -n "$tree" && -n "$tcdir" && -f "$tcdir/$i" ]] && tcs[$i]="$(cat "$tcdir/$i")" && fp="$(stage_fingerprint "$stage" "$tree" "${tcs[$i]}")"; then
      fps[$i]="$fp"
    else ok=0; fi
  done
  # The tree key binds the tree to the toolchain of every stage, so a tree match alone never skips.
  if [[ $ok -eq 1 ]]; then
    key="$( { echo "tree $tree"; for i in "${!stages[@]}"; do echo "toolchain ${stages[$i]}"; printf '%s\n' "${tcs[$i]}"; done; } | git -C "$ROOT" hash-object --stdin)" || key=""
  fi
  [[ -n "$tcdir" ]] && rm -rf "$tcdir"
  echo "key: ${key:-none}"
  if [[ $no_cache -eq 0 ]]; then cache="$(read_cache)" || cache=""; fi
  local all_cached=0
  [[ -n "$key" && -n "$cache" ]] && grep -qx "tree $key" <<<"$cache" && all_cached=1
  for i in "${!stages[@]}"; do
    action=run
    if [[ -n "$cache" && "${fps[$i]}" != none ]]; then
      if [[ $all_cached -eq 1 ]] || grep -qx "stage ${stages[$i]} ${fps[$i]}" <<<"$cache"; then action=cached; fi
    fi
    echo "stage: ${stages[$i]} fingerprint: ${fps[$i]} action: $action"
  done
  echo "timing: fingerprint and cache lookup seconds: $(perl -e "printf '%.3f', $(now) - $start")" >&2
}

record() {
  local key="" tree=0 pairs=() cache="" tmp
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --key) key="${2:-}"; shift 2 ;;
      --tree) tree=1; shift ;;
      *=*) pairs+=("$1"); shift ;;
      *) echo "error: unknown argument $1 for record. Fix: run --help for the form. Verify: the cache file holds the new lines." >&2; exit 2 ;;
    esac
  done
  mkdir -p "$(dirname "$CACHE")"
  # A missing or garbled cache starts empty; the write replaces it whole.
  [[ -e "$CACHE" ]] && cache="$(read_cache 2>/dev/null | tail -n +2)"
  local stage_lines tree_lines p
  stage_lines="$(grep '^stage ' <<<"$cache")"
  tree_lines="$(grep '^tree ' <<<"$cache")"
  for p in ${pairs[@]+"${pairs[@]}"}; do
    stage_lines="$(grep -v "^stage ${p%%=*} " <<<"$stage_lines"; echo "stage ${p%%=*} ${p#*=}")"
  done
  if [[ $tree -eq 1 && -n "$key" && "$key" != none ]]; then
    tree_lines="$(grep -vx "tree $key" <<<"$tree_lines"; echo "tree $key")"
  fi
  tmp="$(mktemp "$CACHE.XXXXXX")" || { echo "error: the cache write failed in $(dirname "$CACHE"). Fix: check that the directory is writable. Verify: the next verdict prints status: cached." >&2; return 1; }
  {
    echo "$HEADER"
    grep . <<<"$stage_lines" | LC_ALL=C sort
    grep . <<<"$tree_lines" | tail -n "$MAX_TREES"
  } >"$tmp" && mv -f "$tmp" "$CACHE"
}

case "${1:-}" in
  tree) tree_fingerprint ;;
  stage)
    [[ -n "${2:-}" ]] || { echo "error: stage needs a stage name. Fix: pass it, for example 'stage test'. Verify: one hash prints." >&2; exit 2; }
    t="$(tree_fingerprint)" && tc="$(stage_toolchain "$2")" && stage_fingerprint "$2" "$t" "$tc" ;;
  plan) shift; plan "$@" ;;
  record) shift; record "$@" ;;
  -h|--help) usage ;;
  *) echo "error: missing or unknown subcommand '${1:-}'. Fix: pass tree, stage, plan, or record; run --help for the forms. Verify: the command prints its result." >&2; exit 2 ;;
esac
