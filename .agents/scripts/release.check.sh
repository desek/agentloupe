#!/usr/bin/env bash
# @agents-index Checks a published release on a local machine: the six assets and checksums, loupe --version per target, the image pull, /healthz, and the notices.
#
# Usage: release.check.sh --version <major>.<minor>.<patch> [--dir DIR] [--human] [--dry-run]
#   --version X.Y.Z  The version of the release to check, with no v. Required.
#   --dir DIR        Download into DIR and keep it. A file that is already in DIR is not downloaded again,
#                    so a changed copy can be checked. Default: a temporary directory, removed at the end.
#   --human          Print a friendly view for a person. Also the default when stdout is a terminal.
#   --dry-run        Print the plan with the six asset names and the image name; no network, no Docker.
#   -h, --help       Print this help.
#
# Purpose: the release check of docs/specification/release.md. It downloads the three archives and the three
# checksum files of github.com/desek/agentloupe with curl and no credential, verifies each checksum, runs
# loupe --version of the host archive and of each Linux archive in a debian:bookworm-slim container of its
# platform, pulls ghcr.io/desek/agentloupe:<version> with no login (and compares latest when the version is
# the newest release), then starts the image with two keys from its own loupe gen-key on 127.0.0.1 and a free
# port, polls GET /healthz for at most 30 seconds, and reads the version in the first log line. It checks
# THIRD-PARTY-NOTICES in each archive and in the image: present, not empty, with the LiteLLM part and the
# Geist part, and with one SHA-256 digest over all four copies. It runs with
# the bash 3.2 of macOS, the default tools of macOS, curl, and Docker; it needs no jq and no gh. It never
# prints a key. It removes its container, its volume, and its temporary directory on each exit path.
#
# Output (stdout, key: value), one line per check, then one result line:
#   check: <name> status: passed|failed|skipped [detail: ...]
#   result: passed|failed exit: <code>
# Checks: download, checksum <asset>, version <target>, notices <target>, pull, latest, health,
#   start-version, notices image.
# Exit codes: 0 every check passed or is skipped with a stated reason, 1 a check failed, 2 bad usage,
#   3 a required tool is missing, 4 a download or a pull failed.
# Reference: docs/specification/release.md, section Verify a release.
set -euo pipefail

REPO="desek/agentloupe"
IMAGE="ghcr.io/desek/agentloupe"
BASE_IMAGE="debian:bookworm-slim"
TARGETS="aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu"
HEALTH_CAP_SECS=30
NOTICES=THIRD-PARTY-NOTICES
# The part titles that release.notices.sh writes; a copy without them is not the complete file.
LITELLM_MARK="Part 2: LiteLLM"
FONTS_MARK="Part 3: Geist fonts"

human=0; dry_run=0; version=""; dir=""; keep=0
[ -t 1 ] && human=1

usage() { awk 'NR>=4 && /^#/ {sub(/^# ?/, ""); print; next} NR>=4 {exit}' "$0"; }
die() { echo "error: $1" >&2; exit "$2"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version needs a value. Fix: pass --version <major>.<minor>.<patch>, for example the version of the newest release. Verify: release.check.sh --version <version> --dry-run prints the six asset names." 2
      version="$2"; shift 2 ;;
    --dir) [ $# -ge 2 ] || die "--dir needs a directory. Fix: pass --dir <dir> or omit it. Verify: re-run with --dry-run." 2
      dir="$2"; keep=1; shift 2 ;;
    --human) human=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1. Fix: run --help for the flags. Verify: release.check.sh --version <version> --dry-run prints the six asset names." 2 ;;
  esac
done
[ -n "$version" ] || die "--version is required. Fix: pass --version <major>.<minor>.<patch> with no v, the version of a published release. Reference: docs/specification/release.md, section Verify a release. Verify: release.check.sh --version <version> --dry-run prints the six asset names." 2
if ! echo "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
  die "the version $version is not of the form <major>.<minor>.<patch> with no v and no suffix. Fix: pass for example --version 1.2.3 for the tag v1.2.3. Verify: re-run with --dry-run." 2
fi
tag="v$version"
url="https://github.com/$REPO/releases/download/$tag"

failed=0; skipped=0
# One machine line per check; the friendly view says the same in a sentence.
report() {
  local name="$1" status="$2" detail="${3:-}"
  [ "$status" = failed ] && failed=1
  [ "$status" = skipped ] && skipped=$((skipped + 1))
  if [ $human -eq 1 ]; then
    echo "$name: $status${detail:+ ($detail)}"
  else
    echo "check: $name status: $status${detail:+ detail: $detail}"
  fi
}
finish() {
  local code="$1"
  if [ $human -eq 1 ]; then
    if [ "$code" -eq 0 ]; then echo "The release $tag passed the check ($skipped checks skipped)."; else echo "The release $tag failed the check (exit $code)."; fi
  else
    echo "result: $([ "$code" -eq 0 ] && echo passed || echo failed) exit: $code"
  fi
  exit "$code"
}

if [ $dry_run -eq 1 ]; then
  for t in $TARGETS; do
    a="loupe-$tag-$t.tar.gz"
    if [ $human -eq 1 ]; then echo "Would download $url/$a and $a.sha256 and verify them."; else
      echo "plan: asset $a url: $url/$a"
      echo "plan: asset $a.sha256 url: $url/$a.sha256"
    fi
  done
  if [ $human -eq 1 ]; then echo "Would pull $IMAGE:$version with no login, start it on 127.0.0.1, and poll /healthz for $HEALTH_CAP_SECS seconds."; else
    echo "plan: image $IMAGE:$version login: none"
    echo "plan: health url: http://127.0.0.1:<free port>/healthz cap_secs: $HEALTH_CAP_SECS"
    echo "plan: notices file: $NOTICES copies: $TARGETS image"
  fi
  exit 0
fi

# Tools: curl and a SHA-256 tool are required; Docker is optional and its checks are skipped without it.
command -v curl >/dev/null || die "curl is not installed, so the assets cannot be downloaded. Fix: install curl (macOS has it at /usr/bin/curl). Verify: curl --version." 3
command -v tar >/dev/null || die "tar is not installed, so the archive cannot be unpacked. Fix: install tar. Verify: tar --version." 3
if command -v shasum >/dev/null; then sha_check() { shasum -a 256 -c "$1"; }; sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }
elif command -v sha256sum >/dev/null; then sha_check() { sha256sum -c "$1"; }; sha_of() { sha256sum "$1" | awk '{print $1}'; }
else die "neither shasum nor sha256sum is installed, so the checksums cannot be verified. Fix: use the shasum of macOS or install coreutils. Verify: shasum -a 256 --version." 3; fi
docker_ok=0
if command -v docker >/dev/null && docker info >/dev/null 2>&1; then docker_ok=1; fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/release.check.XXXXXX")"
container=""
cleanup() {
  if [ -n "$container" ]; then docker rm -f -v "$container" >/dev/null 2>&1 || true; fi
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
[ -n "$dir" ] || dir="$tmp/assets"
mkdir -p "$dir"

# 1. Download with no credential: no netrc, no config file, no header.
for t in $TARGETS; do
  for f in "loupe-$tag-$t.tar.gz" "loupe-$tag-$t.tar.gz.sha256"; do
    [ -s "$dir/$f" ] && continue
    if ! curl -q -fsSL --retry 3 -o "$dir/$f" "$url/$f" 2>"$tmp/curl.err"; then
      report download failed "$f from $url: $(tr '\n' ' ' <"$tmp/curl.err")"
      echo "error: the download of $f failed. Fix: check that the release $tag exists and is public with its six assets (gh release view $tag), and that this machine reaches github.com. Verify: curl -fsSLI $url/$f." >&2
      finish 4
    fi
  done
done
report download passed "6 files into $([ $keep -eq 1 ] && echo "$dir" || echo 'a temporary directory')"

# 2. Each checksum file names its archive and matches its bytes.
verified=" "
for t in $TARGETS; do
  a="loupe-$tag-$t.tar.gz"
  if awk -v a="$a" 'NF == 2 && $2 == a && $1 ~ /^[0-9a-f]{64}$/ {ok++} END {exit !(NR == 1 && ok == 1)}' "$dir/$a.sha256" \
     && (cd "$dir" && sha_check "$a.sha256" >/dev/null 2>&1); then
    report "checksum $a" passed "$(awk '{print $1}' "$dir/$a.sha256")"
    verified="$verified$t "
  else
    report "checksum $a" failed "the archive does not match $a.sha256, or the file is not one sha256sum line"
  fi
done
unpack() { mkdir -p "$tmp/x/$1" && tar -xzf "$dir/loupe-$tag-$1.tar.gz" -C "$tmp/x/$1"; }
bin_of() { echo "$tmp/x/$1/loupe-$tag-$1/loupe"; }

# 3. The archive of the host platform runs on the host.
case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) host=aarch64-apple-darwin ;;
  Linux/x86_64) host=x86_64-unknown-linux-gnu ;;
  Linux/aarch64|Linux/arm64) host=aarch64-unknown-linux-gnu ;;
  *) host="" ;;
esac
if [ -z "$host" ]; then
  report "version host" skipped "no release asset for $(uname -s)/$(uname -m)"
elif [ "${verified#* $host }" = "$verified" ]; then
  report "version $host" skipped "its checksum failed, so it does not run"
else
  unpack "$host"
  out="$("$(bin_of "$host")" --version 2>&1 || true)"
  if [ "$out" = "loupe $version" ]; then report "version $host" passed "$out"; else report "version $host" failed "printed '$out', expected 'loupe $version'"; fi
fi

# 4. Each Linux archive runs in a container of its platform.
for t in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  case "$t" in x86_64-*) platform=linux/amd64 ;; *) platform=linux/arm64 ;; esac
  if [ $docker_ok -eq 0 ]; then report "version $t" skipped "Docker is absent or not running"; continue; fi
  if [ "${verified#* $t }" = "$verified" ]; then report "version $t" skipped "its checksum failed, so it does not run"; continue; fi
  [ -d "$tmp/x/$t" ] || unpack "$t"
  out="$(docker run --rm --platform "$platform" -v "$tmp/x/$t:/x:ro" "$BASE_IMAGE" "/x/loupe-$tag-$t/loupe" --version 2>&1 | tail -n 1 || true)"
  if [ "$out" = "loupe $version" ]; then report "version $t" passed "$out on $platform"; else report "version $t" failed "printed '$out' on $platform, expected 'loupe $version'"; fi
done

# 4b. The notices of each archive: present, not empty, both parts, and one digest over every copy. The
# first copy that passes sets the reference digest; check_notices reports one line per copy.
notices_ref=""
check_notices() {
  local name="$1" file="$2" d
  if [ ! -s "$file" ]; then report "notices $name" failed "$NOTICES is absent or empty"; return; fi
  if ! grep -q "^$LITELLM_MARK" "$file" || ! grep -q "^$FONTS_MARK" "$file"; then
    report "notices $name" failed "$NOTICES lacks the LiteLLM part or the Geist part"; return
  fi
  d="$(sha_of "$file")"
  [ -n "$notices_ref" ] || notices_ref="$d"
  if [ "$d" = "$notices_ref" ]; then report "notices $name" passed "sha256 $d"; else
    report "notices $name" failed "sha256 $d differs from the other copies ($notices_ref)"; fi
}
for t in $TARGETS; do
  if [ "${verified#* $t }" = "$verified" ]; then report "notices $t" skipped "its checksum failed, so it is not unpacked"; continue; fi
  [ -d "$tmp/x/$t" ] || unpack "$t"
  check_notices "$t" "$tmp/x/$t/loupe-$tag-$t/$NOTICES"
done

if [ $docker_ok -eq 0 ]; then
  report pull skipped "Docker is absent or not running"
  report latest skipped "Docker is absent or not running"
  report health skipped "Docker is absent or not running"
  report start-version skipped "Docker is absent or not running"
  report "notices image" skipped "Docker is absent or not running"
  finish "$failed"
fi

# 5. Pull with no login: an empty Docker config holds no credential; the endpoint of the current context stays.
endpoint="$(docker context inspect --format '{{.Endpoints.docker.Host}}' 2>/dev/null || true)"
mkdir -p "$tmp/docker-config"
anon() { DOCKER_CONFIG="$tmp/docker-config" DOCKER_HOST="${endpoint:-${DOCKER_HOST:-}}" docker "$@"; }
pull_digest() { anon pull "$1" 2>"$tmp/pull.err" | awk '$1 == "Digest:" {print $2}'; }
digest="$(pull_digest "$IMAGE:$version" || true)"
if [ -z "$digest" ]; then
  report pull failed "$(tr '\n' ' ' <"$tmp/pull.err")"
  echo "error: the pull of $IMAGE:$version with no login failed. Fix: check that the release workflow pushed the image and that the package is public (GitHub, Packages, agentloupe, Package settings, Change visibility). Verify: docker logout ghcr.io; docker pull $IMAGE:$version." >&2
  finish 4
fi
size="$(docker image inspect --format '{{.Size}}' "$IMAGE:$version" 2>/dev/null || echo unknown)"
report pull passed "$IMAGE:$version digest $digest image_bytes $size"

latest_tag="$(curl -q -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" 2>/dev/null | sed -n 's#.*/releases/tag/##p' || true)"
if [ "$latest_tag" = "$tag" ]; then
  ldigest="$(pull_digest "$IMAGE:latest" || true)"
  if [ "$ldigest" = "$digest" ]; then report latest passed "latest has digest $ldigest"; else report latest failed "latest has digest '${ldigest:-none}', $version has $digest"; fi
else
  report latest skipped "$tag is not the newest release (newest: ${latest_tag:-unknown})"
fi

# 6. Start the image with two fresh keys of its own gen-key; the keys pass by name, never on a command line.
LOUPE_DATA_KEY="$(docker run --rm "$IMAGE:$version" gen-key 2>/dev/null | tr -d '\r\n')"
LOUPE_ACCOUNT_KEY="$(docker run --rm "$IMAGE:$version" gen-key 2>/dev/null | tr -d '\r\n')"
export LOUPE_DATA_KEY LOUPE_ACCOUNT_KEY
if [ -z "$LOUPE_DATA_KEY" ] || [ -z "$LOUPE_ACCOUNT_KEY" ]; then
  report health failed "loupe gen-key of the image printed no key"
  report start-version skipped "the image did not start"
  finish 1
fi
container="loupe-release-check-$$"
if ! docker run -d --name "$container" -p 127.0.0.1::8787 -e LOUPE_DATA_KEY -e LOUPE_ACCOUNT_KEY "$IMAGE:$version" >/dev/null 2>"$tmp/run.err"; then
  report health failed "docker run failed: $(tr '\n' ' ' <"$tmp/run.err")"
  report start-version skipped "the image did not start"
  finish 1
fi
unset LOUPE_DATA_KEY LOUPE_ACCOUNT_KEY
port="$(docker port "$container" 8787/tcp | sed -n 's/^127\.0\.0\.1:\([0-9]*\)$/\1/p' | head -n 1 || true)"
code=""; body=""; waited=0
while [ $waited -lt $HEALTH_CAP_SECS ]; do
  code="$(curl -q -s -o "$tmp/health.body" -w '%{http_code}' "http://127.0.0.1:$port/healthz" 2>/dev/null || true)"
  [ "$code" = 200 ] && break
  sleep 1; waited=$((waited + 1))
done
[ -f "$tmp/health.body" ] && body="$(cat "$tmp/health.body")"
if [ "$code" = 200 ] && [ "$body" = '{"status":"ok"}' ]; then
  report health passed "status 200 body $body on 127.0.0.1:$port after ${waited}s"
else
  report health failed "status '${code:-none}' body '$body' after ${waited}s (cap ${HEALTH_CAP_SECS}s)"
fi
first="$(docker logs "$container" 2>&1 | head -n 1 || true)"
if echo "$first" | grep -q "\"version\":\"$version\""; then
  report start-version passed "the first log line holds version $version"
else
  report start-version failed "the first log line holds no version $version"
fi

# 7. The notices of the image: the same file as in the archives.
if docker run --rm --entrypoint cat "$IMAGE:$version" "/app/$NOTICES" >"$tmp/image.notices" 2>"$tmp/notices.err"; then
  check_notices image "$tmp/image.notices"
else
  report "notices image" failed "cat /app/$NOTICES in the image failed: $(tr '\n' ' ' <"$tmp/notices.err")"
fi
finish "$failed"
