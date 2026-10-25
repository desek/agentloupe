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

* Head before the rewrite: `7a5a50143e4522994985fcc87b076273befd00e6`, 367 commits on `main`, pushed to `desek/agentloupe` (now `desek/agentloupe-tmp`) at 2026-10-25 22:21 CET.
* Bundle: `~/Repo/backups/agentloupe-before-cr0030.bundle`, 11 289 591 bytes, `git bundle verify` printed "is okay".
* Rewrite: 2026-10-25 22:22:01 CET, `git filter-repo --path prototype/ --invert-paths`, finished in 0.38 seconds. 366 commits: the commit `9a13499` became empty and was dropped. The tree of `main` stayed `688dd43f0ce6cca5414a0be8d8dac01e756886b1`. Each kept commit has the same author and date.
* Hash commit: 119 hashes in the tracked files; `bootstrap-sha` names a commit of the new history. The new `main` has 367 commits.
* GitHub, 2026-10-25 22:22:34 CET: `desek/agentloupe` renamed to `desek/agentloupe-tmp`, private; fresh private `desek/agentloupe`; variable `LOUPE_TELEMETRY_HOST` and secret `CLAUDE_CODE_OAUTH_TOKEN` set; `main` pushed at 22:22:46; `git ls-remote` listed only `refs/heads/main`.
* A fresh clone of the new repository: 367 commits; 0 commits that touch `prototype` or the early prototype archive; 0 trees, messages, and file objects with the old zone name.
