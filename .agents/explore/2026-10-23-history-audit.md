---
date: 2026-10-23
branch: main
commit: d93de7d4d6d79c57c016305d530fe0ee37b3c4d4
---

# History audit before the first public release

## Question

Is the git history of this repository fit to publish? Before the first public release, audit five areas: secrets, personal data, stray files, commit quality, and tracked configuration that names the owner's machine. For each check, give the command that was run and the result it produced. End with a table of findings (severity, evidence, action) and a one-line verdict. Constraints: change nothing in the repository, do not commit, do not push, do not rewrite history, and do not search under `target/` or `node_modules/`.

## Response

### Scope

* Refs: `main` and `origin/main` at `d93de7d`, and `feat/cr-0022-release` and `origin/feat/cr-0022-release` at `05ea40b`. There are no tags and no stash. The remote `github.com/desek/agentloupe` is private (`gh repo view` → `"visibility":"PRIVATE"`).
* `git rev-list --all --count` → 301 commits. `git rev-list --all --merges --count` → 0.
* No path under `target/` or `node_modules/` was ever committed: `git log --all --name-only --format= | sort -u | grep -cE '(^|/)(target|node_modules)/'` → 0. The history dump for the searches below was made with the pathspec `-- . ':!target' ':!**/node_modules/**'`, saved to `/tmp/histpatch.txt` (7.7 MB).

### 1. Secrets

**gitleaks**

`gitleaks git --no-banner --redact=100 --exit-code 0 --report-format json --report-path /tmp/gitleaks.json .` (gitleaks 8.30.1) → "300 commits scanned … ~6.13 MB … leaks found: 20". The root commit has nothing for gitleaks to diff, so it reports 300 of the 301 commits. The searches below read `git log -p --all` and cover all 301.

Each finding was read at its commit with `git show <commit>:<file>`:

| # | Commit | File:line | Rule | What is there | Class |
|---|---|---|---|---|---|
| 1–3 | 6c959a2 | src/config/show.rs:193–195 | generic-api-key | `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, and `OPENAI_API_KEY` set to 15-character random strings in the unit test `masks_every_secret` | test fixture |
| 4–10 | db7a1ef | src/config/show.rs:54–60 | generic-api-key | the same test, at an earlier commit: `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, `LOUPE_BOOTSTRAP_TOKEN`, `LOUPE_GITHUB_TOKEN_PERSONAL`, `LOUPE_SUMMARY_API_KEY`, `LOUPE_SUMMARY_OAUTH_TOKEN`, and `LOUPE_ADMIN_TOKEN` | test fixture |
| 11 | bf9eff4 | tests/config_show.rs:93 | generic-api-key | `"legacy-Kp4Wn7"` for the legacy-name rejection test | test fixture |
| 12 | bf9eff4 | tests/serve_start.rs:111 | generic-api-key | `"legacy-Vq3Zr8"` for the legacy-name rejection test | test fixture |
| 13 | 6c959a2 | src/summary/worker.rs:105 | generic-api-key | `AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=`, the base64 of bytes 0x00–0x1f, the test key of the project | test fixture |
| 14 | 6362d58 | tests/common/days.rs:99 | generic-api-key | the same 0x00–0x1f test key | test fixture |
| 15 | a0694c2 | .agents/scripts/summary.session_days.scenario.py:17 | generic-api-key | the same 0x00–0x1f test key | test fixture |
| 16 | 4385a3b | .agents/scripts/backfill.bench.sh:63 | generic-api-key | the same 0x00–0x1f test key | test fixture |
| 17 | 4a7bc92 | tests/live.route.rs:210 | generic-api-key | `sec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==`, the sample nonce from RFC 6455 | placeholder |
| 18–19 | b577bdd | .agents/scripts/github.bench.sh:58–59 | curl-auth-header | `Bearer bench-token` sent to the local `fake_github` on 127.0.0.1 | placeholder |
| 20 | dc70a65 | .agents/scenarios/proxy.redaction.md:20 | curl-auth-header | `x-api-key: sk-test-123` sent to the local proxy | placeholder |

Real credentials: **0**.

**Own search** over `git log -p --all` (`/tmp/histpatch.txt`):

| Pattern (`grep -cE`) | Lines | Distinct values and classification |
|---|---|---|
| `lpt_[A-Za-z0-9_-]{16,}` | 0 | none |
| `sk-ant-` | 23 | Most matches are the search term itself, in CR and scenario text (`grep … 'lpt_…\|sk-ant-'`). Values: `sk-ant-test-Qm7Vx2Lp9Rk4` and `sk-ant-oat-test-Hd3Nw8Zs1Fj6` (test constants), `sk-ant-abc123def` (scrub corpus), and `sk-ant-fake` (scenario script). All fixtures. |
| `ghp_` | 11 | `ghp_one`, `ghp_two`, `ghp_testvalue123`, `ghp_abc123xyz`, `ghp_privacyTokenValue9182736455`, and `ghp_invalidinvalidinvalidinvalid0000` (a documented invalid value), plus search terms. All fixtures. |
| `github_pat_` | 3 | Only as a search term inside a `grep` pattern in CR text. |
| `AKIA` | 0 | none |
| `BEGIN [A-Z ]*PRIVATE KEY` | 0 | none |
| `Bearer [A-Za-z0-9._~+/=-]{20,}` | 0 | none |

Named variables with a value after `=`: `grep -oE '(CLAUDE_CODE_OAUTH_TOKEN|LOUPE_DATA_KEY|LOUPE_ACCOUNT_KEY|LOUPE_GITHUB_TOKEN[A-Z_]*|ANTHROPIC_API_KEY)[ ]?=[ ]?\S{1,70}'` found only these values:

* shell references: `"$key"`, `$K`, `"$(docker …`, `"$CLAUDE_CODE_OAUTH_TOKEN"`, and `"${LOUPE_ACCOUNT_KEY:-$key}"`;
* doc placeholders: `...`, `K`, `<key>`, `<value>`, and `x`;
* fixtures: `test`, `testA-Zk2`, and `bench-token`.

No literal credential. `.env` was never committed (`git log --all --oneline -- .env` → 0 lines). `.env.example` holds only empty `LOUPE_DATA_KEY=` and `LOUPE_ACCOUNT_KEY=`.

### 2. Personal data

**Authors and committers** (`git log --all --format='%an <%ae>' | sort | uniq -c` and the same for `%cn <%ce>`):

* Authors: 300 × `desek <daniel@grenemark.se>` and 1 × `Daniel Grenemark <daniel@grenemark.se>`. The second is `46735d3`, the squash merge of PR #1 on the GitHub web UI.
* Committers: 300 × `desek <daniel@grenemark.se>` and 1 × `GitHub <noreply@github.com>`.
* There are no `Co-authored-by` trailers. The only trailers are two `Intent:` lines. Commit messages hold no email address.

**Emails in content** (`grep -oE '[A-Za-z0-9._%+-]+@…' histpatch.txt`): `bob@corp.com`, `t@example.invalid`, `ops@company.org`, `a@b.co`, `jim@corp.io`, `jane.doe@acme.com`, `git@host.example`, `daniel@example.com`, and `bob@x.io`. All of them are scrubber or test fixtures; `daniel@example.com` is on a reserved domain.

**Owner's name in content**: `Daniel Grenemark` appears in the `requestor`/`stakeholders` front matter of the CRs and in `LICENSE` ("Copyright (c) 2026 Daniel Grenemark"). Both are deliberate attribution. `grenemark.se` appears in the file name `brainstorm/dashboard.grenemark.se-html.zip`, in the README file table, in `docs/cr/CR-0008-dashboard-spa.md:585`, and inside the archive.

**Home paths** (`grep -oE '(/Users/…|/home/…|C:\\Users\\…)' histpatch.txt`):

* `/Users/marker` (12 matches) is the synthetic fixture `tests/fixtures/backfill/*.jsonl`.
* `/Users/<owner>` (2 matches) is `-e /Users/<owner>/Repo/desek/experiments/pi-cc-auth/src/index.ts` in decision D6a of `docs/cr/CR-0010-macos-end-to-end-acceptance.md`. It was added in `01a1d4f` and removed in `0236740`. It is gone at HEAD: `git grep '/Users/<owner>'` → 0. It remains in history.
* At HEAD, `.agents/scripts/pi.session.sh:20` has `EXT="$HOME/Repo/desek/experiments/pi-cc-auth/src/index.ts"`, the owner's directory layout.

**Real session text**:

* Run logs `.agents/scenarios/acceptance.macos.end-to-end.run{1..4}.log` (about 1.8 KB each) hold versions, health output, counts, and `summary: present`, but no summary text. These runs drove `claude -p` and `pi -p` with a one-word prompt.
* `acceptance.macos.end-to-end.day.json` has the one summary text in the tracked tree: "two short sessions, both of which were trivial … asked to reply with one word …". It describes the synthetic acceptance run, not real work.
* `.agents/logs/` is ignored and was never tracked: `git log --all --name-only | grep '\.log$'` lists only the four scenario run logs.
* The scenarios that ran on the owner's real logs report only counts. `backfill.real-logs.md` gives "calls_added 2448 … the dump search found 0 matches", and `summary.live.md` gives "a line of 8 words, line_safe: yes". `summary.no-text-retry.md` and `parser.real-agents.md` give counts and statuses only. Prompts in these files are synthetic ("say hi", "reply with the single word ready").
* `docs/cr`: `git grep -nE '^> |line (was|read|said)|the model (wrote|answered|returned) "'` and a search for summary-style verbs found only synthetic test strings in Gherkin ("Fixed the parser in src/main.rs for https://x.io"). The deleted `CR-0014-backfill-transcript-whole-session.md` (removed in `60d08b6`) is design text, not a transcript.
* `tests/fixtures/summary/scrub_corpus.txt` (53 lines) is a hand-written corpus with fictional names and paths.
* Screenshots: I viewed `private-reports-page.png`, `dashboard.second-pass.1440.png`, the history-only blob `fc2ab96873` (an old `dashboard.second-pass.1440.png`), `dashboard.readability.1440.png`, `acceptance.macos.end-to-end.dashboard.png`, `dashboard.live.follow.png`, and `dashboard.offline.png`. All show seeded data ("Worked on the seeded task number N", groups `ci/desktop/laptop/server`, sessions `agent-0..2`) or the synthetic acceptance day. No real summary text.

**Hostnames and IPs** (`grep -oE '\b([0-9]{1,3}\.){3}[0-9]{1,3}\b' histpatch.txt`):

* `127.0.0.1` (405) is loopback, and `0.0.0.0` (3) is a bind address.
* `10.0.0.12` and `192.168.1.2` are scrub-corpus fixtures.
* `1.1.1.1:1`, `2.2.2.2:2`, and `3.3.3.3:3` are `LOUPE_LISTEN` precedence test values.
* The local hostname (`Daniels-MBP.molnmyra.local`) does not occur: `grep -ciE 'molnmyra|Daniels-MBP'` → 0. The only `.internal` hit is the fixture `www.internal-corp.net`.

**Location hint**: the product default `LOUPE_TZ=Europe/Stockholm` (README, `.env.example`, CR-0001, CR-0007) and "Stockholm time" in the mockup footer suggest where the owner lives. It is a product default, not a leak.

### 3. Stray files

**Twenty largest blobs in history** (`git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(objectsize:disk) %(rest)' | awk '$1=="blob"' | sort -k3 -nr | head -20`):

| Size (bytes) | On disk | Blob | Path | At HEAD | Belongs in a public repo? |
|---|---|---|---|---|---|
| 2,926,145 | 231,003 | 0a90c352f3 | vendor/litellm/model_prices_and_context_window.json | yes | Yes. Vendored price data, with `LICENSE` and `SOURCE.md`. |
| 1,494,344 | 1,265,228 | a9b019fbe0 | .agents/scenarios/dashboard.second-pass.1440.png | yes | Yes. Scenario evidence, seeded data. |
| 1,467,416 | 1,241,530 | 4904b1ad76 | .agents/scenarios/dashboard.readability.1440.png | yes | Yes. Scenario evidence. |
| 1,359,473 | 1,172,872 | 2d6744fa84 | .agents/scenarios/dashboard.second-pass.360.png | yes | Yes. Scenario evidence. |
| 1,326,414 | 1,144,175 | 407b6e801a | .agents/scenarios/dashboard.readability.360.png | yes | Yes. Scenario evidence. |
| 821,472 | 704,472 | fc2ab96873 | .agents/scenarios/dashboard.second-pass.1440.png | history only | Yes. Older evidence, seeded data. |
| 735,461 | 637,700 | 6e601cac09 | .agents/scenarios/dashboard.second-pass.360.png | history only | Yes. Older evidence. |
| 372,214 | 289,503 | 7f997374c0 | .agents/scenarios/private-reports-page.png | yes | Yes. Scenario evidence, seeded data. |
| 211,816 | 82,396 | 5163f87146 | brainstorm/dashboard.grenemark.se-html.zip | yes | **Owner's call** (see below). |
| 115,320 | 105,767 | d635e9e5e1 | .agents/scenarios/acceptance.macos.end-to-end.dashboard.png | yes | Yes. Scenario evidence, synthetic day. |
| 100,623 | 94,102 | 27261b67e1 | .agents/scenarios/dashboard.light-mode.light.1440.png | history only | Yes. |
| 100,148 | 94,260 | d68c0904ef | .agents/scenarios/dashboard.light-mode.dark.1440.png | history only | Yes. |
| 99,002 | 94,205 | 6b01cabc1d | .agents/scenarios/dashboard.sections.6m.png | yes | Yes. |
| 98,832 | 92,617 | d70c1772c0 | .agents/scenarios/dashboard.light-mode.light.1440.png | yes | Yes. |
| 98,260 | 92,625 | 7002794bb9 | .agents/scenarios/dashboard.light-mode.dark.1440.png | yes | Yes. |
| 97,604 | 92,523 | bd550c9d76 | .agents/scenarios/dashboard.headline.1440.png | yes | Yes. |
| 89,034 | 82,733 | 9fdd268375 | .agents/scenarios/dashboard.product-header.1440.png | yes | Yes. |
| 74,344 | 26,396 | 5657010984 | docs/cr/CR-0023-dependency-autofix.md | yes | Yes. |
| 71,596 | 71,638 | 0e746fc182 | web/assets/fonts/GeistMono-Regular.woff2 | yes | Yes. Font with `OFL.txt` and `SOURCE.md`. |
| 71,023 | 20,273 | 2e173cc1e9 | Cargo.lock | yes | Yes. |

**Tracked build output, logs, databases, archives, and screenshots** (`git ls-files | grep -iE '\.(log|db|sqlite3?|zip|tar|gz|…|png|…|woff2?|ico|…)$|(^|/)(dist|build|out|target|coverage)/'`):

* Build output (`dist/`, `build/`, `target/`, `*.o`, `*.rlib`, `*.wasm`, binaries): none.
* Databases (`*.db`, `*.sqlite*`): none. Both are also in `.gitignore`.
* Logs: four files, `.agents/scenarios/acceptance.macos.end-to-end.run{1..4}.log` (1.7–2.0 KB). These are scenario evidence of a synthetic run, so keep them.
* Screenshots: 21 PNGs under `.agents/scenarios/`, 5.5 MB at HEAD. Four of them are tall full-page shots of 1.3–1.5 MB each. They are scenario evidence with seeded data, so keep them. Optionally downscale or crop the four large ones in a future commit to keep clones small.
* Archive `brainstorm/dashboard.grenemark.se-html.zip` (212 KB): a design-tool export of the dashboard mockup with labelled "Sample data". It contains `Main.dc.html`, `support.js`, the React production builds (MIT header kept), and a README. It names the owner's personal domain and "Stockholm time". `README.md:26` and CR-0008 cite it as the source of the design values. **This needs the owner's call.** It holds no secret. Removing it from the tree leaves it in history, which is acceptable.
* Archive `brainstorm/agent-ledger.zip` (58 KB): untested early prototype source (Rust, a k8s manifest with host `ledger.example.com`, and an `.env.example` with empty values). I streamed it through `unzip -p … | grep` for secret shapes, home paths, emails, and IPs: only `0.0.0.0:8787`. It belongs as a reference, which is what `README.md:25` says it is; the owner may drop it.
* Fonts `web/assets/fonts/Geist-Regular.woff2` and `GeistMono-Regular.woff2`, and `web/favicon.ico`: runtime assets, with a license file. Keep.

Files deleted from the tree but still in history (`git log --all --diff-filter=D --name-only`): `docs/cr/CR-0014-backfill-transcript-whole-session.md`, `docs/specification/days-public-api.md`, `docs/specification/parser-sessions.md`, `docs/specification/tokens.md`, `prototype/package-lock.json`, and `web/assets/js/section.lines.js`. None of them is sensitive.

**Total size**:

* `git rev-list --all --objects --disk-usage=human` → **12.60 MiB** reachable.
* All 1,345 blobs: 22.65 MiB uncompressed, 11.99 MiB compressed on disk.
* `git count-objects -vH` → 2,952 loose objects, 21.70 MiB, no pack. `du -sh .git` → 22 MB.
* Tracked working tree: about 14 MB (`git ls-files -z | xargs -0 du -ck`).

A push packs the objects, so the public size will be at or below about 12.6 MiB. I did not run `git gc`, because it would modify `.git`.

### 4. Commit quality

`git log --all --format='%h%x09%s'` → 301 subjects.

* **Conventional Commits syntax** (`^[a-z]+(\([^()]+\))?!?: \S`): **301 of 301 (100%)**. The specification allows types other than `feat` and `fix`.
* **Standard types only** (`feat|fix|docs|chore|refactor|test|build|ci|perf|style|revert`): **140 of 301 (46.5%)**. Types used: `checkpoint` 161, `docs` 126, `fix` 7, `feat` 4, `chore` 2, `test` 1.
* **Subjects that do not use a standard type**: all 161 `checkpoint(CR-XXXX): …` commits. This is the project's own governance type; `release-please-config.json` declares it as a hidden section ("Checkpoints"), with `bootstrap-sha` `446f065`. No subject breaks the syntax.
* **Empty or one-word messages**: none. `git log --all --format='%h|%s' | awk -F'|' '$2=="" || $2 !~ / /'` → no output. The shortest subjects have three words: `docs(cr): approve CR-00NN` (×31 with re-approvals) and `checkpoint(CR-00NN): update specification` (×19). Ten subjects repeat verbatim, for example `docs(cr): approve CR-0010` three times.
* **Merge commits**: none (`git rev-list --all --merges --count` → 0). PR #1 was squash merged as `46735d3`.
* **Other observations**:
  * 284 of 301 commits have an empty body. By type: `checkpoint` 154, `docs` 123, `feat` 2, `fix` 2, `chore` 2, `test` 1.
  * 137 subjects are longer than 72 characters.
  * No subject uses `!` for a breaking change.
  * `feat/cr-0022-release` (local and `origin`) holds the 8 pre-squash `checkpoint(CR-0022)` commits, which are not on `main`. If the branch stays on the remote, those commits are published too.

### 5. Tracked configuration that names the owner's machine

`git ls-files | grep -E 'compose|tool-versions|\.vscode|\.idea|\.editorconfig|rust-toolchain|\.env|\.mise|\.nvmrc'` → `.env.example`, `.tool-versions`, `compose.yaml`, and `rust-toolchain.toml`.

| File | Content | Names the machine? |
|---|---|---|
| `compose.yaml` | Port `127.0.0.1:8787`, a named volume, `env_file: .env` | No |
| `.tool-versions` | `nodejs 22.20.0`, `aqua:nextest-rs/nextest/cargo-nextest 0.9.149` | No. It pins tool versions, not paths. |
| `rust-toolchain.toml` | `channel = "1.97.1"`, rustfmt, clippy | No |
| `.env.example` | Empty secrets, documented defaults | No |
| `compose.override.yaml` | Mounts `/Users/<owner>/.claude/projects` and `/Users/<owner>/.pi/agent/sessions` | Yes, but **untracked**. Ignored by `.gitignore:19` and never committed (`git log --all -- compose.override.yaml` → 0). |
| `.vscode/` and `.idea/` | not present, never committed | n/a |
| `.github/workflows/*.yml` | `~/.cargo/registry` and `~/.cargo/git` cache paths of the runner | No |
| `.agents/scripts/pi.session.sh:20` | `EXT="$HOME/Repo/desek/experiments/pi-cc-auth/src/index.ts"` | Yes: the owner's directory layout and a sibling private project |

### Findings

| # | Finding | Severity | Evidence | Recommended action |
|---|---|---|---|---|
| 1 | No real credential in history | note | gitleaks: 20 findings, all test fixtures (16) or placeholders (4). Own search: 0 `lpt_`, `AKIA`, private key, or long Bearer token; every `sk-ant-` and `ghp_` value is a fake; no literal value for the five named variables. `.env` was never committed. | None. Keep gitleaks in the verdict workflow. |
| 2 | Design mockup archive names the owner's domain | should fix (owner's call) | `brainstorm/dashboard.grenemark.se-html.zip` (212 KB): a design-tool export with sample data, the React production builds, "dashboard.grenemark.se", and "Stockholm time". Cited by `README.md:26` and CR-0008. | Owner decides whether to keep it as the cited design source or remove it (and its README row) in a normal commit. No history rewrite is needed; it holds no secret. |
| 3 | Script hardcodes the owner's directory layout | should fix | `.agents/scripts/pi.session.sh:20` `EXT="$HOME/Repo/desek/experiments/pi-cc-auth/src/index.ts"` | Read the path from an environment variable (for example `PI_CC_AUTH_EXT`) and fail with a clear message when it is unset. |
| 4 | Home path of the owner in history | note | `/Users/<owner>/Repo/desek/experiments/pi-cc-auth/src/index.ts` in CR-0010 D6a, added in `01a1d4f` and removed in `0236740`. Not at HEAD. The user name equals the public GitHub handle. | Accept. Do not rewrite history for it. |
| 5 | Owner's personal email in all commit metadata | note | 301 of 301 commits use `daniel@grenemark.se`. 300 are authored as `desek` and 1 (`46735d3`, the web squash) as `Daniel Grenemark`. | Owner's choice. To keep the address private in future commits, set a GitHub noreply address; changing past commits needs a rewrite, which is not recommended. |
| 6 | Personal data in content is fixtures only | note | Emails are on fixture or reserved domains; IPs are loopback, bind, or fixture; the local hostname is absent; `/Users/marker` is a fixture; run logs, day JSON, and the 7 screenshots I viewed show synthetic or seeded data; scenarios on real logs report counts only. | None. |
| 7 | Default time zone hints at location | note | `LOUPE_TZ` default `Europe/Stockholm` in README, `.env.example`, and the CRs | Accept, or change the default to `UTC` if the owner prefers. |
| 8 | Prototype archive | note | `brainstorm/agent-ledger.zip` (58 KB): early prototype source, `ledger.example.com`, empty `.env.example`, no secret shapes | Keep as the documented reference, or drop it at the owner's discretion. |
| 9 | Large scenario screenshots | note | 21 PNGs, 5.5 MB at HEAD. Four full-page shots are 1.3–1.5 MB each, and older versions remain in history. Total reachable size is 12.60 MiB. | Keep, because scenario evidence may stay. Optionally crop or downscale future captures. |
| 10 | Pre-squash branch on the remote | note | `origin/feat/cr-0022-release` at `05ea40b` holds 8 `checkpoint(CR-0022)` commits not on `main` | Delete the branch on the remote before the repository goes public, unless it is kept on purpose. |
| 11 | Commit message style | note | 100% Conventional Commits syntax; 46.5% standard types (161 `checkpoint` are hidden by release-please). No empty, one-word, or merge commit. 284 empty bodies; 137 subjects over 72 characters; 10 duplicated subjects. | None for the release. Optionally keep subjects to 72 characters or fewer. |
| 12 | Machine-specific compose override stays local | note | `compose.override.yaml` names `/Users/<owner>/...`, but it is ignored (`.gitignore:19`) and was never committed. Tracked `compose.yaml`, `.tool-versions`, and `rust-toolchain.toml` are machine-neutral. | Keep it ignored. |

**Verdict: fit to publish. There are no blocking items. Before the switch to public, the owner should decide on `brainstorm/dashboard.grenemark.se-html.zip` and parametrize the path in `.agents/scripts/pi.session.sh`.**
