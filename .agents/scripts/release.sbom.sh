#!/usr/bin/env bash
# @agents-index Writes the CycloneDX SBOM file of a release archive (cargo-cyclonedx) or of a pushed platform image (syft), with its .sha256 file.
#
# Usage: release.sbom.sh asset --tag v<major>.<minor>.<patch> --target <triple> --out DIR [--human] [--dry-run]
#        release.sbom.sh image --tag v<major>.<minor>.<patch> --platform <os/arch> --digest <sha256:...> --out DIR
#                              [--image-ref REF] [--human] [--dry-run]
#   asset            Write loupe-v<version>-<target>.cdx.json from Cargo.toml and Cargo.lock of the checked-out
#                    tag commit: the metadata component agent-loupe at the tag version, and one component per
#                    crate that the loupe binary uses on the target (normal and build dependencies, no
#                    development dependency), with name, version, package URL, and the Cargo.lock checksum.
#   image            Write loupe-v<version>-image-<os>-<arch>.cdx.json from the pushed platform image, read
#                    from the registry by its digest: the operating system packages and the loupe binary.
#   --tag TAG        The git tag of the release, for example v1.2.3. Required.
#   --target TRIPLE  The Rust target of the archive (mode asset), for example aarch64-apple-darwin.
#   --platform P     The platform of the image (mode image): linux/amd64 or linux/arm64.
#   --digest D       The digest of the pushed platform image (mode image), sha256:<64 hex>.
#   --image-ref REF  The syft source to read instead of the registry, for a local rehearsal, for example
#                    docker:agentloupe-rehearsal:0.1.0. Default: registry:<image>@<digest>.
#   --out DIR        The directory of the SBOM file and its checksum file. Required.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run        Run the tag check, print the plan, and write nothing. When the tag does not exist yet,
#                    the plan uses HEAD and says so.
#   -h, --help       Print this help.
#
# Purpose: the SBOM files of docs/specification/release.md (Supply chain). cargo-cyclonedx and syft come from
# .tool-versions; this script states no tool version. cargo-cyclonedx lists the crates of cargo metadata,
# which unifies the development dependencies and lists optional crates that the build does not enable, so
# the mode asset keeps only the crates that cargo tree --target <triple> -e normal,build lists for the
# package. That set is also the measure of the SBOM coverage. Paths of the build machine are replaced by
# ".", so the file holds no path of the machine; the serial number and the time stamp are the tool's and can
# differ between two runs of one commit. Both modes write CycloneDX JSON 1.5, the newest version that both
# tools write. The checksum file has the format of sha256sum, like the archive checksum files.
#
# Output (stdout, key: value):
#   sbom: <file> kind: asset|image components: <n> sha256: <hex> checksum_file: <file>.sha256 dir: <out>
#   With --dry-run: plan lines (plan: tag, plan: tool, plan: filter, plan: file, plan: checksum).
# Tool output goes to .agents/logs/release.sbom.log.
# Exit codes: 0 written (or planned), 1 the SBOM step failed or Cargo.lock changed during the run, 2 bad usage,
#   3 a required tool is missing (cargo, cargo-cyclonedx, syft, jq, git, or a SHA-256 tool),
#   4 tag check failed (tag absent, tag version differs from Cargo.toml, or HEAD is not the tag commit).
# Reference: docs/specification/release.md, section Supply chain.
set -euo pipefail

# The image name of release.image.sh and the path of the binary in the image (the Dockerfile copies it there).
IMAGE="ghcr.io/desek/agentloupe"
IMAGE_BINARY="/usr/local/bin/loupe"
PLATFORMS="linux/amd64 linux/arm64"
SPEC="1.5"
PACKAGE="agent-loupe"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LOG="$ROOT/.agents/logs/release.sbom.log"
human=0; dry_run=0; mode=""; tag=""; target=""; platform=""; digest=""; image_ref=""; out=""
[[ -t 1 ]] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
fail() { echo "error: $1" >&2; exit "$2"; }
need_value() { [[ $1 -ge 2 ]] || fail "$2 needs a value. Fix: run --help for the flags. Verify: re-run with --dry-run." 2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    asset|image) mode="$1"; shift ;;
    --tag) need_value $# "$1"; tag="$2"; shift 2 ;;
    --target) need_value $# "$1"; target="$2"; shift 2 ;;
    --platform) need_value $# "$1"; platform="$2"; shift 2 ;;
    --digest) need_value $# "$1"; digest="$2"; shift 2 ;;
    --image-ref) need_value $# "$1"; image_ref="$2"; shift 2 ;;
    --out) need_value $# "$1"; out="$2"; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "unknown argument $1. Fix: run --help for the modes and flags. Verify: release.sbom.sh asset --tag <tag> --target <triple> --out <dir> --dry-run prints plan lines." 2 ;;
  esac
done

[[ -n "$mode" ]] || fail "a mode is required. Fix: pass asset (after release.build.sh builds the archive) or image (after release.image.sh pushes a platform image). Reference: docs/specification/release.md, section Supply chain. Verify: release.sbom.sh --help." 2
[[ -n "$tag" ]] || fail "--tag is required. Fix: pass --tag v<major>.<minor>.<patch>, the git tag of the release. Verify: re-run with --dry-run." 2
if ! [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  fail "the tag $tag is not of the form v<major>.<minor>.<patch> with digits only, no leading zero, and no suffix. Fix: pass a tag such as v1.2.3. Reference: docs/specification/release.md, section Tag and image tag. Verify: re-run with --dry-run." 2
fi
version="${tag#v}"
[[ -n "$out" ]] || fail "--out is required. Fix: pass --out <dir>, the directory of the release files. Verify: re-run with --dry-run." 2
if [[ "$mode" == asset ]]; then
  [[ -n "$target" ]] || fail "--target is required in the mode asset. Fix: pass the Rust target of the archive, for example --target \"\$(rustc -vV | sed -n 's/^host: //p')\". Verify: re-run with --dry-run." 2
  file="loupe-v$version-$target.cdx.json"
else
  case " $PLATFORMS " in *" $platform "*) ;; *)
    fail "--platform must be one of $PLATFORMS, got '${platform:-nothing}'. Fix: pass the platform of the pushed image. Verify: re-run with --dry-run." 2 ;;
  esac
  [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || fail "--digest must be sha256:<64 hex>, got '${digest:-nothing}'. Fix: pass the digest that release.image.sh build wrote for the platform. Verify: re-run with --dry-run." 2
  file="loupe-v$version-image-${platform%%/*}-${platform##*/}.cdx.json"
  [[ -n "$image_ref" ]] || image_ref="registry:$IMAGE@$digest"
fi

command -v git >/dev/null || fail "git is not installed. Fix: install git. Verify: git --version." 3
command -v jq >/dev/null || fail "jq is not installed, so the SBOM cannot be filtered. Fix: install jq (preinstalled on the GitHub runners; brew install jq or apt-get install jq). Verify: jq --version." 3
if command -v sha256sum >/dev/null; then SHA=(sha256sum); elif command -v shasum >/dev/null; then SHA=(shasum -a 256); else
  fail "neither sha256sum nor shasum is installed, so the checksum file cannot be made. Fix: install coreutils (Linux) or use the shasum of macOS. Verify: re-run with --dry-run." 3
fi
if [[ "$mode" == asset ]]; then
  command -v cargo >/dev/null || fail "cargo is not installed. Fix: install Rust with rustup. Verify: cargo --version." 3
  cargo cyclonedx --help >/dev/null 2>&1 || fail "cargo-cyclonedx is not installed, so the archive SBOM cannot be made. Fix: mise install (it reads the cargo-cyclonedx pin of .tool-versions). Reference: docs/specification/release.md, section Supply chain. Verify: cargo cyclonedx --help." 3
  tool="cargo-cyclonedx $(cargo cyclonedx --version 2>/dev/null | awk '{print $NF}')"
else
  command -v syft >/dev/null || fail "syft is not installed, so the image SBOM cannot be made. Fix: mise install (it reads the syft pin of .tool-versions). Reference: docs/specification/release.md, section Supply chain. Verify: syft version." 3
  tool="syft $(syft version 2>/dev/null | awk '$1 == "Version:" {print $2}')"
fi

cargo_version() { awk '/^\[/ {pkg = ($0 == "[package]")} pkg && /^version *=/ {gsub(/^version *= *"|".*$/, ""); print; exit}'; }

# The tag check of release.build.sh: the SBOM must describe the commit of the tag and nothing else.
cd "$ROOT"
tag_commit="$(git rev-parse -q --verify "refs/tags/$tag^{commit}" 2>/dev/null || true)"
head_commit="$(git rev-parse HEAD)"
if [[ -n "$tag_commit" ]]; then
  at="tag $tag"; cargo="$(git show "$tag_commit:Cargo.toml" | cargo_version)"
elif [[ $dry_run -eq 1 ]]; then
  at="HEAD (tag $tag does not exist yet)"; cargo="$(git show "HEAD:Cargo.toml" | cargo_version)"
else
  fail "the tag $tag does not exist in this clone, so the SBOM has no tag commit. Fix: fetch the tags (git fetch --tags) and check out the tag. Reference: docs/specification/release.md, section Supply chain. Verify: git rev-parse $tag prints a commit." 4
fi
if [[ "$cargo" != "$version" ]]; then
  fail "the tag version $version differs from the version $cargo in Cargo.toml at $at, so the SBOM step stops. Fix: tag the commit whose Cargo.toml holds $version, or pass --tag v$cargo. Reference: docs/specification/release.md, section Version. Verify: git show $tag:Cargo.toml | grep '^version' prints $version." 4
fi
if [[ "$mode" == asset && -n "$tag_commit" && "$tag_commit" != "$head_commit" ]]; then
  fail "the checked-out commit $head_commit is not the commit $tag_commit of the tag $tag, so the SBOM would not describe the tag. Fix: git checkout $tag, then re-run. Verify: git rev-parse HEAD equals git rev-parse $tag^{commit}." 4
fi

if [[ $dry_run -eq 1 ]]; then
  if [[ "$mode" == asset ]]; then
    gen="cargo cyclonedx --format json --spec-version $SPEC --target $target"
    filter="crates of cargo tree --locked --target $target -e normal,build"
  else
    gen="syft scan $image_ref --platform $platform -o cyclonedx-json@$SPEC"
    filter="the operating system packages and the file $IMAGE_BINARY"
  fi
  if [[ $human -eq 1 ]]; then
    echo "Plan for $tag ($mode): check passed at $at; run '$gen' with $tool; keep $filter; write $file and $file.sha256 into $out. Nothing was written."
  else
    echo "plan: tag $tag version: $version check: passed at: $at"
    echo "plan: tool $tool command: $gen"
    echo "plan: filter keep: $filter"
    echo "plan: file name: $file format: CycloneDX JSON $SPEC dir: $out"
    echo "plan: checksum name: $file.sha256 format: sha256sum"
  fi
  exit 0
fi

mkdir -p "$(dirname "$LOG")" "$out"
work="$(mktemp -d "${TMPDIR:-/tmp}/release.sbom.XXXXXX")"
raw_name="release-sbom-$$"
trap 'rm -rf "$work"; rm -f "$ROOT/$raw_name.json"' EXIT

lock_hash() { "${SHA[@]}" "$ROOT/Cargo.lock" | awk '{print $1}'; }

if [[ "$mode" == asset ]]; then
  lock_before="$(lock_hash)"
  # cargo-cyclonedx writes next to Cargo.toml; the trap removes the raw file on each exit path.
  if ! cargo cyclonedx --format json --spec-version "$SPEC" --target "$target" --override-filename "$raw_name" -q >>"$LOG" 2>&1; then
    tail -n 20 "$LOG" >&2
    fail "cargo cyclonedx failed for $target. Fix: read .agents/logs/release.sbom.log; check that the target is installed (rustup target list --installed). Verify: re-run release.sbom.sh asset --tag $tag --target $target --out $out." 1
  fi
  mv "$ROOT/$raw_name.json" "$work/raw.json"
  # The crates of the binary on this target: normal and build edges only, as name@version.
  if ! cargo tree --locked --target "$target" -e normal,build --prefix none -p "$PACKAGE" 2>>"$LOG" \
      | awk '$1 !~ /^\[/ && NF >= 2 {v = $2; sub(/^v/, "", v); print $1 "@" v}' | sort -u >"$work/keep.txt"; then
    fail "cargo tree failed for $target. Fix: read .agents/logs/release.sbom.log. Verify: cargo tree --locked --target $target -e normal,build prints the crates." 1
  fi
  [[ "$(lock_hash)" == "$lock_before" ]] || fail "Cargo.lock changed during the SBOM run, so the SBOM would not describe the tag. Fix: check out the tag with a clean tree (git status shows nothing) and re-run; the build uses --locked. Verify: git diff --exit-code Cargo.lock." 1
  # Keep the listed crates, drop the rest from the components and the dependency graph, and replace the
  # path of the checkout so the file holds no path of the build machine.
  if ! jq --arg root "$ROOT" --arg version "$version" --arg package "$PACKAGE" --rawfile keep "$work/keep.txt" '
      ($keep | split("\n") | map(select(. != "" and (startswith($package + "@") | not)))) as $k
      | .components |= map(select((.name + "@" + .version) as $id | $k | index($id)))
      | ([.metadata.component["bom-ref"]] + [.components[]["bom-ref"]]) as $refs
      | .dependencies |= (map(select(.ref as $r | $refs | index($r)))
                          | map(.dependsOn |= ((. // []) | map(select(. as $d | $refs | index($d))))))
      | walk(if type == "string" then (split($root) | join(".")) else . end)
      | if .metadata.component.name == $package and .metadata.component.version == $version then .
        else error("the metadata component is not \($package) \($version)") end
    ' "$work/raw.json" >"$work/sbom.json" 2>>"$LOG"; then
    tail -n 5 "$LOG" >&2
    fail "the filter of the archive SBOM failed. Fix: read .agents/logs/release.sbom.log; the metadata component must be $PACKAGE $version. Verify: re-run release.sbom.sh asset --tag $tag --target $target --out $out." 1
  fi
  want="$(grep -cv "^$PACKAGE@" "$work/keep.txt" || true)"
  got="$(jq '.components | length' "$work/sbom.json")"
  [[ "$got" == "$want" ]] || fail "the archive SBOM holds $got components, but cargo tree lists $want crates for $target, so a crate has no entry in Cargo.lock metadata. Fix: read .agents/logs/release.sbom.log and compare the two lists. Verify: re-run release.sbom.sh asset." 1
else
  # Every file gets a SHA-256 digest, so the binary of the image appears with its digest; the filter then
  # keeps the packages and that one file.
  if ! SYFT_FILE_METADATA_SELECTION=all SYFT_FILE_METADATA_DIGESTS=sha256 SYFT_CHECK_FOR_APP_UPDATE=false \
      syft scan "$image_ref" --platform "$platform" -o "cyclonedx-json@$SPEC=$work/raw.json" -q >>"$LOG" 2>&1; then
    tail -n 20 "$LOG" >&2
    fail "syft could not read $image_ref for $platform. Fix: read .agents/logs/release.sbom.log; check the registry login and that the digest was pushed (docker buildx imagetools inspect $IMAGE@$digest). Verify: re-run release.sbom.sh image." 1
  fi
  if ! jq --arg bin "$IMAGE_BINARY" --arg version "$version" '
      .components |= map(select(.type != "file" or .name == $bin))
      | .components |= map(if .type == "file" and .name == $bin then . + {version: $version} else . end)
      | ([.metadata.component["bom-ref"]] + [.components[]["bom-ref"]]) as $refs
      | .dependencies |= ((. // []) | map(select(.ref as $r | $refs | index($r)))
                          | map(.dependsOn |= ((. // []) | map(select(. as $d | $refs | index($d))))))
      | if any(.components[]; .type == "file" and .name == $bin) then . else error("the image holds no \($bin)") end
    ' "$work/raw.json" >"$work/sbom.json" 2>>"$LOG"; then
    tail -n 5 "$LOG" >&2
    fail "the image SBOM lists no $IMAGE_BINARY, or its filter failed. Fix: check that the Dockerfile copies the binary to $IMAGE_BINARY; read .agents/logs/release.sbom.log. Verify: re-run release.sbom.sh image." 1
  fi
  got="$(jq '.components | length' "$work/sbom.json")"
fi

[[ "$(jq -r '.bomFormat' "$work/sbom.json")" == CycloneDX && "$got" -gt 0 ]] || fail "the SBOM is not CycloneDX or holds no component. Fix: read .agents/logs/release.sbom.log. Verify: re-run release.sbom.sh $mode." 1
mv "$work/sbom.json" "$out/$file"
(cd "$out" && "${SHA[@]}" "$file" | awk '{print $1 "  " $2}' >"$file.sha256")
(cd "$out" && "${SHA[@]}" -c "$file.sha256" >/dev/null) || fail "the checksum file $file.sha256 does not verify. Fix: re-run into an empty --out directory. Verify: shasum -a 256 -c $file.sha256 prints OK." 1
digest_hex="$(awk '{print $1}' "$out/$file.sha256")"
if [[ $human -eq 1 ]]; then
  echo "Wrote $file ($mode SBOM, $got components, $tool), SHA-256 $digest_hex, in $out."
else
  echo "sbom: $file kind: $mode components: $got sha256: $digest_hex checksum_file: $file.sha256 dir: $out"
fi
