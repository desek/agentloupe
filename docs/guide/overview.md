<!-- @agents-index User guide: Status of loupe, its scope, how it works, and the open work. -->

# Overview

## Status

The `loupe` binary is implemented and runs in the container. 27 change requests under
[`docs/cr/`](../cr/) carry the specification of each part, and
[`docs/specification/`](../specification/) describes the behaviour of the code. The binary
captures Claude Code and Pi through the proxy, prices and summarizes the calls, publishes the day
records, serves the dashboard with its live section and the private page, and backfills the session
logs that predate the proxy.

Releases are automated. release-please keeps one release pull request from the Conventional Commits
on `main`; its merge makes the tag `v<version>` and the GitHub release. The release workflow then
attaches one archive per platform with its checksum file, and publishes the multi-platform image
`ghcr.io/desek/agentloupe`. See [Install](install.md#install) and
[`docs/specification/release.md`](../specification/release.md).

The design values of the dashboard came from a design mockup with sample data.

The README and the guides under `docs/guide/` are the authority for the user. Where a source differs
from them, the README and the guides are correct.

## Scope

In scope:

- One container that runs on this machine, reached through `localhost`.
- The public instance: a second container in the role `public` that serves only the public
  page, the public API, and the live feed from the public mirror (see "Public hosting").
- Claude Code and Pi, which send their model requests through the proxy.
- Pull requests from GitHub, read through the GitHub API.

Out of scope for now:

- The Cloudflare Tunnel in front of the public instance. It is later work.
- claude.ai chats (web, desktop, mobile). They go directly to claude.ai, so the proxy cannot
  record them. The dashboard footer states which sources it covers.

## How it works

```
Claude Code ─┐                            ┌─► api.anthropic.com
             ├─► proxy ── forwards as-is ─┤
Pi ──────────┘     │                      └─► api.openai.com, chatgpt.com, …
                   ▼ capture (encrypted)
               exchanges ─► parser ─► calls ─► sessions ─(idle)─► summarize ─► agg_queue
                                                                                   │
GitHub API ─► GitHub worker (PRs, lines) ──────────────────────► days ◄─(nightly)──┘
                                                                   │
                                                            GET /v1/days
                                                                   │
                                                                   ▼
                                                    dashboard at GET / (same binary)
```

1. The agents (Claude Code and Pi) send their model requests to the proxy through a base URL.
2. The proxy forwards each request unchanged and stores the exchange. It encrypts the bodies
   with AES-256-GCM and redacts the credentials.
3. A parser reads the stored exchanges. It finds model calls from the usage block in the
   response (Anthropic Messages, OpenAI Responses, OpenAI Chat Completions).
4. The parser prices each call from the vendored LiteLLM price file (see "Cost").
5. The parser groups the calls into sessions. A session closes after 60 minutes with no calls. A subagent is its own session.
   A session that spans midnight counts on each day that has a call.
6. A small model writes one line for each closed session segment. For each session and each day
   with a call of that session, it writes a full session summary, which covers the session up to
   the end of that day, and a session day summary, which says what the session did on that day.
   It reads the whole session, the conversation first. The paragraph of 3 to 8 sentences for each
   day comes from the session day summaries of that day, in a neutral tone. A scrubber then
   removes URLs, paths, email addresses, and numbers from the line.
7. The GitHub worker counts the pull requests and the changed lines of the day (see "GitHub").
8. At 01:00 (`LOUPE_TZ`, default UTC) the binary builds the previous day and publishes it on
   `GET /v1/days`.
9. The same binary serves the dashboard at `/`. The page reads `/v1/days` and shows the data.

## Open work

- **Codex account source.** Confirm the `ChatGPT-Account-Id` header of Codex with a capture of
  real requests, and update the "Accounts" section.
- **Build provenance.** Attest the release assets and sign the image, with an SBOM.
- **Draft releases.** Keep a release as a draft until its assets exist.
- **Release build cache.** Build the release assets with no cargo cache.
- **Pinned actions text.** Align the "Pinned actions" text of `release.md` with the Dependabot
  updates of the actions.
- **Secret scanning of fixtures.** Add a `.gitleaksignore` for the test fixtures.
- **Repository files.** Add `CODEOWNERS` and `.editorconfig`, and sign the commits.
- **Release check after publish.** Run `release.check.sh` automatically after each release.
