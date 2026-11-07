# Judge 2 (clarity): re-score, round 1

Same method as the first critique: the three revised prototypes opened from disk in headless Edge via playwright-core
(port 9452), 1280/1536/1920, Harness light and dark, Live and Still, 8 / 12 / first-run / empty states where the
page offers them, Accounts and Add account clicked through for every tool, Tab and Escape pressed, text contrast
computed from the rendered DOM, every screenshot looked at. Every "fixed now" in a rebuttal was checked on screen.
Owner facts applied: a Codex account has a 5-hour window as well as a week; the capability matrix is API key =
Claude Code, Codex, Pi; Routed = Claude Code, Pi; Antigravity, OpenCode, Oh My Pi = Subscription only.

## Design A (Claude) · score 9/10 (was 8)

**Must-change items now fixed (verified)**
- Rule 2: no tokens-per-second anywhere on the Overview. Latest replies and the in-place Request log show Time,
  Account, Model, Computer, Tokens, First reply only; the subtitle reads "first reply is approximate" (1280 and 1920).
- Contrast: zero text nodes under 4.5:1 in either theme on the Overview (worst dark 5.3:1, worst light 4.51:1 "last
  heard 26 m ago"). Strip labels, countdowns, legend, "Ctrl K" and the tab cost all pass.
- Capability matrix: step 1 tiles grey the kinds a tool does not take; step 2 says "Not for Codex" under Routed,
  "Not for Oh My Pi" under API key and Routed; OpenCode and Antigravity are Subscription only; Pi takes all three.
  Matches the owner's matrix for all six tools.
- First-run chrome zeroed: "0 live · 0 elsewhere", "1 computer", no cost on the Overview tab, empty Sessions dot
  grid, no Projects count, no Setup badge; the first-run Accounts page says "No limits read yet." with "limits read
  never" and dashed "—" rows.
- Ladder labelled: header "ring: week left", a WEEK caption under each ring, one window per row ("5-hour 98% left"
  or "Week · Opus 44% left") with its own ↻ countdown, the week reset date on the right, a legend under the ladder
  (ring and date: the week; bar: the window with the least room; amber ≈ runs out before its reset; red exhausted or
  logged out; — not read).
- Focus: Tab cycles inside the Add account dialog (12 Tabs stay inside, Close included); Escape returns focus to the
  "Add account" button; the follow drawer traps Tab and Escape returns focus to the card that opened it; the tray
  popup closes on Escape.

**Still open**
- Freshness text at 1280: the rebuttal says a short form "limits 14 m · usage now" shows at 1280. It does not: the
  top bar at 1280 has no freshness text in either theme (styles.css line 490 `.app[data-w="1280"] .fresh{display:none}`
  overrides the short/long rule at lines 108-110). Present at 1536 and 1920.
- Tool filter chips are icon-only at 1280 (names appear at 1536+); a stranger must hover to learn the Pi or
  Antigravity glyph. Designer disputes; the clarity lens still counts it.
- Add account is three steps (deferred by the designer to the next round).

**New defects the fixes introduced**
- The four-cards-per-lane cap hides the live ones: at 1280 DEVELOPMENTPC shows codex-main "wrote 2 m ago" first and
  folds two sessions behind "+2 more · 2 writing", so two of the three sessions writing right now are the ones off
  screen. The cap should keep writing sessions above idle ones (1280 and 1536, both themes).
- The ladder legend sits at the very foot of the ladder and is cut by the 1280×720 fold (only its first line shows);
  harmless, but the legend that explains the new ring/bar split is the part a first-time reader needs.

**Updated inventory misses**
- None lost. Checked on screen: shell items (freshness text missing only at 1280, above), tabs with counts, pinned
  projects caret, Setup menu, gear menu (Settings 1.8.4 available, CLI reference, Computers 3, Motion, "Rings show:
  left", Theme, version), readouts with range and dot-grid trends and deltas, ladder, tray popup with rings and
  resets and a Start, toast, finding with basis, exhausted-now branch (12 accounts: "Exhausted now · samykabu-work ·
  5-hour 100% used · back in 1 h 12 m"), tool and computer filters, year clock with busiest day and per-computer
  switch, Needs attention, Latest replies with count and Request log, Accounts page complete (rows, ⋯ menu with the
  seven items, advice, N open, detail drawer with four tabs), Add account with name → folder, optional email, API key
  + endpoint + model, route + model, kept-in-sync / its own / CLI disclosure, first run with found tools, sync
  question and empty state, 12-account state with the ladder scrolling inside itself.
- Honesty note: OpenCode and Antigravity now read "no limits · usage only" instead of "limits never read"; this
  matches the app's caps (no quota API) and is clearer than the dashed "—" of round 1.

## Design B (Codex) · score 9/10 (was 7)

**Must-change items now fixed (verified)**
- The finding is the headline: Overview "codex-main is out of its 5-hour window." with "Use service-account-resal
  now · codex-main 5-hour resets in 42m." under it; Accounts "codex-main is out; use service-account-resal." with
  "5-hour exhausted · 98% 5-hour / 100% week room on service-account-resal"; first run "No accounts connected."
  Valid under the owner's fact that Codex has a 5-hour window.
- Type floor: reset countdowns and state labels are 11 px; smallest text on the Overview is 10 px (was 8); zero
  contrast failures in either theme (worst light 5.0:1).
- "—" vs "None": the run-out column shows "None" for measured accounts with no run-out before reset, "—" only for
  not read, "No forecast" for a daily cap, "n/a" for pay-per-use; the legend line says "None: no run-out before
  reset".
- Rings are back: a ring on every Accounts row (header says "rings: % left (week / daily cap)") and on every tray
  row; the tray shows the tool icon, both windows with their resets and a Start per account.
- "Running now" names the account under each project ("HarnessManager · claude-main · writing", "Bunyan · codex-main
  · waiting") under each named computer, each line its own Follow.
- Tool chip names are visible at 1280 without hover.
- A 12-account state is on the Data switch; it adds a daily-cap routed row ("Daily cap: $13.80 left / $25.00 ·
  55.2% · resets in 8h 18m · Capped") and a pay-per-use API-key row ("Pay per use · no plan windows · n/a").
- Room / Used: a "Show: Room / Used" select sits on the ledger footer.
- Start gives a confirmation toast ("Launch preview: service-account-resal · recent folder HarnessManager. No terminal
  opened.") on one click; Options… keeps the folder/arguments dialog.
- Capability matrix matches the owner's: Routed greyed "Not supported by Codex"; API key and Routed greyed for
  OpenCode, Antigravity and Oh My Pi; Pi takes all three.
- First run: zeroed chrome ("$0", "0 live · 0 elsewhere", "1 computer", "Limits not read"), "Add your first
  account", "Preview detected tools" → "4 installed tools found" → "Use detected profiles →" → the sync dialog with
  "Not now" selected first and no folder prefilled.

**Still open**
- The inspector (measured %, reset, read-ago, ≈ run-out, week elapsed, route, first reply) is still the one rank-1
  basis that sits below the 1280 fold; the headline now carries enough that this is a nit, not a defect.
- Needs attention, Activity and Latest replies remain closed disclosures (designer disputes; allowed by the brief for
  rank 3, and the page is readable without them).

**New defects the fixes introduced**
- The readouts lost their dot-grid trends (deliberately, to stop encoding hashes); the owner named the dot-grid
  trends as something they still love. Allowed by inventory.md ("free to move or drop"), but it is the one owner
  favourite B no longer shows anywhere.
- Accounts rows show the week ring but the Overview headline is about the 5-hour window; codex-main's ring reads 27%
  while its headline says it is out. The row's own bars make it clear, but the ring and the headline disagree at a
  glance (1280, both themes).

**Updated inventory misses**
- None lost. Checked on screen: shell items including freshness, ? tour (now a positioned four-step tour with Skip /
  Next), refresh; nav with counts and pinned fold; Setup fold; readouts with range and deltas (no dot grids, above);
  ledger legend; tray; toast; finding and basis; tool and computer filters; Use now with Start and Options…;
  Resets first; Running now with accounts; year clock with busiest cell outlined and per-computer select; Needs
  attention; Latest replies with count and Request log (speed only there); Accounts finding, 7/14/30 d, Export CSV,
  Start in a folder…, Add account, tokens-per-day fold with legend and scale, tool chips, groups, rows with ring,
  every window and reset, data-bound sparkline, tokens, cost, read-ago, refresh, Start, ⋯, advice, N open; By model;
  detail dialog with four tabs and the full menu; main profile hide-only; Add account complete with the correct
  matrix; first run, detected tools and sync; 12 accounts.

## Design C (OpenCode) · score 7/10 (was 6)

**Must-change items now fixed (verified)**
- Focus: opening Add account moves focus to the first tool tile; Tab cycles inside the dialog (tiles → kinds → name →
  email → Cancel → Create → Close → tiles); clicking a tile keeps focus on it; typing in the name field keeps focus;
  Escape closes and returns focus to "+ Add account"; the detail dialog and the follow drawer behave the same;
  Simulate 3s Redraw keeps the focused element.
- Light-theme contrast: zero failures (worst 4.92:1 "v1.8.3"); dark: zero failures.
- First run and empty state zeroed: "0 live · 0 elsewhere", "1 computer", "Limits not read", "$0.00", Sessions 0,
  Projects 0; first run lists the four main profiles with "— limits not read" and the sync question with "Not now"
  first.
- Needs attention "5 open" lists five items; the 'Test "nothing"' control is gone; the "Load Owner's 8 Accounts Demo"
  button is gone from the empty state.
- Latest replies on the Overview has no speed or cadence column; speed lives only in the Full Request Log dialog.
- The runway is now two instruments: a Capacity column (5h and Wk bars, % left, zero-based) and a linear 7-day
  timeline (NOW · Sat +1d … +7d) with reset gates and the ≈ run-out span only where a run-out precedes the reset.
- "Running now" has named per-computer pills (DEVELOPMENTPC 12 · SAMYMSI 4 · Samys-MacBook-Pro 2) and Follow opens a
  session drawer with account windows and the live tail.
- The Proposed tag is quiet and self-describing ("Proposed: Prefer expiring 5h"); the hero labels are plain ("Use
  next · Reset runway", "Runs out first", "Running now"); the Accounts daily chart opens folded.

**Still open**
- Capability matrix still wrong against the owner's fact on three of six tools: Codex "Supports: Subscription · API
  key · Routed" with Routed "Supported ✓"; OpenCode "Subscription · Routed" with Routed "Supported ✓"; Oh My Pi
  "Subscription · Routed". Routed is Claude Code and Pi only. The rebuttal applied the coordinator's earlier note;
  the owner has since overridden it. Repeated on the empty state's six tool cards.
- "Running now" at 1280 still shows 3 of 18 sessions and truncates the computer ("HarnessManager DEVELOPMENTPC…",
  "edge-gateway SAMYMSI · samyka…"); the other fifteen are behind the pills. Half of "what is running, on which
  computer" needs a click at the laptop width.
- Density: 100 text nodes under 11 px on the Overview, the smallest 8.5 px (the "≈ Out Thu 03:38 (−1 d 2 h)" labels
  on the timeline, which are the forecast the owner asked for).

**New defects the fixes introduced**
- Timeline header overlap at 1280: "NOW 15:42" collides with "Sat (+1d)" (reads "NOW 15:4Sat (+1d)"), both themes.
- The routed lane's text overlaps itself: at 1280 "Routed via cheap-fast (DeepSeek V4 → OpenRouter) · daily cap
  resets in 8 h 18 m" is drawn under "Cap ↻ 8 h 18 m" in the same cell (visible at 1280 and 1920, dark and light).
- The exhausted-now branch is correct in words but the hero's second card in the 12-account state wraps "0% 5-hour
  left, resets in 42 m" across three lines at 1280 and pushes the four chips to the card's foot; minor.

**Updated inventory misses**
- Capability matrix (above) is the only item that is wrong rather than missing.
- Not seen on screen: the "Rings: % left" toggle's "used" state was not clicked this round; the detail dialog's
  Sessions / Projects / Settings tabs were not opened; the main-profile hide-only rule was not checked.
- Present and confirmed: everything listed in the first critique plus the per-computer Running-now pills, the follow
  drawer, the Full Request Log (Input, Output, Cache Read, Speed, First Reply, Cost, right-aligned), the 12-account
  state with an exhausted account and a tray popup that scrolls to show all accounts, Export CSV with a notice bar
  and Dismiss, the Codex update item in Needs attention.

## Across the three (updated)
- The one thing a winning design here must have: the owner's three answers as readable statements above the 1280
  fold with the finding as the headline, measured / ≈ / — told apart without a legend lookup, and the running
  sessions naming computer and account on screen. A and B both do this now; C does the first two.
- The one thing it must not have: a capability matrix that contradicts the owner's (only C still has one), speed on
  the Overview (none of the three any more), or a live session hidden behind a "+N more" while an idle one shows.
- Parts worth combining:
  - B's headline pair (the exception as H1, the next move as the line under it) on A's answer strip.
  - B's Add account dialog (one screen, greyed "Not supported by <tool>") in place of A's three steps.
  - A's board with one lane per computer and the "here" tag, with writing sessions sorted first.
  - A's ladder legend and "Rings show: left / used" in the gear, and A's readouts with the dot-grid trends B dropped.
  - C's Full Request Log columns and its follow drawer with the live tail.
- Ranking: A ≥ B > C. A and B are both shippable bases after small fixes; A edges B on "what is running, where" (a
  board instead of a six-line rail) and keeps the owner's dot grids, B edges A on the Add account flow, the headline
  and the 1280 freshness text. C fixed its keyboard, contrast and first-run defects but still states the wrong
  matrix for three tools, hides fifteen of eighteen sessions at 1280 and overlaps its own timeline labels.
