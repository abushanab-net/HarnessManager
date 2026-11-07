# Design A: "The board"

**Angle in one line:** the Overview is a live board of every session running now, laid out by computer, with the
account ladder beside it; the owner's sentence is read as three statements across the top before anything else.

## The hero and why
The lead is "what is running, and where". A person who opens the app for a glance wants to see the work itself: the
sessions, which computer each is on, which account it burns, and whether it is writing this second. So the first
screen is a **board with one lane per computer** (DEVELOPMENTPC, SAMYMSI, Samys-MacBook-Pro). Every running session
is a card in its lane: the tool's icon, the account, the project, the model, how long it has run, what it has used
(measured), and a pulse that breathes only while the session wrote within the last 30 seconds. This computer's lane
sits in front (a brighter surface, a shadow); the other computers recede, and a computer not heard from in three
minutes goes quiet (grey dot, dimmed lane, "last heard 26 m ago"). A click on a card **follows** it: a drawer opens with
that session's account limits and its latest replies, and stays open across redraws.

The other two parts of the sentence are answered on the **answer strip** above the board, in three cells that read
left to right as one sentence: *Running now · Runs out first · Use next* (with the Start button in the third cell).
The right **ladder** lists every account sorted by room left, each with its ring, the tightest window, the time to its
reset and a bar showing how far the window has run; the exhausted ones sink to the foot in amber. So at 1280 wide and
720 tall, without scrolling: what is running and where, which account is exhausted and when it runs out, which
account to use and when things reset.

## References chosen
- Linear and Vercel for restraint: one accent, whitespace over density, no explanation paragraphs.
- Grafana for the dark-mode discipline of a monitoring board: systematic surfaces, quiet lines, colour only for state.
- "Mission Control" and COSMOS for the live feeling (a quiet pulse where something is happening, telemetry-style
  readouts), without their amber-on-black theatre.
- Kanban boards (Linear's board view) for the lane-per-computer idea: a lane is a place; a card is a thing happening.
- Kept from the owner's favourites, redrawn: the limit rings (now in the ladder and on the tray popup), the dot-grid
  trends (in the readouts strip), the year clock (below the fold, with the per-computer switch).

## What left the first screen and where it went
- The Horizon Dial scene is replaced by the board. Its jobs moved: "which tool talks to which account from which
  computer" is every card (tool icon + account, inside its computer's lane); the selected route's readout is the
  follow drawer; the first-reply time is in the drawer and in Latest replies; the live particles became the card's
  write pulse. Escape closes the drawer (the dial's "Escape returns to the finding").
- The finding sentence became the "Runs out first" cell of the answer strip (same words, shorter) plus the
  attention list; its basis (measured %, reset, read-ago, ≈ for the forecast) stays on the cell.
- The readouts under the brand (Tokens, Cost, Cached with the dot-grid trends and the Today/7 d/30 d/All switch)
  moved below the board into one slim strip; today's cost still shows on the Overview tab.
- The limit rings at the foot of the sidebar became the ladder on the right of the Overview (every account, sorted
  by room) and the tray popup (the same ladder, small; the "Tray" button on the prototype bar shows it).
- The sidebar is gone. Navigation is a row of tabs under the top bar: Overview (today's cost), Sessions (live count as
  a dot grid), Projects (count; the caret folds the pinned projects with their live counts), Accounts (count), Setup
  (updates count; a menu with MCP servers, Skills, System prompts, API keys & routing, Apps). CLI reference, Settings
  (with its newer-version dot) and the version are in the gear menu at the right of the top bar.
- The computer's name and the account count stay under the brand. Search, `N live · M elsewhere`, the computers
  chip, the freshness text, `?` (tour) and refresh stay in the top bar.
- Filters (a chip per tool, All computers / This computer) are on the board's header; they filter the lanes and the
  ladder together.
- Activity (year clock with its per-computer switch), Needs attention, Latest replies (with the count and the
  Request log switch) are below the fold, in that order.
- Accounts page: same content as today; the explanation paragraph is gone (the finding is one line, the basis one
  small line). The tokens-per-day chart and the By-model panel are folded by default (a chevron opens them). Every
  row keeps its ring, windows, sparkline, tokens, cost, read-ago, refresh, Start, ⋯ menu, advice and open count; a
  click opens the detail drawer with the four tabs.
- Add account: one dialog in three steps. Step 1 is the tool as six tiles, each tile saying which kinds it takes
  (Subscription · API key · Routed) and whether it is installed; step 2 is the kind as three cards, greyed with a
  reason where the tool does not take it; step 3 the fields. What is kept in sync, what is the account's own and the
  CLI line are a disclosure at the foot of step 3.
- First run: the Overview shows the tools found on the computer with their main profiles already listed as accounts,
  the sync question (shared folder / a hub you host / not now) and Add account. With no tool installed, it says so and
  offers to prepare an account for any tool.

## Motion and depth, and what each means
- **Write pulse** (card): a soft glow at the card's edge breathes while the session wrote within the last 30 s;
  it stops when the session goes idle. Meaning: this one is working right now.
- **Heartbeat** (lane header dot): a slow pulse while the computer was heard from within three minutes. Meaning: alive.
- **Depth** (lanes): this computer's lane is nearer (brighter surface, a shadow); other computers' lanes are flat;
  a quiet computer's lane is dimmed. Meaning: here vs elsewhere vs not reachable now.
- **Ring draw** (ladder, accounts): a ring draws to its value once on first paint, never on a redraw.
- **Follow drawer**: slides in from the right; Escape closes it.
- Still toggle: `.still` on the app root stops every animation; `prefers-reduced-motion` does the same.
- Redraws: the board and ladder are patched in place by key (session id, account id); nodes are never re-created
  for an existing session, so a pulse does not restart and the drawer stays open.

## Fonts and assets
Inter (UI, tabular figures for numbers) and JetBrains Mono (only the request log and the CLI line), loaded from the
app's own files with a relative path (`../../../../../app/ui/fonts/*.woff2`). The logo (`docs/logo.svg`) and the six
tool icons (`app/ui/icons/*.svg`) are inlined as SVG in `app.js`.

## Honesty of numbers
Measured values are solid; a forecast carries ≈ in its text, and the account's ring and bar turn amber when the pace
runs a window out before its reset; a limit never read shows "—" on a grey dashed ring and "limits never read" in the
ladder, with a Read now button. The tag on this computer's lane reads "here" (its tooltip: "this computer").

## Proposed (not in the app today)
- Nothing new is required. One small thing is tagged "proposed" on screen: in the follow drawer, "Open project
  folder" for a session (today the ⋯ menu opens an account's folder, not a session's project folder).

## Prototype chrome
A bar above the framed app: width (1280 / 1536 / 1920, framed at 720 / 864 / 1080 tall and scaled to fit the
window), theme (Harness light / dark, remembered in localStorage), motion (Live / Still), data (8 accounts / 12
accounts / first run, 2 tools found / first run, nothing installed) and a Tray button that shows the tray popup.

## Changed after the round-1 critiques (see rebuttal.md for the line-by-line answer)
- No tokens-per-second on the Overview: Latest replies and the in-place Request log lost the Speed column.
- The ladder: ring = the week (captioned "WEEK" under it, ↻ date = the week's reset); bar = % left of the window
  with the least room, its reset beside it, "≈ out" beside the reset when the pace runs it out first; one colour
  rule everywhere (red: exhausted now or logged out; amber: runs out before its reset at today's pace; teal
  otherwise), written in the legend; "no limits · usage only" for a tool without limit windows; a gear setting
  flips every ring between left and used; the list scrolls on its own past eight accounts.
- The answer strip is re-written from the same snapshot as the board and ladder on every tick; a window at 0% wins
  the second cell as "Exhausted now"; the computer dots in the first cell are gone.
- Lanes show four cards each with "+N more · N writing"; "last heard 26 m ago" is its own line; the board header
  is the filters only; the board and eight ladder rows end above 720 px at 1280 (measured).
- Add account: kinds per tool from the app's `Caps`; email optional; focus trapped and restored; the keyboard
  legend removed; "Done" adds the entered account to the list with its limits not read.
- First run: no explanation paragraph, the empty lane above the found tools; the tabs and top bar show nothing that
  is not known yet (no cost, no project count, one computer, no update badge, "Limits not read yet").
- Contrast raised to WCAG AA on every visible text node in both themes (dim and muted tokens, light teal and green).
- Freshness kept at 1280 in a short form; the readouts' dot grids are one row of 14; the live dot's pulse is a
  transform/opacity ring, not a box-shadow.
