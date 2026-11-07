# Build: re-score, round 1

Same method as `critique-build.md`: headless Edge over CDP (port 9454), 1280/1536/1920 through each prototype's own switches, dark and
light, Live and Still, `prefers-reduced-motion` emulated, 150 % DPR via `Emulation.setDeviceMetricsOverride`. Each "fixed" claim in the
three `rebuttal.md` files was checked on screen or in the DOM, not taken from the file. The redraw trigger is each design's real
re-render (A: its 4 s tick; B: "Data tick", which now mutates a quota and runs `render()`; C: "Simulate 3s Redraw", which runs
`renderApp()`), with scroll, focus, an open drawer/dialog and the running animation checked before and after. Animation continuity was
measured two ways: `currentTime`/`animation-delay` of the rebuilt element, and, because delay arithmetic is ambiguous, the computed
opacity or transform of the animated element one frame before and two frames after the redraw, five trials each.

| 1280, dark, 8 accounts | A (Claude) | B (Codex) | C (OpenCode) |
|---|---|---|---|
| Running animations, Live | 7 (`heart` ×3, `pulse` ×4; the `box-shadow` `beat` is gone) | 2 (`travel`) | 7 (`livePulse`) |
| Idle 5 s at 150 %: task ms / style recalcs | 61 / 58 (was 230 / 300) | 37 / 56 | 3 / 0 |
| Ten real redraws at 150 %: task ms | 62 (≈ 6 ms each) | 159 (≈ 16 ms each; it now rebuilds `#content` for real) | 196 (≈ 20 ms each; whole app) |
| After a real redraw: scroll / focus / open surface | 200 → 200 / kept / drawer kept | 200 → 200 / restored by `data-focus-key` / dialog kept | 200 → 200 (window) / restored by selector / follow drawer re-created and kept |
| Animation across the redraw (visual, 62 ms gap) | opacity jump ≤ 0.06 in 5 trials: continuous | traveller moves 0.98–1.15 px, exactly its speed: continuous (`--travel-phase` from a pause-aware clock) | opacity jump ≤ 0.07: continuous within a frame (`--pulse-delay` from `Date.now()`) |
| Typing "-xyz" in Add account's name | value "-xyz", input keeps focus | "-xyz", keeps focus | "work-design-xyz", keeps focus (`updateField` patches two preview nodes; no re-render) |
| Still / reduced-motion / after the OS preference is cleared | 0 / 0 / 7 running | 2 paused / 0 / 2 running (no longer persisted) | 0 / 0 / 7 running |
| 1280×720, 10 sessions, 8 accounts | strip 208, board 699 ("+2 more · 2 writing"), ladder's 8th row 683 | ledger's 8th row 599, Use now 280, Resets first 361, live rail 714 | deck ends 317, first lane 511, 8th lane 813 (≈ 5 visible); frame still has no height, document is 1,582 px |
| 12 accounts | ladder scrolls inside its panel (12th row at 881 inside a scroller) | 12th ledger row 783, scrolls in `#content`; cap and API-key rows have their own treatment | 12th lane at 967; document 1,737 px |
| First-run / empty chrome | "0 live · 0 elsewhere · 1 computer", no cost, no project count, Accounts 2 / 0 | "0 live · 0 elsewhere · 1 computer · Limits not read", $0, Projects 0, Accounts 0 | "0 live · 0 elsewhere · 1 computer · Limits not read", $0.00, Sessions 0, Projects 0; **Setup still says "MCP servers 11 · Skills 255 · 1 update" with no tool installed** |
| Fixture text baked into views | 1 | 6 (the Needs-attention list: "5 open", "claude-main · 8% week remaining", "Groq provider paused", unchanged in the 12-account state) | 0 in `views.js`, 9 ids/defaults in `modals.js`; the Setup counts above |
| Inline `style=""` in JS | 31 | 46 | 261 (was 247; deferred by the rebuttal) |
| Console errors | none | none | none |

## Design A (Claude) · buildability 8/10 (was 7)

**Revised estimate: 11 to 13 engineer-days** (was 12 to 14). Down because three things I had priced as app work now exist in the
prototype and were verified: the four-cards-per-lane cap with "+N more" held in state (board ends at 699 px at 1280×720), the ladder
as its own scroller at 12 accounts, and the "Rings show: left / used" setting; and because the `box-shadow` keyframe is gone, so there is
no paint fix to do. Unchanged: the shell without a sidebar (`index.html`, `app.css` shell, `core.js` nav/fold/pins), the keyed patch
path in `home.js` beside `innerHTML`/`warmPage`, the drawer outside `#dialog-root`'s `S.dirty` rule, the e2e rewrite. The rebuttal
accepts all three as the price of the design ("agree but needs a round"), which is the right reading.

**Fixed, verified**
- Live dot: `.live i::after` runs the `heart` keyframe (transform + opacity); idle main-thread cost fell from 230 ms to 61 ms per 5 s at
  150 %, style recalcs from 300 to 58.
- Board fits the fold at 1280×720 with ten sessions: lanes capped at four cards, "+2 more · 2 writing", expansion kept in `st.more`
  across ticks (the tick patched the board with the button still there).
- Twelve accounts exist (`data:twelve`): an exhausted account wins the second cell as "Exhausted now · 5-hour 100% used · back in
  1 h 12 m", the ladder scrolls inside its panel, tab and brand counts follow.
- Speed column gone from both Overview tables (0 occurrences of "t/s" in `app.js`).
- Used/left ring setting in the gear menu, remembered like the theme; ladder header says "ring: week left".
- First-run and empty chrome computed: "0 live · 0 elsewhere", "1 computer", no cost, no project count.
- Redraw story re-proven after the changes: 6 ms a tick, scroll 200 → 200, focus kept, drawer kept, pulse continuous (≤ 0.06 opacity
  change over 62 ms); `patchStrip()` now rewrites the three cells from the tick's data and keeps the Start button node.
- Typing in Add account works (the wizard's step 3 input keeps focus and value).

**Still open**
- The app-side render model: a keyed patch path in `home.js` and an overlay that outlives `render()` without setting `S.dirty`.
  Accepted by the designer for the next round; it is the single biggest line in the estimate (≈ 3.5 days).
- The shell rewrite and the e2e consequences from round 1 (sidebar test deleted, add-flow tests re-sequenced for the three steps,
  measurement tests rewritten) are unchanged.
- Prototype-only settings storage (one localStorage key) still to be mapped onto `state.settings`.

**New defect**
- At 12 accounts (1280 dark) the ladder's internal scroller also shows a horizontal scrollbar along its foot: a row is a few pixels
  wider than the list. One `overflow-x: hidden` or a `minmax(0, 1fr)` column, but it is on screen now.

## Design B (Codex) · buildability 9/10 (was 8)

**Revised estimate: 7 to 8 engineer-days** (was 7 to 9). The three build defects were fixed in the prototype, so nothing is left to
design before coding: capped and API-key rows have their own ledger, tray and Accounts treatment; the traveller's phase is clock-based;
the still/reduced-motion split is done; the frame now uses `container-type: inline-size` with `@container app` rules, so the responsive
layer ports as written rather than from `.compact`/`.wide` classes. Slightly up on the Overview itself (the live rail now lists six
followable project rows with their accounts and writing state, and the finding is computed), slightly down on the tray and Add dialog
(both reuse `ring()` and existing dialog code). The e2e picture is the same as round 1: the overview and three measurement tests
rewritten, selector edits elsewhere, the add/sign-in tests keep their sequence.

**Fixed, verified**
- Rule 5 now met for real: "Data tick" mutates `claude-main`'s 5-hour window and runs `render()`; the traveller moved 0.98 to 1.15 px
  across the rebuild in five trials, which is exactly its 15.8 px/s speed over the 62 ms gap (`--travel-phase` from a clock that
  stops while Still is on). Scroll 200 → 200; the focused filter button carries `data-focus-key` and is re-focused; the open dialog
  stays.
- Reduced-motion no longer leaks into the setting: after emulating `reduce`, reloading, then clearing it, 2 of 2 animations run; only an
  explicit Still click writes `capacity-b-still`.
- Capped accounts: `resal-router` shows "Daily cap: $13.80 left / $25.00 · $11.20 spent · resets in 8h 18m · 55.2% · No forecast ·
  Capped"; `codex-key` shows "Pay per use · no plan windows · n/a · Pay/use" (12-account state, 1280 dark). No fabricated window.
- Tray preview reuses `ring()`, the tool icons, both windows with resets and a Start per row.
- Twelve accounts via the Data switch; the ledger's 12th row ends at 783 px and scrolls in `#content`; the live rail stays beside it.
- Add dialog types without losing focus; the first Accounts row starts higher (the duplicate finding callout and the chart moved).

**Still open**
- The Needs-attention list under the fold is still the fixture ("5 open", the same five items in the 8- and 12-account states, and
  "claude-main · 8% week remaining" while the ledger says claude-main has 8 % of its week). Six baked strings; the app has the real
  list (`attention()` in `home.js`), so this is a prototype leak, not a design gap, but it is the one place the page is not data-driven.
- The tray preview at 12 accounts is a 12-row modal taller than 720 px; `limits.html` is a small always-on-top window and will need the
  compact two-line row or a scroller.
- The tour has step markup (`tourstep`, `tourfocus`); I did not drive it this round.
- Each real redraw now rebuilds 1,545 nodes including the 371-cell year clock (≈ 16 ms at 150 %). Within budget, and the same thing
  the app does today, but keying the clock would halve it.

**New defect**
- None found on screen. (The redraw cost above is a consequence of the fix, not a defect.)

## Design C (OpenCode) · buildability 7/10 (was 5)

**Revised estimate: 11 to 14 engineer-days** (was 13 to 16 "after a design round"). The design round is no longer needed: the views
are computed from `data.js` (0 baked strings in `views.js`), the Reset Horizon is one linear 0–7 d scale with computed ticks and
capacity drawn separately on fixed-length bars, the provider hop and the cadence column are gone, Add account types in place, and
`renderApp()` snapshots/restores focus, window scroll, modal and tray scroll and anchors the pulse to a clock. What remains is port
work: the whole-app `innerHTML` model must become sheet-only in `home.js`, 261 inline styles and a 2,000-line stylesheet must be
tokenised for VektraGrid (the rebuttal defers this to round 2, so it stays in the estimate at ≈ 1.5 days), and the timeline needs a
label-placement pass (see new defects).

**Fixed, verified**
- Keystrokes: `updateField` sets `STATE` and patches only the folder and CLI preview nodes; typing "-xyz" into the name field gives
  "work-design-xyz" with focus kept (round 1: one character, focus on `<body>`).
- Redraw: after "Simulate 3s Redraw" the focused button is restored by a built selector, window scroll 200 → 200, the follow drawer is
  re-created and open, and the pulse dot's opacity changed ≤ 0.07 over 62 ms (continuous within a frame; `--pulse-delay` from
  `Date.now() % 2200`).
- Baked text: `views.js` has none; the deck, nav cost, group resets, by-model table, year-clock totals and "12 here · 6 elsewhere" come
  from `D.deriveMetrics`; the 12-account state shows "claude-audit is exhausted now (0% 5-hour left)" computed.
- Timeline: ticks are "NOW 15:42, Sat (+1d) … +7d" from `D.NOW`; `linearPct()` maps 0–168 h to 0–100 %; forecast dashes and starvation
  spans only when `outMs < resetMs`; capacity (% left) is its own column.
- Provider names and "Speed (cadence)" removed from the Overview; speed lives in the Request log modal only.
- First-run and empty chrome computed: "0 live · 0 elsewhere · 1 computer · Limits not read", "$0.00", Sessions 0, Projects 0.
- Still hides the pulse (0 animations), reduced-motion the same; nothing persisted.
- Tray popup shows all 12 in a scrollable grid; a new account starts unread (`windows: []`, "—") and stays in the current preset.

**Still open**
- The frame has no height: `.workspace > main` is `overflow: visible`, the document is 1,582 px (8 accounts) and 1,737 px (12) at 1280,
  and the browser scrolls. "Above the fold at 1280" is still unverified by the prototype; measured against 720, the deck ends at 317 px
  and about five of eight lanes are visible.
- Whole-app `root.innerHTML` on every state change (≈ 20 ms a redraw at 150 %, the heaviest of the three). Survivable now because of
  the restore code, but `home.js` will only ever replace the sheet; the restore code is throwaway.
- 261 inline `style=""` (up from 247) and 60 raw hex in CSS; deferred to round 2 by the rebuttal.
- "Harvest" (5-hour resets within 2 h and ≥ 75 % left) is still a product rule drawn as a starred gate with no proposed tag.
- Setup counts in the empty state: "MCP servers 11 · Skills 255 · Apps 1 update" with zero tools installed (1280 dark, Empty (0)).

**New defects**
- Label collisions on the linear track at 1280 dark: the ruler's "NOW 15:42" and "Sat (+1d)" overprint each other at the left edge,
  and in the capped `opencode-main` lane the track text ("Routed … (DeepSeek V4 → OpenRouter) · daily cap resets in 8 h 18 m")
  overprints the gate label "Cap ↻ 8 h 18 m". On a 7-day scale a 5-hour gate sits at 3 % of the width, so every 5-hour label starts on
  top of the NOW tick; the design needs the same measure-and-push labelling the dial has (`labelLayer` in `home.js`), which I have added
  to the estimate.
- The lane's Start button is hidden until hover or focus-within; the e2e `start-<account>` buttons would have to be found hidden.

## Across the three
- The one thing a winning design here must have: a real redraw that leaves scroll, focus, open surfaces and motion untouched. All three
  now pass that on their own mechanism (A patches by key; B and C rebuild and restore with a clock-anchored delay), so it is no longer a
  differentiator; what differs is how much of each mechanism survives the port (A's and B's do; C's whole-app restore does not).
- The one thing it must not have: a fixed-pixel frame that hides the 720-tall window (C) or fixture text in a panel the app computes
  (B's attention list, C's Setup counts).
- Parts worth combining:
  - A's `patchBoard`/`patchLadder` keyed updates and the "+N more" lane cap held in state.
  - B's cap/API-key row treatment ("Daily cap … Capped", "Pay per use · n/a") for every ledger, tray and Accounts row.
  - B's `data-focus-key` restore and `@container app` rules: both drop straight into `core.js`/`app.css`.
  - C's `updateField` pattern for dialog previews (patch two nodes, never re-render while typing).
  - C's linear 7-day track once it has a label-placement pass.
- Ranking: B > A > C. B is now a week on the app's own model with nothing left to design; A is a fortnight because of the shell and the
  second render path, which the designer accepts; C moved from "needs a design round" to "needs a port" and is the only one still
  without a modelled fold.

## Cost table

Assumptions unchanged from round 1: one engineer who knows `app/ui`; backend untouched; four themes by mapping tokens onto `app.css`'s
roles; tray popup reuses the window's component; e2e updated; QA at 1920/100 %, 1536/125 %, 1280/150 % and macOS WebKit.

| Design | Engineer-days (was) | Biggest risk now | What the e2e tests lose |
|---|---|---|---|
| A (Claude) | 11 to 13 (12 to 14): shell without sidebar 2, strip/board/ladder keyed patching 2.5, drawer outside the dialog model 1, lower panels 0.5, Accounts 1, 3-step Add 1, first run 0.5, tray 0.5, themes 0.5, e2e 2, QA 0.75 | The keyed render path and the drawer are new infrastructure in `home.js`/`core.js`; accepted by the designer, still the costliest line | Unchanged: overview, sidebar (deleted), three measurement tests, five add/sign-in tests re-sequenced, tour, computers-Overview checks rewritten; ≈ 12 selector edits |
| B (Codex) | 7 to 8 (7 to 9): sidebar trim 0.25, Overview 1.75, folded lower sections 0.5, Accounts 0.75, Add + sign-in 0.5, detail dialog 0.5, first run 0.5, tray 0.25, themes 0.5, e2e 1.25, QA 0.75 | None technical; the attention list is a fixture leak and the tray needs a compact row at 12 | Unchanged: overview, three measurement tests, tour (now steps, closer to the app's), computers-Overview checks rewritten; ≈ 13 selector edits; add/sign-in keep their sequence |
| C (OpenCode) | 11 to 14 (13 to 16 plus a design round): rail 0.5, deck 1, horizon with label placement 2.5, lower panels 0.75, Accounts 1, Add + code step 0.75, modals 1, first run 0.5, tray 0.5, tokenising inline styles 1.5, e2e 2, QA 1 | No modelled fold at 720, label collisions on a 7-day scale, and a restore layer that cannot be ported | Unchanged: overview, three measurement tests, add/sign-in (one dialog then a code step), tour, computers-Overview checks rewritten; lane Start buttons hidden until hover; ≈ 14 selector edits |

Shared, unchanged from round 1: the dial, sidebar-ring and sidebar-readout assertions die in every case; the 371 heatmap cells survive
in each; `finding`/`basis` survive as ids on A's second cell, B's h1 and C's "Runs out first" card.
