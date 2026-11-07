# Build: critique, round 1

Method: each prototype opened in headless Edge (msedge --headless=new, playwright-core over CDP) at 1280, 1536 and 1920 through its own
width switch, Harness dark and light, motion Live and Still, plus `prefers-reduced-motion` emulated. Measured with `document.getAnimations()`,
a `requestAnimationFrame`/`setInterval` hook, CDP `Performance.getMetrics` at deviceScaleFactor 1.5 (the 150 % laptop), node counts,
and a scripted "redraw" of each prototype (A: its own 4 s tick; B: a filter click, which is its `render()`; C: its "Simulate 3s Redraw"
button) with scroll, focus, an open drawer/dialog and a running animation's `currentTime` checked before and after. The app today:
`sheet.innerHTML` replaced on every route change and on every `refreshed` event, scroll restored by `core.js`, folds remembered in
localStorage, dialogs in `#dialog-root` deferring the redraw (`S.dirty`), four themes on `html[data-brand][data-theme]`, container queries
at 70 rem and 56 rem.

Numbers that matter (1280 wide, dark, 8 accounts):

| | A (Claude) | B (Codex) | C (OpenCode) |
|---|---|---|---|
| DOM nodes in the app frame | 1,659 (990 SVG) | 1,646 (1,005 SVG) | 1,271 (582 SVG) |
| Running CSS animations, Live | 9 (`beat` box-shadow ×1, `heart` ×2, `pulse` ×6) | 3 (`travel`, transform) | 6 (`pulseFlow`, translate3d) |
| rAF loops, timers | 0 rAF; one 4 s `setInterval` (its own tick) | none | none |
| Idle 5 s at 150 %: main-thread task time | 230 ms (300 style recalcs: the `beat` keyframe animates `box-shadow`) | 4 ms | 4 ms |
| Ten redraws at 150 %: task time | 58 ms (≈ 6 ms each, patched in place) | 91 ms (≈ 9 ms each, `#content` rebuilt) | 133 ms (≈ 13 ms each, whole app rebuilt) |
| After one redraw: scroll / focus / open drawer / animation phase | kept / kept / kept / continued (6,122 → 6,872 ms) | kept / dialog kept / **traveller restarted (6,138 → 50 ms)** | **focus lost (body)** / modal re-created / **bead restarted (283 → 33 ms)** |
| Still | 0 animations (`animation:none`, transitions too) | 3 paused (`animation-play-state`) | 0 (beads `display:none`) |
| `prefers-reduced-motion` | 0 running | 0 running (but saved to localStorage as the Still setting; it stays on after the OS preference goes) | 0 running |
| Rank-1 content at a 1280×720 window | answer strip ends 208 px, ladder (8) ends 728 px, board ends 848 px (10 cards overflow) | ledger (8 rows) ends 565 px, Use now 296, Resets first 420, live rail 705: all inside | deck ends 304 px, first lane 499 px, 8th lane 814 px (≈ 5 lanes visible), 12 lanes 986 px; the frame has no fixed height, so the fold is not modelled |
| Inline `style="` in JS / raw hex in CSS outside the token blocks | 31 / 50 (icons, chart palette) | 45 / 27 | 247 / 60 |
| Fixture text baked into views (account names, "12 live", "$571.37", axis labels) | 1 | 6 | 35 |
| Platform features | none risky | `<dialog>`/`showModal`, `backdrop-filter` on `::backdrop`, `Array.at` | `backdrop-filter` on the modal |

No prototype uses `:has`, container queries, `popover`, CSS nesting, canvas or WebGL; all load Inter from `app/ui/fonts` over `file://`
without error; no console errors in any. Every width is a fixed-pixel frame (A scales a 16:9 frame, B toggles `.compact`/`.wide`
classes, C one `@media`), so each design's behaviour below 1280 and beside a terminal is undefined and must be re-expressed in the app's
container queries. All three (and the app today) inline the Antigravity icon with its eleven `feGaussianBlur` filters once per use
(22 to 44 filtered elements on a page): a shared paint cost at 150 %, not a differentiator.

## Design A (Claude) · score 7/10

**What works**
- The only prototype that engineered for rule 5 and proved it: the board and ladder are patched by key (`patchBoard`/`patchLadder`,
  `data-session`/`data-lad`), nodes are never re-created for an existing session, so after its tick the card pulse continued
  (6,122 → 6,872 ms), the follow drawer stayed open, the focused Start button stayed focused and the scroll stayed. Six ms a tick at 150 %.
- Everything on screen is computed from `data.js` (`outFirst`, `useNext`, `resetsSoon`, lane counts, the ladder sort); one fixture string
  in the whole `app.js`. Every field it uses exists in the app's payloads: `running[].started_at/cwd/session_id`, sessions' `tokens` and
  `last_active_at` (the "wrote 1 s ago" and the 30 s pulse), peers' `seen` for the three-minute rule, `forecast.runs_out/at_reset`,
  `caps`, per-row `refresh`. Its one invention, "Open project folder", carries the proposed tag on screen.
- Still and reduced-motion really stop everything (0 animations, `transition:none` too); no rAF loop, no timers beyond the prototype's tick.
- Light theme is a full token swap on `.app[data-theme]` (`--bg/--panel/--line/--ink/--teal/--warn/--neg`); a `[data-brand]` block
  drops in. Tool icons and the logo are the app's own SVGs inlined.
- Ring draws once on insertion (`drawRings` on the next frame, a 1.1 s dash-offset transition) and never on a patch: the "draw once,
  never on redraw" rule is in the code, not only in `angle.md`.
- Tray popup is the same `ladHTML`/`patchLad` code as the page ladder: one component for `popup.js`/`limits.html`.

**What fails**
- The `beat` keyframe on the top bar's live dot animates `box-shadow`, which is not compositable: 300 style recalcs and 230 ms of main
  thread per 5 s at 150 %, continuously, on every page (seen on Overview and Accounts at 1280 dark). Replace with a transform/opacity ring.
- Its render model is not the app's. `home.js` would carry a second path (patch by key) next to `core.js`'s `innerHTML` + `warmPage`
  cache, and the follow drawer must live outside the sheet and outside `#dialog-root` (today an open dialog sets `S.dirty` and defers the
  redraw; the drawer wants the opposite). Doable, but it is new infrastructure, not a restyle.
- The sidebar is gone: `index.html`, the shell in `app.css`, `core.js` (`nav.nav a[data-nav]`, `setFold`/`restoreFolds`, `drawPins`
  into `#nav-pins`, `setNavCounts` targets) and the Setup fold-opens-for-its-page rule all change; Setup and the gear become menus.
- At 1280×720 (dark and light) the board does not fit: ten session cards stack in three lanes and the board ends at 848 px while the
  window ends at 720; the ladder's eighth account ends at 688 px, so twelve accounts push past the fold too. The owner's "visible
  without scrolling" (rank 1) holds for the strip and the ladder at 8, not for the board.
- "Latest replies" on the Overview shows "≈ 155 t/s" in a Speed column (seen at 1920 dark); the brief's rule 2 is "No tokens-per-second
  anywhere on the Overview". One column to drop.
- No "limits as used" presentation: the ring shows "left" only; the app has the setting and e2e `appearance: ... limits as used` asserts it.
- Width/theme/still are read from one localStorage key and `st.follow`/`st.detail` are reset on load: fine for a prototype, but the
  app's settings live in `state.settings` (theme) and `moving_routes`; map, do not port.

**Must change before it could win**
- Make the board fit the fold at 1280×720: cap cards per lane with "+N more" or collapse idle sessions; show the ladder's twelve-account case.
- Move the live dot's pulse off `box-shadow`; drop Speed from the Overview table; add the used/left ring setting.
- Specify the drawer's place in the app's dialog model (an overlay that survives `render()` and never sets `S.dirty`).

## Design B (Codex) · score 8/10

**What works**
- Cheapest to build by a wide margin and the closest to the app's own model: `render()` replaces `#content`, then restores `scrollTop`
  and every `<details data-fold>` open state; selection, filters, range and query live in `S`. That is `core.js`'s `renderNow` plus the
  fold memory, already written. The sidebar stays (only `#nav-stats` and `#nav-limits` leave it), so `core.js` barely changes.
- The ledger is a `<table>` with fixed column widths: 8 rows in 565 px at 1280, 12 rows in about 770 px, no horizontal scroll at any
  width, and every row is the same four cells (`bar()` + reset, `bar()` + reset, run-out, state). Nothing to size, nothing to measure.
- Idle cost is nil (4 ms per 5 s at 150 %, no style recalcs); three transform animations; Still pauses them, reduced-motion removes them.
- Nothing invented: `status()` (Exhausted/Close/Ready/Not read) is `100 − used` on the tool's existing windows, `tool.quota` is
  `p.caps.quota`, "Resets first" is a sort on `resets_at`, the health counts are the same sort.
- The Add flow is one dialog with the app's own sequence (tool, kind with unsupported kinds disabled and explained, name, email, synced
  facts, CLI line) then a sign-in dialog with the pasted code and the email check: the five add/sign-in e2e tests map almost 1:1
  (`add-name`, `add-email`, `add-create`, `login-code`, `login-status`, `login-done`).
- Tokens: 14 role variables on `:root`/`[data-theme]`, no raw hex in the JS; a second brand is one more block.

**What fails**
- Rule 5 is claimed, not met: `angle.md` says "animations do not restart", but the live rail's travellers are rebuilt by every
  `render()` and restart at 0 (measured 6,138 → 50 ms after a filter click at 1280 dark). The "data tick" button only rewrites
  `#fresh`, which is not what the app does every few seconds. Fix is ten lines: a negative `animation-delay` from a page clock (as
  `home.js` does with `T0`) or keying the three rail rows.
- Reduced-motion leaks into the setting: on load `S.still` is `matchMedia(reduce)` OR the stored value, and `still(true)` then saves
  `'true'`; after the OS preference is cleared the prototype stays Still (seen at 1536 and 1920 dark: `travel:paused` with motion Live).
  In the app, keep `moving_routes` as the setting and check reduced-motion live, as today.
- Accounts with a daily cap and no windows (`d.caps`: routed OpenCode) have no row treatment: `status()` says "Not read" and both bars
  are dashed, although the cap is measured. One more `bar()` branch, but it is a design gap, not a port detail.
- The tray preview is a modal table of two percentages, not the ring design; `popup.js`/`limits.html` gets a different look from the
  window unless the ledger row is reused there.
- The tour is one modal with four paragraphs; e2e `a walkthrough on a page` drives `tour-next`/`tour-skip` steps, and the app's tour is
  per-element popovers. Either the test or the tour changes.
- `at()` hardcodes `timeZone:'Asia/Riyadh'` and the 1280 layout is a `.compact` class: both must become the app's locale and its
  container queries (small, but they are not copy-paste).

**Must change before it could win**
- Make the travellers survive `render()` (clock-based delay or keyed rows) and show it with a real page redraw, not the `#fresh` tick.
- Give capped (routed) accounts their own row state; reuse the ledger row in the tray popup.
- Keep the Still setting and the OS reduced-motion preference separate.

## Design C (OpenCode) · score 5/10

**What works**
- The Reset Horizon lane is a good instrument to build: per account one row of `ring | 5 h track | week track | next reset | Start`,
  plain divs with percentage `left`/`width`, beads as `translate3d` (composited; 4 ms idle at 150 %), Still hides them.
- Twelve-account and empty presets exist and render; the Add dialog's tool cards state the kinds each tool takes, and the kind cards say
  "Supported" per tool from a `kinds` matrix (the app has `caps`), so the confusion the owner named is addressed with existing data.
- Light theme is a full token block (`--bg-*`, `--ink-*`, `--line-*`, teal/amber/coral/violet with `-dim`/`-border` variants).
- The tray popup as a two-column ring grid reuses `renderRingSvg`; the year clock at 156 px with 371 circles is cheap.
- The one product change ("auto-pick expiring window") is tagged PROPOSED on screen and in `angle.md`.

**What fails**
- It is not buildable from the prototype: the views are template strings with the fixture written in. 35 baked strings in
  `views.js`/`modals.js`: the hero's "98% 5h & 100% week left" and "— 87% of 5h window resets in 1h 48m", "fable ≈ Tue 08:04",
  "12 live · 6 elsewhere" and "All 18", "$571.37" in the nav, "next reset 01:48" on every tool group, the whole By-model table, "Peak Sep 30
  (4.79B)", "53-wk total 142.8B", "10,756 today", the account colours of the stacked chart, and the axis ticks "Sun (≈ codex out)" at
  `left:62%` and "Tue 15:46" at `left:78%`. Proof on screen: the First Run state (1280 dark) shows four accounts, no sessions, and the
  top bar still says "12 live · 6 elsewhere · Limits read just now" with "$571.37" beside Overview. Every one of these must be designed
  as a rule before it can be coded; the estimate below includes that.
- The horizon's geometry is not one scale. `timeToPct` maps 0..5 h to 0..38 % and 5 h..7 d to 38..100 %, then the weekly zone is remapped
  to 42..85 %; and a bar's fill is `left% × gate position`, so the end of the solid bar is "percent left of the distance to the gate",
  not a time, while the dashed run-out beside it is a time. The instrument cannot be built until the mapping is fixed on paper.
- The render model is one `root.innerHTML` of the entire app (proto bar, rail, top bar, main, tray, modal) on every state change,
  including every keystroke (`updateField → renderApp()`). Typed "-xyz" into the Add dialog's name field at 1280 dark: the field kept
  "-" and focus went to `<body>` after the first character. The search box works only because `onSearch` re-focuses by hand. After
  "Simulate 3s Redraw" the focused button is gone and the beads restart (283 → 33 ms); the modal survives only because it is re-created
  from `STATE`. None of this ports; `home.js` would be written from the pictures.
- The prototype frame has no height: `.app-frame` is `min-height: calc(100vh − 33px)` and `main` is `overflow: visible`, so the page is
  1,516 px tall at 1280 and the browser scrolls it. The "above the fold at 1280" claim is unverified; measured against 720: the deck ends
  at 304 px, the first lane starts at 499 px, five of eight lanes are visible, twelve lanes end at 986 px.
- Theming: 247 inline `style=""` in the JS and 60 raw hex in the CSS outside the token blocks (`#3FD6CF`, `#7A9DFF`, `#9E8CFC`,
  `#57CAC9`, `#E5A93C` for chart legends and computer dots), 2,037 lines of CSS. VektraGrid means first moving those to tokens.
- Untagged inventions: "Speed (cadence)" on the Overview with "Steady stream / Fast burst / Low latency" is a classification of tok/s
  that no backend field gives and no threshold defines; the "Harvest ★" rule (5 h resets within 2 h and ≥ 75 % left) is a product rule;
  "OpenAI Direct / Anthropic Direct / Google Cloud" provider names per tool reintroduce the provider hop the app removed on purpose
  (`home.js`: "the vendor behind a tool is the tool, so there is no provider hop", and the e2e asserts no provider bodies).
- `angle.md` promises a "Now-line temporal heartbeat"; no keyframe exists for `.axis-tick.now` and `getAnimations()` lists only
  `pulseFlow`. Nothing to build, but the write-up and the prototype differ.

**Must change before it could win**
- Replace every baked string with a rule from the data and show the 12-account and first-run states computed, not pasted.
- Fix the horizon on one time scale (or admit two panels: a 5-hour strip and a week strip) and state what the solid bar's end means.
- Put the frame in a 720-tall scroller and show what is above the fold at 1280; move inline colours to tokens.

## Across the three
- The one thing a winning design here must have: state in JS and a DOM that is either patched by key (A) or rebuilt with scroll, folds,
  selection and animation phase restored (B), so the redraw every few seconds is invisible; proven with a real redraw, not a label update.
- The one thing it must not have: a view that only renders its fixture (C's baked copy and axis), or a shell rebuilt on every change.
- Parts worth combining:
  - A's keyed `patchBoard`/`patchLadder` and its follow drawer that outlives `render()`.
  - A's ladder row as the tray popup (one component for `popup.js`).
  - B's ledger table as the compact account list (Overview or tray), with a cap row added.
  - B's one-dialog Add flow with disabled-and-explained kinds, because the five add/sign-in e2e tests map onto it.
  - C's horizon lane (ring, two tracks, gate, Start) once its scale is defined, and C's tool/kind capability cards.
- Ranking: B > A > C. B is about a week on the app's own render model with two small fixes; A is about two weeks and a second render path
  but is the only one that proved its redraw story and invented nothing; C needs a design round before it can be estimated honestly.

## Cost table

Assumptions: one engineer who knows `app/ui`; backend untouched except reading fields it already returns (no Rust work); all four themes
(Harness and VektraGrid × light/dark) delivered by mapping each design's tokens onto `app.css`'s role variables; the tray popup reuses the
window's component; the e2e suite updated to pass; QA at 1920/100 %, 1536/125 %, 1280/150 % and macOS WebKit. A day is a focused day.

| Design | Engineer-days | Biggest risk | What the e2e tests lose |
|---|---|---|---|
| A (Claude) | 12 to 14: shell without sidebar 2, strip/board/ladder with keyed patching 2.5, drawer outside the dialog model 1, readouts/clock/attention/replies 0.5, Accounts 1, 3-step Add 1, first run 0.5, tray 0.5, themes 0.5, e2e 2, QA 1 | A second render path beside `core.js`'s `innerHTML`/`warmPage`, and the board overflowing 720 at 1280 with ten sessions | Rewritten: overview, sidebar (deleted: tabs and menus instead), accounts list, the three design-measurement tests (sidebar 256 px, top 52 px, nav heights), the five add/sign-in tests (wizard steps before the fields; CLI line format), tour, computers-Overview checks. Selector edits in about 12 more. The `nav-limits`/`nav-stats`/`routes` assertions are dead in every design. |
| B (Codex) | 7 to 9: sidebar trim 0.25, Overview (health, ledger, inspector, cards, rail) 1.5, folded lower sections 0.5, Accounts 0.75, Add + sign-in 0.5, detail dialog 0.5, first run 0.5, tray 0.25, themes 0.5, traveller phase 0.1, e2e 1.25, QA 0.75 | None technical beyond the traveller restart; the gap is capped accounts shown as "Not read" and a tray that does not look like the window | Rewritten: overview, the three design-measurement tests (sidebar 183 px, top 63 px, 32 px h1), tour (single modal vs steps), computers-Overview checks. Selector edits in about 13 (add/sign-in tests keep their sequence; sidebar fold test keeps its shape with `<details>`). |
| C (OpenCode) | 13 to 16, after a design round: rail 0.5, deck computed 1, horizon with a fixed scale 3, lower panels 0.75, Accounts 1, Add + code step 0.75, detail/request-log/computers/CLI/tour modals 1, first run 0.5, tray 0.5, tokenising 247 inline styles and 2,037 lines of CSS 1.5, e2e 2, QA 1.25 | The prototype does not define the product: baked fixture text throughout, an axis whose ticks are literals, bars that mix percent with time, and a whole-app re-render that drops focus on each keystroke | Rewritten: overview, the three design-measurement tests (rail 192 px), add/sign-in (one dialog then an OAuth-code step; CLI line format), tour (modal), computers-Overview checks; `speed` column assertions change (cadence words). Selector edits in about 14. |

Shared across all three: the overview test's dial block (`[data-testid=routes]`, `path.e`, `.lb` labels, the "accounts" ring text, the
380 px growth at 1800), the sidebar rings (`nav-limit-codex-main` "79%") and the "readouts in the sidebar" assertion go in every case; the
heatmap's 371 cells survive in each (A paths, B paths, C circles) but the `.hm` class and `data-l` levels must be kept or the assertion
rewritten; `finding`/`basis` survive as ids on A's "Runs out first" cell, B's inspector and C's exhaustion card.
