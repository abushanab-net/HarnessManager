# Design committee, round 1: the brief for the critics

Three designers have each made a clickable prototype of Harness Manager's first screen (the shell, the Overview, the
Accounts page, the Add account flow). You are one critic on a committee. You review all three; you wrote none of them;
you must not read any designer's reasoning beyond their `angle.md`. You judge the pages as a user would see them.

## Read first
- `docs/design-committee-2026-11/round1/brief.md`: what the designers were told, the owner's words, the rules.
- `docs/design-committee-2026-11/round1/inventory.md`: what must survive.
- `docs/design-committee-2026-11/shots/home-dark.png`, `accounts-dark.png`, `add-account-dark.png`,
  `sidebar-bottom-dark.png`: the app today, the baseline every design must beat.
- The three designs: `docs/design-committee-2026-11/round1/designs/{a-claude,b-codex,c-opencode}/index.html` and each
  folder's `angle.md`. Open each prototype in a headless browser (Edge:
  `"C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" --headless=new --screenshot=<png> --window-size=W,H
  <file url>`, or playwright-core from `e2e/node_modules/playwright-core/index.mjs` to click through) at 1280, 1536
  and 1920 wide, light and dark, motion on and off; use the page's own switches; click into Accounts, Add account and
  the first-run state. Look at the screenshots as images. Judge what you see, not what angle.md promises.

## Your output
One file, `docs/design-committee-2026-11/round1/critiques/critique-<your seat>.md`, in this shape, nothing else:

```
# <Seat>: critique, round 1

## Design A (Claude) · score N/10
**What works** (3 to 6 bullets, concrete: name the element and why)
**What fails** (3 to 8 bullets, each a defect a user would hit; say the screen width/theme where you saw it)
**Must change before it could win** (the shortest list that would raise the score to 9)

## Design B (Codex) · score N/10
(same)

## Design C (OpenCode) · score N/10
(same)

## Across the three
- The one thing a winning design here must have (one line)
- The one thing it must not have (one line)
- Parts worth combining: "<design>'s <element>" lines, at most five
- Ranking: A > B > C style, with one sentence
```

Scores: 9 or 10 means you would ship it as the base with small fixes; 7 or 8 means a strong base that needs a round;
5 or 6 means parts are worth saving; below 5 means it does not beat the app today. Be hard; the owner wants a design
that makes people say wow and a lazy user say "I got my answer". Never invent a defect you did not see on screen; never
soften a defect you did. If a prototype does not open or a switch does not work, that is a finding, score it, and judge
what does render.

## Your lens
Your seat's lens is in your task message. Stay inside it: the other critics cover the rest. Quote the owner's words
from the brief when a defect contradicts them.
