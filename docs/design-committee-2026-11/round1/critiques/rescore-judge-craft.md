# Judge 1, craft: re-score, round 1

Same method as the first critique: headless Edge 156 through playwright-core (Edge refuses `--remote-debugging-port`
on this machine, so the same msedge.exe is driven over the pipe transport), 1280×720, 1536×864 and 1920×1080, light
and dark, Live and Still (two frames 1.3 s apart compared byte for byte, plus a count of running animations), 8 and
12 accounts, tray, Accounts, Add account with Claude Code and Codex chosen, first run. Every "fixed now" below was
checked on screen, not read from the rebuttal. The owner's two facts applied: Codex has a 5-hour window, and the
capability matrix is API key = Claude Code, Codex, Pi; Routed = Claude Code, Pi; Antigravity, OpenCode, Oh My Pi =
subscription only.

## Design A (Claude) · score 9/10 (was 8)

**Must-change items now fixed (seen)**
- Ring and bar on one ladder row are now named: "WEEK" caption under the ring, "5-hour 98% left" on the bar line,
  the panel header "ring: week left", a four-line legend at the ladder's foot. No contradiction left at a glance.
- The second "Running now · 10 of 10" heading is gone; the board header is the filter row only.
- First run: no paragraph; the empty lane sits above the "Two tools found" panel; shell counts are honest ("0 live ·
  0 elsewhere", "1 computer", "Limits not read yet", no cost on the tab).
- t/s is gone from the Overview's Latest replies (Time, Account, Model, Computer, Tokens, First reply).
- Strip cells no longer break mid-name at 1280 ("sabushanab-resal's 5-hour resets in 22 m" wraps at a word; Start
  shortens to "Start").
- Lane headers and ladder names no longer truncate at 1280 ("Samys-MacBook-Pro", "service-account-resal" whole;
  "last heard 26 m ago" on its own line).
- The keyboard legend left the Add account footers; Email is "optional"; step 2 greys Routed with "Not for Codex".
- Capability tiles match the owner's matrix exactly (Claude Code and Pi: three kinds; Codex: Subscription · API
  key; OpenCode, Antigravity, Oh My Pi: Subscription only).
- 12-account state exists: "EXHAUSTED NOW · samykabu-work · 5-hour 100% used · back in 1 h 12 m" in red takes the
  second cell, the ladder scrolls inside its panel.
- Still: zero running animations (the write pulses and heartbeat stop).

**Still open**
- 1920, both themes: the board's void is smaller (lanes capped at four cards, "+2 more · 2 writing") but still
  there; SAMYMSI ends at ~580 px and the MacBook lane at ~430 px beside a 760 px ladder. The rebuttal defers it.
- Freshness text at 1280: the rebuttal says a short form ("limits 14 m · usage now") shows; on screen the top bar at
  1280 (light and dark) has no freshness text at all. `styles.css` line 490 `.app[data-w="1280"] .fresh{display:none}`
  overrides the short/long rule at lines 108–110. Not fixed.

**New defects the fixes introduced**
- The "WEEK" caption under every ladder and tray ring is 9 px, the only text under 10 px on the page (DOM scan).
- At 1280×720 the ladder's new legend sits at 700–720 px and is cut by the fold ("ring and date: the week / bar:
  the window with the least room" half visible).
- In Still the frame is not pixel-still: relative times ("wrote 4 s ago" → "5 s ago") keep ticking. That is content,
  not motion, and acceptable; noted so nobody reads the byte test as a failure.

## Design B (Codex) · score 8/10 (was 7)

**Must-change items now fixed (seen)**
- The slogans are gone. Overview H1: "codex-main is out of its 5-hour window." with "Use service-account-resal now ·
  codex-main 5-hour resets in 42m." beneath; Accounts H1: "codex-main is out; use service-account-resal."; first
  run: "No accounts connected." The biggest type is now the answer.
- Nav icons are one 16 px stroke set (Overview, Sessions, Projects, Accounts, CLI reference, Settings); the pawn and
  command glyphs are gone.
- No text under 10 px at 1280 (DOM scan: zero); State and reset labels read at arm's length.
- Rings are back on every Accounts row and in the tray (drawn like the inspector's: 27% amber, 8% amber, 100% teal,
  dashed "—" for not read); the tray shows tool icon, 5-hour and week with resets, and a Start per row.
- Add account fits 720 at 1280: tool tiles, kind cards, name, email, the shares disclosure, the CLI line and the
  footer all visible without scrolling. Codex chosen greys Routed with "Not supported by Codex"; data matches the
  owner's matrix.
- The leading-zero counters are gone; the State column carries the words.
- Sparklines now vary by account; the stacked chart has a legend.
- Still: zero running animations; Live has the route travellers and "4 writing" with writing/waiting on each row.

**Still open**
- Live is still faint: two 9 px dashes sliding on 27 px lines and the word "writing" are all that moves or glows;
  the hero itself (ledger, Use now card) has no live signal. Precise, yes; "live" only on close reading.
- The top bar keeps Unicode glyphs (⌕ in the search box, ? and ↻ as buttons) beside the new SVG nav set, and
  "Ctrl K" is the one monospace string on the page.
- The tray popup is now the right content but still lives in a generic 850 px modal titled "Harness / capacity in
  the tray" with "Sample popup" and "Sample data" notes; it does not look like a tray popup.

**New defects the fixes introduced**
- Eyebrow labels in the "OVERVIEW / ACCOUNT CAPACITY", "ACCOUNTS / YOUR LOGINS, SEPARATE", "WELCOME / A CLEAR
  STARTING POINT" style: the slogan moved from the H1 to the eyebrow. Small, but it is the pattern the owner ticked,
  shrunk.
- The run-out column mixes a dashed-underlined "≈ Weekly / 1d 7h" with plain "None"; the underline reads as a link.

## Design C (OpenCode) · score 7/10 (was 5)

**Must-change items now fixed (seen)**
- Density halved on the fold: one visible Start (the hero's) plus Folder…; the lane Start buttons are hover-only;
  the three card titles are plain ("Use next · Reset runway", "Runs out first", "Running now"); the PROPOSED tag is a
  quiet grey chip.
- Monospace cut from 13 selectors to 5; on the Overview fold the only monospace string is "Ctrl K" (DOM scan).
  Names, durations, readouts and the route bar are Inter.
- The axis is linear (NOW 15:42 · Sat (+1d) … +7d) and capacity moved to its own two-bar column, so the 5-hour and
  weekly bars are comparable and the timeline encodes time only.
- Run-out labels no longer clip ("≈ Out Thu 03:38 (−1 d 2 h)" whole at 1536 and 1920).
- Prototype leaks gone: no "Test nothing" control; first run shows "0 live · 0 elsewhere", "1 computer",
  "Limits not read", "$0.00", Sessions 0, Projects 0.
- The hero name no longer breaks mid-word; the Add account subtitle is gone; the Accounts chart opens folded.
- Still: zero running animations. The year clock is crisp wedges with per-computer 53-week totals.

**Still open**
- Capability matrix is wrong against the owner's fact: tiles say Codex "Subscription · API key · Routed", OpenCode
  "Subscription · Routed", Oh My Pi "Subscription · Routed"; the owner says Routed is Claude Code and Pi only and
  those two are subscription only. The rebuttal states the wrong matrix as fixed.
- 15 text nodes under 10 px on the 1280 fold (ring values 9 px, plan chips 9.5 px, brand line 9.5 px).
- The Add account footer keeps the keyboard legend ("Tab next field · Esc cancel").
- Colour is calmer but still four hues on the hero (teal, amber, coral, lavender) plus blue/yellow/purple dots.

**New defects the fixes introduced**
- 1280, both themes: the axis header collides, "NOW 15:42" overprints "Sat (+1d)" ("NOW 15:4Sat (+1d)"), and the
  per-row run-out label is drawn across the hatched gap and the "Wk ↻" gate ("≈ Out Thu 03:38 (−1 d 2 h) Wk ↻ 6 d
  14 h" in one run).
- The tray popup (bottom-left, 8-account grid) truncates every right-column name ("sabushana", "claude-ma",
  "codex-ma", "agy-main" cut) at 1536.
- At 1280 the brand line truncates to "DEVELOPMENTPC · 12 account" in the 12-account state.
- The hero's "Start on service-account-resal — 98% 5h & 100% week left" runs three lines at 1536, pushing the Start
  button beside a three-line heading; the cell under it repeats the same facts ("Most room: 98% 5h · 100% wk").

## Across the three
- The one thing a winning design here must have: the answer as the biggest type, one primary Start, and ring, bar
  and column each naming the window it shows, as A's ladder and B's ledger now do.
- The one thing it must not have: the slogan or lecture smuggled back as an eyebrow, a numbered title or a footer
  legend; if it is not the answer it is not in the H1 slot or the chrome.
- Parts worth combining:
  - A's answer strip and board (the "Exhausted now" red cell in the 12-account state is the right third voice)
  - B's ledger as A's ladder and as the Accounts rows: 5-hour and weekly columns with the reset under each bar,
    State in words, exhausted first
  - B's finding H1 wording on the Accounts page ("codex-main is out; use service-account-resal.")
  - C's capacity-plus-linear-timeline split and its year clock with per-computer 53-week totals
  - A's Add account tiles (correct matrix, kinds as chips, "Not for Codex" on the greyed card)
- Ranking: A > B > C. A fixed everything cheap and is a shipped-looking product with one layout question (1920)
  and one false "fixed" (freshness at 1280) left; B is now precise and honest but still barely live; C made the
  largest leap and is no longer crowded, but a wrong capability matrix, a colliding 1280 axis and a truncating tray
  keep it a round behind.
