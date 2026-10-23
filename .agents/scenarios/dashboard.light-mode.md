<!-- @agents-index User scenario: the dashboard follows the system color scheme, the header control overrides it, and every section and state stays readable. -->
# Scenario: dashboard light mode

Source: the approved change request for the light color scheme of the dashboard.

## Goal

A reader with a light system appearance gets a light page with no action, can select Light, Dark, or System in the header, keeps that choice after a reload, and reads each section and each state in both schemes. The private page stays dark.

## Driver

- `.agents/scripts/web.scheme.scenario.sh` runs steps 1 to 13, 15, and 16. It builds the release binary, seeds 400 days with `.agents/scripts/days.seed.sh` on a scratch data directory, starts `loupe serve` with a random admin token, drives headless Chrome over the DevTools protocol (`web.scheme.scenario.mjs`), writes the four screenshots, and stops the server and Chrome. It prints `checks`, `failed`, and `status`; `.agents/logs/web.scheme.scenario.log` has one line per check.
- `.agents/scripts/web.render.sh --days 400 --scheme light` and `--scheme dark` run step 14 and the first-frame check of steps 4 and 9 (`first_frame_scheme_ok`, with `--stored none|light|dark|invalid`). The states come from the same seeded server: the probe answers `GET /v1/days` with no days (empty state), with status 500 (error state), with all GitHub fields null (GitHub not connected), and with them null on odd days (partial GitHub data), and closes the live socket after a snapshot (paused state).

## Steps

1. Seed 400 days on a scratch data directory.
2. Start `loupe serve` on that data directory.
3. Set the emulated system preference to light. Clear the scheme key `dashboard.scheme`.
4. Open the page at 1440 px on the range 6M. Check that the first frame and the loaded page are light, and that the button "System" is pressed.
5. Read the lowest text contrast and the lowest graphic contrast of the sections.
6. Take the screenshot `dashboard.light-mode.light.1440.png`.
7. Select "Dark" with the mouse. Check that the page is dark at once, that no request to `/v1/days` goes out, that the live socket stays open, and that the fragment stays `#range=6M`.
8. Take the screenshot `dashboard.light-mode.dark.1440.png`.
9. Reload. Check that the page is dark and that "Dark" is pressed.
10. Select "System". Check that the page is light and that the key is absent.
11. Store the invalid value `sepia` and reload. Check that the page follows the preference and that "System" is pressed.
12. With no key and the preference dark, change the preference to light: the page turns light with no reload. Select "Dark" and change the preference again: the page stays dark. With the preference dark, select "Light" and reload: the page is light and "Light" is pressed.
13. Check the place of the control: at 1440 px the brand is at the left, and the range group and then the group "Color scheme" are at the right in one row; at 360 px the group is visible below the range group with no horizontal scroll. Each button is at least 44 px high. From the last range button, Tab reaches System, Light, and Dark; Enter selects System and Space selects Dark. Repeat steps 3 to 13 at 360 px, with the screenshots `dashboard.light-mode.light.360.png` and `dashboard.light-mode.dark.360.png`.
14. In each scheme, show the empty state, the error state with "Try again", "GitHub not connected", "Partial GitHub data", and the paused live section; read the lowest text contrast, the contrast of the paused figures, and the contrast of the focus outline of each button.
15. With the preference light and the key `light`, open `/private/` with the admin token. Check that the page background is the dark `--color-bg`, that the root has no scheme, that `color-scheme` is dark, and that the page has no control.
16. Stop the server and the browser.

## Success condition

Each check passes at both widths and in both schemes, in at least 2 of 2 runs, and the four screenshots are in `.agents/scenarios/`.

## Run log

| Date | Run | Result | Observed |
|---|---|---|---|
| 2026-10-23 | `web.scheme.scenario.sh`, run 1 | passed | `checks: 31 failed: 0 status: passed`; light at 1440 px: `rgb(243, 242, 238)`, System pressed; Dark: `rgb(15, 16, 18)`, key `dark`, 0 new `/v1/days` requests, 1 socket open, `#range=6M`, no reload; text contrast 5.38 light and 4.98 dark, graphic contrast 3.25 light; buttons 44 px; private page `rgb(15, 16, 18)`, no scheme, no control |
| 2026-10-23 | `web.scheme.scenario.sh`, run 2 | passed | `checks: 31 failed: 0 status: passed`, the same values; screenshots from this run |
| 2026-10-23 | `web.render.sh --days 400 --scheme light` | passed | `first_frame_scheme_ok: true min_text_contrast: 5.38 min_graphic_contrast: 3.25 min_paused_figure_contrast: 5.72 min_focus_contrast: 6.11`; the states: empty 6.11, error 6.11, not connected 5.72, partial 5.38, paused message 7.2 |
| 2026-10-23 | `web.render.sh --days 400 --scheme dark` | passed | `first_frame_scheme_ok: true min_text_contrast: 4.98 min_paused_figure_contrast: 4.98 min_focus_contrast: 8.92`; the states: empty 6.46, error 6.8, not connected 4.98, partial 4.98, paused message 6.46 |
| 2026-10-23 | `web.render.sh` with `--scheme light` and `dark`, each with `--stored none`, `light`, `dark`, `invalid` | passed | `first_frame_scheme_ok: true` in 8 of 8 runs |
| 2026-10-23 | negative control, scheme script removed from `index.html`, `--scheme light --stored none` | failed as expected | `first_frame_scheme_ok: false`: the first frame showed the dark background before the module set light |

Outcome: the change is verified, 2 of 2 runs passed at each width and in both schemes.

Screenshots: `dashboard.light-mode.light.1440.png`, `dashboard.light-mode.dark.1440.png`, `dashboard.light-mode.light.360.png`, `dashboard.light-mode.dark.360.png`.
