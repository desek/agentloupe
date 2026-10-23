<!-- @agents-index Specification of the release: version rules, bump rules, tag forms, release flow with release-please, release assets, commit rule, release token, and the release check. -->
# Release

This file is the one source of the version rules of the project. It states what a version is, how a version changes, how a release is made, what a release publishes, and how a user verifies a release.

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
2. release-please keeps at most one open release pull request. Its title has the form `chore(release): release <version>`. A new releasable commit on `main` updates the same pull request.
3. The release pull request changes only `Cargo.toml`, `Cargo.lock`, `.release-please-manifest.json`, and `CHANGELOG.md`.
4. The verdict workflow runs on the release pull request, because release-please uses the release token (see "Release token").
5. A human reviews the release pull request and squash merges it. The type `chore` of the title keeps the merge commit out of the next release notes.
6. The merge starts the release-please workflow again. It makes the tag `v<version>` on the merge commit and a GitHub release. The notes of the release are the changelog section of that version.
7. The push of the tag starts the release workflow. Each tag gets exactly one automatic run. A human can start the release workflow by hand with a tag input, to run a failed release again.
8. When the repository is private, the release workflow fails in its first job, on a Linux x86_64 runner. The error names the visibility. The run starts no macOS job, no arm64 job, and no push to the registry.
9. The release workflow checks out the tag. It runs the verdict workflow on the tag commit first. When the verdict fails, the release workflow builds nothing.
10. The release workflow builds the three binary assets and the two platform images in parallel, on native runners.
11. When all three binary builds and both image builds pass, the release workflow attaches the six asset files to the GitHub release and moves the image tag `latest`.
12. When one job fails, the release workflow fails. The release gets no partial set of assets from that run.

### First release

The changelog starts after the commit `446f065`. The configuration sets `bootstrap-sha` to this commit and `release-as` to `0.1.0`. The first release pull request proposes `0.1.0`, which equals `Cargo.toml`. The commit after the first release removes `release-as` with a `chore(release)` commit. The work before the first release is in `docs/cr/`.

The repository becomes public before the merge of the first release pull request. The human sets the registry package to public one time after the first push. The release workflow does not change a visibility.

## Release assets

A release publishes these assets.

### Binary archives

| Target | Runner | Archive | Checksum file |
|---|---|---|---|
| `aarch64-apple-darwin` | macOS arm64 | `loupe-v<version>-aarch64-apple-darwin.tar.gz` | `loupe-v<version>-aarch64-apple-darwin.tar.gz.sha256` |
| `x86_64-unknown-linux-gnu` | Linux x86_64 | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz.sha256` |
| `aarch64-unknown-linux-gnu` | Linux arm64 | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz.sha256` |

1. `.agents/scripts/release.build.sh` builds each archive with `cargo build --release --locked` for the target of the host, with the toolchain that `rust-toolchain.toml` pins. No cross tool is used.
2. Before the build, the script stops with an actionable error when the tag version differs from the version in `Cargo.toml` at the tag, or when the checked-out commit is not the tag commit. The error names both versions.
3. Each archive holds `loupe`, `upstreams.toml`, the `config/` directory, `LICENSE`, and `README.md`. The binary reads `upstreams.toml` and `config/<env>.toml` at run time. An archive also keeps the execute bit of the binary.
4. Each checksum file holds one line in the format of `sha256sum`: the lowercase hex SHA-256 digest of the archive, two spaces, and the archive name. `shasum -a 256 -c` on macOS and `sha256sum -c` on Linux accept the file.
5. The script prints one machine line for each asset with the archive name, the size of the archive, the size of the binary, and the digest. `--dry-run` prints the plan and builds nothing.
6. The Linux binaries build on runner images with glibc 2.35. A Linux host needs glibc 2.35 or newer. The runtime image `debian:bookworm-slim` has glibc 2.36.
7. The macOS binary is not signed and not notarized. A browser download can get the quarantine attribute. `README.md` (Install) states the command that removes it.

### Container image

| Item | Value |
|---|---|
| Image name | `ghcr.io/desek/agentloupe` |
| Platforms | `linux/amd64`, `linux/arm64` |
| Tags | `<version>` and `latest` |

1. `.agents/scripts/release.image.sh` builds the image from the root `Dockerfile` with no change to the `Dockerfile`. It builds `linux/amd64` on a Linux x86_64 runner and `linux/arm64` on a Linux arm64 runner, and pushes each image by digest.
2. Then the script merges the two digests into one multi-platform image index with the tags `<version>` and `latest`. The tag `latest` moves only when both platform builds pass.
3. The image name is one constant in this script and in this file.
4. `--dry-run` prints the commands and runs nothing.
5. The image carries the OCI labels in the table below. The script sets them at build time.

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
2. The token is stored only as the repository secret `RELEASE_PLEASE_TOKEN`. It is the only repository secret of the project. No file holds its value.
3. The release-please workflow names the secret in one place.
4. The release pull request and the tag show the owner of the token as the author.

Check the expiry date of the token before each release. To rotate the token, do these steps:

1. Make a new fine-grained personal access token with the same repository and the same permissions.
2. Replace the value of the repository secret `RELEASE_PLEASE_TOKEN` with the new token.
3. Push a commit to `main`, or start the release-please workflow again, and check that it passes.
4. Revoke the old token.

When the token expires, release-please fails with an authentication error. No release is made in error.

## Pinned actions

Each workflow pins each action from outside the repository by its full 40-character commit SHA, with the release tag in a comment. At each minor release, a human checks the pins for security fixes. No tool opens update pull requests for the pins.

## Verify a release

`.agents/scripts/release.check.sh --version <version>` checks a published release on a local machine. It runs with the bash 3.2 of macOS, the default tools of macOS, `curl`, and Docker. It does not need `jq` or `gh`.

The script does these checks:

1. It downloads the three archives and the three checksum files of the release from `github.com/desek/agentloupe` with `curl`, with no credential, into a temporary directory.
2. It verifies each checksum. A mismatch or a missing file fails the check.
3. It unpacks the archive of the host platform, runs `loupe --version`, and compares the output with `loupe <version>`.
4. It runs `loupe --version` of each Linux archive in a `debian:bookworm-slim` container of the matching platform. When Docker is absent, it reports `skipped` with the reason.
5. It pulls `ghcr.io/desek/agentloupe:<version>` with no login. When `<version>` is the newest release, it checks that the tag `latest` has the same digest.
6. It starts the image with two keys from `loupe gen-key` of the same image, bound to `127.0.0.1` on a free port, with no named volume. It polls `GET /healthz` with a cap of 30 seconds. It checks the status 200, the body `{"status":"ok"}`, and the field `version` in the first log line.
7. It stops and removes the container and the temporary directory on each exit path.

The script never prints a key that it makes. It prints one machine line for each check, with `status: passed`, `status: failed`, or `status: skipped`, and `--human` gives the friendly view.

| Exit code | Meaning |
|---|---|
| 0 | Each check passed or is skipped with a stated reason. |
| 1 | A check failed. |
| 2 | A usage error. |
| 3 | A tool is missing. |
| 4 | A download or a pull failed. |

The release check uses the network. It is not part of the verdict.
