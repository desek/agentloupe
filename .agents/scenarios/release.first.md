# Scenario: the first release

- Goal: the first public release `v0.1.0` goes from the Conventional Commits on `main` to a checked binary on three platforms and a pulled multi-platform image, with no step that holds project logic in a workflow.
- Surface: the GitHub pull requests, the GitHub release, the registry `ghcr.io/desek/agentloupe`, the `gh` CLI, and `.agents/scripts/release.check.sh`.
- Success condition: each step below passes in one run of the first release, and the release check passes in 2 of 2 runs on the same macOS arm64 machine. `docs/specification/release.md` states the flow; `docs/cr/CR-0022-release-governance.md` states the acceptance criteria.
- Who: "agent" steps run from the coding agent with `gh` and the shell; "owner" steps change the visibility of the repository or of the package, or merge the release pull request, and only the owner runs them.

## Steps

1. Agent: on `main`, check that `Cargo.toml` holds `0.1.0` and that `git ls-remote --tags origin` lists no tag.
2. Agent: push the implemented commit `feat(release): Release governance (CR-0022)` to `main` (a squash merge of the pull request).
3. Agent: check that the verdict workflow runs on the push with the five stage jobs and the job `verdict`, and that each job passes: `gh run list --workflow verdict.yml --branch main --limit 1`.
4. Agent: check that the release-please workflow opens one pull request with the title `chore(release): release 0.1.0`: `gh pr list --state open`.
5. Agent: check that the release pull request changes only files of the set `Cargo.toml`, `Cargo.lock`, `.release-please-manifest.json`, `CHANGELOG.md`, and that `Cargo.toml` still holds `0.1.0`: `gh pr view <n> --json files`.
6. Agent: check that `CHANGELOG.md` on the release branch has the section `0.1.0` with "Features" and the line of CR-0022, and no `checkpoint`, `docs`, `chore`, or `test` commit.
7. Agent: check that the verdict workflow runs on the release pull request and passes (it runs only because release-please uses `RELEASE_PLEASE_TOKEN`): `gh pr checks <n>`.
8. Agent, while the repository is private: `gh workflow run release.yml -f tag=v0.1.0`, then check that the run fails in its first job `visibility` with an error that names the visibility, and that the jobs `verdict`, `build`, `image`, and `publish` are skipped (no macOS runner, no arm64 runner, no push). Owner: `gh repo edit desek/agentloupe --visibility public --accept-visibility-change-consequences`, then `gh repo view desek/agentloupe --json visibility` prints `PUBLIC`. Step 9 waits for this check.
9. Owner: review and squash merge the release pull request.
10. Agent: check that the tag `v0.1.0` points at the merge commit (`git ls-remote --tags origin v0.1.0` against `git rev-parse origin/main`), and that `gh release view v0.1.0` shows the changelog section as its notes.
11. Agent: check that the release workflow ran once for `v0.1.0` (`gh run list --workflow release.yml`), that the job `verdict` ran before `build` and `image`, and that the run passed.
12. Agent: check that the release has six assets: `gh release view v0.1.0 --json assets --jq '.assets[].name'` lists three `loupe-v0.1.0-<target>.tar.gz` and three `.sha256`.
13. Agent: check that `docker buildx imagetools inspect ghcr.io/desek/agentloupe:0.1.0` and `:latest` show one index digest with `linux/amd64` and `linux/arm64`. Owner: set the package `agentloupe` public one time (GitHub, Packages, agentloupe, Package settings, Change visibility) when a pull with no login fails.
14. Agent, on this macOS arm64 machine with Docker: `.agents/scripts/release.check.sh --version 0.1.0; echo $?`.
15. Agent: check the machine output: `check: download`, three `check: checksum`, `check: version aarch64-apple-darwin` with `loupe 0.1.0`, `check: version x86_64-unknown-linux-gnu` and `check: version aarch64-unknown-linux-gnu`, `check: pull`, `check: latest`, `check: health`, and `check: start-version` each `status: passed`, then `result: passed exit: 0`. Run it a second time for the 2 of 2 condition.
16. Agent: `.agents/scripts/release.check.sh --version 0.1.0 --dir /tmp/loupe-v0.1.0` keeps the downloads; append one byte to one archive in that directory and run the same command again: that checksum line is `status: failed` and the exit code is 1. Remove the directory.
17. Agent: record in `docs/specification/operations.md` (Release targets, column Measured): the verdict workflow time, the time from the tag to the assets, the `binary_bytes` of each `asset:` line of the release log, the image size per platform after `docker pull --platform`, the secret scan, the release check result, the count of repository secrets, and, by the owner, the billing usage (0 USD) and the two budgets of 0 USD. Then the agent removes `release-as` from `release-please-config.json` in a `chore(release)` commit.

### Checks of `release.check.sh`

| Check line | Passes when | Skipped when |
|---|---|---|
| `download` | the three archives and the three `.sha256` files download with `curl` and no credential (exit 4 on a failure) | never |
| `checksum <archive>` | the `.sha256` file is one `sha256sum` line that names the archive, and `shasum -a 256 -c` accepts it | never |
| `version aarch64-apple-darwin` (the host target) | the unpacked `loupe --version` prints `loupe <version>` | the host has no release asset, or its checksum failed |
| `version <linux target>` | `loupe --version` in `debian:bookworm-slim` of the matching platform prints `loupe <version>` | Docker is absent, or the checksum failed |
| `pull` | `ghcr.io/desek/agentloupe:<version>` pulls with an empty Docker configuration (exit 4 on a failure); the line holds the digest and `image_bytes` | Docker is absent |
| `latest` | `latest` has the digest of `<version>` | `<version>` is not the newest release, or Docker is absent |
| `health` | the image starts with two keys from its own `loupe gen-key` on `127.0.0.1` and a free port, and `GET /healthz` gives 200 and `{"status":"ok"}` within 30 seconds | Docker is absent |
| `start-version` | the first log line of the container holds `"version":"<version>"` | Docker is absent |

The container, its anonymous volume, and the temporary directory are removed on each exit path. No key is printed.

## Runs

- 2026-10-23, agent: steps 1 to 8 (agent part) ran.
  1. `Cargo.toml` holds `0.1.0`; `git ls-remote --tags origin` printed nothing.
  2. Pull request https://github.com/desek/agentloupe/pull/1 (8 `checkpoint(CR-0022)` commits) was squash merged as `46735d3 feat(release): Release governance (CR-0022)`, after `main` was pushed from `5a72c93` to `e7a1c16`, so the earlier commits stay as they are.
  3. Verdict run 39997205916 on `main`: format, lint, test, web, image, and verdict each `success`; 17:41:59 to 17:48:38 UTC (6 min 39 s).
  4. Release-please run 39997205842 opened https://github.com/desek/agentloupe/pull/2, title `chore(release): release 0.1.0`, branch `release-please--branches--main--components--agent-loupe`, label `autorelease: pending`, author `desek` (the owner of the token).
  5. Files of pull request 2: only `CHANGELOG.md`. `Cargo.toml`, `Cargo.lock`, and the manifest already hold `0.1.0`, so release-please changes none of them.
  6. `CHANGELOG.md`: section `0.1.0` with "Features": `**release:** Release governance (CR-0022)` and "Bug Fixes": `**scripts:** agent.connect.sh writes a unique temporary file, so two shells can connect at once`. No checkpoint, docs, chore, or test line. The compare link of the heading reads `v0.1.0...v0.1.0`, because no earlier tag exists.
  7. Verdict run 39997262577 on pull request 2: the six jobs `success`; 17:42:23 to 17:47:36 UTC.
  8. Release run 39997488693 (manual, tag input `v0.1.0`, repository `PRIVATE`): conclusion `failure` in 9 seconds; job `visibility` failed with "the repository visibility is private, so the release stops before any build"; `verdict`, `build`, `image`, and `publish` skipped.
- Wait for the owner: step 8 (make the repository public), step 9 (merge pull request 2), and step 13 (set the package public if needed). Steps 10 to 17 run after the merge; no tag, no release, and no image exist yet.
- Before the release, the script was exercised locally on 2026-10-23: `release.build.sh --tag v0.1.0` in a scratch clone with a local tag printed `binary_bytes: 14402208` for `aarch64-apple-darwin` (threshold 26 214 400); `release.check.sh --dir` with that archive, two stand-in Linux archives (one changed by one byte), and the local image under a test name gave `checksum ... status: failed` for the changed archive, `version aarch64-apple-darwin status: passed detail: loupe 0.1.0`, `health status: passed`, `start-version status: passed`, and exit 1; no container of the run remained.
