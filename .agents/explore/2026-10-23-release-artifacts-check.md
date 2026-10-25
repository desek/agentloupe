---
date: 2026-10-23
branch: main
commit: 84c2284
---

<!--
@agents-index: Cached local rehearsal of the release workflow at 84c2284 (version 0.1.0, no tag, repository
private): the three binary assets built with release.build.sh (macOS natively, Linux in rust:1.97.1-bookworm
per platform), both platform images built with the release.image.sh command and a local load instead of a
push, each run and probed (--version, /healthz, first log line), release.check.sh in its local modes, and
the toolchain pins compared. Ends with an artifact table, findings with severities, and a verdict.
-->

## Question

Prove on this machine that the release artifacts and the container of this repository build and work as
the release workflow would build them. With the commands that the workflow calls, in a scratch directory
under `/private/tmp`: (1) build the macOS arm64 asset with `release.build.sh`, record its name, size and
checksum file, and run the unpacked binary with `--version` and with `serve` (free port, temporary data dir,
two keys from `loupe gen-key`), read `/healthz` and the version in the first log line; (2) build the Linux
assets as the workflow does (a cross tool if the workflow uses one, otherwise in docker with the rust image
for linux/amd64 and linux/arm64) and run each with `--version` in `debian:bookworm-slim` of its platform;
(3) build the image with the `release.image.sh` build for both platforms with a local tag only, run the amd64
image with the env of `compose.yaml` (scratch env file, fresh keys), read `/healthz` and the start log line,
record the image size per platform and the OCI labels; (4) run `release.check.sh` in its local modes;
(5) compare the Dockerfile base image and the runner toolchain with `rust-toolchain.toml` and
`.tool-versions`. Record every command with its duration and output, clean up everything, and end with an
artifact table, findings, and a verdict. No commit, push, tag, registry push, or visibility change. Do not
touch the container on port 8787 or the compose stack.

## Response

### Setup and constraints

- Host: macOS 27.0 arm64 (Apple M1 Pro class, 10 CPUs), Docker Desktop 4.80.0 (engine 29.6.1, containerd
  image store, buildx 0.35.0), rustc/cargo 1.97.1 via `rust-toolchain.toml`.
- Scratch: `/private/tmp/loupe-relcheck-20261023195419`, holding three `git clone --no-hardlinks`
  clones of the repository at `84c2284` (one for macOS, one per Linux platform, 0.8 s each), so the build
  sees only tracked files, as `actions/checkout` does. Each command ran through a timing wrapper. Durations
  are wall clock.
- **Tag check.** In real mode, `release.build.sh` and `release.image.sh` require the tag `v0.1.0` to exist,
  and it does not. No tag was made anywhere. Instead, a `git` shim on `PATH` answered only
  `git rev-parse -q --verify refs/tags/v0.1.0^{commit}` with `HEAD` (84c2284) and passed every other call
  to the real git. The scripts themselves ran unmodified. Cargo.toml holds `version = "0.1.0"`, so the
  version check ran for real.
- **Concurrent commit.** Another session committed `e13351f` on `main` at 20:03, during this check. It
  changes only `.agents/explore/*`, `.agents/scripts/pipeline.web.sh`, CR-0022, and `operations.md`, and no
  build input (src, Cargo, Dockerfile, `.dockerignore`, release scripts, workflows). So the results also
  hold for `e13351f`.
- The `agentic-transparency-dashboard-loupe-1` container on 127.0.0.1:8787 had the same ID and start time
  (`2026-10-23T10:45:36Z`) and 0 restarts before and after the run. No `docker compose` command was used.
- **How this differs from the workflow:**
  - The Linux assets were built in `rust:1.97.1-bookworm` (glibc 2.36), not on the `ubuntu-22.04` runners
    (glibc 2.35).
  - The amd64 builds ran under emulation on this arm64 host.
  - The image build used `--output type=docker,name=<local tag>` instead of `push-by-digest=true,push=true`.
  - There was no `actions/cache`.
  - The `visibility`, `verdict`, `publish` and `merge` steps did not run, because they need GitHub, the
    registry, or a tag.

### (1) macOS arm64 asset

| Command | Duration | Exit | Observed |
|---|---|---|---|
| `release.build.sh --tag v0.1.0 --out $S/out/mac --dry-run` | 0.3 s | 0 | `plan: tag v0.1.0 version: 0.1.0 check: passed at: HEAD (tag v0.1.0 does not exist yet)`; `plan: build command: cargo build --release --locked --bin loupe target: aarch64-apple-darwin`; archive and checksum plan lines |
| `release.build.sh --tag v0.1.0 --out …` (no shim) | 0.0 s | 4 | `error: the tag v0.1.0 does not exist in this clone, so the build has no tag commit. Fix: …` |
| `release.build.sh --tag v0.2.0 … --dry-run` | 0.1 s | 4 | `error: the tag version 0.2.0 differs from the version 0.1.0 in Cargo.toml at HEAD …` |
| `PATH=$S/shim:$PATH release.build.sh --tag v0.1.0 --out $S/out/mac` (cold `target/`, 185 crates) | 33.3 s | 0 | `asset: loupe-v0.1.0-aarch64-apple-darwin.tar.gz archive_bytes: 4645682 binary_bytes: 14402208 sha256: 7e7018b0b80dc496922d9afbc97e6ad4e7781cb2d9c9e839ebd78a94f9b22b4f checksum_file: loupe-v0.1.0-aarch64-apple-darwin.tar.gz.sha256`; log: ``Finished `release` profile [optimized] target(s) in 32.49s`` |
| `shasum -a 256 -c loupe-v0.1.0-aarch64-apple-darwin.tar.gz.sha256` | 0.1 s | 0 | `…: OK`. The file is 107 bytes, one line `<hex>  <name>` |
| `tar -tvzf …` | 0.0 s | 0 | One directory `loupe-v0.1.0-aarch64-apple-darwin/` holding `LICENSE`, `README.md`, `config/container.toml`, `config/local.toml`, `loupe` (`-rwxr-xr-x`, 14402208 bytes), and `upstreams.toml`. Owner `root wheel` (uid/gid 0) |
| `./loupe --version` (from the unpacked dir) | 0.3 s | 0 | `loupe 0.1.0` |
| `file` / `otool -L` / `otool -l` | – | 0 | Mach-O 64-bit arm64. Links only CoreFoundation, libiconv and libSystem. `minos 11.0`, `sdk 27.0`. No quarantine xattr |
| `./loupe gen-key` ×2 | <0.1 s | 0 | Two distinct 44-character base64 keys (not printed) |
| `env -i PATH=/usr/bin:/bin HOME=… LOUPE_DATA_KEY=… LOUPE_ACCOUNT_KEY=… ./loupe serve --listen 127.0.0.1:56925 --data-dir $S/macdata &` and a 0.25 s poll of `/healthz` | 0.35 s to first 200 | – | `HTTP/1.1 200 OK`, `content-type: application/json`, body `{"status":"ok"}` |
| First log line | – | – | `{"fields":{"version":"0.1.0"},"file":"src/cli/serve.rs:39","level":"INFO","message":"loupe serve starting",…}` |
| `kill <pid>` (SIGTERM) | ~1 s | – | `shutdown signal received, draining open requests`, then `database closed, exit`. The process was gone afterwards |

The start log also showed 19 migrations applied, a price index of 4218 entries (commit `cfee0ffe…`), no
provider detected, github disabled, and `listening 127.0.0.1:56925`. `env -i` kept the
`CLAUDE_CODE_*` variables of this shell out of the run.

### (2) Linux assets

`release.yml` builds on native runners (`ubuntu-22.04`, `ubuntu-22.04-arm`) with `rustup toolchain install`
and then `release.build.sh`. It uses no cross tool, and `release.md` (Release assets, item 1) says the same.
So each Linux asset was built in `rust:1.97.1-bookworm` of its platform. The container ran the same two
steps (`rustup toolchain install`, then `release.build.sh --tag v0.1.0 --out /out`), with the repo clone at
`/src` and the Linux git shim at `/shim`.

| Command | Duration | Exit | Observed |
|---|---|---|---|
| `docker pull -q --platform linux/arm64 rust:1.97.1-bookworm` | 12.6 s | 0 | – |
| `docker pull -q --platform linux/amd64 rust:1.97.1-bookworm` | 14.8 s | 0 | Image glibc 2.36, git 2.39.5, rustup 1.29.0 |
| `docker run --rm --platform linux/arm64 -v repo-arm64:/src -v out:/out -v shim:/shim rust:1.97.1-bookworm /build.sh` | 61.5 s | 0 | `active toolchain 1.97.1-aarch64-unknown-linux-gnu … overridden by '/src/rust-toolchain.toml'` (downloaded clippy). `asset: loupe-v0.1.0-aarch64-unknown-linux-gnu.tar.gz archive_bytes: 5069279 binary_bytes: 16341728 sha256: 30d80f84985203582dc8f629425e4163d19414231375426b81cd244ba08dbd52` |
| Same for `--platform linux/amd64` (emulated) | 131.2 s | 0 | `1.97.1-x86_64-unknown-linux-gnu`. `asset: loupe-v0.1.0-x86_64-unknown-linux-gnu.tar.gz archive_bytes: 5118180 binary_bytes: 16834544 sha256: ffd543d875028b342535763dc55ed7746fd1db4e912a3b7de3ccb5524b30653c` |
| `shasum -a 256 -c` on macOS, both files | 0.1 s / 0.0 s | 0 | `OK` both |
| `tar -tvzf`, both | 0.0 s | 0 | Same six entries as macOS, owner `0 0` (GNU tar `--owner=0 --group=0 --numeric-owner --sort=name`), `loupe` is `-rwxr-xr-x` |
| `docker run --rm --platform linux/amd64 debian@sha256:7ff6ba31… sh -c 'sha256sum -c …; uname -m; ldd --version; loupe --version'` | 0.5 s | 0 | `…x86_64-unknown-linux-gnu.tar.gz: OK`, `x86_64`, `GLIBC 2.36`, **`loupe 0.1.0`** |
| Same for linux/arm64 | 0.4 s | 0 | `…aarch64-unknown-linux-gnu.tar.gz: OK`, `aarch64`, `GLIBC 2.36`, **`loupe 0.1.0`** |
| `objdump -T loupe \| grep -o 'GLIBC_[0-9.]*' \| sort -uV \| tail -3`, both | 0.4 s / 0.3 s | 0 | Highest needed `GLIBC_2.34` on both. `NEEDED`: libgcc_s, libm, libc (and ld-linux on x86_64). The "glibc 2.35 or newer" of README/release.md holds |
| Extra: `serve` and `/healthz` in `debian:bookworm-slim` with two in-container `gen-key` keys, probed with bash `/dev/tcp`, amd64 | 1.1 s | 0 | `HTTP/1.0 200 OK`, `{"status":"ok"}`, first log line `{"fields":{"version":"0.1.0"},…"message":"loupe serve starting"…}`, exit 0 after SIGTERM |
| Same for arm64 | 0.6 s | 0 | Same result |

`debian:bookworm-slim` was referenced by its local digest (`sha256:7ff6ba31…`, which is also the current
registry digest), so no local tag could move.

### (3) Container image

| Command | Duration | Exit | Observed |
|---|---|---|---|
| `release.image.sh --tag v0.1.0 --dry-run` | 0.4 s | 0 | `plan: … check: passed at: HEAD (tag v0.1.0 does not exist yet) image: ghcr.io/desek/agentloupe created: 2026-10-23T17:49:46Z`. The build command line has the seven `--label`s and `--output type=image,name=ghcr.io/desek/agentloupe,push-by-digest=true,name-canonical=true,push=true --metadata-file … --file Dockerfile .`. The merge command has the seven `index:` annotations and `--tag …:0.1.0 --tag …:latest` |
| `release.image.sh build --tag v0.1.0 --digest-dir …` (no shim) | 0.0 s | 4 | Tag-absent error, before any docker call |
| `docker buildx create --name loupe-relcheck --driver docker-container --bootstrap` (an isolated empty-cache builder, as `setup-buildx-action` makes) | 6.1 s | 0 | It pulled `moby/buildkit:buildx-stable-1`, which moved that existing local tag. The tag was restored at cleanup (see finding F9) |
| `docker buildx build --builder loupe-relcheck --platform linux/arm64 <the 7 labels of the dry run, verbatim> --output type=docker,name=loupe-relcheck:0.1.0-arm64 --metadata-file … --file Dockerfile .` | 66.7 s | 0 | `containerimage.digest sha256:0b281882…`. Base images resolved from the registry: `rust:slim-bookworm@sha256:86234e85…` and `debian:bookworm-slim@sha256:7ff6ba31…`. `rustup show active-toolchain` auto-installed `1.97.1-aarch64-unknown-linux-gnu` (7.6 s) with a deprecation warning. ``Finished `release` … in 49.60s`` |
| Same with `--platform linux/amd64` (emulated) | 146.2 s | 0 | `containerimage.digest sha256:56e5686c…`. ``Finished … in 2m 05s`` |
| `docker image inspect` both | 0.1 s | 0 | See the table below |

| Platform | Content size (compressed) | Disk usage (unpacked, after run) | Image ID | User | Entrypoint / Cmd | Healthcheck |
|---|---|---|---|---|---|---|
| linux/amd64 | 33,468,364 B (33.5 MB) | 136 MB | `sha256:56e5686c…` | `loupe` (uid 10001) | `[loupe]` / `[serve]` | bash `/dev/tcp` GET /healthz |
| linux/arm64 | 33,378,747 B (33.4 MB) | 158 MB | `sha256:0b281882…` | `loupe` (uid 10001) | `[loupe]` / `[serve]` | same |

OCI labels. They are the same on both platforms and match the table in `release.md`:

```
org.opencontainers.image.created=2026-10-23T17:49:46Z      (commit time of 84c2284 in UTC, not the wall clock)
org.opencontainers.image.description=A minimal AI gateway that gives you the illusion that you can keep up with your agents.
org.opencontainers.image.licenses=MIT
org.opencontainers.image.revision=84c22846fc6f9cd5deddd5341b4a2effb4f8e030
org.opencontainers.image.source=https://github.com/desek/agentloupe
org.opencontainers.image.title=Agentloupe
org.opencontainers.image.version=0.1.0
```

The amd64 image was run with the settings of `compose.yaml`: restart `unless-stopped`, port on 127.0.0.1, a
named volume at `/data`, `env_file`, and `LOUPE_ENV=container`. The differences were a free port instead of
8787, a scratch volume `loupe-relcheck-data`, and a scratch env file with mode 0600. The env file held
`LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`, each 44 characters that decode to 32 bytes, from
`docker run --rm loupe-relcheck:0.1.0-amd64 gen-key`. The repository `.env` was never read.

| Command | Duration | Exit | Observed |
|---|---|---|---|
| `docker run -d --name loupe-relcheck-amd64 --platform linux/amd64 --restart unless-stopped -p 127.0.0.1:57906:8787 -v loupe-relcheck-data:/data --env-file $S/scratch.env -e LOUPE_ENV=container loupe-relcheck:0.1.0-amd64` | 0.4 s | 0 | Container started |
| Poll `curl http://127.0.0.1:57906/healthz` | 0.86 s to first 200 | – | `200`, `{"status":"ok"}`. After 8 s, `State.Health.Status = healthy` (the Dockerfile HEALTHCHECK works) |
| `docker logs … \| head -1` | – | – | `{"fields":{"version":"0.1.0"},"file":"src/cli/serve.rs:39","level":"INFO","message":"loupe serve starting","time":"2026-10-23T18:05:44.680Z"}`. Later lines: `db /data/loupe.db` with 19 migrations, then `listening 0.0.0.0:8787` |
| `docker exec … sh -c 'id; uname -m; ls -la /data; loupe --version'` | – | 0 | `uid=10001(loupe)`, `x86_64`, `/data` owned by `loupe`, `loupe 0.1.0` |
| Extra: `docker run --rm --platform linux/arm64 loupe-relcheck:0.1.0-arm64 --version` | 0.3 s | 0 | `loupe 0.1.0` |
| Extra: arm64 image started the same way on a free port | – | – | `/healthz` 200 `{"status":"ok"}`, first log line holds `"version":"0.1.0"` |
| `docker run --rm --platform linux/amd64 loupe-relcheck:0.1.0-amd64 --version` | 0.4 s | 0 | `loupe 0.1.0` |

### (4) release.check.sh

All runs used the bash 3.2.57 of macOS (`/bin/bash`).

| Command | Duration | Exit | Observed |
|---|---|---|---|
| `release.check.sh --version 0.1.0 --dry-run` | 0.0 s | 0 | Six `plan: asset …` lines with the correct names and the `https://github.com/desek/agentloupe/releases/download/v0.1.0/…` URLs, `plan: image ghcr.io/desek/agentloupe:0.1.0 login: none`, `plan: health … cap_secs: 30` |
| `… --dry-run --human` | 0.0 s | 0 | The same plan as sentences |
| `… --version v0.1.0 --dry-run` | 0.0 s | 2 | `error: the version v0.1.0 is not of the form <major>.<minor>.<patch> with no v …` |
| `… --version 0.1.0 --dir $S/checkdir`, with the six local assets copied in (a file already in `--dir` is not downloaded) | 2.6 s | 4 | `download passed` (no file fetched); `checksum … passed` ×3 with the three digests above; `version aarch64-apple-darwin passed: loupe 0.1.0`; `version x86_64-unknown-linux-gnu passed: loupe 0.1.0 on linux/amd64`; `version aarch64-unknown-linux-gnu passed: loupe 0.1.0 on linux/arm64`; `pull failed: … ghcr.io/desek/agentloupe:0.1.0: not found`; `result: failed exit: 4` |

What it can verify before a release is published: the plan and argument checks (`--dry-run`). Through
`--dir` it can also verify the checksum format and match of each asset, `--version` of the host binary, and
`--version` of each Linux binary in `debian:bookworm-slim` of its platform. It cannot verify the download
from GitHub, the anonymous pull, `latest`, `/healthz`, or the start log version, because it always pulls
`ghcr.io/desek/agentloupe:<version>` and has no local-image option. The pull failure was the expected one
(nothing is published, and the repository is private), and the script stopped with exit 4 as documented.
Section (3) did the health and start-version checks by hand on the local images. The script's temporary
directory was removed by its trap.

### (5) Toolchain and base images against the pins

| Place | Rust used | How | Matches `rust-toolchain.toml` (1.97.1, rustfmt, clippy)? |
|---|---|---|---|
| This host | 1.97.1 aarch64-apple-darwin | rustup override from `rust-toolchain.toml` | yes |
| Workflow `build` job (`macos-15`, `ubuntu-22.04`, `ubuntu-22.04-arm`) | 1.97.1 | `rustup toolchain install` reads the file; the runner's preinstalled Rust is not used | yes (by construction; not observed on a runner) |
| Local Linux builds (`rust:1.97.1-bookworm`) | 1.97.1 | image default and the file agree; rustup added clippy | yes |
| Dockerfile build stage, `FROM rust:slim-bookworm` (floating; registry digest `86234e85…`, created 2026-10-21) | base ships **1.99.1**, build uses **1.97.1** | `rustup show active-toolchain` auto-installs the pin (7.6 s, deprecation warning) | yes for the binary; the base image's own Rust differs and is unused |
| Pre-existing local `rust:slim-bookworm` (`16113eb5…`, used by local compose builds with the docker driver) | 1.99.1, rustup 1.29.1 | same mechanism | yes for the binary |
| Dockerfile runtime `FROM debian:bookworm-slim` (floating; digest `7ff6ba31…`) | – | glibc 2.36 | consistent with the binary built on bookworm in the same Dockerfile |
| Workflow `image` job (`ubuntu-24.04`, `ubuntu-24.04-arm`) | from the Dockerfile | native per platform, no QEMU | yes |
| `.tool-versions` | no Rust entry; `nodejs 22.20.0`, `cargo-nextest 0.9.149` | used by the verdict jobs `web` and `test` only | no overlap with `rust-toolchain.toml`, so no conflict. Neither the release build nor the Dockerfile uses Node or nextest |

Other differences:
- The Linux release assets are built on Ubuntu 22.04 (glibc 2.35), while the image binary is built on
  Debian bookworm (glibc 2.36). The release assets and the image therefore contain different builds of the
  same commit. This is by design (release.md, item 6). The measured need is GLIBC_2.34, so both platforms
  are covered.
- The macOS SDK is 27.0 here and 15.x on `macos-15`. `minos` is 11.0 in both cases, from the rustc default,
  so there is no user-visible effect.

### Cleanup

The cleanup took 3 s:
- Removed the containers `loupe-relcheck-amd64` and `loupe-relcheck-arm64` (`rm -f -v`), and the volumes
  `loupe-relcheck-data` and `loupe-relcheck-data-arm64`.
- Removed the builder `loupe-relcheck` (`docker buildx rm`, together with its buildkit container and its
  4.9 GB state volume).
- Removed the images `loupe-relcheck:0.1.0-amd64`, `loupe-relcheck:0.1.0-arm64` and `rust:1.97.1-bookworm`.
- Put the tag `moby/buildkit:buildx-stable-1` back on its previous image `28a89871…` and deleted the newly
  pulled `cec9f139…`.
- Deleted the scratch directory (1.6 GB) and the transcript.

After cleanup, the lists of containers, images, volumes, networks and buildx builders were identical to
the lists taken before the run. `debian:bookworm-slim` still has its 254 MB of local content. No
`loupe serve` or check process is left. In the repository, no tag was made and no commit is mine; the
only file written is this report.

### Artifact table

| Name | Platform | Size | Checksum present | `--version` | `/healthz` | Start log version |
|---|---|---|---|---|---|---|
| `loupe-v0.1.0-aarch64-apple-darwin.tar.gz` | macOS arm64 (host) | 4,645,682 B archive / 14,402,208 B binary | yes, `.sha256` verifies (`7e7018b0…`) | `loupe 0.1.0` | 200 `{"status":"ok"}` | `0.1.0` |
| `loupe-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` | linux/amd64 (bookworm-slim, emulated) | 5,118,180 B / 16,834,544 B | yes, verifies with shasum and sha256sum (`ffd543d8…`) | `loupe 0.1.0` | 200 `{"status":"ok"}` (extra check) | `0.1.0` |
| `loupe-v0.1.0-aarch64-unknown-linux-gnu.tar.gz` | linux/arm64 (bookworm-slim) | 5,069,279 B / 16,341,728 B | yes, verifies with shasum and sha256sum (`30d80f84…`) | `loupe 0.1.0` | 200 `{"status":"ok"}` (extra check) | `0.1.0` |
| image `ghcr.io/desek/agentloupe:0.1.0` (local `loupe-relcheck:0.1.0-amd64`) | linux/amd64 | 33.5 MB content / 136 MB unpacked | n/a (digest `sha256:56e5686c…`); 7 OCI labels correct | `loupe 0.1.0` | 200 `{"status":"ok"}`, Docker health `healthy` | `0.1.0` |
| image (local `loupe-relcheck:0.1.0-arm64`) | linux/arm64 | 33.4 MB content / 158 MB unpacked | n/a (digest `sha256:0b281882…`); 7 OCI labels correct | `loupe 0.1.0` | 200 `{"status":"ok"}` (extra check) | `0.1.0` |

### Findings

| # | Severity | Finding | Recommended action |
|---|---|---|---|
| F1 | low | `release.build.sh` and `release.image.sh` have no real-build mode before a tag exists. A local rehearsal of the first release needs a git shim (as here) or a throwaway tag. `release.image.sh build` can only push. | Add a rehearsal flag, for example `--at-head` (uses HEAD and states it, as `--dry-run` does) for both scripts, and `--load <local tag>` for the image build. Or document the shim method in `release.md`. |
| F2 | low | `release.check.sh` cannot check the health or the start-log version of a local image. It always pulls `ghcr.io/desek/agentloupe:<version>`, so before publication it stops at `pull` with exit 4. | Add `--image <ref> --no-pull` (skip `pull` and `latest`, keep `health` and `start-version`), so the same script checks a local build. |
| F3 | low | When every file is already in `--dir`, `release.check.sh` reports `check: download status: passed detail: 6 files into <dir>`, although nothing was downloaded. | Report `detail: 0 downloaded, 6 present in <dir>`, or `skipped` with that reason. |
| F4 | low | The Dockerfile's `RUN rustup show active-toolchain \|\| rustup toolchain install` relies on rustup's deprecated auto-install, and the build log warns about it. It still works, and the `\|\|` covers a future rustup. | Use `RUN rustup toolchain install`, the same command as the workflow. |
| F5 | note | `rust:slim-bookworm` and `debian:bookworm-slim` are floating tags. The base now ships Rust 1.99.1, so each clean image build downloads the 1.97.1 toolchain (about 7.6 s), and two builds of one tag can get different base layers even though the `created` label is the same. | Accept, or pin both `FROM` lines by digest (`@sha256:…`). A digest pin does not state a Rust version, so it fits release.md's "no version in the Dockerfile" rule. |
| F6 | note | These Linux assets were built on bookworm (glibc 2.36); the workflow builds on Ubuntu 22.04 (2.35). The measured floor is GLIBC_2.34, so the README claim holds for this build. A runner build can only lower the floor. | No action. Optionally run `objdump -T` on the first published Linux assets. |
| F7 | note | This machine cannot show that the `ubuntu-22.04-arm` and `macos-15` runners have `rustup` on PATH, which `rustup toolchain install` needs. | Read the first release run's `build` logs for the three runners. |
| F8 | note | The archives are not byte-reproducible, because the file mtimes are the build time. The spec does not require reproducibility. | Optional: set the mtime from the commit time (`--mtime` with GNU tar, `touch -t` before bsdtar). |
| F9 | note (side effect of this check, reverted) | `docker buildx create` pulled and retagged the shared local `moby/buildkit:buildx-stable-1`. The tag was restored to `28a89871…`. | For future local rehearsals, add `--driver-opt image=moby/buildkit@sha256:<digest>` or use the default docker builder. |
| F10 | note | Under emulation on this arm64 host, the amd64 builds took 131 s (asset) and 146 s (image), against 61.5 s and 66.7 s for native arm64. The workflow uses native runners for each platform, so this affects only local rehearsals. | No action. |

**Verdict:** the artifacts work as intended. All three binary assets and both platform images build with
the workflow's commands and pins. Each has a valid checksum file or the correct OCI labels, prints
`loupe 0.1.0`, answers `/healthz` with 200 `{"status":"ok"}`, and logs `version 0.1.0` on its first start
line. No item failed. The `release.check.sh` pull failure is expected, because nothing is published yet.
