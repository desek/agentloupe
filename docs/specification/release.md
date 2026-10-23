<!-- @agents-index Specification of the release: version rules, bump rules, tag forms, release flow with release-please, release assets, third-party notices, commit rule, release token, repository settings, and the release check. -->
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
5. A human reviews the release pull request and squash merges it. The main ruleset (see "Repository settings") lets the merge happen only when the check `verdict` passed. The type `chore` of the title keeps the merge commit out of the next release notes. The merge setting deletes the release branch after the merge. release-please makes the branch again for the next release.
6. The merge starts the release-please workflow again. It makes the tag `v<version>` on the merge commit and a GitHub release. The notes of the release are the changelog section of that version.
7. The push of the tag starts the release workflow. Each tag gets exactly one automatic run. A human can start the release workflow by hand with a tag input, to run a failed release again.
8. When the repository is private, the release workflow fails in its first job, on a Linux x86_64 runner. The error names the visibility. The run starts no macOS job, no arm64 job, and no push to the registry.
9. The release workflow checks out the tag. It runs the verdict workflow on the tag commit first. When the verdict fails, the release workflow builds nothing.
10. The release workflow builds the three binary assets and the two platform images in parallel, on native runners.
11. When all three binary builds and both image builds pass, the release workflow attaches the six asset files to the GitHub release and moves the image tag `latest`.
12. When one job fails, the release workflow fails. The release gets no partial set of assets from that run.

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

### Binary archives

| Target | Runner | Archive | Checksum file |
|---|---|---|---|
| `aarch64-apple-darwin` | macOS arm64 | `loupe-v<version>-aarch64-apple-darwin.tar.gz` | `loupe-v<version>-aarch64-apple-darwin.tar.gz.sha256` |
| `x86_64-unknown-linux-gnu` | Linux x86_64 | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz.sha256` |
| `aarch64-unknown-linux-gnu` | Linux arm64 | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz` | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz.sha256` |

1. `.agents/scripts/release.build.sh` builds each archive with `cargo build --release --locked` for the target of the host, with the toolchain that `rust-toolchain.toml` pins. No cross tool is used.
2. Before the build, the script stops with an actionable error when the tag version differs from the version in `Cargo.toml` at the tag, or when the checked-out commit is not the tag commit. The error names both versions.
3. Before the build, the script calls `.agents/scripts/release.notices.sh`, which writes `THIRD-PARTY-NOTICES` (see "Third-party notices").
4. Each archive holds `loupe`, `upstreams.toml`, the `config/` directory, `LICENSE`, `README.md`, and `THIRD-PARTY-NOTICES`. The binary reads `upstreams.toml` and `config/<env>.toml` at run time. An archive also keeps the execute bit of the binary.
5. Each checksum file holds one line in the format of `sha256sum`: the lowercase hex SHA-256 digest of the archive, two spaces, and the archive name. `shasum -a 256 -c` on macOS and `sha256sum -c` on Linux accept the file.
6. The script prints one machine line for each asset with the archive name, the size of the archive, the size of the binary, and the digest. `--dry-run` prints the plan and builds nothing.
7. The Linux binaries build on runner images with glibc 2.35. A Linux host needs glibc 2.35 or newer. The runtime image `debian:bookworm-slim` has glibc 2.36.
8. The macOS binary is not signed and not notarized. A browser download can get the quarantine attribute. `README.md` (Install) states the command that removes it.

### Container image

| Item | Value |
|---|---|
| Image name | `ghcr.io/desek/agentloupe` |
| Platforms | `linux/amd64`, `linux/arm64` |
| Tags | `<version>` and `latest` |

1. `.agents/scripts/release.image.sh` calls `release.notices.sh` before the build. It builds the image from the root `Dockerfile` with no change to the `Dockerfile`. It builds `linux/amd64` on a Linux x86_64 runner and `linux/arm64` on a Linux arm64 runner.
2. The release image holds `/app/LICENSE` and `/app/THIRD-PARTY-NOTICES`. After the build and before the push, the script checks that the built image holds both files. When one file is absent, the script stops with exit code 1 and pushes nothing.
3. The script pushes each platform image by digest. Then it merges the two digests into one multi-platform image index with the tags `<version>` and `latest`. The tag `latest` moves only when both platform builds pass.
4. The image name is one constant in this script and in this file.
5. `--dry-run` prints the commands and runs nothing.
6. The image carries the OCI labels in the table below. The script sets them at build time.

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

1. It downloads the three archives and the three checksum files of the release from `github.com/desek/agentloupe` with `curl`, with no credential, into a temporary directory.
2. It verifies each checksum. A mismatch or a missing file fails the check.
3. It unpacks the archive of the host platform, runs `loupe --version`, and compares the output with `loupe <version>`.
4. It runs `loupe --version` of each Linux archive in a `debian:bookworm-slim` container of the matching platform. When Docker is absent, it reports `skipped` with the reason.
5. It pulls `ghcr.io/desek/agentloupe:<version>` with no login. When `<version>` is the newest release, it checks that the tag `latest` has the same digest.
6. It starts the image with two keys from `loupe gen-key` of the same image, bound to `127.0.0.1` on a free port, with no named volume. It polls `GET /healthz` with a cap of 30 seconds. It checks the status 200, the body `{"status":"ok"}`, and the field `version` in the first log line.
7. It checks the third-party notices. It prints one check line `notices <target>` for each of the three archives and one check line `notices image` for the image. A check passes when the file `THIRD-PARTY-NOTICES` is present and not empty, holds the LiteLLM section and the Geist section, and has the same SHA-256 digest as the other copies. When Docker is absent, `notices image` reports `skipped` with the reason.
8. It stops and removes the container and the temporary directory on each exit path.

The script never prints a key that it makes. It prints one machine line for each check, with `status: passed`, `status: failed`, or `status: skipped`, and `--human` gives the friendly view.

| Exit code | Meaning |
|---|---|
| 0 | Each check passed or is skipped with a stated reason. |
| 1 | A check failed. |
| 2 | A usage error. |
| 3 | A tool is missing. |
| 4 | A download or a pull failed. |

The release check uses the network. It is not part of the verdict.
