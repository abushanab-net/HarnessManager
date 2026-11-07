# Design A: rebuttal, round 1

One line per defect the critics raised against A. "Fixed now" means it is in the prototype as of this file and was
re-verified in headless Edge at 1280/1536/1920, light and dark, with zero console errors, a text-contrast scan of
every visible text node (none under 4.5:1), and the keyboard, redraw and Still checks re-run. The angle is unchanged:
the board of what is running, the three-cell sentence above it, the ladder beside it.

## Rule 2, honesty and the instruments
- Speed column (≈ N t/s) in Latest replies and the in-place Request log on the Overview (craft, clarity, lazy user,
  build, dataviz, Astra): **agree, fixed now.** Both tables on the Overview drop the column; the subtitle no longer
  mentions speed. The inventory's "speed" column lives on the Sessions page's log, which is not in this round.
- Ladder bar lies: labelled "98% left" but encoding the elapsed fraction of the window (dataviz, craft, clarity):
  **agree, fixed now.** The bar now encodes % left of the window with the least room, the same shape and meaning as
  the Accounts rows, the follow drawer and the tray; the time to that window's reset stands beside it.
- Ladder mixes two windows without saying so (craft, clarity, lazy user): **agree, fixed now.** The ring carries a
  "WEEK" caption under it; the date on the row is marked ↻ (the week's reset); line two names its window
  ("5-hour 98% left"); the panel header says "ring: week left" and the legend says what ring, bar and colours mean.
- Ring colour follows no single rule (87% amber, 72% teal with a forecast) (dataviz): **agree, fixed now.** One rule
  everywhere (ladder, tray, Accounts rows, drawer): red when the account cannot be used now (a window exhausted or
  the login expired), amber when any window runs out before its reset at today's pace, teal otherwise; the legend
  states it.
- Reset time replaced by the forecast where a forecast exists (Astra): **agree, fixed now.** Every window shows
  "↻ 3 d 18 h" and, when the pace runs it out first, "· ≈ out Sun 08:10" after it (rows, drawer, ladder).
- Answer strip built once, not from the tick's snapshot (Astra): **agree, fixed now.** `patchStrip()` rewrites the
  three cells from the same data on every tick (the Start button node is kept so focus survives).
- No exhausted-now branch (Astra): **agree, fixed now.** A window at 0% left wins the second cell as "Exhausted
  now · 5-hour 100% used · back in 1 h 12 m (22:22)" in red; the 12-account state shows it (samykabu-work).
- Readouts dot grids fold 28 values into two rows and collide with "Cached" at 1280 (dataviz): **agree, fixed now.**
  One row of 14 (the last 14 values), tighter readouts at 1280; no collision at any width.
- Three unlabeled computer dots in the "Running now" cell repeat the computers chip (dataviz): **agree, fixed now.**
  Removed; the text already says "2 of 3 computers alive".
- No 12-account state; ladder runs below the fold and is not sticky at 12 (dataviz, build, Astra): **agree, fixed
  now.** The data switch has "12 accounts" (an exhausted account, a second Codex, an unread OpenCode and a Pi API-key
  account); the ladder list scrolls on its own (8 rows end at 683 px at 1280×720; 12 scroll inside it).
- "Limits as used" setting not seen (clarity, build): **agree, fixed now.** Gear menu › "Rings show: left / used"
  flips every ring and the ladder header; remembered in localStorage like the theme.
- The write pulse is redundant with "wrote 4 s ago" (dataviz, marginal): **disagree**; the pulse is readable from
  across the room and stops when idle, the text is for reading up close; both stay.

## Clarity, completeness and the lazy user
- Add-account tiles state the wrong capability matrix (clarity, inventory check): **agree, fixed now.** Kinds come
  from the app's `Caps` in `crates/core/src/harness/*.rs`: Claude Code = Subscription · API key · Routed; Codex =
  Subscription · API key; Pi = Subscription · API key · Routed; OpenCode, Antigravity, Oh My Pi = Subscription only.
  (The coordinator's note listed Routed for Codex, OpenCode and Oh My Pi; `routable` is false for all three in the
  code, so the code wins, as instructed.)
- First-run chrome is stale ($571, 23 projects, live dots, 3 computers, 1 update) (clarity): **agree, fixed now.**
  With no sessions and no reads, the tabs show no cost, no project count, an empty dot grid, "1 computer", no update
  badge, "Limits not read yet" in the top bar.
- First-run Accounts page forecasts a week it says it never read (lazy user): **agree, fixed now.** The found main
  profiles carry no advice or forecast; the finding is "No limits read yet."; the basis says "limits read never".
- Freshness text dropped at 1280 (craft, clarity, Astra): **agree, fixed now.** A shortened form ("limits 14 m ·
  usage now") shows at 1280 with the full text as its tooltip; the full text shows at 1536 and 1920.
- Contrast: strip labels, countdowns, legend, "Ctrl K", tab cost under 4.5:1 (clarity): **agree, fixed now.** The
  dim and muted tokens were raised in both themes (dark dim 5.3:1 on a panel, light dim 5.2:1 on white; light teal
  and green deltas darkened a step); the scan reports zero text nodes under 4.5:1 on the Overview, Accounts and
  first run at every width in both themes.
- Add account does not add the entered account (Astra): **agree, fixed now.** "Done" appends it to the list with
  "— not read" limits (or "none on Pi" for a tool without limits), its kind as the tag, "Added today", then opens
  Accounts scrolled to the row; the account count in the tab and the brand follows. A duplicate name is refused.
- Focus not trapped in the dialog; focus not restored on Escape (clarity, Astra): **agree, fixed now.** Tab cycles
  inside the Add, Start-in-a-folder, detail and follow surfaces; Escape or Close returns focus to the control that
  opened them (verified: 12 Tabs stay inside; Escape lands on the Add account button).
- Email required in Add account (lazy user): **agree, fixed now.** Email is optional ("checked after you log in if
  given"); only the name is required.
- Three wizard steps on one page so a Codex account takes 4 clicks (lazy user): **agree but needs a round.** The
  steps are what makes "which tool takes which kind" explicit before the fields; collapsing them into one screen is a
  layout the critics should price against B's one-dialog flow, not a patch.
- "Tab from the top of the page visits 13 controls before the first nav tab" (clarity): **disagree**; twelve of those
  are the prototype's own width/theme/data switches above the app frame, not the design (the app's top bar has six).
- The filter chips and All computers / This computer would never be used (lazy user): **disagree**; both are in
  inventory.md (the Overview's filters) and cost one line.
- The readouts strip "only appeared after a filter click" (lazy user): **disagree**; it is below the fold at 1536 by
  design (rank 2); a chip click does not create it.
- Computers chip and main-profile hide-only not tested (clarity): the chip opens Settings › Computers and the ⋯ menu
  of a main profile shows "Hide from the list" instead of Remove; both exist and can be clicked.

## Craft
- 1920: the board leaves a void beside the shorter lanes (craft): **agree but needs a round.** Lanes are now capped
  at four cards each with "+N more" (the void is smaller, lanes end at ~700 px vs the ladder's ~760); letting lanes
  flow into free width or letting the ladder widen is a layout question for the next round.
- 1280: lane header truncates "Samys-Ma…", ladder names truncate (craft, clarity): **agree, fixed now.** "last heard
  26 m ago" moved to its own line under the lane header; the ladder is 314 px at 1280 with a 36 px ring; a DOM
  measurement reports no clipped name at any width.
- "Running now · 10 of 10" under the strip's "10 sessions": two headings for one fact (craft): **agree, fixed now.**
  The board header is the filters only; "showing 6 of 10" appears only while a filter is active.
- First run: explanation paragraph under the heading, board below it (craft, lazy user): **agree, fixed now.** The
  paragraph is gone; the empty lane sits above the "Two tools found" panel; the sync panel lost its sub-line.
- Strip cells break mid-name at 1280 (craft): **agree, fixed now.** Account names and durations in the strip are
  non-breaking spans; the Start button says only "Start" at 1280 (the name is in the cell's heading already).
- Keyboard legend in every Add-account footer (craft): **agree, fixed now.** Removed; Enter, Tab and Escape work.
- Light theme "Ctrl K" at 2.6:1 (clarity): **agree, fixed now** (see contrast above).

## Build
- The live dot's `beat` animates `box-shadow` (300 style recalcs per 5 s) (build): **agree, fixed now.** It is a
  pseudo-element ring animating transform and opacity, the same `heart` keyframe as the lane dots.
- The board does not fit the fold at 1280×720 with ten sessions (build): **agree, fixed now.** Four cards per lane
  with "+N more · 1 writing"; the board ends at 699 px at 1280×720 (measured); the expanded state is kept across
  ticks in state, not in the DOM.
- A second render path (patch by key) beside `core.js`'s `innerHTML` + `warmPage`, and a drawer outside the dialog
  model (build): **agree but needs a round**; it is the price of rule 5 being proven rather than promised, and the
  build critic's own table shows it is the only prototype whose redraw kept focus, scroll, drawer and animation phase.
- The sidebar is gone, so `index.html`, `app.css`, `core.js` nav code and the e2e measurements change (build):
  **disagree** as a defect; the brief allows removing the sidebar, and the owner ticked two of the sidebar's three
  parts as the problem.
- Width/theme/still in one localStorage key, `st.follow`/`st.detail` reset on load (build): **agree**, prototype
  only; the note to map onto `state.settings` stands.
- The chip labels are icon-only at 1280 (self-reported risk): unchanged; the official icons are the labels the owner
  asked for, with the name as tooltip.

## Still open for the next round (not cheap, or a design decision)
- Lanes flowing into free width at 1920 (or a wider ladder).
- The Add-account steps as one screen vs three.
- The app-side render model for the keyed board and the drawer's place in the dialog model.
