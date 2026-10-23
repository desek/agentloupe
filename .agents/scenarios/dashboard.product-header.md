<!-- @agents-index User scenario: the product header, the headline eyebrow and sparkline, the Today live card, and the default range 1M of CR-0021. -->
# Scenario: dashboard product header

Source: the change request CR-0021 "Dashboard product header", status implemented, verified after the fact.

## Goal

A reader opens the dashboard with no fragment and sees a product header (the mark and the wordmark "Agentloupe" at the left, the range buttons at the right, no update line). Under it, the headline states the unit and the range in one eyebrow, with a sparkline as tall as the digits. The live card "Today" shows the connection and five figures. The page selects the last 30 days. This holds at 1440 px and at 360 px, with no horizontal scroll.

## Driver

- `.agents/scripts/web.header.scenario.sh` runs every step. It builds the release binary, seeds 400 days with `.agents/scripts/days.seed.sh` in a scratch data directory, and starts `loupe serve` with `LOUPE_TZ=UTC` on a free port. It drives headless Chrome over the DevTools protocol (`web.header.scenario.mjs`), writes the two screenshots, and stops the server and Chrome. It prints `checks`, `failed`, and `status`. `.agents/logs/web.header.scenario.log` has one line per check with the observed values.
- `--dry-run` prints the plan and changes nothing.

## Preconditions

- Google Chrome at the default macOS path, or `CHROME` set. Node from `mise`. `cargo`, `curl`, `perl`, `sqlite3`, `openssl`.
- `LOUPE_SITE_TITLE` is not set (the script starts the server with `env -i`).
- 400 seeded days that end yesterday in UTC, and no live traffic. The live socket sends a snapshot of zeros.
- The emulated system preference is dark, with no stored scheme key. The scheme switch of CR-0020 is in the header after the range group.
- The scenario uses a scratch server. It does not use the container on port 8787.

## Steps

Do steps 1 to 7 at 1440 by 900 px, then again at 360 by 740 px.

1. Open the page with no fragment. Wait for the first draw and for the first live frame, which empties the status note.
2. Take the screenshot `dashboard.product-header.<width>.png`.
3. Header: read the `<title>` and the `h1`; the brand, the mark, the wordmark, and the range group; the text of the header.
4. Headline: read the eyebrow text, the shown (uppercase) text, and its computed style. Read the box of the figure and of the sparkline, and the baseline of the digits (the bottom of a zero-height inline block in the figure).
5. Live card: read the accessible name, the heading, the dot, the status note, the five figure labels and their top edges, and the size of the sparkline.
6. Ranges: read `aria-pressed` of each button and the point count of the headline sparkline. Check the horizontal scroll. Click `3M` with the mouse, then read the fragment, the pressed button, the eyebrow, and the point count.
7. Open the page with the unknown fragments `#range=9Z` and `#x`, and read the pressed button, the eyebrow, and the point count.

## Success condition

Each check passes in 2 of 2 runs. The checks, at both widths unless stated:

| Criterion | Check |
|---|---|
| AC-1 | `<title>` and the one `h1` are "Agentloupe"; the `h1` is in the brand |
| AC-3 | 1440 px: brand at the left edge, range group at the right, then only the scheme switch, which ends at the right edge; one row, centered, header height 48 px. 360 px: brand at the left, range group on a second line |
| AC-3 | the mark is the first child of the brand, inline `svg`, 44 by 44 px, `aria-hidden="true"`, `focusable="false"`; the header loads no image file |
| AC-3 | ring and handle in `currentColor`; three bars in `var(--color-input)` with three different heights |
| AC-3 | wordmark 20 px, weight 600, 10 px after the mark |
| AC-3 | range buttons `1M 3M 6M 1Y` in this order |
| AC-3 | no element `#updated` or `.updated`, no word "Updated" in the header, no text "No update yet" on the page |
| AC-4 | eyebrow text "Tokens per day · 1M avg", shown as "TOKENS PER DAY · 1M AVG", first in the headline, above the figure |
| AC-4 | eyebrow style equals `--font-mono`, `--size-label`, `--color-text-muted`, uppercase, letter spacing 0.04 em |
| AC-4 | headline sparkline height 0.72 times `--size-headline`, within 0.5 px |
| AC-4 | 1440 px: sparkline right of the figure, bottom edge on the baseline within 1 px |
| AC-5 | 360 px: the top of the sparkline is under the bottom of the figure |
| AC-6 | name "Today, live", heading "Today", an 8 by 8 px dot before it in `--color-positive` with the animation `live-pulse` |
| AC-6 | status note with the role `status`, empty, at the right end of the row |
| AC-6 | figure labels In flight, Calls, Tokens, Tokens per minute, API-equivalent cost*; at 1440 px in one row |
| AC-6 | no "Open segments" and no "Live, today" on the page |
| AC-6 | live sparkline height 0.72 times `--size-figure-small`, within 0.5 px; width equal to the content width of the card |
| AC-9 | no fragment: `1M` has `aria-pressed="true"`, the others `false`; the sparkline has 30 points |
| AC-5 | `scrollWidth` is not more than the width of the viewport |
| AC-9 | click on `3M`: fragment `#range=3M`, `3M` pressed, eyebrow "Tokens per day · 3M avg", 91 points |
| AC-9 | `#range=9Z` and `#x`: `1M` pressed, eyebrow "Tokens per day · 1M avg", 30 points |

The screenshots `dashboard.product-header.1440.png` and `dashboard.product-header.360.png` are in `.agents/scenarios/`.

Not in this scenario: AC-2 (an override of the title needs a second server, and `tests/web.serve.rs` covers the title in the served page), AC-7 (the connection states), and AC-8 (the sum of the last six buckets, which `live.section.js:perMinute` computes).

## Run log

| Date | Run | Result | Observed |
|---|---|---|---|
| 2026-10-23 13:41 CEST | driver draft 1 | failed, driver defect | `checks: 43 failed: 1`. The one-row check wanted the range group within 120 px of the right edge of the header. The range group ends at 1232.5 px and the header at 1296.5 px, because the scheme switch of CR-0020 comes after it, per `dashboard.md` "Header" item 1. The check now wants the switch as the last element at the right edge. |
| 2026-10-23 13:41 CEST | driver draft 2, two runs | failed, driver defect | `checks: 43 failed: 1` in each run. The one-row check wanted the header no taller than the brand (44 px) plus 1 px, but the header has `min-height: 48px`. The check now accepts 48 px. No page code changed. |
| 2026-10-23 13:41 CEST | `web.header.scenario.sh`, run 1 | passed | `checks: 43 failed: 0 status: passed`. 1440 px: brand 128.5 to 289.5 px, range group 1016.1 to 1232.5 px, switch last, at 1296.5 px; header 48 px. Mark 44 by 44 px, 10 px to the wordmark of 20 px, weight 600. Eyebrow "TOKENS PER DAY · 1M AVG", 12 px, `rgb(154, 154, 160)`, spacing 0.48 px. Sparkline 51.83 px (want 51.84), bottom 189 px, baseline 189 px. Dot 8 by 8 px, `rgb(63, 168, 100)`, `live-pulse`. Five figures at top 278 px. Live sparkline 21.59 px (want 21.6), 1126 px wide, card content 1126 px. 30 points at 1M, 91 at 3M; `#range=9Z` and `#x` give 1M. `scrollWidth` 1425 at 1440. 360 px: range group on a second line (top 76 px, brand bottom 64 px); sparkline top 248 px under the figure bottom 240 px; `scrollWidth` 360. |
| 2026-10-23 13:41 CEST | `web.header.scenario.sh`, run 2 | passed | `checks: 43 failed: 0 status: passed`, the same values; screenshots from this run |

Outcome: the change is verified against the listed acceptance criteria, 2 of 2 runs passed at both widths. No finding against the page.

Screenshots: `dashboard.product-header.1440.png`, `dashboard.product-header.360.png`.
