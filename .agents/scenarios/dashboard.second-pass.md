<!-- @agents-index User scenario: the lines are a footer line of the wide pull request card, and the price note is an asterisk sentence in the footer. -->
# Scenario: dashboard second pass

Source: the approved change request for the second pass of the dashboard.

## Goal

A reader sees the pull requests and the lines in one wide card, and finds the source of the cost prices in the footer.

## Steps

1. Run `.agents/scripts/days.seed.sh` with 400 days on a scratch data directory, as `.agents/scripts/web.render.sh` does.
2. Start `loupe serve` on that data directory.
3. Open the page at 1440 px and select the range 6M.
4. Read the labels of the cards. Look for a card for the lines.
5. Read the pull request card: the width, the legend row, the chart width, and the last line.
6. Read the label of the cost card, the label of the live cost figure, and look for a footnote in the cost card.
7. Read the footer and measure the gap above it.
8. Repeat steps 4 to 7 at 360 px. Check for no horizontal scroll.
9. Stop the server and the browser.

## Success condition

No card for the lines exists. The pull request card has the width of the tokens-per-day card and holds one horizontal legend row with opened and merged and their sums, a chart that fills the card width, and a last line "<n> lines added <n> lines removed" with thin-space groups. The cost label is "Cost (API-equivalent)*", the live label is "API-equivalent cost*", and the cost card has no footnote. The footer shows "Covers Claude Code and Pi. claude.ai chats are not covered." and "* LiteLLM list prices at <short commit>, not money paid.", where LiteLLM links to `https://github.com/BerriAI/litellm` with `rel="noopener noreferrer"`. The gap above the footer is 24 px. The page has no horizontal scroll at 360 px.

## Runs

| Date | Width | Result | Observation |
|---|---|---|---|
| 2026-10-21 | 1440 px | passed | no lines card; pull request card 572 px, tokens card 572 px; legend horizontal, opened 133, merged 90; chart 530 px of 530 px content; last line `41 085 lines added 14 265 lines removed`; labels `Cost (API-equivalent)*` and `API-equivalent cost*`; no cost footnote; footer "* LiteLLM list prices at cfee0ff, not money paid." with the link and `rel="noopener noreferrer"`; footer gap 24 px |
| 2026-10-21 | 360 px | passed | the same values; cards 328 px; chart 286 px of 286 px content; no horizontal scroll |
| 2026-10-21, verification run 1 | 1440 px | passed | 6M; labels have no lines card; pull request card has the class `wide`, 572 px, on the row of the tokens-per-day card, 572 px; legend one row, `opened 270`, `merged 180`, each with a color square; chart 530 px of 530 px content; footer line `85 270 lines added 28 780 lines removed`; `Cost (API-equivalent)*`, `API-equivalent cost*`; no cost footnote; footer has the coverage sentence and `* LiteLLM list prices at cfee0ff, not money paid.`, link `https://github.com/BerriAI/litellm`, `rel="noopener noreferrer"` |
| 2026-10-21, verification run 1 | 360 px | passed | the same values; cards 328 px; chart 286 px of 286 px content; no horizontal scroll |
| 2026-10-21, verification run 2 | 1440 px and 360 px | passed | the values of run 1, the same JSON output byte for byte; screenshots taken in this run |
| 2026-10-21, GitHub null | 1440 px and 360 px | passed | seed `--github none`; the pull request card shows only `GitHub not connected`, with no legend, no chart, and no footer line |

Outcome: the change is verified, 2 of 2 verification runs at each width passed, after 1 of 1 at implementation. The driver was a headless Chrome DevTools protocol probe built on `.agents/scripts/web.render.cdp.mjs`, because the render script measures timing only. The verification runs used a scratch probe on the same protocol that seeds with `.agents/scripts/days.seed.sh`, also with `--github none` for the GitHub-null state, which `web/test/section.prs.test.js` also asserts.

Screenshots: `dashboard.second-pass.1440.png`, `dashboard.second-pass.360.png`.
