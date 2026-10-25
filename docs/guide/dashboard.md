<!-- @agents-index User guide: The public dashboard, its live section, and the private page on localhost. -->

# Dashboard and private page

## Dashboard

The dashboard is a stateless single-page app. The `loupe` binary serves its files at `/`, built
into the binary, so one artifact ships the full product. The page gets all its data from the
public API. It has no login, no cookies, and no state on the server, because all the data that
it shows is public. The two roles serve the same page: `loupe` on port 8787 and `loupe-public`
on port 8788, which reads the published days and the live counters from the public mirror.

| Section | Data |
|---|---|
| Header | One row: the loupe mark, the title from `LOUPE_SITE_TITLE` (default `Agentloupe`), and the range selector (1M, 3M, 6M, 1Y). Below it, token velocity as the headline |
| Live | Below the headline, the card "Today": a dot that pulses while the WebSocket route `/live` sends frames, then in flight, calls, tokens, tokens per minute, and API-equivalent cost of the current day, and a sparkline of tokens per 10 seconds over the last 10 minutes |
| Pull requests | PRs opened and PRs merged for the range, bar chart per week |
| Lines changed | The pull request card ends with one line that shows lines added and lines removed for the selected range, with no net value |
| Tokens per day | Input and output, two line charts, each on its own scale |
| Cost (API-equivalent) | A card in the totals row: the total for the range, and a daily line chart in the style of the token charts. The label says that the figure is a list price, not money paid. Next to it, the subscription cost from `LOUPE_SUBSCRIPTION_<NAME>`, spread over the days of the range. |
| Token usage, last 12 months | Heatmap per day, five levels from quartiles |
| Daily log | One line per day, with the PR count and the token count |
| Footer | The sources that the page covers, today Claude Code and Pi |

The headline is the token velocity: the input and output tokens of the range divided by the
days of the range, in tokens per day. The ranges are 1M (the last 30 days), 3M (the last 91 days), 6M (the last 182
days), and 1Y (the last 365 days), each ending on the newest day of the API. 1M is the default. The URL fragment
keeps the range, for example `#range=6M`. The live section updates while calls occur. Calls that `loupe parse --once` or `loupe backfill` store from a second process appear at the next page load, not as live events. When the
socket closes it shows "Paused, reconnecting" and connects again; `LOUPE_LIVE_ENABLED=false`
removes the route, and the page then hides the section. The subscription cost card shows the subscription
cost of the range next to the API-equivalent cost. The page shows only sums for the range. It
shows no streaks and no record days.

The design uses the Geist and Geist Mono fonts. The values to copy (colors, sizes, spacing)
are in the inline styles of `Main.dc.html` in the mockup archive. An implementation must
replicate these values in its own components. It must not copy the mockup runtime
(`support.js`, `vendor/`).

## Private page

A private page at `/private/` shows usage, cost, and sessions per proxy token and per tag. It
needs `LOUPE_ADMIN_TOKEN`, sent as a header or through a login form. Its data never appears in
the public API.

Two more routes need the same authorization:

- `GET /private/api/providers` returns the provider status of each provider: `ok`,
  `unauthorized`, `unreachable`, or `not_configured`, with `checked_at`. The status comes from a
  cached ping of the models list. It never changes `GET /healthz`.
- `GET /private/api/config` returns the same entries as `loupe config show`. Each secret is `set`
  or `unset`.
