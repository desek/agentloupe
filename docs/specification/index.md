<!-- @agents-index Index of the system specification: one row per product area file and what it covers. -->
# System specification

This directory describes how the `loupe` binary behaves. Each file covers one product area.

| File | Covers |
|---|---|
| `proxy.md` | The proxy routes, the forward to the upstream, the capture of each exchange, and the redaction of credentials. |
| `proxy.tokens.md` | The proxy tokens, the `loupe token` subcommands, the bootstrap token, and the token gate. |
| `accounts.md` | The hash of the provider account, the order of its sources, and the accounts table. |
| `parser.md` | The parser that finds calls in exchanges, the token columns, and the sessions and segments that group the calls. |
| `backfill.md` | The backfill of calls and sessions from the session logs of Claude Code and Pi, and the summaries of backfilled segments. |
| `cost.md` | The API-equivalent cost of each call from the vendored price file. |
| `summaries.md` | The one-line session summaries, the day paragraph, and the retention of raw bodies. |
| `days.md` | The day records, the aggregation queue, the publisher, `loupe aggregate`, and `loupe day`. |
| `api.md` | The public API routes under `/v1/days`, the errors, the headers, and the version policy. |
| `live.md` | The live route `/live`, the live hub, the live events and their allowlist, the snapshot, the rate limit, and the limits. |
| `dashboard.md` | The dashboard page at `GET /`, the static routes and their headers, the data request, the ranges, the sections and their formulas, the states, the fonts, and the design tokens. |
| `private-reports.md` | The private page under `/private/`, the admin token, the login, the cookie, the report, session, providers, and configuration routes, and their headers. |
| `github.md` | The GitHub worker and the GitHub token labels. |
| `configuration.md` | The configuration loader, the variables, the provider credentials and their detection, and `loupe config show`. |
| `operations.md` | The subcommands, logging, storage, health, the provider ping, shutdown, the container, and the pipeline. |
| `release.md` | The version rules, the bump rules, the tag and image tag forms, the release flow with release-please, the release assets and their checksum files, the third-party notices, the commit rule for a change request, the release token, the repository settings and their script, and the release check. |
