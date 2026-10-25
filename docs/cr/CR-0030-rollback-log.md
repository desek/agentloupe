<!-- @agents-index Rollback log of the second history rewrite, which removes prototype/: the plan, the backups, the rollback steps, and the checks. -->
# CR-0030 rollback log

## Change

The history rewrite of requirements 8 to 12 of `CR-0030-remove-prototype.md`. It is a destructive change: every commit hash after the first commit that touches `prototype/` changes, and one commit that changes only `prototype/` is dropped. The approved change request is CR-0030 (approved on 2026-10-25).

## Plan

1. Push the current `main` to `desek/agentloupe`, so that repository holds every commit before the rewrite.
2. Write a git bundle of all refs to `~/Repo/backups/agentloupe-before-cr0030.bundle`, outside the repository.
3. Clone `desek/agentloupe` with `git clone --no-local` into a scratch directory.
4. Run `git filter-repo --path prototype/ --invert-paths`.
5. Check: `git log --all -- prototype` is empty; the tree of the rewritten `main` equals the tree before; each commit that is not empty keeps its author and date.
6. Add one commit that maps the commit hashes in the tracked files to the new hashes, from `.git/filter-repo/commit-map`.
7. Rename `desek/agentloupe` to `desek/agentloupe-tmp` (private). Make a fresh private `desek/agentloupe`. Set the variable `LOUPE_TELEMETRY_HOST` and the secret `CLAUDE_CODE_OAUTH_TOKEN`.
8. Push only `main` to the fresh repository. Move the local clone to the rewritten `main`.

## Rollback

The rewrite is reversible until the fresh repository becomes public. The history before it stays in `desek/agentloupe-tmp` and in the bundle.

1. Delete the fresh `desek/agentloupe`, or keep it private.
2. Rename `desek/agentloupe-tmp` back to `desek/agentloupe`.
3. In the local clone, run `git fetch origin` and `git reset --hard origin/main`, or clone the bundle.

### Checks after a rollback

* `git rev-parse main` equals the head before the rewrite, recorded below.
* `git log --oneline -- prototype` lists the commits that add and change `prototype/`.
* `gh pr list --repo desek/agentloupe --state all` lists the Dependabot pull request #1.

## Record

Filled in after the change.
