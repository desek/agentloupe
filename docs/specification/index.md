<!-- @agents-index Index of the system specification: one row per product area file and what it covers. -->
# System specification

This directory describes how the `loupe` binary behaves. Each file covers one product area.

| File | Covers |
|---|---|
| `proxy.md` | The proxy routes, the forward to the upstream, the capture of each exchange, and the redaction of credentials. |
| `proxy.tokens.md` | The proxy tokens, the `loupe token` subcommands, the bootstrap token, and the token gate. |
| `accounts.md` | The hash of the provider account, the order of its sources, and the accounts table. |
| `parser.md` | The parser that finds calls in exchanges, the token columns, and the sessions and segments that group the calls. |
| `cost.md` | The API-equivalent cost of each call from the vendored price file. |
| `summaries.md` | The one-line session summaries, the day paragraph, and the retention of raw bodies. |
| `days-public-api.md` | The day records, the nightly publish, and the routes under `/v1/days`. |
| `dashboard.md` | The dashboard page at `GET /`. |
| `github.md` | The GitHub worker and the GitHub token labels. |
| `configuration.md` | The configuration loader, the variables, and `loupe config show`. |
| `operations.md` | The subcommands, logging, storage, health, shutdown, the container, and the pipeline. |
