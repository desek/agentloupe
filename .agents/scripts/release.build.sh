#!/usr/bin/env bash
# @agents-index Builds the release asset of the host target: loupe-v<version>-<target>.tar.gz and its .sha256 file, after the tag check.
#
# Usage: release.build.sh --tag v<major>.<minor>.<patch> [--out DIR] [--human] [--dry-run]
#   --tag TAG    The git tag of the release, for example v1.2.3. Required. The version is the tag without the v.
#   --out DIR    The directory for the archive and its checksum file. Default: target/release-assets.
#   --human      Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run    Run the tag check, print the plan, and build nothing. When the tag does not exist yet,
#                the plan checks the version against Cargo.toml at HEAD and says so.
#   -h, --help   Print this help.
#
# Purpose: one release asset per host, built on a native runner with no cross tool. The target is the host
# target of the toolchain that rust-toolchain.toml pins; this script states no version and no toolchain.
# Before the build it stops when the tag version differs from the version in Cargo.toml at the tag, or when
# the checked-out commit is not the tag commit. The archive holds one directory loupe-v<version>-<target>/
# with loupe (execute bit kept), upstreams.toml, config/, LICENSE, README.md, and THIRD-PARTY-NOTICES, which
# release.notices.sh writes before the build (it needs cargo about of .tool-versions). The checksum file holds
# one line in the format of sha256sum, which shasum -a 256 -c and sha256sum -c accept.
#
# Telemetry: the environment value LOUPE_TELEMETRY_HOST (the repository variable in the release workflow)
# and LOUPE_BUILD_COMMIT (default: the tag commit) pass to the build. With a host the binary is built-in and
# sends the start request; with no host or an empty host it is built-out, and the release does not fail.
#
# Output (stdout, key: value), one line per asset:
#   asset: <archive> archive_bytes: <n> binary_bytes: <n> sha256: <hex> checksum_file: <archive>.sha256 dir: <out>
#     telemetry: built-in host: <host> | telemetry: built-out
# With --dry-run: plan lines (plan: tag, plan: notices, plan: build, plan: telemetry, plan: archive, plan: checksum).
# Full cargo output goes to .agents/logs/release.build.log.
# Exit codes: 0 built (or planned), 1 notices, build, or archive failed, 2 bad usage, 3 required tool missing,
#   4 tag check failed (tag absent, tag version differs from Cargo.toml, or HEAD is not the tag commit).
# Reference: docs/specification/release.md, section Release assets.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/release.build.log"
human=0; dry_run=0; tag=""; out="$ROOT/target/release-assets"
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag) [[ $# -ge 2 ]] || fail "--tag needs a value. Fix: pass --tag v<major>.<minor>.<patch>, for example the tag of the release. Verify: release.build.sh --tag <tag> --dry-run prints plan lines." 2
      tag="$2"; shift 2 ;;
    --out) [[ $# -ge 2 ]] || fail "--out needs a directory. Fix: pass --out <dir> or omit it for target/release-assets. Verify: re-run with --dry-run." 2
      out="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the flags. Verify: release.build.sh --tag <tag> --dry-run prints plan lines." 2 ;;
  esac
done

[[ -n "$tag" ]] || fail "--tag is required. Fix: pass --tag v<major>.<minor>.<patch>, the git tag of the release. Reference: docs/specification/release.md, section Tag and image tag. Verify: release.build.sh --tag <tag> --dry-run prints plan lines." 2
if ! [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  fail "the tag $tag is not of the form v<major>.<minor>.<patch> with digits only, no leading zero, and no suffix. Fix: pass a tag such as v1.2.3. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
fi
version="${tag#v}"

for tool in git cargo rustc tar; do
  command -v "$tool" >/dev/null || fail "$tool is not installed, so the release build cannot run. Fix: install it (rustup for cargo and rustc, the system package for git and tar). Verify: re-run release.build.sh --dry-run." 3
done
if command -v sha256sum >/dev/null; then SHA=(sha256sum); elif command -v shasum >/dev/null; then SHA=(shasum -a 256); else
  fail "neither sha256sum nor shasum is installed, so the checksum file cannot be made. Fix: install coreutils (Linux) or use the shasum of macOS. Verify: re-run release.build.sh --dry-run." 3
fi

# The version in the [package] table of a Cargo.toml text on stdin; the first version line of that table.
cargo_version() { awk '/^\[/ {pkg = ($0 == "[package]")} pkg && /^version *=/ {gsub(/^version *= *"|".*$/, ""); print; exit}'; }

# The tag check runs before any build; a release from the wrong commit or version must not exist.
cd "$ROOT"
tag_commit="$(git rev-parse -q --verify "refs/tags/$tag^{commit}" 2>/dev/null || true)"
head_commit="$(git rev-parse HEAD)"
if [[ -n "$tag_commit" ]]; then
  at="tag $tag"; cargo="$(git show "$tag_commit:Cargo.toml" | cargo_version)"
elif [[ $dry_run -eq 1 ]]; then
  at="HEAD (tag $tag does not exist yet)"; cargo="$(git show "HEAD:Cargo.toml" | cargo_version)"
else
  fail "the tag $tag does not exist in this clone, so the build has no tag commit. Fix: fetch the tags (git fetch --tags) and check out the tag, or pass the tag of an existing release. Reference: docs/specification/release.md, section Release assets. Verify: git rev-parse $tag prints a commit." 4
fi
if [[ "$cargo" != "$version" ]]; then
  fail "the tag version $version differs from the version $cargo in Cargo.toml at $at, so the build stops. Fix: tag the commit whose Cargo.toml holds $version (the merge commit of the release pull request), or pass --tag v$cargo. Reference: docs/specification/release.md, section Version. Verify: git show $tag:Cargo.toml | grep '^version' prints $version." 4
fi
if [[ -n "$tag_commit" && "$tag_commit" != "$head_commit" ]]; then
  fail "the checked-out commit $head_commit is not the commit $tag_commit of the tag $tag, so the build stops. Fix: git checkout $tag, then re-run. Reference: docs/specification/release.md, section Release assets. Verify: git rev-parse HEAD equals git rev-parse $tag^{commit}." 4
fi

# The host target of the pinned toolchain: rustup reads rust-toolchain.toml in the root.
target="$(rustc -vV | sed -n 's/^host: //p')"
[[ -n "$target" ]] || fail "rustc -vV printed no host target. Fix: run rustup show in the repository so the pinned toolchain installs. Verify: rustc -vV prints a host line." 3
name="loupe-v$version-$target"
archive="$name.tar.gz"
# The telemetry values of the build; white space at the start and the end does not count.
telemetry_host="${LOUPE_TELEMETRY_HOST:-}"
telemetry_host="${telemetry_host#"${telemetry_host%%[![:space:]]*}"}"
telemetry_host="${telemetry_host%"${telemetry_host##*[![:space:]]}"}"
build_commit="${LOUPE_BUILD_COMMIT:-${tag_commit:-$head_commit}}"
if [[ -n "$telemetry_host" ]]; then telemetry="built-in host: $telemetry_host"; else telemetry="built-out"; fi
build=(env "LOUPE_TELEMETRY_HOST=$telemetry_host" "LOUPE_BUILD_COMMIT=$build_commit" cargo build --release --locked --bin loupe)
notices=("$ROOT/.agents/scripts/release.notices.sh")
files="loupe upstreams.toml config/ LICENSE README.md THIRD-PARTY-NOTICES"

if [[ $dry_run -eq 1 ]]; then
  if [[ $human -eq 1 ]]; then
    echo "Plan for $tag on $target: check passed at $at; write THIRD-PARTY-NOTICES with release.notices.sh; run '${build[*]}' (telemetry $telemetry); pack $archive with $files; write $archive.sha256 into $out. Nothing was built."
  else
    echo "plan: tag $tag version: $version check: passed at: $at"
    echo "plan: notices command: .agents/scripts/release.notices.sh file: THIRD-PARTY-NOTICES"
    echo "plan: build command: ${build[*]} target: $target"
    echo "plan: telemetry state: $telemetry"
    echo "plan: archive name: $archive files: $files dir: $out"
    echo "plan: checksum name: $archive.sha256 format: sha256sum"
  fi
  exit 0
fi

mkdir -p "$(dirname "$LOG")" "$out"
# The notices come first: a crate with a license outside the accepted list stops the release before the build.
if ! "${notices[@]}" >"$LOG" 2>&1; then
  tail -n 10 "$LOG" >&2
  fail "release.notices.sh failed, so the archive would lack THIRD-PARTY-NOTICES. Fix: follow the error of release.notices.sh above (install cargo about from .tool-versions, or settle the license). Reference: docs/specification/release.md, section Third-party notices. Verify: .agents/scripts/release.notices.sh exits 0." 1
fi
if ! "${build[@]}" >>"$LOG" 2>&1; then
  tail -n 30 "$LOG" >&2
  fail "cargo build failed for $target. Fix: read .agents/logs/release.build.log and correct the code; the same commit must pass pipeline.verdict.sh. Verify: re-run release.build.sh --tag $tag." 1
fi
bin="${CARGO_TARGET_DIR:-$ROOT/target}/release/loupe"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$name"
cp "$bin" "$stage/$name/loupe"
chmod 0755 "$stage/$name/loupe"
cp -R upstreams.toml config LICENSE README.md THIRD-PARTY-NOTICES "$stage/$name/"
export COPYFILE_DISABLE=1  # no AppleDouble files of macOS in the archive
# Fixed owner and order keep the archive free of the user name of the runner.
if tar --version 2>/dev/null | grep -q GNU; then
  owner=(--owner=0 --group=0 --numeric-owner --sort=name)
else
  owner=(--uid 0 --gid 0)
fi
tar -C "$stage" "${owner[@]}" -czf "$out/$archive" "$name" || fail "tar could not write $out/$archive. Fix: check that $out is writable and has space. Verify: re-run release.build.sh --tag $tag." 1
(cd "$out" && "${SHA[@]}" "$archive" | awk '{print $1 "  " $2}' >"$archive.sha256")
(cd "$out" && "${SHA[@]}" -c "$archive.sha256" >/dev/null) || fail "the checksum file $archive.sha256 does not verify. Fix: re-run the build into an empty --out directory. Verify: shasum -a 256 -c $archive.sha256 prints OK." 1

archive_bytes="$(wc -c <"$out/$archive" | tr -d ' ')"
binary_bytes="$(wc -c <"$bin" | tr -d ' ')"
digest="$(awk '{print $1}' "$out/$archive.sha256")"
if [[ $human -eq 1 ]]; then
  echo "Built $archive ($archive_bytes bytes, binary $binary_bytes bytes), SHA-256 $digest, in $out; telemetry $telemetry."
else
  echo "asset: $archive archive_bytes: $archive_bytes binary_bytes: $binary_bytes sha256: $digest checksum_file: $archive.sha256 dir: $out telemetry: $telemetry"
fi
