---
date: 2026-10-24
source: cr (CR-0023, as amended by CR-0024)
surface: cli (deps.autofix.sh with gh), the GitHub pull request and its verdict workflow, the workflow file
outcome: reproduced
runs: "dry run 2 of 2 on the recorded fixtures and 2 of 2 read-only against desek/agentloupe, shell tests 24 of 24, last run 2026-10-24; the real run waits"
---

# Scenario: dependency autofix, dry run and the steps that wait

- Goal: the release manager sees, from the shell and from GitHub, that the daily dependency autofix of `docs/specification/dependencies.md` finds the Dependabot alerts and pull requests, plans one fix per alert on the branch `deps/autofix-<date>`, plans the merges and the release pull request step, writes nothing in a dry run, and keeps every credential out of `claude -p`, the verdict, and the output. The real run waits for the public repository, the Dependabot alerts, and the secret `CLAUDE_CODE_OAUTH_TOKEN`.
- Surface: the shell with `.agents/scripts/deps.autofix.sh` and the real `gh` login of the owner (read-only); the fake `gh` of `tests/scripts/deps.autofix.test.sh` with the recorded answers of `tests/fixtures/deps/` for the alert list, because the alerts are off on the private repository; the pull request on GitHub and its verdict run, read with `gh`.
- Success condition: (1) each fixture dry run plans the items of the recorded lists with 0 write calls, 0 `claude -p` runs, a clean tree, and no branch on the origin; (2) each real dry run makes only GET calls and stops with the actionable error that names the setting Dependabot alerts, exit 4; (3) the 24 shell tests pass in the test stage; (4) the full local verdict exits 0 and the hosted check `verdict` of the pull request is green; (5) `gitleaks` and the pattern search find 0 credentials in the recorded logs.
- Who: "agent" steps run from the coding agent with `gh` and the shell; "owner" steps change the visibility, turn on a setting, or create a secret.

## Preconditions

- The verdict workflow `.github/workflows/verdict.yml` is active on `desek/agentloupe` (`gh api repos/desek/agentloupe/actions/workflows/verdict.yml --jq .state` prints `active`).
- The repository is private; the Dependabot alerts are off (`gh api 'repos/desek/agentloupe/dependabot/alerts?state=open'` answers HTTP 403 "Dependabot alerts are disabled for this repository.").
- The shell of the agent holds no `RELEASE_TOKEN`, no `GH_TOKEN`, and no `CLAUDE_CODE_OAUTH_TOKEN`; `gh` is logged in as the owner.
- `jq`, `git`, and `gh` are installed. A dry run needs no `claude`, `cargo`, or `corepack`.

## Steps (private repository, agent)

1. Fixture dry run: in a scratch repository made by the `setup` function of `tests/scripts/deps.autofix.test.sh` (a local bare origin, the script and its prompt template, a stand-in verdict), serve the full `alerts.json` (3 alerts), `pulls.json` (3 Dependabot pull requests, the grouped one with a red check), the release pull request `0.1.1`, the files of each pull request (the action pin with a tag), and run `deps.autofix.sh --dry-run --fixtures <dir>` two times. Record the write calls of the fake `gh`, the `claude -p` calls of the fake `claude`, `git status --porcelain`, and the branches of the origin.
2. Real dry run: put a recording wrapper of `gh` first on `PATH` (it appends its arguments to a file and runs the real binary), and run `.agents/scripts/deps.autofix.sh --dry-run` two times in the checkout. Record the output, the exit code, the duration, and every `gh` call.
3. Check the repository after step 2: the branches, the open pull requests, and the issues with the label `dependencies` (`gh api repos/desek/agentloupe/branches`, `gh pr list`, `gh issue list --label dependencies --state all`).
4. Run `tests/scripts/deps.autofix.test.sh` through `cargo nextest run --locked --test deps_autofix` (four groups): nothing open, the dry run of an alert, an alert with a matching pull request, no patched version, the precondition, a missing credential, the alerts off, the full run of one alert to the release merge, the credential rule, an invalid result, a path outside the allowed paths, a code change only after a failed version verdict, a credential in the diff, the redaction, the hold time, one Dependabot pull request and the release step with `1.0.0`, the release step of a later run, the rebase, a red pull request to `claude -p`, the pin with a tag, the authentication error, the timeout, the interface, and the thin caller workflow.
5. Run `gitleaks dir` and `grep -cE 'sk-ant-|ghp_|github_pat_|gho_|ghs_'` over the dry run log and the outputs and issue texts of three fixture runs of one alert.
6. Run `.agents/scripts/pipeline.verdict.sh` on the branch, push it, open the pull request, and watch the check `verdict`.

## Steps that wait

These steps need the public repository, the Dependabot alerts, and the secret `CLAUDE_CODE_OAUTH_TOKEN`, which the owner has not created. They follow the order of `docs/cr/CR-0023-dependency-autofix.md`, section "Model-Based Testing".

7. Owner: switch the repository to public (CR-0022, CR-0024) and apply `repo.settings.sh`, which turns on the Dependabot alerts, the main ruleset with the check `verdict`, and auto-merge.
8. Owner: create the repository secret `CLAUDE_CODE_OAUTH_TOKEN` with `claude setup-token`, record its expiry date in `docs/specification/dependencies.md` (Tokens), and add the permission "Dependabot alerts: read" (and "Workflows: write" when a pin merge needs it) to the token of `RELEASE_PLEASE_TOKEN`.
9. Agent, with the consent of the owner recorded here with the date: Part A on the public repository. On a scratch branch `deps/scenario-<date>`, add a crate at a version with a known advisory and a known patched version, push it, wait for the Dependabot alert, and run `deps.autofix.sh --dry-run` two times; check the alert, the package, the range, the patched version, the manifest, the branch, and the planned merge, and that no branch, pull request, issue, commit, or `claude -p` process followed.
10. Agent: Part B, the first real run. Start the workflow with `gh workflow run deps-autofix.yml`; check one active run and a second dispatch that waits; the branch `deps/autofix-<date>` with one `fix(deps)` commit of allowed paths; the squash merge after the green check `verdict`; the closed alert; the release pull request with a patch version and its merge by the script; the tag and the release; the run time, the tokens, and the cost of the item line.
11. Agent: Part C, the first weekly Dependabot pull request (a Monday run after 03:00 UTC): the rebase when behind and the squash merge when green. Check whether Dependabot supports the pnpm version of `prototype/package.json` (Checks at implementation, item 7).
12. Agent: Part D, the failure path with `--max-turns 1` on a seeded alert: no commit for the item, one issue with the label `dependencies`, and no new `claude -p` run on a second run of the same day.
13. Agent: Part E, `gitleaks` and the pattern search over the run logs (`gh run view <id> --log`), the issue text, and the pull request text of each run of Parts B to D.
14. Agent: the checks of auto-merge with the bypass of the release token (Checks at implementation, item 4) and of one squash commit with many fixes in the changelog (item 6); the 14 first scheduled runs for the row "Share of runs with a defined end".

## Runs

- 2026-10-24, agent: steps 1 to 6 ran; steps 7 to 14 wait. The log of steps 1 to 3 is `.agents/scenarios/deps.autofix.dryrun.log`.
  1. Fixture dry runs, 2 of 2: exit 0; `count: alerts: 3 pull_requests: 3`. The alert `alert:11` (smallvec) became one item with the path `pull_request` through `pr:21`, which moves smallvec to the patched version 1.13.3, with `plan: merge`. `alert:12` (esbuild of the prototype) planned one `claude -p` run with the prompt fields and the commit `chore(deps): bump esbuild to 0.25.0 (GHSA-dddd-eeee-ffff)`. `alert:13` (no patched version) planned `no_patched_version` and no run. `pr:22` (a pin to a tag) planned `pin_not_sha` and no merge. `pr:23` (grouped, red check) planned one `claude -p` run. The branch `deps/autofix-2026-10-24` from main, its pull request, the squash merge after a green check, the release pull request `0.1.1` merge, and the issue with 2 items to record were planned. 0 write calls, 0 `claude -p` runs, a clean tree, and only `main` on the origin, in both runs.
  2. Real dry runs, 2 of 2: exit 4 in 1.62 and 1.53 seconds; `repo: desek/agentloupe visibility: private mode: dry-run`, then the error "the Dependabot alerts are off for desek/agentloupe: the alert list answered 'Dependabot alerts are disabled for this repository.'", which names the setting, and the result line. The `gh` log holds 3 calls per run, each a GET (`repos/desek/agentloupe`, `.../actions/workflows/verdict.yml`, `.../dependabot/alerts?state=open&per_page=100`).
  3. After step 2: the branches are `main` and the release-please branch, the open pull request is `#2 chore(release): release 0.1.0`, 0 issues have the label `dependencies`, and the local tree is unchanged.
  4. Shell tests: 24 of 24 passed in 4 nextest groups (the longest group 46 seconds in the test stage). Three fixture runs of one alert took 4.65 to 4.93 seconds from the start to the release merge, and three runs with nothing open 1.08 to 1.16 seconds; these are local times with the fakes, not the hosted times of the targets. The item line of the fixture result gives `cost_usd: 0.22509 total_cost_usd: 0.22509`, a difference of 0 percent on the recorded counts.
  5. `gitleaks dir`: no leaks found; the pattern search: 0 matches in each file.
  6. `pipeline.verdict.sh`: `verdict: passed` in 483.4 seconds (test stage 284.4 seconds with a cold build, 450 tests). The hosted check `verdict` of <https://github.com/desek/agentloupe/pull/7> on `ea826b2`: run 40045426258, `success` in 7 minutes 8 seconds on the Linux runners (GNU tools), the test job `stage: test status: passed seconds: 318.9`.
