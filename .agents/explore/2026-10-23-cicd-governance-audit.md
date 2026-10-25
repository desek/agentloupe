---
date: 2026-10-23
branch: main
commit: 84c2284
---

<!--
@agents-index: Cached audit of the CI, release, and repository governance of desek/agentloupe against
a professional public open-source project, at 84c2284 while the repository is still private and the
first release pull request (#2, v0.1.0) is open. Covers workflows, branch rules, community files,
release governance, supply chain, and secrets hygiene, and ends with a gap table with severities.
-->

## Question

Audit the continuous integration, delivery, and governance of this repository against what a
professional public open-source project carries. Check, with evidence: (1) the workflows (least
privilege, SHA pins, concurrency, timeouts, head checkout, thin callers, no secrets, token flow, tag
trigger, artifact retention, checksums, provenance or signing); (2) branch governance on `main`
(ruleset or protection, required pull request and `verdict` check, linear history, no force push, no
deletion, owner bypass, auto-merge for CR-0023); (3) the repository files of a public project; (4)
release governance (semver rules, bump rules, notes source, version in binary and start log, one
version source, first-release bootstrap); (5) supply chain (lock files, vendored price file, Dependabot,
cargo audit/deny, base image digest pins); (6) secrets hygiene. End with a gap table (severity,
evidence, minimal action) and a one-line verdict.

## Response

**State at the time of the audit.** The repository `desek/agentloupe` is **private** on the free plan.
CR-0022 is implemented; the first release pull request #2 (`chore(release): release 0.1.0`) is open
with a green verdict; the release workflow was dispatched once and stopped as designed at the
visibility gate (run 39997488693). No tag, release, or image exists yet. The scenario
`.agents/scenarios/release.first.md` waits for the owner to make the repository public, then merge #2.

**Sources read:** `.github/workflows/{verdict,release,release-please}.yml`, `release-please-config.json`,
`.release-please-manifest.json`, `.agents/scripts/release.{build,image,check}.sh`,
`docs/specification/release.md`, `docs/specification/operations.md` (Container, Pipeline, Hosted
workflows, Release targets), `README.md`, `LICENSE`, `Dockerfile`, `compose.yaml`, `.dockerignore`,
`.gitignore`, `.env.example`, `docs/cr/CR-0022-release-governance.md`,
`docs/cr/CR-0023-dependency-autofix.md`, `vendor/litellm/SOURCE.md`, `Cargo.toml`, `build.rs`.
**Commands:** `gh api repos/desek/agentloupe`, `.../rulesets`, `.../branches/main/protection`,
`.../rules/branches/main`, `.../actions/permissions`, `.../actions/permissions/workflow`,
`.../vulnerability-alerts`, `.../private-vulnerability-reporting`, `.../community/profile`,
`gh secret list`, `gh variable list`, `gh run list`, `gh pr view 2`, `git ls-remote` of each pinned
action, `gitleaks dir` on `git archive HEAD` and `.github`, `gitleaks git .` (history), `shasum` of the
vendored price files. Nothing under `target/` or `node_modules/` was searched.

### Repository settings (gh)

| Setting | Value |
|---|---|
| visibility / private | `private` / `true` |
| default_branch | `main` |
| allow_auto_merge | `false` |
| delete_branch_on_merge | `false` |
| allow_squash / merge commit / rebase | `true` / `true` / `true` |
| security_and_analysis | `null` (not available on a private free repo) |
| vulnerability-alerts | 404 (Dependabot alerts off) |
| private-vulnerability-reporting | 404 |
| rulesets, branches/main/protection, rules/branches/main | 403 "Upgrade to GitHub Pro or make this repository public" — **no rule exists, and none can until public** |
| secrets | exactly one: `RELEASE_PLEASE_TOKEN` (2026-10-23); no variables; 0 environments |
| actions/permissions | `enabled: true`, `allowed_actions: all`, `sha_pinning_required: false` |
| workflow token default | `default_workflow_permissions: read`, `can_approve_pull_request_reviews: false` |
| community profile | `health_percentage: 42`; present: license, readme; absent: code_of_conduct, contributing, issue_template, pull_request_template |

### (1) Workflows

| Check | Result | Evidence |
|---|---|---|
| Least-privilege `permissions` | **Pass, one nit.** verdict: top-level `contents: read` only (verdict.yml:21-22). release: top-level `contents: read`; `image` job adds `packages: write` (release.yml:88-90); `publish` has `contents: write` + `packages: write` (115-117); `build` inherits read. release-please: `contents: write`, `pull-requests: write` for `GITHUB_TOKEN` (release-please.yml:12-14) — but the action authenticates with the PAT (line 27), so these `GITHUB_TOKEN` grants are unused and could be `{}`. Repo default token is `read`. |
| Actions pinned by full SHA + version comment | **Pass.** 8 actions, every `uses:` is a 40-char SHA with `# vX.Y.Z`; `git ls-remote` confirms each SHA equals its tag: checkout v7.0.1, cache v6.1.0, upload-artifact v7.0.2, download-artifact v8.0.2, setup-buildx v4.4.1, login v4.7.0, mise-action v5.2.2, release-please-action v5.0.0 (all MATCH). The repo setting `sha_pinning_required` is `false`, so the rule is convention only. |
| Concurrency groups | **Pass.** verdict: per-PR group with cancel on PR, `run_id` group (never cancelled) for push/call (verdict.yml:26-28). release-please: single group, no cancel (16-19). release: `release-<tag>`, no cancel (22-24). |
| Timeouts | **Fail.** No `timeout-minutes` in any of the three files (grep empty). Every job falls back to 360 min, including the `macos-15` build. |
| Head-commit checkout on PRs | **Pass.** `HEAD_REF: inputs.ref \|\| github.event.pull_request.head.sha \|\| github.sha` (verdict.yml:33), used by every checkout; never the merge ref. Trigger is `pull_request`, not `pull_request_target`. `persist-credentials: false` on every checkout. |
| No project logic in workflows | **Pass.** Each stage job is one script call (`pipeline.{format,lint,test,web,image}.sh`); release jobs call `release.build.sh` / `release.image.sh`. Inline shell is limited to the visibility gate (release.yml:39-44), the fail-closed aggregator (verdict.yml:120-123), and `gh release upload` (142) — all platform work allowed by operations.md "Hosted workflows". No version, target triple, or image name in a workflow (the matrix lists runner labels only). |
| No secret in the files | **Pass.** `gitleaks dir .github`: no leaks. Only `secrets.RELEASE_PLEASE_TOKEN` and `secrets.GITHUB_TOKEN` references. |
| Token flow | **Pass.** release-please uses `RELEASE_PLEASE_TOKEN` (fine-grained PAT, contents/PRs/issues RW, one repo — release.md "Release token"), so the release PR gets a verdict run (run 39997262577 on PR #2) and the tag push will trigger release.yml. Registry login and `gh release upload` use `GITHUB_TOKEN` with `packages: write` (release.yml:97-101, 134-142). The PAT owner is the admin, so once a ruleset with admin bypass exists the PAT also bypasses it (CR-0023 P4 already addresses this). |
| Tag trigger | **Pass.** `tags: ['v[0-9]+.[0-9]+.[0-9]+']` + `workflow_dispatch` with a tag input (release.yml:8-16); scripts re-check `^v(0\|[1-9][0-9]*)\...$`, the tag commit = HEAD, and tag version = `Cargo.toml` (release.build.sh:50-81, release.image.sh:64-99). Verdict reruns on the tag (`uses: ./.github/workflows/verdict.yml` with `ref: refs/tags/...`) before any build. Private-repo gate first. No tag ruleset protects `v*` tags. |
| Artifact retention | **Pass.** Intermediate artifacts `retention-days: 3`, `if-no-files-found: error` (release.yml:78-79, 107-108). Verdict uploads none. |
| Checksum files | **Pass, integrity only.** One `<archive>.sha256` per archive in `sha256sum` format, self-verified after writing (release.build.sh:122-123); `release.check.sh` re-verifies after download. No aggregate `SHA256SUMS`. Checksums are produced by the same job that produced the archive, so they prove integrity, not origin. |
| Provenance / signing | **Absent (option).** No `actions/attest-build-provenance`, no `attestations: write`/`id-token: write`, no cosign, no SBOM. `docker buildx build` is called without `--provenance`/`--sbom` (release.image.sh:137-139); BuildKit may attach its default unsigned min-mode provenance, but nothing is signed or verified. CR-0022 explicitly scoped attestations, SBOM, image signing, and macOS notarization out ("a follow-up CR can add them"). |
| Other | Release builds cache `target/` keyed `cargo-release-<runner>-…` (release.yml:66-72). Caches written on a tag ref are scoped to that tag, so a new tag will almost never hit, while the cache still feeds the published binary — little gain, some poisoning surface. The GitHub release is created (published) by release-please before the assets exist; for up to the 45-min target it has no assets. `release.check.sh` is manual, not run after `publish`. |

### (2) Branch governance

| Check | Result | Evidence |
|---|---|---|
| Ruleset / protection on `main` | **None — and impossible while private on the free plan.** | rulesets, protection, and rules endpoints all return 403 "Upgrade to GitHub Pro or make this repository public". |
| Require PR | No. Owner pushes `checkpoint(...)` commits directly to `main` (HEAD 84c2284 is a direct push; CR-0022 Out of Scope: "The owner commits to `main` directly today"). |
| Require `verdict` status check | No. The check name will be `verdict` (workflow `verdict`, job `verdict`). |
| Linear history | Not enforced; de facto linear (0 merge commits in 293). Merge-commit and rebase merges are both still allowed, although release.md step 5 prescribes squash. |
| No force push / no deletion | Not enforced. |
| Owner bypass | n/a now; CR-0023 req. 45 plans the admin in the bypass list. |
| Auto-merge | `allow_auto_merge: false`. CR-0023 req. 45 requires it on. `delete_branch_on_merge: false`. |
| Commit signing | 292 of 293 commits unsigned (`%G?` = N); not required. |

### (3) Repository files

| File | State |
|---|---|
| `LICENSE` | **Present**, MIT, "Copyright (c) 2026 Daniel Grenemark"; `Cargo.toml` `license = "MIT"`; GitHub detects `MIT`. |
| `README.md` | **Present** with Install (release asset, registry image, from source, check a release; l.237-304), usage (`The loupe binary` l.82, `Connect the agents` l.306, Dashboard, Accounts, Cost, …), Status (l.14), License (l.530). **Stale:** Status says "Thirteen change requests" (there are 23); "Open work" (l.521-528) still says "Write the `loupe` binary from scratch", contradicting Status. No contributing/security pointers, no badges. |
| `CHANGELOG.md` | **Absent on `main`**; release PR #2 creates it (only changed file). |
| `CONTRIBUTING.md` | Absent. |
| `SECURITY.md` | Absent; no disclosure contact anywhere; private vulnerability reporting off (404). |
| `CODE_OF_CONDUCT.md` | Absent. |
| PR template | Absent (`.github/` holds only `workflows/`). |
| Issue templates | Absent. |
| `CODEOWNERS` | Absent (root, `.github/`, `docs/`). |
| `.editorconfig` | Absent. |

### (4) Release governance

| Check | Result | Evidence |
|---|---|---|
| SemVer rules written down | **Pass.** release.md "Version": `<major>.<minor>.<patch>`, no leading zero, first public release `0.1.0`; CR `target-version` is a plan, not a version. |
| Bump rules | **Pass.** release.md "Bump rules": fix→patch, feat→minor, breaking→major from 1.0.0; below 1.0.0 breaking→minor via `bump-minor-pre-major: true`, `bump-patch-for-minor-pre-major: false` (config l.8-9); 1.0.0 only via `Release-As: 1.0.0`. |
| Release notes source | **Pass.** `changelog-sections` (config l.10-23): feat/fix/perf/revert visible; checkpoint/docs/chore/test/ci/build/refactor/style hidden; matches release.md "Release notes". Commit rule for CRs (checkpoint commits + one feat/fix implemented commit) written in release.md. All 293 commit subjects are Conventional (0 non-matching). |
| Version in binary | **Pass.** `#[command(name = "loupe", version, ...)]` (src/cli/mod.rs:30) → `loupe <version>`; tested by `tests/cli.version.rs`. |
| Version in start log | **Pass.** `tracing::info!(version = env!("CARGO_PKG_VERSION"), "loupe serve starting")` (src/cli/serve.rs:39); `release.check.sh` checks it. |
| `Cargo.toml` as the one source | **Pass.** `version = "0.1.0"` (Cargo.toml:4); the only other occurrences are the allowed exceptions `.release-please-manifest.json` and `release-as` in the config; scripts read it from `Cargo.toml` at the tag; no workflow states it. |
| First release bootstrap | **Pass with follow-ups.** `bootstrap-sha: 8611e9a…`, `packages["."].release-as: "0.1.0"`, manifest `".": "0.1.0"`, `include-v-in-tag: true`, `include-component-in-tag: false`. Consequences: (a) PR #2 changelog heading links `compare/v0.1.0...v0.1.0` (manifest already at 0.1.0); (b) `release-as` must be removed right after the release or every later release PR proposes 0.1.0 again (scenario step 17 plans it); (c) the release branch is `release-please--branches--main--components--agent-loupe`, not the `release-please--branches--main` of CR-0022 judgement call 6 (harmless, but the spec is wrong). |
| Spec drift ahead | release.md "Pinned actions" says "No tool opens update pull requests for the pins", but approved CR-0023 adds the `github-actions` Dependabot ecosystem and lists only two other release.md edits (req. 47). |

### (5) Supply chain

| Check | Result | Evidence |
|---|---|---|
| `Cargo.lock` committed | **Pass.** Tracked; ~291 packages; every build uses `--locked` (release.build.sh:88, Dockerfile:17). |
| Toolchain pins | **Pass.** `rust-toolchain.toml` channel `1.97.1`; `.tool-versions` nodejs 22.20.0, cargo-nextest 0.9.149. |
| pnpm lock in prototype | **Pass.** `prototype/package.json` and `prototype/pnpm-lock.yaml` tracked. |
| Vendored price file with commit | **Pass.** `vendor/litellm/SOURCE.md` pins upstream commit `cfee0ffe…` with SHA-256s; recomputed SHA-256 of both files match (`da7983…`, `0aebfc…`); `build.rs` reads the commit; upstream `LICENSE` vendored. |
| Dependabot | **Absent** (`.github/dependabot.yml` none; alerts 404). Planned by CR-0023 (cargo `/`, npm `/prototype`, github-actions `/`, weekly) — note it does **not** include the `docker` ecosystem, so base images stay unwatched. |
| cargo audit / cargo deny | **Absent.** No `deny.toml`, no audit/deny/osv/trivy/grype in any script, workflow, or spec (outside CR text). No vulnerability or license check runs before publishing. |
| Docker base image pin | **Fail.** `FROM rust:slim-bookworm` and `FROM debian:bookworm-slim` by tag, not `@sha256:` digest (Dockerfile:5, 20). Rust itself is pinned via rustup; the OS layers float, so two builds of one tag can differ. |
| Third-party notices in artefacts | **Gap.** The binary embeds ~291 crates, the LiteLLM price JSON (embedded via `src/price/embed.rs`) and the Geist fonts (`build.rs` embeds `web/`); the release archive holds only the project `LICENSE` + README (release.build.sh:113) and the image no notices file. MIT/Apache/OFL require the notices to travel with distributed copies. |

### (6) Secrets hygiene

| Check | Result | Evidence |
|---|---|---|
| `.env` ignored | **Pass.** `.gitignore` `.env`, `.env.*`, `!.env.example`; `git check-ignore .env` matches; `.env` exists locally, untracked. `.dockerignore` is an allowlist (`*` then `!…`), so `.env` never enters the build context. `compose.override.yaml` and `.agents/logs/` ignored. |
| Example env file | **Pass.** `.env.example` lists every variable with empty values / commented lines and how to generate each key. |
| No secret value in tracked files | **Pass (fixtures only).** `gitleaks dir` on `git archive HEAD`: 18 findings, all test fixtures or placeholders — the 0..31 byte key `AAECAwQ…` (tests/scripts), fake values inside `#[cfg(test)]` of `src/config/show.rs`, RFC 6455 sample `sec-websocket-key`, `sk-test-123`, `bench-token`, `sk-ant-oat-test-…`. `gitleaks git` over the full history: 20 findings in the same 10 files, same fixture class. No real token shapes (`ghp_`, `github_pat_`, `AKIA`, private keys). No `.gitleaksignore`, so an automated scan is not at zero. The `/Users/<owner>` home path is gone (CR-0022 D1). |
| Repository secrets | Exactly `RELEASE_PLEASE_TOKEN`, matching the release target "New repository secrets = 1". Secret scanning and push protection are unavailable while private. |

### Gaps

Severity: **blocking** = do before the first public release (i.e., in the window between making the
repository public and merging PR #2); **should fix** = soon, before or with the next release;
**note** = option or cosmetic.

| # | Gap | Severity | Evidence | Minimal action |
|---|---|---|---|---|
| 1 | No ruleset on `main`: no required PR, no required `verdict`, force push and deletion allowed | blocking | rulesets/protection/rules → 403 (private free plan) | Right after `gh repo edit --visibility public`, create a `main` ruleset: block force push + deletion, require linear history, require PR and status check `verdict`, admin (Repository role) in bypass. Settles CR-0023 req. 45 at the same time. |
| 2 | No security policy or private disclosure channel | blocking | `SECURITY.md` absent; private-vulnerability-reporting 404 | Add `SECURITY.md` (supported versions = latest 0.x, contact = GitHub private vulnerability reporting + email); enable private vulnerability reporting once public. |
| 3 | GitHub security features off | blocking | `security_and_analysis: null`; vulnerability-alerts 404 | Once public: enable secret scanning + push protection and Dependabot alerts (also the precondition of CR-0023 req. 3). |
| 4 | Distributed binary and image carry no third-party license notices | blocking | archive = loupe, upstreams.toml, config/, LICENSE, README.md (release.build.sh:113); binary embeds ~291 crates, LiteLLM JSON, Geist fonts | Generate `THIRD-PARTY-NOTICES` (e.g. `cargo about`) plus `vendor/litellm/LICENSE` and `OFL.txt`, add it to the archive file list and to the image. |
| 5 | No job timeouts | should fix | no `timeout-minutes` in any workflow (default 360 min) | Add `timeout-minutes` per job (e.g. 30 for stages, 60 for release builds, 10 for gates/publish). |
| 6 | No vulnerability / license audit before publishing | should fix | no `deny.toml`, no audit tool in any script or workflow | Run `cargo deny check advisories licenses` (or `cargo audit`) once by hand before merging PR #2; then add it as a pipeline script stage through a CR (CR-0023 freezes the workflows). |
| 7 | Docker base images not pinned by digest and not watched | should fix | Dockerfile:5, 20 by tag; CR-0023 omits the `docker` ecosystem | Pin both `FROM` lines `@sha256:…`; add the `docker` ecosystem to CR-0023's `dependabot.yml`. |
| 8 | Merge settings contradict the squash-only spec; branches not cleaned; auto-merge off | should fix | merge commit + rebase `true`; `delete_branch_on_merge: false`; `allow_auto_merge: false` | `gh repo edit --enable-merge-commit=false --enable-rebase-merge=false --delete-branch-on-merge --enable-auto-merge`. |
| 9 | Actions policy allows any action and does not enforce SHA pins | should fix | `allowed_actions: all`, `sha_pinning_required: false` | Turn on "Require actions to be pinned to a full-length commit SHA"; optionally restrict to the 4 owners in use (actions, docker, jdx, googleapis). |
| 10 | No tag protection | should fix | no ruleset possible; a `v*` tag push triggers a job with `packages: write` / `contents: write` | Tag ruleset on `v*`: block deletion and update; restrict creation to the admin/PAT owner. |
| 11 | Missing community files | should fix | community profile 42 %; no CONTRIBUTING, CODE_OF_CONDUCT, PR template, issue templates | Add `CONTRIBUTING.md` (Conventional Commits, `pipeline.verdict.sh`, CR process, squash), `CODE_OF_CONDUCT.md` (Contributor Covenant), `.github/pull_request_template.md`, `.github/ISSUE_TEMPLATE/{bug,feature}.yml` + `config.yml` pointing security reports to SECURITY.md. |
| 12 | README Status and Open work are stale | should fix | "Thirteen change requests" vs 23 in `docs/cr/`; Open work says "Write the `loupe` binary from scratch" | Update both sections; add short Contributing / Security pointers. |
| 13 | `release-as: 0.1.0` must be removed after the release | should fix | config l.26; release.md rule 3 | Do scenario step 17 (`chore(release)` commit) immediately after v0.1.0 publishes. |
| 14 | No build provenance or signature | note | no attest-build-provenance, no cosign, no SBOM; CR-0022 scoped out | Follow-up CR: `actions/attest-build-provenance` on the archives and the image digest (`id-token: write`, `attestations: write`), optional `cosign sign` keyless, `--sbom=true`; document `gh attestation verify`. |
| 15 | Release is published before its assets exist | note | release-please creates the release; assets attach later in `publish` | Accept and document, or create the release as draft (`"draft": true` with `force-tag-creation`) and publish it in the `publish` job. |
| 16 | Release builds use a cargo cache that rarely hits | note | release.yml:66-72; tag-scoped caches | Drop `actions/cache` from `build` for a clean-room release build. |
| 17 | Unused `GITHUB_TOKEN` write grants in release-please.yml | note | l.12-14 vs PAT at l.27 | Set `permissions: {}` (and update operations.md "Hosted workflows"). |
| 18 | release.md contradicts approved CR-0023 on action update PRs | note | release.md "Pinned actions" vs CR-0023 req. 1 (github-actions ecosystem) | Add the "Pinned actions" section to CR-0023 req. 47's list of release.md edits. |
| 19 | Changelog compare link and release branch name | note | PR #2 heading `compare/v0.1.0...v0.1.0`; branch `…--components--agent-loupe` vs CR-0022 JC 6 | Edit the first CHANGELOG heading link in PR #2 if wanted; correct the branch name in CR-0022/spec. |
| 20 | Secret scan not at zero because of fixtures | note | gitleaks: 18 (tree) / 20 (history) fixture findings, no `.gitleaksignore` | Add a `.gitleaksignore` with the fingerprints, so a future CI scan can require 0. |
| 21 | No CODEOWNERS, no .editorconfig, unsigned commits | note | absent files; 292/293 commits `N` | Optional with one maintainer; add `.editorconfig` (rustfmt covers Rust, not md/sh/yml); consider signed commits later. |
| 22 | CR-0023 design: npm `fix(deps)` in `prototype/` will cut binary releases | note | CR-0023 req. 2 gives npm `fix(deps)`; prototype is not shipped | Use `chore(deps)` (hidden) for `/prototype`, or drop that ecosystem. |
| 23 | Release check is manual | note | `release.check.sh` not called by any workflow | Optional post-publish job calling `release.check.sh --version` on Linux. |

**Verdict:** the pipeline and release machinery are of professional quality (least-privilege, SHA-pinned,
fail-closed thin callers with written semver governance), but the repository governance around it is
not yet there — do the four blocking items (main ruleset, SECURITY.md + private reporting, security
features, third-party notices) in the window between going public and merging PR #2.
