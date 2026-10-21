<!-- @agents-index Specification of the public dashboard page: static routes, headers, page rules, data request, sections, states, fonts, and design tokens. -->
# Dashboard

The dashboard is one page that the `loupe` binary serves at `GET /`. The page reads the public API `GET /v1/days` and shows the day records. It shows no private data.

## Routes

| Route | Response |
|---|---|
| `GET /` | `200` with the page `web/index.html`. |
| `GET /assets/<path>` | `200` with the file `web/assets/<path>`. |
| `GET /favicon.ico` | `200` with the favicon file. |

The content type comes from the file extension:

| Extension | Content type |
|---|---|
| `.html` | `text/html; charset=utf-8` |
| `.js` | `text/javascript; charset=utf-8` |
| `.css` | `text/css; charset=utf-8` |
| `.woff2` | `font/woff2` |
| `.svg` | `image/svg+xml` |
| `.ico` | `image/x-icon` |

`HEAD` on each route returns the same status and headers as `GET`, with no body.

## Files in the binary

The binary contains every file of `web/` at compile time. The build makes the list of the files from the `web/` directory, and a change to a file of `web/` changes the binary after `cargo build`. At run time, the static handler does not read the file system. The site title is the only run-time change to a built-in file.

## Path rules

1. A path that does not match a built-in file returns `404` with no body.
2. A path that holds `..`, a backslash, or a percent-encoded slash returns `404`.
3. A path `/<token>/` or `/<token>/assets/...` does not serve the page. The static files need no proxy token.

## Headers

Every static response has these headers:

1. `etag`, made from a SHA-256 hash of the file content. For `index.html`, the hash is of the content after the site title is put in.
2. `cache-control: no-cache`.

A request with an `if-none-match` value that matches the `etag` returns `304` with no body.

The `GET /` response also has these headers:

1. `content-security-policy: default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`
2. `x-content-type-options: nosniff`
3. `referrer-policy: no-referrer`

The static routes send no CORS header.

## Method rule

A method other than `GET` and `HEAD` on `/`, on `/assets/...`, or on `/favicon.ico` returns `405` with `allow: GET, HEAD`.

## No credentials

The static routes do not read a cookie or an `authorization` header. They do not set a cookie. They do not read a table of the database.

## Reserved names

The router keeps a list of reserved first path segments. The proxy does not get a request whose first segment is on the list. The list is `healthz`, `v1`, `assets`, `favicon.ico`, and `private`. The list holds the names that the static handler and the public API serve, and one file in the router holds it. `proxy.md` gives the proxy routes.

## Site title

The variable `LOUPE_SITE_TITLE` sets the site title. The default is `AI agent activity`. The default page does not show the product name.

At start, the binary puts the title into `index.html` one time. It escapes the characters `&`, `<`, `>`, `"`, and `'` as HTML entities. The title shows in the `<title>` element and in the heading of the header. The binary computes the `etag` of `index.html` from the result. `configuration.md` gives the validation of the value.

## Page technology

1. The page uses plain HTML, CSS, and JavaScript ES modules. It uses no framework, no bundler, no transpiler, and no package manager.
2. The JavaScript is in small single-purpose modules with hierarchical names under `web/assets/js/`. Each file has one `@agents-index` line in its top comment.
3. The calculation modules (`view.*`, `stats.*`, `format.*`, `range.*`) do not touch the DOM. Node runs their unit tests with no browser.
4. The page loads no file from a host other than its own origin.
5. The page has no inline script, no inline style that the policy blocks, and no `eval`.
6. The page does not use a cookie, `localStorage`, `sessionStorage`, or IndexedDB.
7. Each chart is inline SVG that the page draws. The page uses no chart library.
8. The page gets data only from `GET /v1/days` on the same origin. It calls no other route of the binary, except the static files.

## Data request

1. On load, the page requests `GET /v1/days?from=<F>`. `F` is the browser date minus 399 days. The page does not send `to`, so the server default (today in `LOUPE_TZ`) sets the end.
2. If the server refuses the range, the page retries one time with `F` one day later. A browser date is never more than one day from the date in `LOUPE_TZ`, so the server limit of 400 days stays met.
3. The records of this response feed every section.
4. The subscription cost of a range comes from the envelope field `subscription_cost_usd`. On the first selection of a range, the page requests `GET /v1/days?from=<first day of the range>` with no `to`. It keeps the `subscription_cost_usd` of that response in memory. A second selection of the same range makes no new request.
5. The page does not compute the subscription cost itself.

`api.md` gives the envelope and the record fields. `days.md` gives the day records.

## Ranges

| Range | Days | Fragment |
|---|---|---|
| `3M` | the last 91 days | `#range=3M` |
| `6M` | the last 182 days | `#range=6M` |
| `1Y` | the last 365 days | `#range=1Y` |

Each range ends on the `to` value of the response. Both ends are in the range. The URL fragment keeps the selected range, so a reload or a shared link keeps it. A missing or unknown fragment selects `3M`.

## Missing days and fields

A day in the range with no record counts as zero for every sum, and draws as zero in a chart. A day has a record only when it has agent calls or GitHub activity.

The page shows only fields of the allowlist of the public API. It keeps the record fields that it does not show out of the DOM.

## Sections

The sections show in this order. In each formula, the sum is over the days of the selected range, and a day with no record adds zero.

### Headline: token velocity

The headline is directly below the header, before every other section.

1. The figure is the sum of `input_total + output`, divided by the number of days of the range, rounded to an integer.
2. The figure uses the short form (for example `1.3M`), with the label "tokens per day".
3. The figure is the largest text on the page. Its font size is at least two times the font size of a figure of the totals row.
4. Below the figure, one line gives the range in words, for example "average over the last 3 months".
5. A sparkline with no axes shows the daily `input_total + output` of the range.
6. The headline is visible with no scroll at a width of 360 px and of 1440 px.

### Header

1. The site title.
2. The time of the last update from `updated_at`, as a local date and time with the time zone name. When `updated_at` is `null`, the header shows "No update yet".
3. The range selector with the buttons `3M`, `6M`, and `1Y`. The selected button has `aria-pressed="true"`. The keyboard can reach and use each button.

### Totals row

The totals row shows only sums for the range. The page shows no streak, no count of active days in a row, and no record day.

### Pull requests

1. The sum of `prs_opened` and the sum of `prs_merged`.
2. A bar chart with one group per ISO week (Monday to Sunday) and two bars per group: opened and merged.

### Lines changed in the range

1. The sum of `lines_added`, the sum of `lines_removed`, and the net value (added minus removed).
2. A change of range changes these figures.

### Tokens per day

1. One line chart for `input_total` and one line chart for `output`. Each chart has its own vertical scale.
2. The sum of `input_total` and the sum of `output`.

### Cost (API-equivalent)

1. A card in the totals row with the label "Cost (API-equivalent)".
2. The sum of `cost_usd` in US dollars with two decimals.
3. The text "List price of the tokens, not money paid."
4. When the sum of `unpriced_calls` is more than zero, the text "<n> calls not priced", where `<n>` is that sum.
5. A daily line chart of `cost_usd`, in the style of the token charts.

### Subscription cost

1. A card in the same row as the API-equivalent cost, with the label "Subscription cost".
2. The `subscription_cost_usd` of the range in US dollars with two decimals.
3. The text "Money paid for subscriptions, pro-rated per day."
4. When the value is `0`, the card shows "No subscription configured" in place of the figure.
5. The card shows no subscription name.

### Token usage, last 12 months

1. A heatmap of the last 365 days, one cell per day, in columns of weeks from Monday to Sunday.
2. The value of a cell is `input_total + output`.
3. Each cell has one of five levels. Level 0 is a value of zero. Levels 1 to 4 come from the quartiles of the values above zero in the 365 days.
4. Each cell has a `title` with the date and the token count.
5. The range selector does not change the heatmap.

### Daily log

1. One row per record in the range, newest first.
2. Each row has the date, the `summary`, the PR count (`prs_opened`), and the token count (`input_total + output`).
3. A record with `summary` `null` shows "No summary".
4. The page puts the summary in as text, never as HTML.

### Footer

The footer names the covered sources "Claude Code and Pi". It states that claude.ai chats are not covered.

## States

### Empty state

When the response holds no record, the page shows the header, the headline with the figure `0`, the footer, and one empty state. The empty state has the text "No days published yet" and the hint "Days appear after the nightly publish, or after `loupe aggregate --date today`." The page draws no empty chart.

### GitHub not connected

1. When `prs_opened`, `prs_merged`, `lines_added`, and `lines_removed` are `null` in every record of the response, the pull request section and the lines section show "GitHub not connected" in place of the figures and the charts.
2. When some records have `null` in these fields, a `null` counts as zero, and each of the two sections shows "Partial GitHub data".

### Error

When the request fails or returns a status other than `200`, the page shows "Data not available" with the HTTP status or the error name. A button "Try again" sends the request again. For a `400` with a body, the page also shows `error.message`.

## Number formats

1. In the totals, an integer has a thin space (U+2009) as the group mark, for example `12 345`.
2. In chart labels and in the headline, a number uses the short form with one decimal: `k` for thousands and `M` for millions, for example `1.3M`.
3. A US dollar amount has two decimals.
4. The formats do not depend on the locale of the browser.

## Fonts

1. The repository vendors the Geist and Geist Mono font files as `.woff2` under `web/assets/fonts/`.
2. `web/assets/fonts/OFL.txt` holds the SIL Open Font License of the fonts. The license lets the repository and the binary hold a copy.
3. `web/assets/fonts/SOURCE.md` names the upstream repository, the release tag, the date of the fetch, and the SHA-256 hash of each file.
4. `.agents/scripts/fonts.vendor.sh` fetches the files of one stated release tag and writes their hashes to `SOURCE.md`. `--dry-run` shows the work and changes no file. `--check` compares the hashes of the files on disk with `SOURCE.md`.
5. The verdict does not call the fetch. The fetched files are committed.
6. The CSS declares each font with `@font-face` and `font-display: swap`.
7. The fallback for Geist is `system-ui, sans-serif`. The fallback for Geist Mono is `ui-monospace, monospace`.

## Design tokens

`web/assets/css/tokens.css` holds the design values as CSS custom properties. The page has one color scheme, dark. It has no light theme and no theme switch. Text has a contrast of at least 4.5 to 1 on its background. The page works at a width of 360 px with no horizontal scroll, and at 1440 px.

| Token | Value | Use |
|---|---|---|
| `--color-bg` | `#0F1012` | page background |
| `--color-ink-inverse` | `#111214` | text on a light fill, for example a selected button |
| `--color-surface` | `#15161A` | card background |
| `--color-border` | `#24262B` | card border |
| `--color-rule` | `#2A2C31` | chart axis and divider lines |
| `--color-text` | `#ECEAE4` | primary text, figures, selected button fill |
| `--color-text-body` | `#B5B4AE` | body text, unselected button text |
| `--color-text-muted` | `#9A9AA0` | secondary text, update time |
| `--color-text-faint` | `#85868C` | chart labels, legends |
| `--color-input` | `#5E86E0` | input tokens series, opened pull requests |
| `--color-output` | `#BB7F35` | output tokens series, lines removed |
| `--color-positive` | `#3FA864` | merged pull requests, lines added |
| `--color-highlight` | `#D9C27A` | tag background |
| `--color-link` | `#9DB6EE` | link |
| `--color-link-hover` | `#C9D7F6` | link on hover |
| `--heat-0` | `#1D1F23` | heatmap level 0 |
| `--heat-1` | `#1B4A2F` | heatmap level 1 |
| `--heat-2` | `#21703E` | heatmap level 2 |
| `--heat-3` | `#2E9A55` | heatmap level 3 |
| `--heat-4` | `#4FCB74` | heatmap level 4 |
| `--font-sans` | `'Geist', system-ui, sans-serif` | text |
| `--font-mono` | `'Geist Mono', ui-monospace, monospace` | figures, labels |
| `--size-headline` | `72px` | headline figure, at least two times `--size-figure` |
| `--size-figure` | `34px` | totals figures (the largest figure size of the mockup) |
| `--size-figure-small` | `30px` | figures of a card pair |
| `--size-title` | `40px` | site title |
| `--size-body` | `15px` | body text |
| `--size-text` | `13px` | card text |
| `--size-label` | `12px` | labels |
| `--size-axis` | `11px` | chart axis labels |
| `--radius-card` | `12px` | card corners |
| `--radius-bar` | `3px` | top corners of a bar |
| `--max-width` | `1200px` | width of the content column |
| `--radius-cell` | `2px` | heatmap cell and legend swatch |
| `--radius-pill` | `999px` | range buttons |

The page contains no file of the mockup support script and no file of the mockup vendor directory.

## Toolchain and pipeline

`.tool-versions` pins Node with the line `nodejs 22.20.0`. The `web` stage of the pipeline runs `node --test` on the calculation modules and checks the size of `web/`. `operations.md` gives the stage and its targets.
