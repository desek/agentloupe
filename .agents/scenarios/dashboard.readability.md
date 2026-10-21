<!-- @agents-index User scenario: the dashboard cards have a gap, the cost has a footnote, the token shares and the lines figures are labelled. -->
# Scenario: dashboard readability

Source: the approved change request for dashboard readability.

## Goal

A reader of the dashboard sees separate cards, the source of the cost prices, the share of each token kind, and a label on each lines figure.

## Steps

1. Run `.agents/scripts/days.seed.sh --data-dir <dir> --days 400` with a key from `loupe gen-key`. The seed has all four token kinds and lines added and removed.
2. Start `loupe serve` on that data directory.
3. Open the page at 1440 px and select the range 6M.
4. Measure the gap between each pair of cards and each pair of rows.
5. Read the cost card label and the footnote line.
6. Read the tokens card: the total, the bar, and the four kinds.
7. Read the lines card.
8. Repeat steps 4 to 7 at 360 px. Check for no horizontal scroll.
9. Stop the server and the browser.

## Success condition

Each gap is at least 24 px. The cost label is "Cost (API-equivalent)*". The footnote names LiteLLM, `model_prices_and_context_window.json`, and the short price commit, and links to `https://github.com/BerriAI/litellm` with `rel="noopener noreferrer"`. The tokens card shows the total in the totals format, a bar with one segment for each nonzero kind, and four labels with a percentage with one decimal. Each lines figure has a label. The page has no horizontal scroll at 360 px.

## Runs

| Date | Width | Result | Observation |
|---|---|---|---|
| 2026-10-21 | 1440 px | passed | min gap 24 px; label `Cost (API-equivalent)*`; footnote "* Prices from LiteLLM, price file model_prices_and_context_window.json at commit cfee0ff. Source: github.com/BerriAI/litellm."; total `170 930 280`; 4 segments; Input 4.8%, Cache read 91.7%, Cache write 0.1%, Output 3.4%; lines `85 270` lines added, `28 780` lines removed |
| 2026-10-21 | 360 px | passed | the same values; no horizontal scroll |
| 2026-10-21 | 1440 px | passed | run 2 with agent-browser 0.27.1 and the release binary: the same values; min gap 24 px; link `https://github.com/BerriAI/litellm` with `rel="noopener noreferrer"` |
| 2026-10-21 | 360 px | passed | run 2: the same values; min gap 24 px; no horizontal scroll |

Outcome: the change is verified, 2 of 2 runs at each width. The empty state of the tokens card (a range with zero tokens) has no run: the seed script cannot make days with zero tokens. The code path shows the empty state when the share module returns null, and `web/test/stats.shares.test.js` asserts that null.

Screenshots: `dashboard.readability.1440.png`, `dashboard.readability.360.png`.
