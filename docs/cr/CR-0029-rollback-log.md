<!-- @agents-index Rollback log of the history rewrite of the public repository cleanup: the plan, the backups, the rollback steps, and the checks. -->
# CR-0029 rollback log

## Change

The history rewrite of requirements 14 to 24 of `CR-0029-public-repository-cleanup.md`. It is a destructive change: every commit hash changes. The approved change request is CR-0029 (approved on 2026-10-25).

## Plan

1. Push the current `main` to the old repository `desek/agentloupe`, so the archive holds every commit before the rewrite.
2. Write a git bundle of all refs to `~/Repo/backups/agentloupe-before-cr0029.bundle`, outside the repository. Record its size and `git bundle verify` output below.
3. Clone the old repository with `git clone --no-local` into a scratch directory.
4. Run `git filter-repo --path brainstorm/ --invert-paths --replace-text <rules> --replace-message <rules>`. The rules file is outside the repository. It replaces the city name of the old zone with `Stockholm` in three letter cases.
5. Check: the commit count is the same; the tree of the rewritten `main` equals the tree of the old `main`; `git log --all -- brainstorm` is empty; `git grep -i 'nic[o]sia'` over every commit and `git log --all -i --grep 'nic[o]sia'` find nothing.
6. Add one commit that maps the old commit hashes in the tracked files, `bootstrap-sha` included, to the new hashes from `.git/filter-repo/commit-map`.
7. The owner renames the old repository to `desek/agentloupe-archive` (private) and makes a new private repository `desek/agentloupe`.
8. Push only `main` to the new repository. Change the remotes of the local clone: `origin` is the new repository, `archive` is the old one.

## Rollback

The rewrite is reversible until the new repository becomes public. The old history stays in two places: the archive repository and the bundle.

1. Delete the new repository `desek/agentloupe`, or keep it private.
2. Rename `desek/agentloupe-archive` back to `desek/agentloupe`.
3. In the local clone, run `git fetch archive` and `git reset --hard archive/main`, or clone the bundle: `git clone ~/Repo/backups/agentloupe-before-cr0029.bundle`.
4. Set `origin` back to `https://github.com/desek/agentloupe.git`.

### Checks after a rollback

* `git rev-parse main` in the local clone equals the old head recorded below.
* `git rev-list --count main` equals the old count recorded below.
* `gh repo view desek/agentloupe --json visibility` prints `PRIVATE`, and `gh pr list --repo desek/agentloupe --state all` lists the old pull requests.

## Record

* Old head before the rewrite: `61f449e84e2e3c16d9463d075952ee1d74720417`, 356 commits on `main`, pushed to the old repository at 2026-10-25 21:03 CET.
* Bundle: `~/Repo/backups/agentloupe-before-cr0029.bundle`, 10 253 872 bytes, `git bundle verify` printed "is okay", heads `refs/heads/main` at `61f449e` and the release branch at `cc566da`.
* Rewrite: 2026-10-25 21:04:20 CET, `git filter-repo` finished in 0.68 seconds. 356 commits; the tree of `main` stayed `1c1f5915b02c54bb8d302ad84c676ea936249a0a`; authors and dates the same.
* Checks of the rewrite: 0 commits touch the archive directory; 0 commit trees, 0 commit messages, and 0 file objects hold the old zone name; one ref, `refs/heads/main`.
* Hash commit: `36fa730`, 138 hashes in 38 files; `bootstrap-sha` is `8611e9ad021bb17920895adee7f750f252f78d66`. The new `main` has 357 commits. The verdict passed on it at 21:10 CET (261.0 seconds).
* GitHub, 2026-10-25 21:15:44 CET: `desek/agentloupe` renamed to `desek/agentloupe-archive`, private; new private `desek/agentloupe`; variable `LOUPE_TELEMETRY_HOST` and secret `CLAUDE_CODE_OAUTH_TOKEN` set; `main` pushed at 21:16; `git ls-remote` listed only `refs/heads/main`.
* A fresh clone of the new repository showed 357 commits and the same check results.
