#!/usr/bin/env bash
# @agents-index Builds and pushes the release image of the host platform by digest, or merges the platform digests into the tagged index on ghcr.io.
#
# Usage: release.image.sh build --tag v<major>.<minor>.<patch> --digest-dir DIR [--human] [--dry-run]
#        release.image.sh merge --tag v<major>.<minor>.<patch> --digest-dir DIR [--human] [--dry-run]
#   build            Build the image of the host platform from the root Dockerfile, set the OCI labels, push
#                    it by digest (no tag), and write the digest to DIR/<os>-<arch>.
#   merge            Read one digest per platform from DIR and make one multi-platform index with the
#                    tags <version> and latest. With a digest missing, it stops and moves no tag.
#   --tag TAG        The git tag of the release, for example v1.2.3. Required. The image tag is the version
#                    with no v.
#   --digest-dir DIR The directory of the digest files. Required.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run        Run the tag check, print the commands, and run no build and no push. When the tag does
#                    not exist yet, the plan uses HEAD and says so.
#   -h, --help       Print this help.
#
# Purpose: the one place that names the image and its platforms; the workflow only logs in to the registry
# and calls this script. The Dockerfile does not change; the labels come from the command line. The label
# created is the commit time of the tag in UTC, so two builds of one tag get the same label. The caller logs
# in to ghcr.io before a real run.
#
# Output (stdout, key: value):
#   build: image: <name> platform: <os/arch> digest: <sha256:...> version: <version> revision: <commit>
#   merge: image: <name> tags: <version>,latest digest: <index digest> platforms: linux/amd64,linux/arm64
#   With --dry-run: command: <the command that a real run would run>, one line each.
# Full docker output goes to .agents/logs/release.image.log.
# Exit codes: 0 done (or planned), 1 build, push, or merge failed, 2 bad usage, 3 required tool missing,
#   4 tag check failed (tag absent, tag version differs from Cargo.toml, or HEAD is not the tag commit).
# Reference: docs/specification/release.md, section Container image.
set -euo pipefail

# The one constant of the image name; docs/specification/release.md states the same name.
IMAGE="ghcr.io/desek/agentloupe"
PLATFORMS="linux/amd64 linux/arm64"
SOURCE="https://github.com/desek/agentloupe"
TITLE="Agentloupe"
DESCRIPTION="A minimal AI gateway that gives you the illusion that you can keep up with your agents."
LICENSES="MIT"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/release.image.log"
human=0; dry_run=0; tag=""; dir=""; mode=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    build|merge) mode="$1"; shift ;;
    --tag) [[ $# -ge 2 ]] || fail "--tag needs a value. Fix: pass --tag v<major>.<minor>.<patch>. Verify: re-run with --dry-run." 2
      tag="$2"; shift 2 ;;
    --digest-dir) [[ $# -ge 2 ]] || fail "--digest-dir needs a directory. Fix: pass --digest-dir <dir>. Verify: re-run with --dry-run." 2
      dir="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the modes and flags. Verify: release.image.sh build --tag <tag> --digest-dir <dir> --dry-run prints command lines." 2 ;;
  esac
done

[[ -n "$tag" ]] || fail "--tag is required. Fix: pass --tag v<major>.<minor>.<patch>, the git tag of the release. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
if ! [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  fail "the tag $tag is not of the form v<major>.<minor>.<patch> with digits only, no leading zero, and no suffix. Fix: pass a tag such as v1.2.3. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
fi
version="${tag#v}"
# A dry run with no mode plans both modes, so one call shows the whole image step.
if [[ -z "$mode" && $dry_run -eq 0 ]]; then
  fail "a mode is required. Fix: pass build (on each platform runner) or merge (after both builds). Reference: docs/specification/release.md, section Container image. Verify: release.image.sh build --tag $tag --digest-dir <dir> --dry-run prints command lines." 2
fi
if [[ -z "$dir" ]]; then
  [[ $dry_run -eq 1 ]] || fail "--digest-dir is required. Fix: pass --digest-dir <dir>, the same directory in build and merge. Verify: re-run with --dry-run." 2
  dir="<digest-dir>"
fi

for tool in git docker; do
  command -v "$tool" >/dev/null || fail "$tool is not installed, so the image step cannot run. Fix: install it (Docker Desktop or Docker Engine with buildx). Verify: re-run release.image.sh with --dry-run." 3
done

cargo_version() { awk '/^\[/ {pkg = ($0 == "[package]")} pkg && /^version *=/ {gsub(/^version *= *"|".*$/, ""); print; exit}'; }

cd "$ROOT"
tag_commit="$(git rev-parse -q --verify "refs/tags/$tag^{commit}" 2>/dev/null || true)"
head_commit="$(git rev-parse HEAD)"
if [[ -n "$tag_commit" ]]; then
  at="tag $tag"; commit="$tag_commit"
elif [[ $dry_run -eq 1 ]]; then
  at="HEAD (tag $tag does not exist yet)"; commit="$head_commit"
else
  fail "the tag $tag does not exist in this clone. Fix: fetch the tags (git fetch --tags) and check out the tag. Reference: docs/specification/release.md, section Container image. Verify: git rev-parse $tag prints a commit." 4
fi
cargo="$(git show "$commit:Cargo.toml" | cargo_version)"
if [[ "$cargo" != "$version" ]]; then
  fail "the tag version $version differs from the version $cargo in Cargo.toml at $at, so the image step stops. Fix: tag the commit whose Cargo.toml holds $version, or pass --tag v$cargo. Reference: docs/specification/release.md, section Version. Verify: git show $tag:Cargo.toml | grep '^version' prints $version." 4
fi
if [[ -n "$tag_commit" && "$tag_commit" != "$head_commit" ]]; then
  fail "the checked-out commit $head_commit is not the commit $tag_commit of the tag $tag. Fix: git checkout $tag, then re-run. Verify: git rev-parse HEAD equals git rev-parse $tag^{commit}." 4
fi
created="$(TZ=UTC git log -1 --date=format-local:%Y-%m-%dT%H:%M:%SZ --format=%cd "$commit")"

# The labels of docs/specification/release.md, set at build time so the Dockerfile does not change.
labels=(
  --label "org.opencontainers.image.source=$SOURCE"
  --label "org.opencontainers.image.version=$version"
  --label "org.opencontainers.image.revision=$commit"
  --label "org.opencontainers.image.licenses=$LICENSES"
  --label "org.opencontainers.image.title=$TITLE"
  --label "org.opencontainers.image.description=$DESCRIPTION"
  --label "org.opencontainers.image.created=$created"
)
# The index carries the source, so the registry links the package to the repository.
annotations=(
  --annotation "index:org.opencontainers.image.source=$SOURCE"
  --annotation "index:org.opencontainers.image.version=$version"
  --annotation "index:org.opencontainers.image.revision=$commit"
  --annotation "index:org.opencontainers.image.licenses=$LICENSES"
  --annotation "index:org.opencontainers.image.title=$TITLE"
  --annotation "index:org.opencontainers.image.description=$DESCRIPTION"
  --annotation "index:org.opencontainers.image.created=$created"
)

# Prints a command as one line with each argument quoted when it holds a space.
show() { local a out=""; for a in "$@"; do [[ "$a" == *" "* ]] && a="'$a'"; out="$out $a"; done; echo "command:${out}"; }

platform_key() { echo "${1//\//-}"; }

do_build() {
  local platform meta digest
  if [[ $dry_run -eq 1 ]]; then platform="<host os/arch>"; else
    platform="$(docker version --format '{{.Server.Os}}/{{.Server.Arch}}')"
    case " $PLATFORMS " in *" $platform "*) ;; *)
      fail "the Docker host platform $platform is not one of $PLATFORMS. Fix: run the build on a Linux x86_64 or a Linux arm64 runner. Verify: docker version --format '{{.Server.Os}}/{{.Server.Arch}}'." 1 ;;
    esac
  fi
  meta="${TMPDIR:-/tmp}/release.image.meta.$$.json"
  local cmd=(docker buildx build --platform "$platform" "${labels[@]}"
    --output "type=image,name=$IMAGE,push-by-digest=true,name-canonical=true,push=true"
    --metadata-file "$meta" --file Dockerfile .)
  if [[ $dry_run -eq 1 ]]; then show "${cmd[@]}"; echo "command: write the digest to $dir/<os>-<arch>"; return 0; fi
  mkdir -p "$(dirname "$LOG")" "$dir"
  if ! "${cmd[@]}" >>"$LOG" 2>&1; then
    tail -n 30 "$LOG" >&2
    fail "the image build or push for $platform failed. Fix: read .agents/logs/release.image.log; check the registry login (packages: write) and that pipeline.image.sh passes on the tag commit. Verify: re-run release.image.sh build --tag $tag --digest-dir $dir." 1
  fi
  digest="$(tr -d ' \n' <"$meta" | sed -n 's/.*"containerimage.digest":"\(sha256:[0-9a-f]*\)".*/\1/p')"
  rm -f "$meta"
  [[ -n "$digest" ]] || fail "the build wrote no image digest. Fix: update Docker buildx so it writes containerimage.digest to the metadata file. Verify: re-run the build." 1
  echo "$digest" >"$dir/$(platform_key "$platform")"
  if [[ $human -eq 1 ]]; then echo "Pushed $IMAGE@$digest for $platform ($version)."; else
    echo "build: image: $IMAGE platform: $platform digest: $digest version: $version revision: $commit"; fi
}

do_merge() {
  local p f sources=() digest
  for p in $PLATFORMS; do
    f="$dir/$(platform_key "$p")"
    if [[ $dry_run -eq 1 ]]; then sources+=("$IMAGE@<digest of $p>"); continue; fi
    [[ -s "$f" ]] || fail "the digest of $p is missing ($f), so the index is not made and the tag latest does not move. Fix: run release.image.sh build on the $p runner and pass its digest file to the merge. Verify: ls $dir lists one file per platform." 1
    sources+=("$IMAGE@$(tr -d ' \n' <"$f")")
  done
  local cmd=(docker buildx imagetools create "${annotations[@]}" --tag "$IMAGE:$version" --tag "$IMAGE:latest" "${sources[@]}")
  if [[ $dry_run -eq 1 ]]; then show "${cmd[@]}"; return 0; fi
  mkdir -p "$(dirname "$LOG")"
  if ! "${cmd[@]}" >>"$LOG" 2>&1; then
    tail -n 30 "$LOG" >&2
    fail "the merge of the platform images failed. Fix: read .agents/logs/release.image.log and check the registry login. Verify: docker buildx imagetools inspect $IMAGE:$version lists both platforms." 1
  fi
  digest="$(docker buildx imagetools inspect "$IMAGE:$version" 2>>"$LOG" | awk '$1 == "Digest:" {print $2; exit}')"
  if [[ $human -eq 1 ]]; then echo "Tagged $IMAGE:$version and latest at $digest for ${PLATFORMS// /, }."; else
    echo "merge: image: $IMAGE tags: $version,latest digest: $digest platforms: ${PLATFORMS// /,}"; fi
}

[[ $dry_run -eq 1 ]] && echo "plan: tag $tag version: $version check: passed at: $at image: $IMAGE created: $created"
case "$mode" in
  build) do_build ;;
  merge) do_merge ;;
  "") do_build; do_merge ;;
esac
