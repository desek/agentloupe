---
name: "CR-0030 Remove the prototype"
description: "prototype/, the offline browser simulation that explored the requirements, leaves the tree and the whole git history before the first public release. The npm lane of the dependency flows, which only kept the prototype up to date, goes with it. The rewritten history goes to a fresh private repository desek/agentloupe."
id: "CR-0030"
status: "approved"
date: 2026-10-25
requestor: Daniel Grenemark
stakeholders: Daniel Grenemark (owner, owner of the GitHub account), reviewer agent (approver), readers of the public repository
priority: "high"
target-version: 0.1.0
source-branch: main
source-commit: 9834837
---

# Remove the prototype

## Change Summary

`prototype/` is an offline Vite simulation with fake data. It explored the product before the `loupe` binary and the dashboard under `web/` existed. The product no longer uses it. This CR removes it from the tree and from every commit, and removes the npm lane of Dependabot, of the dependency autofix, and of the freshness flow, because that lane served only the prototype. The rewritten history goes to a fresh private repository `desek/agentloupe`, because the current one already holds a Dependabot pull request ref with prototype commits.

## Motivation and Background

The owner wrote, on 2026-10-25, after CR-0029: "the "prototype/" also seems redundant tbh, we have the final solution?" The supervisor found that the binary and `web/` do not use `prototype/`, and that only the dependency automation depends on it. The owner then chose "Remove from tree and history": a new CR, the removal of `prototype/` and the npm lane, one more history rewrite, the current new repository renamed to `desek/agentloupe-tmp` (private, the owner can delete it later, because `gh` has no delete scope), a fresh `desek/agentloupe`, and a push of `main` only.

## Change Drivers

* The README and the guides describe the product. A second, fake-data application in the repository confuses a reader.
* The npm lane runs weekly Dependabot updates, autofix fixes, and freshness updates for code that never ships.
* No repository is public yet, so a rewrite now changes nothing that a reader has cloned.

## Current State

* `git ls-files prototype` lists 28 files. Two commits touch `prototype/`: `9a13499` changes only `prototype/`, and `953169b` changes `prototype/` and one other file.
* `.github/dependabot.yml` has an `npm` entry for `/prototype` with the prefix `chore(deps)`.
* `.agents/scripts/deps.autofix.sh` and `.agents/scripts/deps.freshness.sh` have an npm lane: `prototype/package.json` and `prototype/pnpm-lock.yaml` in `ALWAYS_PATHS`, `prototype/src/` in the code paths, the function `run_prototype`, the status `prototype_failed`, the break check `prototype`, the Bash permissions `pnpm --dir prototype` and `corepack pnpm`, and the use count over `prototype/src`.
* `.agents/scripts/deps.autofix.prompt.md`, `deps.freshness.update.prompt.md`, and `deps.freshness.assess.prompt.md` name the npm ecosystem and `prototype/`.
* `tests/scripts/deps.autofix.test.sh`, `tests/scripts/deps.freshness.test.sh`, and `tests/fixtures/deps/alerts.json` make a prototype and an npm alert in their scratch repositories.
* `docs/specification/dependencies.md` and `docs/specification/release.md` (commit types of dependency updates) state the npm lane. `.taxonomy` rows **freshness class** and **breaking update** name the prototype. `SECURITY.md` names `prototype/` as out of scope.
* `desek/agentloupe` is private with `main` at `9834837` and the Dependabot pull request #1 (`chore(deps): bump vite ... in /prototype`).

## Proposed Change

1. Update the specification: the dependency flows have the ecosystems `cargo` and `github-actions` only.
2. Remove the npm lane from the Dependabot configuration, the two scripts, the three prompts, and their tests and fixtures.
3. Remove `prototype/` and every reference that states it exists, outside the records of earlier work.
4. Rewrite the history with `git filter-repo --path prototype/ --invert-paths`, then one commit that maps the commit hashes in the tracked files.
5. Rename the current `desek/agentloupe` to `desek/agentloupe-tmp`, make a fresh private `desek/agentloupe`, set its variable and its secret, and push `main` only.

## Requirements

### Functional Requirements

1. `prototype/` **MUST NOT** be in the tree. `.gitignore` **MUST** list `prototype/`, so a local copy with `node_modules` never returns.
2. `.github/dependabot.yml` **MUST** have the entries `cargo` and `github-actions` only.
3. `deps.autofix.sh` and `deps.freshness.sh` **MUST** have no npm lane: no `prototype` path, no `run_prototype`, no status `prototype_failed`, no break check `prototype`, no pnpm or corepack permission, and no npm use count. A Dependabot alert or a stale dependency of the ecosystem `npm` **MUST** be skipped with a recorded reason, not fixed.
4. The three prompts **MUST NOT** name the npm ecosystem or `prototype/`.
5. The script tests and the fixtures **MUST** make no prototype. A test **MUST** show that an `npm` alert is skipped with its reason. The npm alert in the fixture uses a manifest path without `prototype` or `pnpm`, for example `package-lock.json`.
6. `docs/specification/dependencies.md`, `docs/specification/release.md`, `.taxonomy`, `SECURITY.md`, the README, and the guides **MUST NOT** state that `prototype/` exists. The records under `docs/cr/` and `.agents/` stay as records of earlier work.
7. The specification **MUST** change before the scripts.
8. Before the rewrite, the supervisor **MUST** write `docs/cr/CR-0030-rollback-log.md` and a git bundle of all refs outside the repository.
9. The rewrite **MUST** run on a fresh clone of `desek/agentloupe` and **MUST** remove the path `prototype/` from every commit. A commit that becomes empty is dropped; every other commit keeps its order, author, and date.
10. After the rewrite, `git log --all -- prototype` **MUST** be empty, and the tree of the rewritten `main` **MUST** equal the tree of `main` before the rewrite.
11. One commit on top **MUST** map each commit hash in the tracked files that names a commit of the history before this rewrite to its new hash, `bootstrap-sha` included.
12. The supervisor renames the current `desek/agentloupe` to `desek/agentloupe-tmp` (private), makes a fresh private `desek/agentloupe`, sets the variable `LOUPE_TELEMETRY_HOST` and the secret `CLAUDE_CODE_OAUTH_TOKEN`, and pushes only `main`. The local remotes stay `origin` (the new repository) and `archive`.

### Non-Functional Requirements

1. The verdict **MUST** pass before the rewrite and on the new `main`.
2. The binary, the page under `web/`, and the public API **MUST NOT** change.

## Measurable Targets

| Metric | Method | Baseline | Threshold |
|---|---|---|---|
| Prototype in the tree | `git ls-files prototype` | 28 files | 0 |
| Prototype in the history | `git log --all --oneline -- prototype` on a fresh clone | 2 commits | 0 |
| npm lane | `git grep -n -i -E 'prototype/\|pnpm\|corepack' -- .github .agents/scripts tests docs/specification .taxonomy SECURITY.md README.md docs/guide` (the path, not the word, so the records of the early prototype archive of CR-0029 stay) | many lines | 0 |
| Commit count | `git rev-list --count main` before and after the rewrite | the count before | the count before minus the commits that become empty |
| Refs on the fresh remote | `git ls-remote` before the first workflow run | none | only `refs/heads/main` |
| Verdict | `.agents/scripts/pipeline.verdict.sh` | passed at `9834837` | passed |

## Affected Components

* `prototype/`, `.gitignore`, `.github/dependabot.yml`.
* `.agents/scripts/deps.autofix.sh`, `deps.freshness.sh`, `deps.common.sh` if it holds npm code, and the three prompts.
* `tests/scripts/deps.autofix.test.sh`, `tests/scripts/deps.freshness.test.sh`, `tests/fixtures/deps/`, and the Rust wrappers `tests/deps.autofix.rs` and `tests/deps.freshness.rs` (their `@agents-index` lines name a fake corepack).
* `docs/specification/dependencies.md`, `docs/specification/release.md`, `.taxonomy`, `SECURITY.md`.
* `release-please-config.json` (`bootstrap-sha`), and each tracked file with a commit hash.

## Scope Boundaries

### In Scope

* The removal, the npm lane, the specification, the rewrite, and the fresh repository.

### Out of Scope

* A change of the binary, the page, or the API.
* The deletion of `desek/agentloupe-tmp`, which the owner does.
* The release steps of `release.md` after this CR.

## Alternative Approaches Considered

* **Keep the prototype as a design reference.** Rejected by the owner.
* **Remove it from the tree only.** Rejected by the owner: the history keeps it.
* **Force push to the current new repository.** Rejected: pull request #1 keeps a ref with prototype commits.

## Impact Assessment

### User Impact

A reader finds one product in the repository.

### Technical Impact

The dependency flows cover cargo and the action pins only. Every commit hash after `953169b` changes.

### Business Impact

None.

## Implementation Approach

1. Specification. 2. Scripts, prompts, tests, configuration. 3. Remove `prototype/`. 4. Verdict. 5. Rollback log, bundle, rewrite, hash commit. 6. Repositories and push.

## Test Strategy

### Tests to Add

* In `tests/scripts/deps.autofix.test.sh`: an `npm` alert is skipped with its reason.

### Tests to Modify

* `tests/scripts/deps.autofix.test.sh`, `tests/scripts/deps.freshness.test.sh`, `tests/fixtures/deps/alerts.json`: no prototype in the scratch repositories.
* `tests/deps.autofix.rs` and `tests/deps.freshness.rs`: their `@agents-index` lines name no corepack.

### Tests to Remove

* The cases of the npm lane and of the prototype check.

## Acceptance Criteria

### AC-1: No prototype in the repository

```gherkin
Given a fresh clone of the new desek/agentloupe
When git ls-files prototype and git log --all -- prototype run
Then each prints nothing
And .gitignore lists prototype/
```

### AC-2: The dependency flows have no npm lane

```gherkin
Given .github/dependabot.yml, the autofix and freshness scripts, their prompts, and their tests
When the grep of the target "npm lane" runs, with the pattern prototype/ or pnpm or corepack
Then it finds nothing
And an npm alert is skipped with its reason in the autofix test
```

### AC-3: The history keeps its shape

```gherkin
Given main before and after the rewrite
When the commits are compared
Then each commit that is not empty keeps its order, author, and date
And the tree of the rewritten main equals the tree before
And bootstrap-sha names a commit of the new history
```

### AC-4: The fresh repository holds only main

```gherkin
Given the fresh desek/agentloupe before its first workflow run
When git ls-remote runs
Then it lists only refs/heads/main
And desek/agentloupe-tmp is private
```

### AC-5: The verdict passes

```gherkin
Given the new main
When pipeline.verdict.sh runs
Then it exits 0
```

## Quality Standards Compliance

### Build & Compilation

`cargo build` passes.

### Linting & Code Style

`pipeline.lint.sh` and `pipeline.format.sh` pass; `shellcheck` passes on each changed script.

### Test Execution

`pipeline.test.sh` passes.

### Documentation

The specification changes first.

### Code Review

A reviewer agent checks blocking findings only.

### Verification Commands

```bash
.agents/scripts/pipeline.verdict.sh
git log --all --oneline -- prototype
```

## Risks and Mitigation

### Risk 1: The rewrite loses a commit with other content

Mitigation: the tree comparison of requirement 10, the bundle, and the current repository kept as `desek/agentloupe-tmp`.

### Risk 2: A dependency flow breaks without its npm lane

Mitigation: the script tests run in the verdict, and requirement 3 skips an npm item with a reason.

## Dependencies

* CR-0023 and CR-0027 (the dependency flows), CR-0029 (the first rewrite and the new repository).

## Estimated Effort

Small to medium: one directory, one configuration entry, two scripts, three prompts, two test scripts, two specification files, one rewrite.

## Decision Outcome

Approved on 2026-10-25 by a third reviewer agent in a dynamic workflow, separate from the author, after the edits of two reviews. It found no blocking finding.

## Open Questions

None.

## Judgement Calls

1. **Skip, not fail, an npm item.** An old npm alert can still exist on GitHub after the removal; the flows record it and go on.
2. **Empty commits dropped.** The default of `git filter-repo`. The commit `9a13499` changes only `prototype/`.
3. **Records keep their text.** The CRs and scenarios of earlier work describe the prototype as it was. The owner asked to remove the directory, not the history of the decisions.
4. **The temporary repository name.** `desek/agentloupe-tmp`, private, for the owner to delete.

## Notes

### Review notes, 2026-10-25

The first reviewer found two blocking findings, each with the smallest edit: the grep of the target "npm lane" matches the path `prototype/`, not the word, so the records of the early prototype archive stay (target row and AC-2); and requirement 5 names the manifest path of the npm alert in the fixture.

The second reviewer found one more blocking finding: `tests/deps.autofix.rs` and `tests/deps.freshness.rs` match the grep target and were not in scope. Edit: both are in "Affected Components" and in "Tests to Modify".

### Hashes in this record

Each commit hash in this CR and in `CR-0030-rollback-log.md` names a commit of the history before this rewrite, which `desek/agentloupe-tmp` and the bundle keep. The rewrite dropped `9a13499`, which changed only `prototype/`. The hash commit of requirement 11 does not change these two files. The other tracked files name the new hashes; `bootstrap-sha` is `8611e9a`.

