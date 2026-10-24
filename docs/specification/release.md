<!-- @agents-index Specification of the release: version rules, bump rules, tag forms, release flow with release-please, release assets, release build profile, SBOM files, third-party notices, commit rule, release token, repository settings, the release check, the advisory response, and the telemetry endpoint. -->
# Release

This file is the one source of the version rules of the project. It states what a version is, how a version changes, how a release is made, what a release publishes, which repository settings protect the release, and how a user verifies a release.

## Version

The project uses semantic versioning. A version has the form `<major>.<minor>.<patch>`. Each part is a decimal number with no leading zero.

1. The `version` field of the `[package]` table in `Cargo.toml` is the one source of the version.
2. Only the merge of the release pull request changes this field. No other commit changes it.
3. No source file, script, workflow file, or configuration file states the version. The exceptions are the files that release-please writes from `Cargo.toml`: `Cargo.lock` and `.release-please-manifest.json`. A third exception is the `release-as` value in `release-please-config.json` for the first release. The commit after the first release removes it.
4. A script or a test that needs the version reads it from `Cargo.toml`, from the tag, or from its input.
5. `CHANGELOG.md`, the change requests, and the scenarios record past versions. They are not a source of the version.
6. The first public release is `0.1.0`.

### Meaning of `target-version` in a change request

The `target-version` field of a change request is an internal plan. It is not a released version. No tag is made for a `target-version` value. Only release-please makes a version, from the commits on `main`.

### Version shown by the binary

`loupe --version` prints `loupe <version>`, then a newline, on stdout, and exits with code 0. The version comes from `Cargo.toml` at build time.

The first log line of `loupe serve` holds the key field `version` with the same value. `operations.md` (Logging) states the line.

The body of `GET /healthz` does not hold the version. The dashboard does not show the version.

A build with a telemetry host also holds the full commit of the build and the target triple of the build. The binary uses them only in the start request (`telemetry.md`). `loupe --version` and the first log line do not show them. A build with no telemetry host holds no commit of the project.

## Bump rules

release-please reads the Conventional Commits on `main` since the last release, and proposes the next version.

From `1.0.0`:

| Commit | Bump |
|---|---|
| `fix` | patch |
| `feat` | minor |
| a breaking change (`feat!`, `fix!`, or a `BREAKING CHANGE:` footer) | major |

Below `1.0.0`:

| Commit | Bump |
|---|---|
| `fix` | patch |
| `feat` | minor |
| a breaking change | minor, not major |

Below `1.0.0`, the configuration sets `bump-minor-pre-major` to true and `bump-patch-for-minor-pre-major` to false. Thus a breaking change cannot publish `1.0.0` by accident.

The move to `1.0.0` is a decision of a human. The human makes it with the commit footer `Release-As: 1.0.0`.

A commit of a hidden type (see "Release notes") does not cause a bump. When no releasable commit exists since the last release, release-please proposes no release.

## Tag and image tag

| Item | Form | Example |
|---|---|---|
| Git tag | `v<major>.<minor>.<patch>`, digits only, no pre-release suffix, no component name | `v0.1.0` |
| Image tag | `<major>.<minor>.<patch>`, with no `v` | `0.1.0` |
| Moving image tag | `latest` | `latest` |

The release workflow starts only on a tag of the git tag form. It ignores a tag with a suffix.

## Release flow

1. Each push to `main` starts the release-please workflow. It reads the configuration files `release-please-config.json` and `.release-please-manifest.json` at the root. The release type is `rust` for the root package.
2. release-please keeps at most one open release pull request. Its head branch is `release-please--branches--main--components--agent-loupe`, the name that release-please makes for the package `agent-loupe`. Its title has the form `chore(release): release <version>`. A new releasable commit on `main` updates the same pull request.
3. The release pull request changes only `Cargo.toml`, `Cargo.lock`, `.release-please-manifest.json`, and `CHANGELOG.md`.
4. The verdict workflow runs on the release pull request, because release-please uses the release token (see "Release token").
5. A human, or the dependency autofix, squash merges the release pull request. The main ruleset (see "Repository settings") lets the merge happen only when the check `verdict` passed. The type `chore` of the title keeps the merge commit out of the next release notes. The merge setting deletes the release branch after the merge. release-please makes the branch again for the next release.
6. The merge starts the release-please workflow again. It makes the tag `v<version>` on the merge commit and a GitHub release. The notes of the release are the changelog section of that version.
7. The push of the tag starts the release workflow. Each tag gets exactly one automatic run. A human can start the release workflow by hand with a tag input, to run a failed release again.
8. When the repository is private, the release workflow fails in its first job, on a Linux x86_64 runner. The error names the visibility. The run starts no macOS job, no arm64 job, and no push to the registry.
9. The release workflow checks out the tag. It runs the verdict workflow on the tag commit first. When the verdict fails, the release workflow builds nothing.
10. The release workflow builds the three binary assets and the two platform images in parallel, on native runners.
11. When all three binary builds and both image builds pass, the release workflow attaches the 16 asset files to the GitHub release (see "Release assets"). Then it pushes the image index with the tag `<version>`. Then the promote step moves the image tag `latest` to that index (see "Container image").
12. When one job fails, the release workflow fails. The release gets no partial set of assets from that run.

### Releases from the dependency autofix

The daily dependency autofix is a source of `fix(deps)` releases. `dependencies.md` states the flow.

1. The autofix merges a green fix as a `fix(deps)` squash commit on `main`. release-please then proposes a patch release in the release pull request.
2. In the same run, the autofix waits for the release-please run on the merge commit, and for the check `verdict` of the release pull request. When the check is green, the autofix squash merges the release pull request. The tag, the release, and the release workflow follow as for a merge by a human.
3. The autofix merges the release pull request only when a `fix(deps)` change is in it: in a run that merged a `fix(deps)` commit, or in a later run when the changelog section of the open release pull request lists a `fix(deps)` change.
4. The autofix never merges a release pull request that proposes `1.0.0` or a new major version. A human merges it.
5. When the check is not green before the wait cap, the release pull request stays open, and the next daily run tries again.
6. One autofix branch for each day gives at most one autofix release for each day.
7. A release that the autofix merges also carries each other unreleased releasable commit on `main`, for example a `feat` commit of the owner.

The weekly freshness flow of `dependencies.md` is a second source of `fix(deps)` releases. It uses the same shared steps and the same rules as the autofix:

1. The freshness flow merges its green updates as one squash commit on `main`. The commit is `fix(deps)` when it holds one or more `cargo` updates, and release-please then proposes a patch release. An update that changes only `prototype/` is `chore(deps)` and makes no release.
2. The freshness flow then does the same release pull request step as the autofix, with items 2 to 5 and item 7 of this list.
3. A `cargo` update of a dev dependency is also `fix(deps)`. It makes a patch release with no change of the binary.
4. One freshness branch for each week gives at most one freshness release for each week.

### First release

The changelog starts after the commit `446f065`. The configuration sets `bootstrap-sha` to this commit and `release-as` to `0.1.0`. The first release pull request proposes `0.1.0`, which equals `Cargo.toml`. Right after `v0.1.0` publishes, a `chore(release)` commit removes `release-as`. This commit goes to `main` through the admin bypass of the main ruleset. The work before the first release is in `docs/cr/`.

The repository is private before the first release. On the free plan, the rulesets and the security features exist only on a public repository. Thus the first release follows this order:

1. All file changes for the release land on `main`. Each stale remote branch is deleted.
2. The owner makes the repository public. No script and no workflow changes the visibility.
3. The owner runs `.agents/scripts/repo.settings.sh` (see "Repository settings"). Then `repo.settings.sh --check` exits with code 0.
4. The owner merges the first release pull request.

The first release pull request does not merge before step 3 is complete. The human sets the registry package to public one time after the first push. The release workflow does not change a visibility.

## Release assets

A release publishes these assets.

A release has 16 asset files: the three archives, the three archive SBOM files, the two image SBOM files, and one checksum file for each of these eight files. "Supply chain" states the SBOM files. A release made before the SBOM files has six asset files: the three archives and their checksum files.

### Binary archives

| Target | Runner | Archive | Checksum file |
|---|---|---|---|
| `aarch64-apple-darwin` | macOS arm64 | `loupe-v<version>-aarch64-apple-darwin.tar.gz` | `loupe-v<version>-aarch64-apple-darwin.tar.gz.sha256` |
| `x86_64-unknown-linux-gnu` | Linux x86_64 | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz.sha256` |
| `aarch64-unknown-linux-gnu` | Linux arm64 | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz.sha256` |

1. `.agents/scripts/release.build.sh` builds each archive with `cargo build --release --locked` for the target of the host, with the toolchain that `rust-toolchain.toml` pins. No cross tool is used. The build uses the release build profile (see "Release build profile").
2. Before the build, the script stops with an actionable error when the tag version differs from the version in `Cargo.toml` at the tag, or when the checked-out commit is not the tag commit. The error names both versions.
3. Before the build, the script calls `.agents/scripts/release.notices.sh`, which writes `THIRD-PARTY-NOTICES` (see "Third-party notices").
4. Each archive holds `loupe`, `upstreams.toml`, the `config/` directory, `LICENSE`, `README.md`, and `THIRD-PARTY-NOTICES`. The binary reads `upstreams.toml` and `config/<env>.toml` at run time. An archive also keeps the execute bit of the binary.
5. Each checksum file holds one line in the format of `sha256sum`: the lowercase hex SHA-256 digest of the archive, two spaces, and the archive name. `shasum -a 256 -c` on macOS and `sha256sum -c` on Linux accept the file.
6. The script prints one machine line for each asset with the archive name, the size of the archive, the size of the binary, the digest, the name of the archive SBOM file, and the count of its components. `--dry-run` prints the plan and builds nothing.
7. The Linux binaries build on runner images with glibc 2.35. A Linux host needs glibc 2.35 or newer. The runtime image `debian:bookworm-slim` has glibc 2.36.
8. The macOS binary is not signed and not notarized. A browser download can get the quarantine attribute. `README.md` (Install) states the command that removes it.
9. The build job passes the repository variable `LOUPE_TELEMETRY_HOST` and the full commit of the checked-out tag as `LOUPE_BUILD_COMMIT` to the script, and the script passes both to the build. The commit is the tag commit, not the commit of the workflow event, because a manual run starts from the default branch. Thus each release asset is `built-in` and sends the start request. A build from source with no `LOUPE_TELEMETRY_HOST` is `built-out` and sends nothing.
10. The machine output of the script names the telemetry state of each build: `built-in` with the host, or `built-out`. When the repository variable is not set, the release does not fail, and each build is `built-out`.
11. After the build, the script writes the archive SBOM file and its checksum file into its output directory, next to the archive (see "Supply chain"). When the SBOM step fails, the script stops with exit code 1, so no archive leaves the build job with no SBOM file.

### Container image

| Item | Value |
|---|---|
| Image name | `ghcr.io/desek/agentloupe` |
| Platforms | `linux/amd64`, `linux/arm64` |
| Tags | `<version>` and `latest` |

1. `.agents/scripts/release.image.sh` calls `release.notices.sh` before the build. It builds the image from the root `Dockerfile` with no change to the `Dockerfile`. It builds `linux/amd64` on a Linux x86_64 runner and `linux/arm64` on a Linux arm64 runner.
2. The release image holds `/app/LICENSE` and `/app/THIRD-PARTY-NOTICES`. After the build and before the push, the script checks that the built image holds both files. When one file is absent, the script stops with exit code 1 and pushes nothing.
3. The script pushes each platform image by digest. After the push, it writes the image SBOM file of that platform and its checksum file into the directory that the flag `--sbom-dir` names (see "Supply chain"). When the SBOM step fails, the script stops with exit code 1.
4. The mode `merge` merges the two platform digests into one multi-platform image index with the tag `<version>` only. It runs only when both platform builds pass.
5. The mode `promote --tag <tag>` moves the tag `latest` to the index digest of the tag `<version>`. The publish job of the release workflow calls it after the merge. The advisory response calls it to move `latest` to a fixed version (see "Advisory response"). The mode moves `latest` only forward: it refuses a move to a version that is older than the version that `latest` points to, with exit code 1, and `latest` does not move. The mode does not need the checkout of the tag commit, because the advisory response runs from a checkout of the default branch. It stops with exit code 4 when the tag does not exist, or when the version in `Cargo.toml` at the tag differs from the tag version.
6. The image build turns off the provenance and SBOM attestations of BuildKit. Thus the pushed image holds no unsigned provenance. The image SBOM is a release file, not an attestation in the image index.
7. The image name is one constant in this script and in this file.
8. `--dry-run` prints the commands and runs nothing.
9. The `Dockerfile` accepts `LOUPE_TELEMETRY_HOST` and `LOUPE_BUILD_COMMIT` as build arguments in the build stage only, each with an empty default. The image job passes the repository variable and the full commit of the tag to the script, and the script passes both to the build. The runtime stage does not get them. Thus the release image is `built-in` and sends the start request. The build context holds no git data, so the commit comes only from the build argument.
10. A local image build with no build argument, by the image stage or by `docker compose up -d --build`, is `built-out` and sends nothing.
11. The machine output of the script names the telemetry state of the image: `built-in` with the host, or `built-out`.
12. The image carries the OCI labels in the table below. The script sets them at build time.

| Label | Value |
|---|---|
| `org.opencontainers.image.source` | `https://github.com/desek/agentloupe` |
| `org.opencontainers.image.version` | the version |
| `org.opencontainers.image.revision` | the full commit of the tag |
| `org.opencontainers.image.licenses` | `MIT` |
| `org.opencontainers.image.title` | the title of the project |
| `org.opencontainers.image.description` | one sentence that describes the project |
| `org.opencontainers.image.created` | the commit time of the tag, not the wall clock |

Two builds of one tag get the same `created` label.

## Release build profile

The release profile of Cargo sets how `cargo build --release` compiles the `loupe` binary. Each release build uses it: `release.build.sh`, the build stage of the `Dockerfile` (thus the release image and the image stage of the verdict), and the bench and scenario scripts that build with `--release`. These builds select the profile with `--release`. They need no other flag and no change of their own.

### Settings

The section `[profile.release]` of `Cargo.toml` holds exactly these five settings.

| Setting | Value | Reason |
|---|---|---|
| `strip` | on | The binary holds no symbol table and no debug info. A reader with `nm` does not see the names of the modules and the functions. The binary gets smaller. |
| `lto` | fat | The link step optimizes across all crates. This gives the smallest binary and the widest inlining across crates. With one codegen unit, thin LTO keeps little of its parallel gain. |
| `codegen-units` | 1 | The compiler builds each crate as one unit, so the optimizer sees all the code of the crate. The binary gets smaller, and the build gets longer. |
| `panic` | abort | A panic ends the process and does not unwind the stack. The binary needs no unwind code for a panic, so it gets smaller. The unwinding check below makes this setting safe. |
| `debug` | off | The binary holds no debug info. This is the Cargo default; the setting makes it explicit. |

The section does not set the optimization level, so it stays the Cargo default 3. A level for size makes the binary smaller but slower, and the proxy is on the path of each agent call.

### Rules

1. The dev profile and the test profile stay the Cargo defaults. `Cargo.toml` has no section for the dev, test, or bench profile, and no custom profile. No Cargo configuration file, no profile environment variable, and no compiler flag variable in a script, a workflow, or the `Dockerfile` sets or overrides a setting of the profile.
2. The test stage keeps the test profile. Cargo ignores the `panic` setting in a test build.
3. The value of `lto` is fat. When the row "Image job time under the profile" or the row "Release build time under the profile" fails its threshold with fat LTO, the value becomes thin. Then each row of "Measured values" is measured again, and this section states both measurements and the reason.

### Effects that a user sees

The profile changes only these four behaviours that a user sees:

1. The binary and the archive are smaller.
2. A panic ends the whole process with the signal SIGABRT and runs no destructor. Before the profile, a panic ended only one task or one thread, and the process continued. The service of `compose.yaml` starts again by its restart policy. A native run stops, and the user starts it again. The store uses SQLite in WAL mode, which keeps each committed transaction through a process end. A queued capture that is not yet written is lost, as on a kill.
3. A backtrace holds addresses and no symbol names. The panic message keeps its text and the source file and line. A developer reproduces a panic with the dev profile, which keeps the symbols.
4. The start time to the line `listening` stays within the threshold of its row.

### Unwinding check

The `panic` setting lands only after these four checks pass on the working tree with the profile. The checks run again at each later change of the profile. The scenario of the supply chain under `.agents/scenarios/` records the commands, the results, and the class of each hit.

1. Explicit check: a search of `src` for `catch_unwind`, `resume_unwind`, `UnwindSafe`, `panic::set_hook`, and `panic::take_hook` gives no hit. A hit blocks the `panic` setting until the code no longer needs it.
2. Implicit check: a search of `src` for `spawn_blocking`, `JoinError`, `is_panic`, `into_panic`, the join of a thread, and `into_inner()` lists each place that can act after a panic of other code. Each hit gets one class:
   1. "Inert under abort": the code handles a panic or a poisoned lock that cannot occur under abort, and the process end is the accepted outcome.
   2. "Needs unwinding": the code needs a result, a cleanup, or a recovery after a panic for correct behaviour.

   A hit of the class "needs unwinding" blocks the `panic` setting. The profile then lands with the other four settings only, and this section names the hit.
3. Tests under the release profile: `cargo nextest` runs all tests with the release profile, through its flag for the Cargo profile, or through `--release` when the pinned runner does not accept that flag. The run exits with code 0 and passes the same count of tests as the test stage at the same commit. This check is not a stage of the verdict, because each test binary links with fat LTO and one codegen unit, which takes much longer than the test stage. Cargo builds the test harness with unwind, so this check proves the settings `strip`, `lto`, `codegen-units`, and `debug`, and the binary check proves the `panic` setting.
4. Binary check: the archive binary that `release.build.sh` builds with the profile prints the version with `--version`, writes the line `listening` after `loupe serve`, answers `GET /healthz` with status 200 and the body `{"status":"ok"}`, and on SIGTERM writes the line `database closed, exit` and exits with code 0. Each Linux binary passes `--version` and `GET /healthz` in `debian:bookworm-slim` of its platform.

### Measured values

Each row is measured before and after the profile with the same method: the same host, the same container image, and the same commit except `Cargo.toml`. The macOS arm64 values come from the host. The Linux values come from `rust:1.97.1-bookworm` of each platform. The values of the first release run after the profile are recorded next to them, and are not compared with them. The before and after values were measured on 2026-10-24: before at commit `75f6fe2`, after at the same tree with only the profile added (commit `c453326`), each built by `release.build.sh` in a scratch clone with a local tag `v0.1.0` that was never pushed, on the macOS arm64 host and in `rust:1.97.1-bookworm` of each Linux platform (Linux x86_64 under emulation). The scenario `.agents/scenarios/release.assurance.md` holds the run log. The values of the first release run follow the public repository.

| Metric | Method | Baseline | Threshold | Before | After |
|---|---|---|---|---|---|
| Binary size under the profile | the binary size that `release.build.sh` prints for each target | macOS arm64 14,402,208 bytes; Linux x86_64 16,834,544 bytes; Linux arm64 16,341,728 bytes (artifact check of 2026-10-23, commit `d93de7d`) | for each target: at most 85 percent of the before value, and at most 25 MiB | macOS arm64 14,456,096 bytes; Linux x86_64 16,900,424 bytes; Linux arm64 16,335,152 bytes | macOS arm64 10,071,536 bytes (69.7 percent); Linux x86_64 12,179,184 bytes (72.1 percent); Linux arm64 11,229,520 bytes (68.7 percent); passed |
| Archive size under the profile | the archive size that `release.build.sh` prints for each target | macOS arm64 4,645,682 bytes; Linux x86_64 5,118,180 bytes; Linux arm64 5,069,279 bytes (same check) | for each target: below the before value | macOS arm64 4,684,613 bytes; Linux x86_64 5,161,142 bytes; Linux arm64 5,099,403 bytes | macOS arm64 3,517,140 bytes; Linux x86_64 3,997,630 bytes; Linux arm64 3,794,807 bytes; passed |
| Start time to the line `listening` | 10 starts of the archive binary with an empty environment, two keys from `loupe gen-key`, a new temporary data directory each time, and a listen address on `127.0.0.1` with port 0; for each start, the time from the spawn to the stdout line with the message `listening`; the median; on the macOS arm64 host and on `linux/arm64` in `debian:bookworm-slim` | none; the artifact check recorded only 0.35 s to the first answer of `GET /healthz` on macOS | on each host: the after median at most 110 percent of the before median | macOS arm64 52.8 ms; `linux/arm64` 63.4 ms (the median of three interleaved rounds of 10 starts: 53.6, 52.8, 51.3 ms and 63.4, 62.7, 66.4 ms) | macOS arm64 54.8 ms (104 percent); `linux/arm64` 66.7 ms (105 percent) (rounds 53.3, 56.0, 54.8 ms and 66.7, 67.3, 59.7 ms); passed |
| Verdict under the profile | `.agents/scripts/pipeline.verdict.sh; echo $?` on the working tree with the profile, and the conclusion of the hosted verdict of its pull request | none | exit code 0 and conclusion `success` | none | `pipeline.verdict.sh` exit code 0 (446 tests, 908.1 s); hosted verdict runs 40037532419 and 40038342459 `success`; passed |
| Tests under the release profile | the run of the unwinding check, item 3; the exit code and the count of passed tests, against the test stage at the same commit | none | exit code 0; equal counts | none | `cargo nextest run --locked --no-fail-fast --cargo-profile release` (the pinned cargo-nextest 0.9.149 accepts the flag): exit code 0, 438 of 438 passed in 298 s; the test stage at the same tree: 438 of 438; passed |
| Explicit unwinding hits | the search of the unwinding check, item 1 | 0 on 2026-10-23 at commit `2561b8e` | 0 | 0 | 0; passed |
| Implicit unwinding hits that need unwinding | the search of the unwinding check, item 2; each hit classified | 15 hits on 2026-10-23 at commit `2561b8e` (13 recoveries of a poisoned lock in 9 files, 1 result of `spawn_blocking`, 1 thread join), not classified | each hit classified; 0 of the class "needs unwinding" for the `panic` setting | 15 hits at commit `75f6fe2`: 13 recoveries of a poisoned lock in 9 files, 1 result of `spawn_blocking` (`src/live/route.rs`), 1 thread join (`src/serve/mod.rs`) | 15 classified "inert under abort", 0 "needs unwinding"; passed. The poisoned lock recoveries take the guard of a lock that only a panic poisons, which under abort ends the process first. The `spawn_blocking` error arm returns 503 for a snapshot read that failed or panicked; under abort the panic ends the process, and the arm still serves a read error. The thread join waits for the capture writer to drain on a clean exit and ignores its result |
| Binary check under the profile | the binary check of the unwinding check, item 4, on the macOS arm64 binary and on both Linux binaries | none | each item passes | none | macOS arm64: `loupe 0.1.0`, the line `listening`, `/healthz` 200 `{"status":"ok"}`, `database closed, exit`, exit code 0; Linux x86_64 and Linux arm64 in `debian:bookworm-slim`: the same four items; passed |
| Symbols of the binary | the count of the lines of `nm` on the macOS arm64 archive binary | none; measured before the profile | at most 5 percent of the before count | 21,762 lines | 163 lines (0.75 percent); passed |
| Image job time under the profile | the duration of the image job of the hosted verdict on a pull request that changes one Rust file, against the test job of the same run | image 3 min 34 s, test 6 min 32 s (verdict run 39998277995) | the image job at most as long as the test job of the same run | image 3 min 34 s, test 6 min 32 s (verdict run 39998277995) | image 3 min 33 s, test 6 min 45 s (run 40037532419); image 3 min 26 s, test 6 min 34 s (run 40038342459); passed |
| Release build time under the profile | the wall time of `release.build.sh` on the macOS arm64 host with a cold target directory, and the duration of each build job of the first release run | 33.3 s on the host (artifact check) | at most 5 minutes on the host; each build job within the 45-minute budget from the tag to the published assets | 36.8 s on the host; Linux arm64 72 s, Linux x86_64 138 s (emulated), in the container | 68.3 s on the host (71.8 s with the archive SBOM step); Linux arm64 87 s, Linux x86_64 221 s (emulated); passed on the host; the build jobs of the first release run follow the public repository |

### No secrecy claim

The profile is build hygiene. It makes casual reading of the binary harder and the asset smaller. It is no secrecy claim, because a native binary is always readable with a disassembler. Its strings also stay readable, for example the log and error messages, the source file paths in panic messages, and the telemetry host that the build sets (see "Telemetry endpoint"). No text of the project, for example `README.md`, this file, `SECURITY.md`, or the release notes, states that the profile hides, protects, or obfuscates code or data. The project does not obfuscate, encrypt, or pack the binary.

## Supply chain

Each release publishes a software bill of materials (SBOM) for each archive and for each platform image. A user reads from the SBOM files which components a release holds, with no build from source. The SBOM files are release assets, each with a checksum file.

### SBOM files

| SBOM file | Covers | Tool | Made by |
|---|---|---|---|
| `loupe-v<version>-aarch64-apple-darwin.cdx.json` | the archive of `aarch64-apple-darwin` | cargo-cyclonedx | `release.build.sh` |
| `loupe-v<version>-x86_64-unknown-linux-gnu.cdx.json` | the archive of `x86_64-unknown-linux-gnu` | cargo-cyclonedx | `release.build.sh` |
| `loupe-v<version>-aarch64-unknown-linux-gnu.cdx.json` | the archive of `aarch64-unknown-linux-gnu` | cargo-cyclonedx | `release.build.sh` |
| `loupe-v<version>-image-linux-amd64.cdx.json` | the platform image `linux/amd64` | syft | `release.image.sh build` |
| `loupe-v<version>-image-linux-arm64.cdx.json` | the platform image `linux/arm64` | syft | `release.image.sh build` |

1. Each SBOM file is CycloneDX JSON, in the newest specification version that both tools write. The five files have one format.
2. Each SBOM file has one checksum file `<name>.sha256`, in the format of the archive checksum files (see "Binary archives", item 5). The release check verifies it.
3. The archive SBOM tool is cargo-cyclonedx of the CycloneDX project. It filters the dependency graph by target, so each archive gets the crates of its target. The image SBOM tool is syft of Anchore. It reads the pushed platform image from the registry by its digest.
4. `.tool-versions` pins cargo-cyclonedx and syft. syft is pinned under its name in the registry of mise. The registry of mise has no entry for cargo-cyclonedx, so it is pinned with the GitHub release backend of mise, whose versions carry the tag prefix `cargo-cyclonedx-`. No script and no workflow file states a version of these tools.

### Archive SBOM

`.agents/scripts/release.sbom.sh` writes the SBOM files. Its mode `asset` takes the tag, the target, and the output directory, and writes the archive SBOM file of that target.

1. The mode reads `Cargo.toml` and `Cargo.lock` of the checked-out tag commit. It does the same tag check as `release.build.sh` and stops with exit code 4 when the checked-out commit is not the tag commit.
2. The mode stops with exit code 1 when `Cargo.lock` changes during the run.
3. The metadata component of the file is the package `agent-loupe` with the version of the tag.
4. The file holds one component for each crate of `Cargo.lock` that the `loupe` binary uses on that target, with the name, the version, the package URL, and the checksum from `Cargo.lock`. It holds the normal and the build dependencies, and no development dependency, like the third-party notices.
5. The LiteLLM price file and the Geist fonts are not components. `THIRD-PARTY-NOTICES` names them.
6. Two runs on one commit give the same set of components by name, version, package URL, and hash. The file holds no path of the build machine. The serial number and the time stamp can differ between two runs, because the tool makes them.

### Image SBOM

The mode `image` of `release.sbom.sh` takes the tag, the platform, the digest of the pushed platform image, and the output directory. It reads the platform image from the registry by its digest and writes the image SBOM file of that platform. The file lists the operating system packages of the image and the `loupe` binary.

### SBOM script interface

`release.sbom.sh` follows the same rules as the other release scripts: the index line, a docstring with the purpose, the usage, and the parameters, a complete `--help`, a machine output on stdout by default, `--human`, actionable errors on stderr, and documented exit codes. `--dry-run` prints the plan and writes nothing.

| Exit code | Meaning |
|---|---|
| 0 | The SBOM file and its checksum file are written. |
| 1 | The SBOM step failed, or `Cargo.lock` changed during the run. |
| 2 | A usage error. |
| 3 | A tool is missing. The error names the install from `.tool-versions`. |
| 4 | The checked-out commit is not the tag commit, or the tag version differs from the version in `Cargo.toml` at the tag. |

### Parts that follow the public repository

These parts start after the switch of the repository to public. They have no date.

1. A signed build provenance attestation for the three archives, the five SBOM files, and the digest of the image index. The publish job makes it, and `latest` then moves only after the attestation of the image passes.
2. The verify of each attestation with `gh attestation verify` in the release check, bound to the repository, to the release workflow as the signer, and to the tag as the source ref.
3. An image signature with cosign, as a later step after the attestation.
4. The draft security advisory of the advisory response (see "Advisory response").

Until these parts exist, a checksum file proves only the integrity of a download, not its origin, because the same job makes the archive and its checksum file.

## Third-party notices

The `loupe` binary embeds third-party parts: the crates of `Cargo.lock`, the LiteLLM price file of `vendor/litellm/`, and the Geist fonts of `web/assets/fonts/`. Their licenses require that their notices travel with each distributed copy. Thus each archive and the release image carry the file `THIRD-PARTY-NOTICES`.

`.agents/scripts/release.notices.sh` writes `THIRD-PARTY-NOTICES` at the root of the tree. It uses cargo about. The configuration of cargo about is in `about.toml` and its template in `about.hbs`, both at the root.

1. The file is one plain text file. It holds these parts, in this order:
   1. A header that names the project and the version from `Cargo.toml`.
   2. The licenses of the crates that the `loupe` binary uses on the three release targets. The list is the union of the three targets. It includes the build dependencies and excludes the development dependencies. Each license text shows one time, with the crates that use it under it.
   3. The text of `vendor/litellm/LICENSE`, with the upstream commit from `vendor/litellm/SOURCE.md`.
   4. The text of `web/assets/fonts/OFL.txt`, with the names of the fonts.
2. Two runs of the script on one commit give byte-identical files. The file holds no time stamp, no host name, and no path of the build machine.
3. The accepted licenses of `about.toml` are equal to the allowed licenses of `deny.toml`. `operations.md` (Dependency check) states the license policy.
4. The script makes the file at release time. Git does not track the file, and `.gitignore` ignores it. A committed copy would go stale on each change of `Cargo.lock`.
5. The script follows the same rules as the other release scripts: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes. `--dry-run` prints the plan and writes nothing.

| Exit code | Meaning |
|---|---|
| 0 | The file is written. |
| 1 | A crate has a license outside the accepted list, or `vendor/litellm/LICENSE` or `web/assets/fonts/OFL.txt` is missing. The error names the crate and the license, or the missing file. |
| 2 | A usage error. |
| 3 | cargo about is missing. The error names the install from `.tool-versions`. |

`.dockerignore` allows `LICENSE` and `THIRD-PARTY-NOTICES` into the build context. Each image holds `/app/LICENSE`. Only the release image holds `/app/THIRD-PARTY-NOTICES`. A local image build, by the image stage or by `docker compose up -d --build`, passes when no `THIRD-PARTY-NOTICES` file exists, because a local image is not distributed.

The verdict does not make the notices and does not need cargo about. The release check verifies the notices (see "Verify a release").

## Release notes

The changelog and the release notes come from the commit types.

| Commit type | Section |
|---|---|
| `feat` | Features |
| `fix` | Bug Fixes |
| `perf` | Performance |
| `revert` | Reverts |
| a breaking change | the breaking changes section of release-please |

These commit types are hidden. They do not show in the notes and do not cause a bump: `checkpoint`, `docs`, `chore`, `test`, `ci`, `build`, `refactor`, and `style`.

### Commit types of dependency updates

A dependency update gets its type from what it changes. Only an update of the shipped binary makes a release.

| Update | Type | Section |
|---|---|---|
| A `cargo` update, by Dependabot, by the dependency autofix, or by the freshness flow | `fix(deps)` | Bug Fixes, a patch bump |
| An `npm` update of `prototype/`, by Dependabot, or a fix of the dependency autofix or an update of the freshness flow that changes only `prototype/` | `chore(deps)` | hidden, because the prototype is not shipped |
| A pin update of an action, by Dependabot | `ci(deps)` | hidden, because a pin does not change the binary or the image |

The squash commit of an autofix pull request or of a freshness pull request is `fix(deps)` when the pull request holds one or more `cargo` changes, and `chore(deps)` when it holds only `prototype/` changes. The squash body keeps one line for each change, with the type of that change.

## Commit rule for a change request

Most work of a change request is in hidden commits. One commit carries the change request into the release notes.

1. Each implementation step of a change request is a `checkpoint(<id>)` commit. `<id>` is the id of the change request, as in its file name under `docs/cr/`.
2. The commit that marks a change request implemented is the implemented commit. It is a `feat`, `fix`, or `feat!` commit. The same commit changes the status of the change request to `implemented`.
3. The summary of the implemented commit is the title of the change request, then its id in parentheses. For example: `feat(release): Release governance (<id>)`.
4. The type of the implemented commit follows the change:

| Change | Type |
|---|---|
| The change request corrects a behaviour that a specification already states. | `fix` |
| The change request adds or changes a behaviour. | `feat` |
| The change request breaks a documented interface: a public API route, a CLI flag, a configuration variable, or the day record schema. | `feat!` with a `BREAKING CHANGE:` footer |

## Release token

release-please uses a fine-grained personal access token. GitHub does not start a workflow from an event that the `GITHUB_TOKEN` of a workflow makes. With this token, the verdict workflow runs on the release pull request, and the tag push starts the release workflow.

1. The token has the repository permissions contents, pull requests, and issues, each read and write, on this repository only. release-please needs the issues permission for the labels of the release pull request.
2. The token is stored only as the repository secret `RELEASE_PLEASE_TOKEN`. The project has two repository secrets: this token and `CLAUDE_CODE_OAUTH_TOKEN` of the dependency autofix (`dependencies.md`, Tokens). No file holds the value of either.
3. The release-please workflow names the secret in one place.
4. The release pull request and the tag show the owner of the token as the author.
5. The host of the start request is the repository variable `LOUPE_TELEMETRY_HOST`, not a secret. The host is visible in each release binary, so it is not secret. The variable does not change the count of repository secrets. No workflow holds a Cloudflare token.
6. The dependency autofix uses the same token for its push, its pull requests, its merges, and its read of the Dependabot alerts. For this use, the token also has the repository permission "Dependabot alerts: read", and "Workflows: write" when the merge of a pin update of an action needs it. `dependencies.md` (Tokens) states the use.

Check the expiry date of the token before each release. To rotate the token, do these steps:

1. Make a new fine-grained personal access token with the same repository and the same permissions.
2. Replace the value of the repository secret `RELEASE_PLEASE_TOKEN` with the new token.
3. Push a commit to `main`, or start the release-please workflow again, and check that it passes.
4. Revoke the old token.

When the token expires, release-please fails with an authentication error. No release is made in error.

## Pinned actions

Each workflow pins each action from outside the repository by its full 40-character commit SHA, with the release tag in a comment. At each minor release, a human checks the pins for security fixes.

Dependabot opens a weekly update pull request for the pins, with the type `ci(deps)`. The type is hidden, so a pin update makes no release. Dependabot updates the SHA and the version comment. The dependency autofix merges such a pull request only when its check `verdict` is green and its diff keeps a full SHA pin. It never merges a diff that replaces a SHA with a tag or a branch name, and it never lets Claude Code change a workflow file. `dependencies.md` states the rules.

## Repository settings

The repository `desek/agentloupe` is public from the first release. A set of repository settings protects `main`, the release tags, and the users. `.agents/scripts/repo.settings.sh` applies these settings and finds drift from them.

### Desired settings

| Setting | Desired state |
|---|---|
| Main ruleset | One active branch ruleset with the name `main` on the default branch. It requires a pull request before a merge, with zero required approvals and squash as the one allowed merge method. It requires the status check `verdict` from the GitHub Actions app, with no rule that the branch is up to date. It requires a linear history. It blocks a force push and the deletion of the branch. The repository admin role is in the bypass list in the mode "always". |
| Tag ruleset | One active tag ruleset with the name `release-tags` on the tags `refs/tags/v*`. It restricts the creation, the update, and the deletion of a matching tag. The repository admin role is in the bypass list in the mode "always", so the release token, which belongs to the owner, can make the tag of a release. |
| Merge settings | Squash merge on, merge commit off, rebase merge off, delete the head branch on merge on, and auto-merge on. The squash commit title is the pull request title. The squash commit message is the list of the commit messages. |
| Actions policy | Actions enabled, all actions allowed, and actions must be pinned to a full-length commit SHA. The default workflow token is `read`. Actions cannot approve a pull request. |
| Secret scanning | Secret scanning on, and push protection on. |
| Dependabot alerts | On. The script does not change the Dependabot security updates. |
| Private vulnerability reporting | On. `SECURITY.md` names it as the one channel for a vulnerability report. |

The repository admin bypass lets the owner push the direct `checkpoint` commits and the `chore(release)` commit that removes `release-as`. It does not permit a force push of the published history of `main`. Each setting is free for a public repository on the free plan.

### Settings script

1. The script reads and writes each setting only through `gh api`, with the `gh` login of the person who runs it. The login needs the admin permission on the repository. The script needs no new secret and never prints a token.
2. The repository is one constant in the script. A flag `--repo` replaces it for a test.
3. The script has three modes:
   1. The default mode applies the settings.
   2. `--dry-run` reads each setting, prints the planned change, and writes nothing.
   3. `--check` reads each setting, prints each difference as drift, and writes nothing.
4. The script is idempotent. It compares each setting with the desired state and writes only a setting that differs. A second run directly after a successful run makes zero write calls.
5. The script finds a ruleset by its name and updates that ruleset in place. It never makes a second ruleset with the same name. It never deletes a ruleset that it does not own.
6. When the repository is private, the default mode and `--check` stop before any write with exit code 5 and an error that names the visibility. `--dry-run` reports each setting that it cannot read on a private repository as `unavailable` with the reason, and exits with code 0.
7. The script never changes the visibility of the repository. Only the owner does that.
8. The script never runs in a workflow, and no workflow calls it.
9. The script runs with the bash 3.2 of macOS and `gh`. It does not need `jq`.
10. The script follows the same rules as the release scripts: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes.

The script prints one machine line for each setting. The line holds the name of the setting and one status. When the status is not `unchanged`, the line also holds a detail.

| Status | Meaning |
|---|---|
| `unchanged` | The setting equals the desired state. |
| `applied` | The script wrote the setting in this run. |
| `planned` | `--dry-run` found a difference and wrote nothing. |
| `drift` | `--check` found a difference and wrote nothing. |
| `unavailable` | The script cannot read the setting. The detail gives the reason. |

The last line gives the count of writes and the result.

| Exit code | Meaning |
|---|---|
| 0 | Success. |
| 1 | `--check` found drift. |
| 2 | A usage error. |
| 3 | `gh` is missing, or the `gh` login has no admin permission on the repository. |
| 4 | An API call failed. |
| 5 | The repository is private, in the default mode or in `--check`. |

## Verify a release

`.agents/scripts/release.check.sh --version <version>` checks a published release on a local machine. It runs with the bash 3.2 of macOS, the default tools of macOS, `curl`, and Docker. It does not need `jq` or `gh`.

The script does these checks:

1. It downloads the three archives, the five SBOM files, and the eight checksum files of the release from `github.com/desek/agentloupe` with `curl`, with no credential, into a temporary directory. A missing SBOM file or SBOM checksum file is not a download failure: its `sbom` check fails.
2. It verifies each checksum. A mismatch or a missing file fails the check.
3. It checks each SBOM file. A check passes when the text holds the CycloneDX format marker and at least one component, and, for an archive SBOM file, the metadata component `agent-loupe` with the version of the release. It prints one check line `sbom <file>` for each of the five files. A missing SBOM file fails its check.
4. It unpacks the archive of the host platform, runs `loupe --version`, and compares the output with `loupe <version>`.
5. It runs `loupe --version` of each Linux archive in a `debian:bookworm-slim` container of the matching platform. When Docker is absent, it reports `skipped` with the reason.
6. It pulls `ghcr.io/desek/agentloupe:<version>` with no login. When `<version>` is the newest release, it checks that the tag `latest` has the same digest.
7. It starts the image with two keys from `loupe gen-key` of the same image, bound to `127.0.0.1` on a free port, with no named volume. It polls `GET /healthz` with a cap of 30 seconds. It checks the status 200, the body `{"status":"ok"}`, and the field `version` in the first log line.
8. It checks the third-party notices. It prints one check line `notices <target>` for each of the three archives and one check line `notices image` for the image. A check passes when the file `THIRD-PARTY-NOTICES` is present and not empty, holds the LiteLLM section and the Geist section, and has the same SHA-256 digest as the other copies. When Docker is absent, `notices image` reports `skipped` with the reason.
9. It stops and removes the container and the temporary directory on each exit path.

The flag `--skip-supply-chain` is for a release made before the SBOM files. With the flag, the script does not download the SBOM files and their checksum files, and reports each `sbom` check as `skipped` with the reason "skipped by flag". Without the flag, a missing SBOM file fails the check. A failed `sbom` check gives exit code 1.

The verify of the attestations follows the public repository (see "Supply chain").

The script starts each binary and the image with the run-time opt-out of the start request set, so a release check sends no start request and does not count as a start of a user. In the image start check, it checks that the log holds the line `telemetry` with the state `disabled` or `built-out`.

The script never prints a key that it makes. It prints one machine line for each check, with `status: passed`, `status: failed`, or `status: skipped`, and `--human` gives the friendly view.

| Exit code | Meaning |
|---|---|
| 0 | Each check passed or is skipped with a stated reason. |
| 1 | A check failed. |
| 2 | A usage error. |
| 3 | A tool is missing. |
| 4 | A download or a pull failed. |

The release check uses the network. It is not part of the verdict.

## Advisory response

A new advisory can affect a crate in the `Cargo.lock` of a release that is already published. The advisory response finds each such hit, warns the readers of the affected release, and points them to a fixed version. It never deletes anything. `.agents/scripts/advisory.respond.sh` holds all the logic of the response.

The script, its dry run, and the daily workflow `advisory-response.yml` that calls it exist now. The workflow has no published release to check before the public repository, so its runs end with no hit. The draft security advisory follows the public repository: on a private repository the script reports the draft action as `blocked` and needs no token for it. On a public repository, a run that writes stops with exit code 3 before any write when the token of the draft advisory is absent; the workflow passes no such token yet.

### Check of the published releases

1. The script lists the published releases of the repository whose tag has the form `v<major>.<minor>.<patch>`. It skips a draft release and a pre-release.
2. For each release, the script exports the tree of its tag with `git archive` into a temporary directory. There it runs the advisory check of cargo deny against the `Cargo.lock` of the tag. It does not change the working tree, the index, or the branches of the clone.
3. The policy is the `deny.toml` of the checked-out default branch, so an ignore entry that the owner adds today also applies to the past releases. The flag `--deny-config` names another policy file, for the tests and the seeded scenario.
4. The script fetches the advisory database one time in a run. Then it checks each release offline against that database. When the fetch or the read of the database fails, the script stops with exit code 5 and claims no clean result.
5. Only an advisory of the kind vulnerability is a hit. An unmaintained, unsound, or informational notice and a yanked crate do not start a response. An ignore entry of the policy applies.
6. The notes of each release carry one hidden marker with the ids of the advisories that the script already handled for that release. A hit whose advisory id is in the marker of its release gives no new action. Thus a run is idempotent, and a later advisory on a marked release still gets a response.

### Response to a new hit

For each new hit, the script does these actions:

1. Warning note: the script adds one warning note at the top of the notes of each affected release, through the releases API. The note names the advisory id, the crate, the upstream link, the link to the issue, and the fixed version when it exists. The earlier notes text stays unchanged below the note. The same write updates the marker.
2. Issue: for each upstream advisory id, the script keeps exactly one open issue with the label `security`. It opens the issue, or comments on it when it exists. The title has the form "Security: <advisory id> in <crate> affects published releases". The issue links the upstream advisory and the draft advisory, lists the affected versions, and states the fix path: the daily dependency autofix workflow when it exists, else the manual fix steps below. The script makes the label `security` when it is absent.
3. Draft advisory: for each upstream advisory id, the script makes one draft repository security advisory, or updates the existing draft of that id. The draft names the upstream advisory id and its aliases, the crate and its version, the upstream link, the affected versions of this project, and the fixed version when it exists. The ecosystem is "other" with the package name `agentloupe`, because the crate is not published. The severity is the upstream severity when it exists. The script never publishes the draft and never requests a CVE. A human does that. This action follows the public repository. It needs a separate token with only the permission for repository security advisories on this repository.

### Fix release and the tag latest

1. A fix release for an advisory is a published release whose version is greater than each affected version, and whose own check gives no hit for that advisory.
2. When a fix release exists, the script names the fixed version in the note of each affected release, in the draft, and in the issue.
3. Then the script makes sure that the image tag `latest` points to the newest published release that has no hit for that advisory and that is newer than each affected release. When `latest` points elsewhere, the script moves it with the promote step of `release.image.sh` (see "Container image").
4. The script moves `latest` only after the fix release exists, and only forward to a fixed version. It never moves `latest` to an affected version or to a version older than the version that `latest` points to. An older version can lack the fixes of a newer one, and a backward move surprises a user of `latest`.
5. When the fix release exists, `latest` points to it, and the draft names the fixed version, the script comments on the issue with the link of the run and closes the issue.

### No delete rule

The script never deletes or replaces a release, a release asset, a git tag, an image tag, or a package version. It never changes the draft state or the pre-release state of a release. The only image tag that it moves is `latest`. It never uploads an asset.

Reason: a reader of the dashboard, or a user who pins a version, can need an old version. A deleted asset breaks each pinned install and each checksum link. The warning note and the advisory tell that user the risk, and the version stays available.

### Limit

GitHub has no yank for a release. A published release stays downloadable, and its image tag stays pullable. The response is a warning note, a draft advisory that a human publishes, an issue, and the move of `latest` to the fixed version.

### Manual fix steps

When no autofix workflow makes the fix, the owner does these steps:

1. Make a `fix(deps)` commit that updates the affected crate in `Cargo.lock` to a patched version.
2. Open a pull request. Squash merge it when the check `verdict` passes.
3. Merge the release pull request that release-please opens for the patch release.
4. The next run of the advisory response finds the fix release, updates the note, the draft, and the issue, and moves `latest`.

### Dry run

With `--dry-run`, the script lists the releases and does the checks of "Check of the published releases". It prints each planned action: the draft, the note, the issue, and the move of `latest`. It makes no write call to the GitHub API, no write to the registry, and no write to git.

### Advisory script interface

The script follows the same rules as the release scripts: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes. It prints one machine line for each release, with the tag, the status `clean`, `hit`, or `marked`, and the advisory ids. It prints one line for each action and one summary line with the counts. It never prints a token.

| Exit code | Meaning |
|---|---|
| 0 | No new hit, or each action is done or planned. |
| 1 | One or more actions failed. |
| 2 | A usage error. |
| 3 | A tool or a credential is missing. |
| 4 | A GitHub API call failed. |
| 5 | The read of the advisory database failed. |

## Telemetry endpoint

The release builds send the start request to the telemetry endpoint. `telemetry.md` gives the full rules.

1. The endpoint is the Cloudflare Worker `agentloupe-telemetry` on the custom domain `telemetry.agentloupe.dev` of the owner's zone `agentloupe.dev`. Its `workers.dev` subdomain and its preview URLs are off. Its source is under `telemetry/`.
2. The Worker has one route: a `GET` of `/` and a commit of 40 lowercase hex characters. It answers 204 with no body after one atomic insert-or-update, 404 for each other path or method with no write, and 503 with no body when the store fails.
3. The store is the D1 database `agentloupe-telemetry` with one table `starts` of four columns: `sha`, `firstseen`, `lastseen`, and `count`. The dates are UTC dates.
4. The Worker stores no IP address, no `user-agent`, no header, no query, no time of day, and no country. Its logs are off. It has no read route.
5. `.agents/scripts/telemetry.deploy.sh` tests and deploys the Worker through `wrangler`, which `.tool-versions` pins as `npm:wrangler 4.100.0`. Only the owner runs the script, with the owner's consent. No workflow runs it.
6. The owner sets the repository variable `LOUPE_TELEMETRY_HOST` to the host that the deploy script prints, `telemetry.agentloupe.dev`. The release builds read it (see "Binary archives" and "Container image"). Until the owner sets it, each release is `built-out`.
