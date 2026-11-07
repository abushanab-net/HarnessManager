# Design committee, round 2: the brief for the critics

The owner picked Design C (Chrono-Runway) as the base. The whole app now exists as one clickable prototype:
`docs/design-committee-2026-11/round2/app/index.html`. The lead (now on Codex, keeping C's design) repaired the base
and registered the Account detail page; Designer A drew Sessions, Session and Projects (`pages-a.js`); Designer B drew
MCP servers, Skills, System prompts, API keys & routing, Apps, CLI reference and Settings (`pages-b.js`). The rail's
nav reaches every page; each page's own switches (width 1280×720 / 1536×864 / 1920×1080, theme, motion, data state)
sit above the frame.

## Read first
- `round2/brief.md` (the round), `round2/app/design-language.md` (the language every page must follow),
  `round2/inventory-pages.md` and `round1/inventory.md` (what must survive, page by page), `round1/brief.md` (the rules).
- The app today for comparison: `docs/design-committee-2026-11/shots/*.png`.
- Your own round-1 critique and re-score, for continuity of your lens.

## Your output
One file: `round2/critiques/critique-<seat>.md`:

```
# <Seat>: critique, round 2

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score N/10
**Still open from round 1** / **What works** / **What fails** (width/theme where seen) / **Must change**

## Sessions · N/10   ## Session · N/10   ## Projects · N/10        (Designer A)
## MCP servers · N/10  ## Skills · N/10  ## System prompts · N/10  ## API keys & routing · N/10
## Apps · N/10  ## CLI reference · N/10  ## Settings · N/10         (Designer B)
(each: What works / What fails / Must change; and "Inventory misses" for the clarity judge)

## Across the app
- Is it one app? Where the three hands show (components, voice, spacing, type) and what to unify.
- The three worst defects in the whole app, ranked.
- Ranking of the twelve pages from best to worst, one line.
```

Scores: 9 or 10 means implement as drawn; 7 or 8 one fix round; below 7 redraw. Render every page yourself at the
three sizes in both themes, click its actions, open its dialogs, check its empty state; judge what renders, not what
the designers' reports claim. Use your own Edge port and your own scratchpad; kill your Edge when done; edit nothing.
