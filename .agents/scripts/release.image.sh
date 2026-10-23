#!/usr/bin/env bash
# @agents-index Builds, checks, and pushes the release image of the host platform by digest with its SBOM, merges the digests into the version index, or promotes latest.
#
# Usage: release.image.sh build --tag v<major>.<minor>.<patch> --digest-dir DIR --sbom-dir DIR [--human] [--dry-run]
#        release.image.sh merge --tag v<major>.<minor>.<patch> --digest-dir DIR [--human] [--dry-run]
#        release.image.sh promote --tag v<major>.<minor>.<patch> [--human] [--dry-run]
#        release.image.sh latest [--human]
#   build            Write THIRD-PARTY-NOTICES with release.notices.sh, build the image of the host platform
#                    from the root Dockerfile with the OCI labels and with the BuildKit provenance and SBOM
#                    attestations off, check that it holds /app/LICENSE and /app/THIRD-PARTY-NOTICES, push it
#                    by digest (no tag), write the digest to DIR/<os>-<arch>, then write the image SBOM file
#                    and its .sha256 file with release.sbom.sh image into the SBOM directory. A missing file
#                    stops the step before the push; a failed SBOM step fails the build.
#   merge            Read one digest per platform from DIR and make one multi-platform index with the tag
#                    <version> only. With a digest missing, it stops and tags nothing. latest moves in promote.
#   promote          Move the tag latest to the index of the tag <version>, only forward: a version older than
#                    the version that latest points to is refused with exit code 1, and latest stays. It needs
#                    no checkout of the tag commit (the advisory response runs from the default branch), but
#                    the tag must exist and the version in Cargo.toml at the tag must equal the tag version.
#   latest           Read-only: print the version and the index digest that the tag latest points to, or none.
#                    The advisory response reads it to plan a move; it needs no tag and no login.
#   --tag TAG        The git tag of the release, for example v1.2.3. Required except in latest. The image tag is the version
#                    with no v.
#   --digest-dir DIR The directory of the digest files. Required in build and merge.
#   --sbom-dir DIR   The directory of the image SBOM file and its checksum file. Required in build.
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
# Telemetry: the environment value LOUPE_TELEMETRY_HOST (the repository variable in the release workflow)
# and LOUPE_BUILD_COMMIT (default: the tag commit) pass to the image build as build arguments. With a host
# the image is built-in; with no host it is built-out, and the release does not fail.
#
# Output (stdout, key: value):
#   build: image: <name> platform: <os/arch> digest: <sha256:...> version: <version> revision: <commit>
#     telemetry: built-in host: <host> | telemetry: built-out
#   build, then: sbom: <file> kind: image components: <n> sha256: <hex> checksum_file: <file>.sha256 dir: <sbom dir>
#   merge: image: <name> tags: <version> digest: <index digest> platforms: linux/amd64,linux/arm64
#   promote: image: <name> latest: <version> digest: <index digest> from: <old version|none> status: moved|unchanged
#   latest: image: <name> version: <version|none> digest: <index digest|none>
#   With --dry-run: command: <the command that a real run would run>, one line each.
# Full docker output goes to .agents/logs/release.image.log.
# Exit codes: 0 done (or planned), 1 notices, build, file check, push, SBOM, merge, or promote failed, or a
#   backward move of latest was refused, 2 bad usage,
#   3 required tool missing,
#   4 tag check failed (tag absent, tag version differs from Cargo.toml, or HEAD is not the tag commit; promote
#   does not check HEAD).
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
human=0; dry_run=0; tag=""; dir=""; sbom_dir=""; mode=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    build|merge|promote|latest) mode="$1"; shift ;;
    --tag) [[ $# -ge 2 ]] || fail "--tag needs a value. Fix: pass --tag v<major>.<minor>.<patch>. Verify: re-run with --dry-run." 2
      tag="$2"; shift 2 ;;
    --digest-dir) [[ $# -ge 2 ]] || fail "--digest-dir needs a directory. Fix: pass --digest-dir <dir>. Verify: re-run with --dry-run." 2
      dir="$2"; shift 2 ;;
    --sbom-dir) [[ $# -ge 2 ]] || fail "--sbom-dir needs a directory. Fix: pass --sbom-dir <dir>. Verify: re-run with --dry-run." 2
      sbom_dir="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the modes and flags. Verify: release.image.sh build --tag <tag> --digest-dir <dir> --dry-run prints command lines." 2 ;;
  esac
done

# The mode latest only reads the registry: no tag, no checkout check.
if [[ "$mode" == latest ]]; then
  command -v docker >/dev/null || fail "docker is not installed, so the tag latest cannot be read. Fix: install Docker with buildx. Verify: docker buildx version." 3
  v="$(docker buildx imagetools inspect --raw "$IMAGE:latest" 2>/dev/null | tr -d ' \n' | sed -n 's/.*"org.opencontainers.image.version":"\([^"]*\)".*/\1/p' | head -n 1 || true)"
  d="$(docker buildx imagetools inspect "$IMAGE:latest" 2>/dev/null | awk '$1 == "Digest:" {print $2; exit}' || true)"
  if [[ $human -eq 1 ]]; then echo "latest of $IMAGE is ${v:-none} (${d:-no digest})."; else
    echo "latest: image: $IMAGE version: ${v:-none} digest: ${d:-none}"; fi
  exit 0
fi
[[ -n "$tag" ]] || fail "--tag is required. Fix: pass --tag v<major>.<minor>.<patch>, the git tag of the release. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
if ! [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  fail "the tag $tag is not of the form v<major>.<minor>.<patch> with digits only, no leading zero, and no suffix. Fix: pass a tag such as v1.2.3. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
fi
version="${tag#v}"
# A dry run with no mode plans both modes, so one call shows the whole image step.
if [[ -z "$mode" && $dry_run -eq 0 ]]; then
  fail "a mode is required. Fix: pass build (on each platform runner), merge (after both builds), or promote (after the merge). Reference: docs/specification/release.md, section Container image. Verify: release.image.sh build --tag $tag --digest-dir <dir> --dry-run prints command lines." 2
fi
if [[ -z "$dir" && "$mode" != promote ]]; then
  [[ $dry_run -eq 1 ]] || fail "--digest-dir is required. Fix: pass --digest-dir <dir>, the same directory in build and merge. Verify: re-run with --dry-run." 2
  dir="<digest-dir>"
fi
if [[ -z "$sbom_dir" && ( "$mode" == build || -z "$mode" ) ]]; then
  [[ $dry_run -eq 1 ]] || fail "--sbom-dir is required in build. Fix: pass --sbom-dir <dir>, the directory that the workflow uploads with the image SBOM files. Reference: docs/specification/release.md, section Container image. Verify: re-run with --dry-run." 2
  sbom_dir="<sbom-dir>"
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
# promote moves only a tag of the registry, so the checkout can be the default branch.
if [[ "$mode" != promote && -n "$tag_commit" && "$tag_commit" != "$head_commit" ]]; then
  fail "the checked-out commit $head_commit is not the commit $tag_commit of the tag $tag. Fix: git checkout $tag, then re-run. Verify: git rev-parse HEAD equals git rev-parse $tag^{commit}." 4
fi
created="$(TZ=UTC git log -1 --date=format-local:%Y-%m-%dT%H:%M:%SZ --format=%cd "$commit")"

# The telemetry values of the image build; white space at the start and the end does not count.
telemetry_host="${LOUPE_TELEMETRY_HOST:-}"
telemetry_host="${telemetry_host#"${telemetry_host%%[![:space:]]*}"}"
telemetry_host="${telemetry_host%"${telemetry_host##*[![:space:]]}"}"
build_commit="${LOUPE_BUILD_COMMIT:-$commit}"
if [[ -n "$telemetry_host" ]]; then telemetry="built-in host: $telemetry_host"; else telemetry="built-out"; fi
build_args=(--build-arg "LOUPE_TELEMETRY_HOST=$telemetry_host" --build-arg "LOUPE_BUILD_COMMIT=$build_commit")

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

# The files that the release image must hold; release.md (Container image) states them.
REQUIRED_FILES="/app/LICENSE /app/THIRD-PARTY-NOTICES"

do_build() {
  local platform meta digest
  if [[ $dry_run -eq 1 ]]; then platform="<host os/arch>"; else
    platform="$(docker version --format '{{.Server.Os}}/{{.Server.Arch}}')"
    case " $PLATFORMS " in *" $platform "*) ;; *)
      fail "the Docker host platform $platform is not one of $PLATFORMS. Fix: run the build on a Linux x86_64 or a Linux arm64 runner. Verify: docker version --format '{{.Server.Os}}/{{.Server.Arch}}'." 1 ;;
    esac
  fi
  meta="${TMPDIR:-/tmp}/release.image.meta.$$.json"
  local check_tag="agentloupe-release-check:$version"
  local notices=("$ROOT/.agents/scripts/release.notices.sh")
  # The first build loads the image into Docker for the file check; the second build pushes the same
  # layers from the build cache by digest, so nothing reaches the registry before the check passes.
  # The BuildKit attestations are off, so the pushed image holds no unsigned provenance and no SBOM
  # manifest; the image SBOM is a release file.
  local load=(docker buildx build --platform "$platform" "${labels[@]}" "${build_args[@]}" --provenance=false --sbom=false --load --tag "$check_tag" --file Dockerfile .)
  local check=(docker run --rm --platform "$platform" --entrypoint /bin/sh "$check_tag" -c "for f in $REQUIRED_FILES; do test -s \$f || { echo missing \$f; exit 1; }; done")
  local cmd=(docker buildx build --platform "$platform" "${labels[@]}" "${build_args[@]}" --provenance=false --sbom=false
    --output "type=image,name=$IMAGE,push-by-digest=true,name-canonical=true,push=true"
    --metadata-file "$meta" --file Dockerfile .)
  if [[ $dry_run -eq 1 ]]; then
    show .agents/scripts/release.notices.sh; show "${load[@]}"; show "${check[@]}"; show "${cmd[@]}"
    echo "command: write the digest to $dir/<os>-<arch>"
    show .agents/scripts/release.sbom.sh image --tag "$tag" --platform "$platform" --digest "<digest>" --out "$sbom_dir"
    echo "telemetry: $telemetry"; return 0
  fi
  mkdir -p "$(dirname "$LOG")" "$dir"
  if ! "${notices[@]}" >>"$LOG" 2>&1; then
    tail -n 10 "$LOG" >&2
    fail "release.notices.sh failed, so the image would lack THIRD-PARTY-NOTICES; nothing was built or pushed. Fix: follow the error of release.notices.sh above (install cargo about from .tool-versions, or settle the license). Reference: docs/specification/release.md, section Third-party notices. Verify: .agents/scripts/release.notices.sh exits 0." 1
  fi
  if ! "${load[@]}" >>"$LOG" 2>&1; then
    tail -n 30 "$LOG" >&2
    fail "the image build for $platform failed; nothing was pushed. Fix: read .agents/logs/release.image.log; check that pipeline.image.sh passes on the tag commit. Verify: re-run release.image.sh build --tag $tag --digest-dir $dir." 1
  fi
  if ! "${check[@]}" >>"$LOG" 2>&1; then
    docker image rm "$check_tag" >/dev/null 2>&1 || true
    tail -n 5 "$LOG" >&2
    fail "the built image for $platform lacks one of $REQUIRED_FILES or holds it empty, so nothing was pushed. Fix: check that the Dockerfile copies LICENSE and THIRD-PARTY-NOTICES into /app and that .dockerignore allows both. Reference: docs/specification/release.md, section Container image. Verify: re-run release.image.sh build --tag $tag --digest-dir $dir." 1
  fi
  docker image rm "$check_tag" >/dev/null 2>&1 || true
  if ! "${cmd[@]}" >>"$LOG" 2>&1; then
    tail -n 30 "$LOG" >&2
    fail "the image build or push for $platform failed. Fix: read .agents/logs/release.image.log; check the registry login (packages: write) and that pipeline.image.sh passes on the tag commit. Verify: re-run release.image.sh build --tag $tag --digest-dir $dir." 1
  fi
  digest="$(tr -d ' \n' <"$meta" | sed -n 's/.*"containerimage.digest":"\(sha256:[0-9a-f]*\)".*/\1/p')"
  rm -f "$meta"
  [[ -n "$digest" ]] || fail "the build wrote no image digest. Fix: update Docker buildx so it writes containerimage.digest to the metadata file. Verify: re-run the build." 1
  echo "$digest" >"$dir/$(platform_key "$platform")"
  if [[ $human -eq 1 ]]; then echo "Pushed $IMAGE@$digest for $platform ($version), telemetry $telemetry."; else
    echo "build: image: $IMAGE platform: $platform digest: $digest version: $version revision: $commit telemetry: $telemetry"; fi
  # The image SBOM of this platform, read from the registry by the pushed digest; no SBOM fails the build.
  local sbom_args=(image --tag "$tag" --platform "$platform" --digest "$digest" --out "$sbom_dir")
  [[ $human -eq 1 ]] && sbom_args+=(--human)
  if ! "$ROOT/.agents/scripts/release.sbom.sh" "${sbom_args[@]}" 2>>"$LOG"; then
    tail -n 5 "$LOG" >&2
    fail "release.sbom.sh image failed for $platform, so the image has no SBOM file and the build fails. Fix: follow the error in .agents/logs/release.image.log (install syft of .tool-versions with mise install, and jq; check the registry login). Reference: docs/specification/release.md, section Supply chain. Verify: .agents/scripts/release.sbom.sh image --tag $tag --platform $platform --digest $digest --out $sbom_dir exits 0." 1
  fi
}

do_merge() {
  local p f sources=() digest
  for p in $PLATFORMS; do
    f="$dir/$(platform_key "$p")"
    if [[ $dry_run -eq 1 ]]; then sources+=("$IMAGE@<digest of $p>"); continue; fi
    [[ -s "$f" ]] || fail "the digest of $p is missing ($f), so the index is not made and the tag latest does not move. Fix: run release.image.sh build on the $p runner and pass its digest file to the merge. Verify: ls $dir lists one file per platform." 1
    sources+=("$IMAGE@$(tr -d ' \n' <"$f")")
  done
  local cmd=(docker buildx imagetools create "${annotations[@]}" --tag "$IMAGE:$version" "${sources[@]}")
  if [[ $dry_run -eq 1 ]]; then show "${cmd[@]}"; return 0; fi
  mkdir -p "$(dirname "$LOG")"
  if ! "${cmd[@]}" >>"$LOG" 2>&1; then
    tail -n 30 "$LOG" >&2
    fail "the merge of the platform images failed. Fix: read .agents/logs/release.image.log and check the registry login. Verify: docker buildx imagetools inspect $IMAGE:$version lists both platforms." 1
  fi
  digest="$(docker buildx imagetools inspect "$IMAGE:$version" 2>>"$LOG" | awk '$1 == "Digest:" {print $2; exit}')"
  if [[ $human -eq 1 ]]; then echo "Tagged $IMAGE:$version at $digest for ${PLATFORMS// /, }; latest moves in promote."; else
    echo "merge: image: $IMAGE tags: $version digest: $digest platforms: ${PLATFORMS// /,}"; fi
}

# The version that an index tag carries in its annotation; empty when the tag does not exist.
index_version() {
  { docker buildx imagetools inspect --raw "$1" 2>/dev/null || true; } \
    | tr -d ' \n' | sed -n 's/.*"org.opencontainers.image.version":"\([^"]*\)".*/\1/p' | head -n 1
}
index_digest() { { docker buildx imagetools inspect "$1" 2>>"$LOG" || true; } | awk '$1 == "Digest:" {print $2; exit}'; }

# Moves latest only forward: to the version of the tag, never below the version that latest holds.
do_promote() {
  local current target_digest latest_digest status
  local cmd=(docker buildx imagetools create --tag "$IMAGE:latest" "$IMAGE@<index digest of $version>")
  if [[ $dry_run -eq 1 ]]; then
    echo "command: docker buildx imagetools inspect $IMAGE:latest (refuse when its version is newer than $version)"
    show "${cmd[@]}"; return 0
  fi
  mkdir -p "$(dirname "$LOG")"
  target_digest="$(index_digest "$IMAGE:$version")"
  [[ -n "$target_digest" ]] || fail "the image $IMAGE:$version does not exist, so latest cannot move to it. Fix: run release.image.sh merge for $tag first. Verify: docker buildx imagetools inspect $IMAGE:$version." 1
  current="$(index_version "$IMAGE:latest")"
  if [[ -n "$current" && "$(printf '%s\n%s\n' "$current" "$version" | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1)" != "$version" ]]; then
    fail "latest points to $current, which is newer than $version, so latest does not move backward. Fix: promote a version newer than $current, or leave latest as it is. Reference: docs/specification/release.md, section Container image. Verify: docker buildx imagetools inspect $IMAGE:latest shows $current." 1
  fi
  latest_digest="$(index_digest "$IMAGE:latest" 2>/dev/null || true)"
  if [[ "$latest_digest" == "$target_digest" ]]; then status=unchanged; else
    cmd=(docker buildx imagetools create --tag "$IMAGE:latest" "$IMAGE@$target_digest")
    if ! "${cmd[@]}" >>"$LOG" 2>&1; then
      tail -n 30 "$LOG" >&2
      fail "the move of latest to $version failed. Fix: read .agents/logs/release.image.log and check the registry login (packages: write). Verify: docker buildx imagetools inspect $IMAGE:latest." 1
    fi
    status=moved
  fi
  if [[ $human -eq 1 ]]; then echo "latest of $IMAGE is $version at $target_digest (was ${current:-none}; $status)."; else
    echo "promote: image: $IMAGE latest: $version digest: $target_digest from: ${current:-none} status: $status"; fi
}

[[ $dry_run -eq 1 ]] && echo "plan: tag $tag version: $version check: passed at: $at image: $IMAGE created: $created"
case "$mode" in
  build) do_build ;;
  merge) do_merge ;;
  promote) do_promote ;;
  "") do_build; do_merge; do_promote ;;
esac
