# Judge 2 (clarity): critique, round 1

Method: each prototype opened from disk in headless Edge via playwright-core at 1280/1536/1920, Harness light and dark,
Live and Still, 8 accounts and first run; Accounts and Add account clicked through; Tab and Escape pressed; text contrast
computed from the rendered DOM; every screenshot looked at. The tool capability matrix was checked against the app's
own `Caps` (crates/core/src/harness/*.rs): Claude Code = Subscription/API key/Routed, Codex = Subscription/API key,
Pi = Subscription/API key/Routed, OpenCode, Antigravity and Oh My Pi = Subscription only.

## Design A (Claude) · score 8/10
**What works**
- The answer strip reads the owner's sentence left to right in under five seconds at 1280 dark: "Running now 10
  sessions · 6 here · 4 elsewhere", "Runs out first codex-main · Week 73% used · ≈ out Sun 00:56 · resets Tue 15:46 ·
  read 14 m ago", "Use next service-account-resal" with the Start button inside the cell. Measured, ≈ and read-ago all
  sit in the sentence.
- The board answers "what is running, on which computer" better than the other two: one lane per computer, a card per
  session with tool icon, account, project, model, elapsed, tokens, cost and "wrote 4 s ago"; this computer is tagged
  "here", the MacBook lane is dimmed with "last heard 26 m ago". Every rank-1 job is above the fold at 1280×720.
- The ladder on the right is honest at a glance: solid teal rings for measured accounts, amber ring and bar for the
  one running out (codex-main 27%), dashed grey ring with "—" and a "Read now" button for opencode-main and agy-main,
  "login expired" in red; a one-line legend "measured · ≈ at today's pace · — not read" at its foot.
- The follow drawer (click a card) shows the session's account limits, latest replies and first reply; Escape closes
  it and the tray popup; the single "proposed" item (Open project folder) carries a visible PROPOSED tag.
- Navigation is one row of tabs with the counts the inventory asks for (Overview $571, Sessions dot grid, Projects 23
  with a pinned-projects caret, Accounts 8, Setup 1 update with a menu of MCP servers 11 / Skills 255 / System prompts /
  API keys & routing / Apps); CLI reference, Settings (1.8.4 available), Computers 3 and the version live in the gear.
- First run is readable by a stranger: "Two tools found on DEVELOPMENTPC", the main profiles listed with Sign in, the
  sync question as three cards, an empty "Running now" lane that says what will appear there; "No AI coding tool
  found" names the six tools and offers to prepare an account anyway.

**What fails**
- Rule 2 broken: Latest replies on the Overview shows "≈ 155 t/s" in a Speed column at every width (1280 dark, after
  one scroll; 1920 dark, on screen without scrolling), and the in-place Request log keeps the column. The brief says
  "No tokens-per-second anywhere on the Overview."
- Contrast: 23 small texts fail AA in dark (the strip labels "Running now / Runs out first / Use next", the ladder's
  reset countdowns "4 h 08 m … 22 m", the legend: 3.5:1) and 32 in light ("Ctrl K" 2.6:1, the Overview tab's "$571"
  2.8:1, the same labels 3.0:1). Seen at 1280, both themes.
- The Add account tiles state the wrong capability matrix: OpenCode and Oh My Pi show API key and Routed as available
  (teal), Pi shows all three; the app's caps say OpenCode and Oh My Pi take Subscription only. A stranger would pick
  "Routed" for OpenCode and fail later. (1280 dark, Accounts › Add account step 1.)
- First-run chrome is stale: with 0 accounts the Overview tab still says "$571", Projects "23", Sessions shows five
  live dots, the top bar "3 computers" and Setup "1 update". A new user's first screen contradicts itself (1280 dark,
  both first-run states).
- The ladder mixes windows without saying so: its header says "Room · week left", the ring is the week, but every
  row's bar and countdown are the 5-hour window ("5-hour 98% left … 4 h 08 m") while the date on the right is the
  week reset. Two windows, one row, no label on the ring (1280 and 1920, both themes).
- Keyboard: inside the Add account dialog, Tab leaves the dialog after "Next" and lands on the page chrome; after
  Escape, focus goes to body rather than back to the Add account button. Tab from the top of the page visits 13
  controls before the first nav tab.
- At 1280 the top bar drops the freshness text ("Limits read 14 m ago · usage just now" is only shown at 1536+); it
  survives only as "read 14 m ago" inside two cells.

**Must change before it could win**
- Remove tokens-per-second from the Overview (Latest replies and the in-place log); keep it in a separate Request log.
- Raise the strip labels, countdowns, legend, "Ctrl K" and tab cost to 4.5:1 in both themes.
- Take the tile matrix from the app's caps (OpenCode, Antigravity, Oh My Pi: Subscription only; Codex: no Routed).
- Zero the chrome on first run (cost, project count, live dots, computers, updates) or hide those counts.
- Label the ladder row's ring and bar (or show one window per row) and trap focus in the dialog, restoring it on Escape.

## Design B (Codex) · score 7/10
**What works**
- Honesty is the best of the three and needs no reading: solid bars are measured, the run-out column carries ≈,
  unread accounts get a dashed outline and "—", exhausted gets a vermilion outline, and a legend "Measured · ≈
  Estimated · — Not read" sits under the ledger at every width. Speed appears only in the Request log dialog, never on
  the Overview.
- Every rank-1 job is above the fold at 1280×720: the four counters (01 Exhausted · 02 Close · 03 Ready · 02 Not
  read), the ledger with 5-hour and weekly room and each reset under its bar, "Use now" with Start, "Resets first 18m
  · sabushanab-resal", "Running now 6 · 3 here" with the three computers.
- The Add account dialog is the clearest of the three: "01 / choose your tool" (Installed · Prepare · not installed),
  "02 / how will it connect?" where an unsupported kind stays visible, greyed and says "Not supported by OpenCode",
  "03 / give it its own space" with the folder path under the name, "Email · optional · checked after sign-in"; the
  matrix matches the app's caps exactly, including Pi and Oh My Pi.
- Accessibility is solid: Tab walks search, status, nav, pinned projects, Setup, filters, then the ledger; the dialog
  traps focus and opens on Close; Escape closes it and returns focus to "+ Add account"; every text passes AA in
  both themes (worst 4.9:1 light); Still pauses all three animations; reduced motion gives zero.
- First run is self-consistent: "$0", "0 live · 0 elsewhere", "1 computer", "Limits not read"; "Prepare your first
  account", "Install a tool", "Preview detected tools" and "Connect your other computers" (the sync dialog with
  shared folder / hub / not now) are each one click.
- The account detail dialog and its Settings tab hold the whole row menu (Open folder, Rename, Profile settings,
  Sync now, Show window, Remove from list, Remove with folder) and the four tabs the inventory lists.

**What fails**
- The finding is demoted to a slogan: the Overview headline is "Room to keep going.", Accounts is "Keep your options
  open.", first run is "Make room for your work." The sentence the owner asked for ("codex-main … runs out … before it
  resets") exists only as a 10 px line in the inspector under the ledger, at the very bottom of the 1280 fold.
- The rank-1 "when each limit resets" is the smallest type on the page: reset countdowns are 8 px, the state column
  and legend 9 px, 86 texts under 11 px at 1280 (a laptop at 150% still renders them at 12 physical px). Seen at
  1280 dark and light.
- "—" means two things in the run-out column: "Not read" (per the legend) and "no run-out expected" (service-
  account-resal, samykabu-teksa, samykabu-personal, all measured). A lazy user cannot tell a healthy account from an
  unread one by that column (1280 and 1920, both themes).
- The limit rings are gone from where the owner loves them: no ring on any Accounts row (only one in the Overview
  inspector and the detail dialog), and the tray preview is a plain table of percentages with no ring, no tool icon
  and no reset time.
- "Running now" on the Overview names projects per computer but not the account each session burns ("3 account
  routes · Follow ↗"); the account appears only after a click in the live dialog. Half of "what is running, on which
  computer, follow one" is one click away rather than on screen.
- The tool filter chips are icons only at 1280 and 1536 (names appear on hover or when active); a stranger who does
  not know the Pi or Antigravity glyph cannot filter without hovering.
- No 12-account state on the page's switches (angle.md says it was verified; the committee cannot see it).

**Must change before it could win**
- Put the finding sentence back as the Overview and Accounts headline; keep the slogans out.
- Raise reset countdowns, state and legend to 11 px or more; nothing on the Overview under 10 px.
- Use a different mark for "no run-out expected" than for "not read" (blank or "none", not "—").
- Return a ring to each Accounts row and to the tray popup, with the reset time.
- Name the account on each Running-now line; show chip labels at 1280.

## Design C (OpenCode) · score 6/10
**What works**
- The three hero cards do answer the sentence above the fold at 1280: "1 · Best account to use now · Start on
  service-account-resal — 98% 5h & 100% week left" with the Start button, "2 · Exhaustion forecast · codex-main will
  likely run out of its week ≈ Sun 00:56, 2 d 14 h before reset · Measured 73% used … read 18s ago", "3 · Running now
  · 12 live · 6 elsewhere" with Follow buttons.
- The Accounts page is the most complete of the three and keeps the finding as its heading: ring, email, plan chip,
  added date, every window with % left and reset, a 7-day sparkline, tokens, cost at list, read-ago, refresh, Start,
  ⋯ (Open folder, Rename, Profile settings & cost cap, Sync now, Show window, Remove from list, Remove with folder), an
  advice line and open count per row; group summaries; tokens-per-day chart; a By model link; a detail dialog with
  four tabs.
- The "Rings: % left" toggle is the only implementation of the inventory's "what is left (or what is used, a
  setting)"; the tray popup (a button in the nav) shows all eight rings with tool icons and reset times.
- The Request log dialog is a real log: input, output, cache read, cost ≈, first reply, every numeric column right-
  aligned.
- Escape behaves as the inventory asks: it closes a modal, then the tray, then returns the selected route to the
  codex-main finding.

**What fails**
- Keyboard: dialogs never take focus. After "Add account", focus stays on body and Tab walks the prototype bar and the
  nav instead of the dialog; every click re-renders the page and drops focus to body (after choosing a tool tile, Tab
  starts again from the top). Escape works, but a keyboard user cannot complete Add account without tabbing the whole
  page each step (1280 dark).
- Light theme contrast: 20 texts fail AA, including the rank-1 forecast text "27% left · ≈ Sun 00:56" and "fable ≈
  Tue 08:04" (amber on light, 4.0:1), the "1 update" badges (3.6:1), the axis tick "Sun (≈ codex out)" (3.7:1) and
  "v1.8.3" (3.1:1). Dark fails only the version and "tray active" (3.95:1). Seen at 1280.
- The Add account matrix is wrong for four of six tools and says so in words: OpenCode "Supports: Subscription · API
  key · Routed", Antigravity "Subscription · API key", Pi "API key · Routed" (no Subscription), Oh My Pi "API key ·
  Routed"; every kind card for OpenCode reads "Supported ✓". The app's caps disagree on all four. The empty state
  repeats the same wrong list under each tool.
- First run and the empty state keep live data: with 0 accounts the top bar still says "12 live · 6 elsewhere",
  "3 computers", the Overview tab "$571.37", Sessions "18", Projects "23" (1280 dark, First Run and Empty (0)).
- The Reset Horizon needs a paragraph: its axis is non-linear ("NOW 15:42 · +1.5h · +5h Window · Sat (+24h) · Sun (≈
  codex out) · Tue 15:46 · +7d Week"), two windows share one lane, and the dense monospace labels ("5h ↻ 4 h 08 m",
  "Wk ↻ 4 d 7 h", "≈ out Thu 0…") are the "too many monospace numbers everywhere" the owner ticked. The page's own tour
  spends its second step explaining it.
- "Running now" at 1280 shows 3 of 18 sessions and truncates the computer ("codex-main · DEV-P…", "service-account-
  resa…"); which computer each session is on is cut off, and the other 15 are behind "All 18". The weakest answer to
  "what is running, on which computer" of the three.
- Honesty slips: the Latest replies "Speed (cadence)" column replaces a number with invented labels ("Steady stream",
  "Fast burst", "Low latency") that are neither measured nor marked ≈; "Needs attention · 5 open" lists four items; a
  dev control 'Test "nothing"' sits in the panel header; the hero card wears a "PROPOSED · MODE: MOST ROOM + NEXT
  RESET" tag whose meaning is not on the screen (what it proposes is only in angle.md).
- Every click re-renders the whole page from innerHTML (the theme switch visibly lags a frame); the brief's rule 5
  (redraw every few seconds without losing state) is at risk for hover, focus and scroll, which is what this judge
  saw with focus.

**Must change before it could win**
- Move focus into every dialog, trap it, and restore it after Escape; stop dropping focus on each re-render.
- Fix light-theme amber and muted text to 4.5:1.
- Take the capability text from the app's caps and grey the unsupported kinds instead of "Supported ✓".
- Zero the live counts, cost and computers on first run and empty; fix the attention count; remove the test button.
- Give the runway a linear axis or split it into a 5-hour row and a week row; replace the cadence labels with nothing.

## Across the three
- The one thing a winning design here must have: the owner's three answers as three readable statements above the
  fold at 1280 with measured / ≈ / — told apart without a legend lookup, and the session cards saying computer + account.
- The one thing it must not have: tokens-per-second, a slogan where the finding should be, or a capability matrix
  the app's own caps contradict.
- Parts worth combining:
  - A's answer strip (Running now · Runs out first · Use next with Start) and the board with one lane per computer.
  - B's Add account dialog (01/02/03, "Not supported by <tool>", folder path under the name) and its focus handling.
  - B's four counters (Exhausted · Close · Ready · Not read) and its legend line under the ledger.
  - C's Accounts page rows and ⋯ menu, its "Rings: % left" toggle, and its tray popup with rings and reset times.
  - C's Request log columns (input, output, cache read, cost ≈, first reply, right-aligned).
- Ranking: A > B > C. A answers the sentence and "what is running where" fastest and is complete, but needs a round
  for Rule 2, contrast and the matrix; B is the most honest and accessible but hides the finding behind slogans and
  8 px type; C is the most complete on Accounts and the least usable by keyboard, with a hero that needs explaining.

## Inventory check
Checked line by line against inventory.md on the rendered pages (not angle.md). "Hidden" is fine; "missing" or
"wrong" is the finding.

### Design A (Claude)
- Missing from the Overview: nothing from the inventory. Hidden one click away: Setup items (menu), CLI reference /
  Settings / version / Computers (gear), readouts, year clock, Needs attention and Latest replies (one scroll).
- Wrong: Latest replies and the in-place Request log show speed as "≈ N t/s" on the Overview (Rule 2).
- Wrong: Add account step-1 tiles mark API key and Routed available for OpenCode and Oh My Pi (app caps: Subscription
  only).
- Not seen on screen: the "what is used" ring setting (rings show left only; angle.md does not mention it); the
  computers chip opening Settings › Computers (the gear has "Computers 3"; the chip was not tested); main-profile
  hide-only rule (the ⋯ menu was opened on a non-main account).
- Freshness text absent from the top bar at 1280 (present at 1536 and 1920; "read 14 m ago" survives in two cells).
- Present and confirmed: brand + computer + account count; Ctrl K; N live · M elsewhere (click scrolls to the board);
  computers chip; ? tour and refresh; tabs with counts; pinned projects caret with live counts; readouts with range,
  dot-grid trends and deltas; ladder rings with bar and reset; grey dashed "—" ring with Read now; tray popup; toast
  ("Limits are read after the first sign-in", "Sync set up"); finding with basis; tool chips + All computers / This
  computer; Start <account>; year clock with per-computer switch and busiest day; Needs attention with time and
  reason; Latest replies with count and Request log switch; Accounts finding and basis, 7/14/30 d, Export CSV,
  Start in a folder…, Add account, tokens-per-day fold, tool chips with sort note, tool groups with logged in /
  consoles / next reset / tokens / cost, full rows (ring, name, label, email, plan, added, every window with % and
  reset, sparkline, tokens, cost, read-ago, refresh, Start, ⋯ with the seven items, advice, N open), By model fold,
  detail drawer with four tabs; Add account with tool, kind (greyed "Not for Codex"), name with folder, email checked
  after login, API key + endpoint + model, route from the routing page + model, kept-in-sync / its own / CLI line in a
  disclosure, Create and log in; first run with tools found, main profiles, sync question, empty state.

### Design B (Codex)
- Missing from Accounts rows: the ring per account (ring exists only in the Overview inspector and the detail dialog).
- Missing from the tray popup: rings, tool icons and reset times (a table of 5-hour and week percentages only).
- Missing from the Overview finding: the sentence as a headline (slogan instead); the finding text is a 10 px line in
  the inspector.
- Missing from "Running now": the account per session (projects and computer only; account is in the live dialog).
- Ambiguous: "—" in the run-out column for measured accounts with no forecast, same glyph as "Not read".
- Not seen on screen: the "room / used" presentation setting (claimed in the Settings placeholder); a 12-account
  state (no switch); a toast after an action (role=status exists in the DOM; none was seen).
- Hidden one click away: Needs attention, Activity and Latest replies are closed disclosures below the ledger;
  Request log is a dialog; the full live list is a dialog; Setup items are a closed fold.
- Present and confirmed: shell items; nav with counts and pinned fold; readouts with range, trends and deltas;
  finding with basis in the inspector (measured, read 2m ago, ≈ run-out, pace basis, week elapsed, route, first
  reply); tool and computer filters; Use now with Start (dialog with account, recent folder, extra arguments, skip
  permission prompts); year clock with busiest day and per-computer select; Needs attention with five kinds and an
  empty text; Latest replies with count and "first reply approximate", request log with speed only there; Accounts
  finding callout, 7/14/30 d, Export CSV, Start in a folder…, Add account, tokens-per-day fold, tool chips, groups
  with logged in / consoles / next reset / tokens / cost, rows with every window and reset, sparkline, tokens, cost,
  read-ago, refresh, Start, ⋯, advice, N open; By model fold; detail dialog with four tabs and the full row menu;
  main profile marked; Add account with the correct matrix, name with folder, optional email checked after sign-in,
  API key + endpoint + model, route + model, synced / own facts and CLI line, "Create and log in →" then a simulated
  login step; first run (empty), detected-tools variant and the sync dialog with shared folder / hub / not now.

### Design C (OpenCode)
- Wrong: the capability text under each tool (OpenCode, Antigravity, Pi, Oh My Pi) and the "Supported ✓" badges
  contradict the app's caps; repeated in the empty state's tool cards.
- Wrong: Latest replies "Speed (cadence)" uses invented labels in place of a measured number (not marked ≈ or
  proposed).
- Wrong: first run and empty state keep "12 live · 6 elsewhere", "3 computers", "$571.37", "Sessions 18",
  "Projects 23" with 0 accounts.
- Inconsistent: "Needs attention · 5 open" with four items; a 'Test "nothing"' control in the panel.
- Not fully on screen at 1280: Running now shows 3 of 18 sessions with the computer truncated; the rest is behind
  "All 18".
- "Proposed" tag present on the hero but its meaning (auto-pick an expiring window when starting) is not readable
  from the screen; the toggle it refers to was not found on the Overview.
- Not seen on screen: main-profile hide-only rule (the ⋯ menu was opened on a non-main account); the detail tabs
  Sessions / Projects / Settings were not opened.
- Present and confirmed: shell items including freshness text, ? tour and refresh, notice bar (CSV export) with
  Dismiss; nav with counts, pinned projects fold, Setup fold with counts and "1 update", CLI reference, Settings dot,
  version, a Tray limits popup button; three hero cards; runway with legend pills, 5h+Week / 5-hour / Weekly views,
  Rings % left toggle, tool and computer filters, Start per lane, selected route bar with first reply and Escape
  reset; readouts with range, dot-grid trends and deltas; Needs attention with reasons and times; year clock with
  busiest day and per-computer dots; Latest replies with count and a Request log dialog; Accounts page complete
  (see "What works"); Add account with tool, kind, name with folder, expected email, API key + endpoint + model,
  route (routed), kept-in-sync / isolated facts, CLI line, "Create & sign in (browser code)…" for Claude and
  "(terminal)…" for others; first run with four detected main profiles, Start per profile, sync choices (shared
  folder / self-hosted hub / not now), "Add another account"; empty state with six tool cards and "Add first
  account"; a 12-account state that grows the runway and the Accounts list vertically.
