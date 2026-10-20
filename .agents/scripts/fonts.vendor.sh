#!/usr/bin/env bash
# @agents-index Vendors the Geist and Geist Mono variable woff2 files and OFL.txt of one release tag; writes SOURCE.md with hashes.
#
# Usage: fonts.vendor.sh [--tag TAG] [--dry-run] [--check] [--human]
#   --tag TAG    Release tag of vercel/geist-font to fetch. Default: v1.7.2.
#   --dry-run    Print the URLs and the target files; fetch and change nothing.
#   --check      Compare the SHA-256 of the files on disk with SOURCE.md; no network.
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#
# Output (stdout, key: value): one line per file with file, sha256, status; then mode and status.
# The verdict never calls the fetch; it only uses the committed files. --check is hermetic.
# Exit codes: 0 done or check passed, 1 fetch failed or hash mismatch, 2 bad usage, 3 tool missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/web/assets/fonts"
REPO=vercel/geist-font
tag=v1.7.2; mode=fetch; human=0
[[ -t 1 ]] && human=1
# local name -> upstream path at the tag
FILES=(
  "Geist-Variable.woff2=fonts/Geist/webfonts/Geist[wght].woff2"
  "GeistMono-Variable.woff2=fonts/GeistMono/webfonts/GeistMono[wght].woff2"
  "OFL.txt=OFL.txt"
)

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag) [[ $# -ge 2 ]] || { echo "error: --tag needs a value. Fix: pass --tag v1.7.2. Verify: the script prints status: passed." >&2; exit 2; }; tag="$2"; shift 2 ;;
    --dry-run) mode=dry-run; shift ;;
    --check) mode=check; shift ;;
    --human) human=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument $1. Fix: run --help for the flags. Verify: the script prints status: passed." >&2; exit 2 ;;
  esac
done

sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
say() { if [[ $human -eq 1 ]]; then echo "$2"; else echo "$1"; fi; }

if [[ $mode == check ]]; then
  [[ -f "$DIR/SOURCE.md" ]] || { echo "error: web/assets/fonts/SOURCE.md is missing, so the hashes cannot be checked. Fix: run .agents/scripts/fonts.vendor.sh to fetch the fonts. Reference: docs/specification/dashboard.md, section Fonts. Verify: fonts.vendor.sh --check prints status: passed." >&2; exit 1; }
  bad=0
  for pair in "${FILES[@]}"; do
    f="${pair%%=*}"
    want="$(grep -F "| \`$f\` |" "$DIR/SOURCE.md" | awk -F'|' '{gsub(/[ `]/,"",$4); print $4}')"
    have="$( [[ -f "$DIR/$f" ]] && sha "$DIR/$f" || echo missing)"
    st=passed; [[ "$want" == "$have" ]] || { st=failed; bad=1; }
    say "file: $f sha256: $have status: $st" "$f: $st"
  done
  if [[ $bad -eq 1 ]]; then
    echo "error: a font file differs from SOURCE.md. Fix 1: run .agents/scripts/fonts.vendor.sh --tag <tag of SOURCE.md> to fetch the files again. Fix 2: if the change is wanted, run the fetch for the new tag so SOURCE.md is written again. Reference: docs/specification/dashboard.md, section Fonts. Verify: fonts.vendor.sh --check prints status: passed." >&2
    say "mode: check status: failed" "The check failed."; exit 1
  fi
  say "mode: check status: passed" "The check passed."; exit 0
fi

command -v curl >/dev/null || { echo "error: curl is not installed, so the fonts cannot be fetched. Fix: install curl. Verify: re-run and see status: passed." >&2; exit 3; }
base="https://raw.githubusercontent.com/$REPO/$tag"
if [[ $mode == dry-run ]]; then
  for pair in "${FILES[@]}"; do
    say "file: ${pair%%=*} url: $base/${pair#*=} status: planned" "Would fetch $base/${pair#*=} to web/assets/fonts/${pair%%=*}"
  done
  say "mode: dry-run status: passed" "Dry run: no file changed."; exit 0
fi

work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
for pair in "${FILES[@]}"; do
  f="${pair%%=*}"; p="${pair#*=}"
  url="$base/$(printf '%s' "$p" | sed 's/\[/%5B/g; s/\]/%5D/g')"
  curl -fsSL -o "$work/$f" "$url" || { echo "error: the fetch of $url failed. Fix 1: check the network. Fix 2: check that the tag $tag exists in $REPO. Reference: docs/specification/dashboard.md, section Fonts. Verify: re-run and see status: passed." >&2; exit 1; }
done
grep -q "SIL OPEN FONT LICENSE" "$work/OFL.txt" || { echo "error: OFL.txt of $tag does not hold the SIL Open Font License, so the copy is not confirmed. Fix: stop and ask the owner before you vendor the fonts. Verify: the license text names the SIL Open Font License." >&2; exit 1; }
mkdir -p "$DIR"
{
  echo "<!-- @agents-index Source record of the vendored Geist fonts: upstream repository, release tag, fetch date, and SHA-256 of each file. -->"
  echo "# Font source"
  echo
  echo "| Field | Value |"
  echo "|---|---|"
  echo "| Repository | \`https://github.com/$REPO\` |"
  echo "| Tag | \`$tag\` |"
  echo "| Fetched | \`$(date -u +%Y-%m-%d)\` |"
  echo "| License | SIL Open Font License 1.1, see \`OFL.txt\` |"
  echo
  echo "| File | Upstream path | SHA-256 |"
  echo "|---|---|---|"
  for pair in "${FILES[@]}"; do
    f="${pair%%=*}"; cp "$work/$f" "$DIR/$f"
    echo "| \`$f\` | \`${pair#*=}\` | \`$(sha "$DIR/$f")\` |"
  done
} >"$DIR/SOURCE.md"
for pair in "${FILES[@]}"; do f="${pair%%=*}"; say "file: $f sha256: $(sha "$DIR/$f") status: fetched" "Fetched $f"; done
say "mode: fetch status: passed" "The fonts of $tag are in web/assets/fonts/."
