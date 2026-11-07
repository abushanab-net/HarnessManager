# Design committee, round 2: every page in the Chrono-Runway direction

## The decision
The owner picked **Design C · Chrono-Runway** as the base (7 Nov 2026), with no parts named from A or B and no
comments. Everything in this round is built on C's prototype, copied to `round2/app/` (index.html, views.js,
modals.js, styles.css, data.js). C's identity stays: the dispatch deck (Use next · Runs out first · Running now), the
Reset Horizon lanes, the capacity column, the rail sidebar, its tokens and type. The round-1 brief
(`../round1/brief.md`), inventory (`../round1/inventory.md`) and the rules in them still bind.

## Phases
1. **The lead (Designer C)** repairs the base and sets the language. In `round2/app/`:
   - Fix every item still open in the re-scores (`../round1/critiques/rescore-*.md`, Design C sections), at least:
     the Add-account capability matrix (API key: Claude Code, Codex, Pi; Routed: Claude Code, Pi; Antigravity,
     OpenCode, Oh My Pi: subscription only); the tick labels that overprint at 1280 ("NOW 15:42" / "Sat (+1d)") and the
     routed lane's gate label drawn over its text; the tray popup clipping its second column at 1280/1536 and with 12
     accounts; the 5-hour row that is a <3% sliver on the 7-day axis (give the 5-hour window a readable drawing: its
     own short track or a magnified first segment, labelled); the legend calling the time-to-reset span "Measured";
     every text under 10 px raised to 11 px or more; the keyboard legend in Add account; chip counts that disagree
     with the rows (SAMYMSI 4 vs 3); the first-run Accounts page forecasting on limits it never read; the "Harvest"
     rule tagged as proposed on screen; Setup counts ("MCP 11 · Skills 255") in the empty state; a modelled 720-tall
     fold (the width switch also sets the height: 1280×720, 1536×864, 1920×1080) so the fold claims can be checked;
     per-region rendering with state restore instead of whole-app innerHTML on every change (scroll, focus, open
     dialog, selection and animation phase must survive a data redraw and a keystroke).
   - Add a **page registry** so the other designers can add pages without touching the lead's files:
     `window.HM.pages[id] = { title, render(state) → html, mount?(root, state) }` in their own file
     (`pages-a.js`, `pages-b.js`, loaded by index.html after views.js); the rail's nav items route to registered pages
     by id (`sessions`, `session`, `projects`, `mcp`, `skills`, `prompts`, `routing`, `apps`, `cli`, `settings`); an
     unregistered page shows the lead's placeholder. Account detail (`account`) is the lead's own page, registered the
     same way, with its tabs (Overview, Sessions, Projects, Settings).
   - Write `round2/app/design-language.md`: the tokens (colour roles in both themes, type scale, spacing, radii), the
     components with the exact class names to reuse (cards, deck cards, lanes, chips, pills, tables, dialogs, rings,
     bars, the year clock, empty states, toasts), the rules (measured / ≈ / — marks; one primary action per fold; the
     finding as the page head, no slogans, no explanation paragraphs; monospace only in logs, CLI and paths; icons;
     keyboard and focus; motion and Still), how to register a page, and how to verify a page (widths, themes, fold).
2. **Designers A and B, in parallel**, each draw their pages in C's language in their own file, reading
   design-language.md, `round2/inventory-pages.md` (every page's features, nothing may be lost) and the round-1
   materials. Designer A: Sessions, Session (following one session live), Projects. Designer B: MCP servers, Skills,
   System prompts, API keys & routing, Apps, CLI reference, Settings (including Computers). Each page: light and dark,
   1280/1536/1920, its empty state, its actions working as clicks, mock data added to its own file (never editing
   data.js: extend `window.HM.data` from the page file), verified with screenshots.
3. **The critics** (the same six seats) review every page against inventory-pages.md and design-language.md, score
   per page, and the owner reviews one page with the whole app in a frame and a form per page.

## Rules that still hold
Every rule of round 1: honest marks, no tokens-per-second on the Overview, nothing in the inventory lost (hidden is
fine; say where), no invented features without a "proposed" tag, redraw-safe, the lazy user reads no explanations,
the official icons and the Harness mark, write only in your own files, do not touch app/ui, do not commit.
