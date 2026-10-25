---
date: 2026-10-25
source: cr (CR-0029, CR-0030)
surface: git clone of the public repository, and the README as the GitHub renderer shows it (browser, chrome-devtools-mcp)
outcome: reproduced
runs: "3 of 3 passed for the history checks (two fresh clones from GitHub and the rewrite clone), 1 of 1 for the README render, last run 2026-10-25"
---

## Scenario: a reader clones the new repository and opens the README

### Goal

A reader of the new repository `desek/agentloupe` sees the dashboard and the purpose at the top of the README, and finds no removed content in any commit.

### Preconditions

- `gh` logged in with access to `desek/agentloupe`.
- Google Chrome for the browser driver.

### Steps at the user surface

1. Run `git clone https://github.com/desek/agentloupe.git <dir>` and `cd <dir>`.
2. Run `git log --all --oneline -- brainstorm` and `git log --all --oneline -- prototype`.
3. Run `git grep -li 'nic[o]sia' $(git rev-list --all)` and `git log --all -i --grep 'nic[o]sia' --oneline`.
4. Run `git rev-list --all --objects | awk '{print $1}' | git cat-file --batch-check='%(objecttype) %(objectname)' | awk '$1=="blob"{print $2}' | git cat-file --batch | grep -aci 'nic[o]sia'`.
5. Run `git ls-remote origin`.
6. Render the README with `gh api -X POST /markdown -f text="$(cat README.md)" -f mode=gfm -f context=desek/agentloupe` into an HTML file with a `<base>` of the clone, open it in the browser driver, and read the first image.

### Success condition

- Steps 2, 3, and 4 print nothing, or 0.
- Step 5 lists only `HEAD` and `refs/heads/main` (before Dependabot or release-please make a branch).
- In step 6 the first image is `docs/images/dashboard.png`, loaded, under the title and the tagline.

### Run log

- Run 1, 2026-10-25 21:04 CET, on the rewrite clone before the push: 0, 0, 0, 0; one ref `refs/heads/main`.
- Run 2, 2026-10-25 21:16 CET, on a fresh clone from GitHub: 357 commits; 0, 0, 0, 0; remote refs `HEAD refs/heads/main`. README render: the image loaded with a natural width of 2560 pixels, 116 pixels from the top, after the title and the tagline; 15 links to `docs/guide/`.
- Run 3, 2026-10-25 22:23 CET, after the second rewrite, on a fresh clone of the fresh `desek/agentloupe`: 367 commits; 0 commits for `prototype` and for the early prototype archive; 0 trees, messages, and file objects with the old zone name; remote refs `HEAD refs/heads/main`.
