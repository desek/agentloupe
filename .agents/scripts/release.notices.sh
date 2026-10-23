#!/usr/bin/env bash
# @agents-index Writes THIRD-PARTY-NOTICES with cargo about: a header, the crate licenses of the loupe binary, the LiteLLM license, and the Geist font license.
#
# Usage: release.notices.sh [--out FILE] [--human] [--dry-run]
#   --out FILE   The file to write. Default: THIRD-PARTY-NOTICES at the root of the tree.
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run    Print the plan (the parts, the commands, the output path) and write nothing.
#   -h, --help   Print this help.
#
# Purpose: the third-party notices of docs/specification/release.md (Third-party notices). The licenses of
# the embedded crates, of the LiteLLM price file, and of the Geist fonts require their notices to travel
# with each distributed copy, so release.build.sh puts the file into each archive and release.image.sh into
# the release image. The file holds, in this order: a header with the project and the version of
# Cargo.toml; the crate part that cargo about renders from about.toml and about.hbs (the union of the three
# release targets, build dependencies in, dev dependencies out, each license text once with its crates);
# vendor/litellm/LICENSE with the upstream commit of vendor/litellm/SOURCE.md; web/assets/fonts/OFL.txt
# with the font names and the tag of web/assets/fonts/SOURCE.md. Two runs on one commit give byte-identical
# files: no time stamp, no host name, no path of the build machine. The crates are fetched with
# cargo fetch --locked, then cargo about reads only local files (--frozen). Git ignores the file.
# Idempotent: a run writes the same bytes again; the file is replaced atomically.
#
# Output (stdout, key: value):
#   notices: <path relative to the root or absolute> version: <v> bytes: <n> sha256: <hex> crates: <n> licenses: <n>
#   With --dry-run: plan lines (plan: header, plan: crates, plan: litellm, plan: fonts, plan: output).
# Exit codes: 0 written (or planned), 1 a crate has a license outside the accepted list of about.toml, or
#   vendor/litellm/LICENSE or web/assets/fonts/OFL.txt is missing, or cargo about failed, 2 bad usage,
#   3 cargo or cargo about is missing.
# Reference: docs/specification/release.md, section Third-party notices.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/release.notices.log"
TARGETS_NOTE="aarch64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu"
FONT_NAMES="Geist and Geist Mono"
# The section titles are the markers that release.check.sh looks for.
LITELLM_TITLE="Part 2: LiteLLM model price data"
FONTS_TITLE="Part 3: Geist fonts"
human=0; dry_run=0; out="$ROOT/THIRD-PARTY-NOTICES"
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) [[ $# -ge 2 ]] || fail "--out needs a file. Fix: pass --out <file> or omit it for THIRD-PARTY-NOTICES at the root. Verify: release.notices.sh --dry-run prints plan lines." 2
      out="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the flags. Verify: release.notices.sh --dry-run prints plan lines." 2 ;;
  esac
done

cd "$ROOT"
for f in vendor/litellm/LICENSE vendor/litellm/SOURCE.md web/assets/fonts/OFL.txt web/assets/fonts/SOURCE.md about.toml about.hbs; do
  [[ -s "$f" ]] || fail "$f is missing or empty, so the notices would be incomplete. Fix: restore it from git (git checkout -- $f); refresh vendored files only with prices.vendor.sh or fonts.vendor.sh. Reference: docs/specification/release.md, section Third-party notices. Verify: git ls-files $f prints the path." 1
done
command -v cargo >/dev/null || fail "cargo is not installed, so the crates cannot be listed. Fix: install Rust with rustup. Verify: cargo --version." 3
if ! about_version="$(cargo about --version 2>/dev/null)"; then
  fail "cargo about is not installed, so the crate licenses cannot be rendered. Fix: mise install (it reads the cargo-about pin of .tool-versions), or install the pinned version from .tool-versions by hand. Reference: docs/specification/release.md, section Third-party notices. Verify: cargo about --version prints the pinned version." 3
fi

version="$(awk '/^\[/ {pkg = ($0 == "[package]")} pkg && /^version *=/ {gsub(/^version *= *"|".*$/, ""); print; exit}' Cargo.toml)"
litellm_commit="$(sed -n 's/^| Commit | `\([0-9a-f]*\)` |$/\1/p' vendor/litellm/SOURCE.md)"
fonts_tag="$(sed -n 's/^| Tag | `\(.*\)` |$/\1/p' web/assets/fonts/SOURCE.md)"
[[ -n "$litellm_commit" ]] || fail "vendor/litellm/SOURCE.md names no upstream commit in its Commit row. Fix: refresh it with .agents/scripts/prices.vendor.sh. Verify: grep '| Commit |' vendor/litellm/SOURCE.md." 1
[[ -n "$fonts_tag" ]] || fail "web/assets/fonts/SOURCE.md names no tag in its Tag row. Fix: refresh it with .agents/scripts/fonts.vendor.sh. Verify: grep '| Tag |' web/assets/fonts/SOURCE.md." 1
show_out="${out#"$ROOT"/}"

if [[ $dry_run -eq 1 ]]; then
  if [[ $human -eq 1 ]]; then
    echo "Would write $show_out for agent-loupe $version: the crate licenses by $about_version (about.toml, about.hbs), the LiteLLM license at $litellm_commit, and the OFL of $FONT_NAMES $fonts_tag. Nothing was written."
  else
    echo "plan: header project: agent-loupe version: $version"
    echo "plan: crates command: cargo fetch --locked; cargo about generate --frozen about.hbs tool: $about_version"
    echo "plan: litellm license: vendor/litellm/LICENSE commit: $litellm_commit"
    echo "plan: fonts license: web/assets/fonts/OFL.txt fonts: $FONT_NAMES tag: $fonts_tag"
    echo "plan: output path: $show_out"
  fi
  exit 0
fi

mkdir -p "$(dirname "$LOG")" "$(dirname "$out")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/release.notices.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
: >"$LOG"
if ! cargo fetch --locked >>"$LOG" 2>&1; then
  tail -n 20 "$LOG" >&2
  fail "cargo fetch --locked failed, so the crate sources with their license files are not local. Fix: check the network to crates.io and that Cargo.lock is current (cargo metadata --locked). Verify: re-run release.notices.sh." 1
fi
# NO_COLOR and a fixed locale keep the output and the error text plain and stable.
if ! NO_COLOR=1 LC_ALL=C cargo about generate --frozen --config about.toml about.hbs -o "$tmp/crates.txt" >>"$LOG" 2>&1; then
  # Each refused crate shows as the path of its Cargo.toml and its license line; name both, no path.
  refused="$(awk '/registry\/src\/.*\/Cargo\.toml/ {n = $0; sub(/\/Cargo\.toml.*/, "", n); sub(/.*\//, "", n); next}
                  n != "" && /license *=/ {l = $0; sub(/.*license *= *"/, "", l); sub(/".*/, "", l); print n " (" l ")"; n = ""}' "$LOG" | sort -u | paste -sd ';' - | sed 's/;/; /g')"
  if [[ -n "$refused" ]]; then
    fail "a crate has a license outside the accepted list of about.toml: $refused. Fix: if the license is permissive, add it to accepted in about.toml and to allow in deny.toml; if it is copyleft, stop and ask the owner. Reference: docs/specification/operations.md, section Dependency check. Verify: re-run release.notices.sh and .agents/scripts/deps.deny.sh." 1
  fi
  tail -n 20 "$LOG" >&2
  fail "cargo about failed. Fix: read .agents/logs/release.notices.log. Verify: re-run release.notices.sh." 1
fi

crates="$(grep -c '^  [^ ].* [0-9][^ ]*\( <.*>\)\{0,1\}$' "$tmp/crates.txt" || true)"
licenses="$(grep -c '^License: ' "$tmp/crates.txt" || true)"
rule="================================================================================"
{
  echo "THIRD-PARTY-NOTICES"
  echo
  echo "Agentloupe (agent-loupe) version $version"
  echo "https://github.com/desek/agentloupe"
  echo
  echo "Agentloupe itself is under the MIT license, see LICENSE. The loupe binary"
  echo "embeds third-party parts under their own licenses. This file gives their"
  echo "notices: the Rust crates of the binary on the release targets"
  echo "$TARGETS_NOTE,"
  echo "the LiteLLM model price data, and the Geist fonts."
  echo
  cat "$tmp/crates.txt"
  echo "$rule"
  echo "$LITELLM_TITLE"
  echo "$rule"
  echo
  echo "The binary embeds the model price data of LiteLLM (model_prices_and_context_window.json"
  echo "and its JSON schema), from https://github.com/BerriAI/litellm at the commit"
  echo "$litellm_commit."
  echo
  cat vendor/litellm/LICENSE
  echo
  echo "$rule"
  echo "$FONTS_TITLE"
  echo "$rule"
  echo
  echo "The dashboard embeds the fonts $FONT_NAMES, from https://github.com/vercel/geist-font"
  echo "at the tag $fonts_tag, under the SIL Open Font License 1.1."
  echo
  cat web/assets/fonts/OFL.txt
} >"$tmp/notices"
mv -f "$tmp/notices" "$out"

if command -v sha256sum >/dev/null; then digest="$(sha256sum "$out" | awk '{print $1}')"; else digest="$(shasum -a 256 "$out" | awk '{print $1}')"; fi
bytes="$(wc -c <"$out" | tr -d ' ')"
if [[ $human -eq 1 ]]; then
  echo "Wrote $show_out for agent-loupe $version: $crates crates under $licenses license texts, the LiteLLM license, and the Geist font license ($bytes bytes, SHA-256 $digest)."
else
  echo "notices: $show_out version: $version bytes: $bytes sha256: $digest crates: $crates licenses: $licenses"
fi
