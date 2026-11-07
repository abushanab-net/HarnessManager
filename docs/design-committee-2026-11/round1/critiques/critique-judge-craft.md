# Judge 1, craft: critique, round 1

Rendered in headless Edge 156 through playwright-core at 1280×720, 1536×864 and 1920×1080, Harness light and dark,
Live and Still (two frames 1.3 s apart compared byte for byte), Accounts, Add account and first run, using each
page's own switches. Edge 156 on this machine refuses `--remote-debugging-port` (its launcher exits 21 and opens no
port), so playwright-core drove the same msedge.exe over its pipe transport; nothing else differs from the brief.

## Design A (Claude) · score 8/10
**What works**
- The answer strip reads left to right as the owner's sentence: "10 sessions" → amber "codex-main" → teal
  "service-account-resal" with the only primary button on the fold. The eye lands in that order at 1280, 1536 and
  1920, light and dark.
- One accent. Teal means alive or chosen, amber means "runs out", red means "login expired", nothing else is
  coloured except the three computer dots, which are used the same way in lanes, chips and the replies table.
- One family (Inter, tabular figures) in about five styles; monospace is confined to the request log, the CLI
  hint and folder paths. "Too many monospace numbers" is cured, not rearranged.
- Depth means something: this computer's lane is brighter with a shadow, the MacBook lane not heard from in 26 m is
  dimmed with the reason in its header; "writing" cards carry a teal left rule; Still freezes everything (frame
  pairs identical) and Live only breathes where a session wrote.
- The two sidebar things the owner ticked are gone from the shell, and so is the sidebar: readouts became a slim
  strip below the board, rings became the "Room" ladder with each account's window, reset and bar, and the tray
  popup is that same ladder, small.
- Brand: the real Harness mark, official icons on every card, chip, ladder row and tile. The Add account tiles
  (step 1) show each tool's kinds as chips with its install state, the clearest answer to "which tool takes which
  kind". Light is a true sibling of dark, not an inversion.

**What fails**
- 1920, both themes: the board does not balance. DEVELOPMENTPC's lane runs to ~850 px while SAMYMSI and the
  MacBook end at ~580 and ~430 px, leaving a ~900×450 px void in the middle of the hero.
- 1280, both themes: the top bar drops the freshness text ("Limits read 14 m ago · usage just now") altogether;
  the MacBook lane header truncates to "Samys-Ma…" and ladder names to "service-account-r…" while the lane has
  spare width.
- All widths: the ladder mixes two windows per row without saying so. The ring is week-left (only the panel header
  says "week left"); the bar and text beneath are the 5-hour window, so a "100%" ring above "5-hour 98% left" reads
  as a contradiction at a glance.
- "Running now · 10 of 10" sits 60 px under the strip's "10 sessions": two headings for one fact.
- First run, 1536 light and dark: an explanation paragraph under the heading ("Their main profiles are already
  listed… the board above fills as sessions start"), the pattern the owner ticked, and the board is below it, not
  above.
- 1280: strip cells break mid-name ("sabushanab-/resal's 5-hour resets in 22 m", "2 of 3 computers / alive").
- Below the fold at 1536 and 1920: the Overview's Latest replies has a Speed column in t/s ("≈ 155 t/s"), which the
  brief forbids on the Overview; outside my lens, but a user sees it.
- Every Add account step carries a keyboard legend in its footer ("Tab next · Enter continue · Esc cancel"): help
  text the lazy user never reads, taking half the footer.

**Must change before it could win**
- Let lanes flow into the free width (or let the ladder take it) at 1920; keep the freshness text at 1280 by
  shortening it, not dropping it; never truncate a name where width exists.
- Label the ring's window on the row ("week") or make ring and bar the same window.
- Cut the first-run paragraph to one line and drop one of the two "running" headings; move t/s out of the
  Overview's replies table.

## Design B (Codex) · score 7/10
**What works**
- The ledger is the most disciplined instrument of the three: fixed columns, tabular Inter, bars on a pixel grid,
  hairlines only; eight rows with both resets fit the 1280 fold and twelve just extend.
- Colour is reserved for state: teal room, amber close, red exhausted (the 0% bar outlined red), dashed grey for
  "not read". The four counters Exhausted / Close / Ready / Not read are the fastest read on any of the three pages.
- One font, no monospace anywhere but the CLI line. The "Use now" card is the only filled teal surface, so Start is
  unmistakable; "Resets first · 18m" is one large number with its reason beneath.
- The dialog system (Add account, tray preview, live sessions) shares one radius, one hairline, one footer; light
  theme is coherent and AA. Still works (frame pairs identical).

**What fails**
- The biggest type on the page is a slogan: "Room to keep going." on the Overview, "Keep your options open." on
  Accounts, "Make room for your work." on first run (all widths, both themes). The H1 slot carries no information,
  so the eye lands on copy before the ledger. The mood was "precise".
- Nav icons are Unicode glyphs (◫ ▣ ▱ ♙ ⌘ ⚙): a chess pawn for Accounts, a Mac command sign for CLI reference, each
  at a different weight; next to the official tool icons they look unfinished (all widths).
- The type scale has too many tiny steps: at 1280 the State column and legend are 8 px, reset times 9 px, footer
  labels 9 px, chips 10 px; seven sizes between 8 and 13 px, and "Exhausted"/"Close" are barely legible.
- "Live" barely exists: the only motion is a 9 px dash sliding along a 27 px line in the Running now rail; the page
  reads as static tables. Precise, yes; live and future, no.
- The counters use leading zeros ("01 02 03 02"), a dashboard affectation that reads as codes, and sit top-right,
  detached from the ledger they summarise.
- The tray preview is a bare two-column table with no rings, no icons and no Start, inside a generic modal; it does
  not share the Overview's design.
- 1280 dark: the single-page Add dialog is taller than the 720 frame; the CLI line and sync disclosure scroll
  behind the footer.
- Accounts rows have no ring (the owner's favourite) and the 7-day sparkline is six near-identical teal bars.

**Must change before it could win**
- Replace the three slogans with the finding in one line, or with nothing; the ledger is the first thing.
- One real 16 px icon set in the nav; a four-step type scale with a 10 px floor.
- One honest live signal (a write pulse on running rows or lane dots); rings back on Accounts rows and in the tray,
  drawn like the inspector's.
- Fit Add account in 720 (two steps, or a fixed header over a scrolling body).

## Design C (OpenCode) · score 5/10
**What works**
- The Reset Horizon idea, every account's windows drawn toward their reset gates with the pace deficit hatched in
  amber, is the one instrument here that encodes time spatially; "use this one because it resets soon" is a read of
  position, not a lookup.
- The 53-week year clock is the best drawn of the three (crisp, per-computer legend with coloured dots, busiest day
  in the centre), and the dot-grid readouts kept their trend.
- The Add account dialog is tidy: tool tiles with "Supports: Subscription · API key · Routed" and the version, kind
  cards explained, synced-vs-own facts in two columns, the CLI line; it fits 1280×720.
- The route bar ("DEVELOPMENTPC + SAMYMSI → codex-main → Codex → OpenAI Direct") is an honest replacement for the
  dial's selected route. Still truly stills the beads and the NOW halo (frame pairs identical).
- Harness mark and official icons everywhere, including the lanes and the tray grid.

**What fails**
- It reproduces the complaint. At 1280 dark the fold holds three dense cards, a legend, two filter rows, a
  seven-tick axis, eight lanes and ten Start buttons; nothing has room. The owner said the app "looks crowded and
  disorganised"; this Overview is more crowded than today's.
- Monospace everywhere: 13 mono selectors in the CSS plus mono chips in the markup, so "12 live · 6 elsewhere",
  "Ctrl K", every ring value, duration, readout, axis tick, the route bar, "Max 5x", "1 open" and "wss://" are all
  JetBrains Mono. The owner ticked "too many monospace numbers everywhere"; C has more than the app today.
- Colour noise: teal, amber, coral, purple, blue/yellow/lavender dots, red-outlined boxes, green stars and dashed
  grey on one screen. The purple "PROPOSED · MODE: MOST ROOM + NEXT RESET" tag is the loudest element of the hero
  card, louder than the finding it annotates (1280 and 1536, both themes).
- Clipped labels: the run-out boxes on the timeline overlap the bar ends and are cut to "≈ Out Thu 03…" at 1280 and
  1536; the hero's account name breaks mid-word ("service-account-/resal") at 1280 and 1536; ring labels truncate
  to "service-account-r…".
- The time axis is not a scale: ticks read "NOW 15:42 · +1.5h · +5h Window · Sat (+24h) · Sun (≈ codex out) · Tue
  15:46 · +7d Week", unequal and account-specific, so the 5-hour and weekly bars on the same track cannot be
  compared by eye; the geometry does not mean what the legend says.
- Hierarchy by repetition: a Start on every lane plus the hero's Start and Folder…, eleven primary buttons on the
  1536 fold, so no single "use next" action stands out.
- Prototype leaks into product chrome: a "Test "nothing"" control in the Needs attention header; first run still
  shows "12 live · 6 elsewhere", "$571.37", Projects 23 and Skills 255 in the shell (1536, both themes).
- Headings as lectures: numbered all-caps mono card titles ("● 1 · BEST ACCOUNT TO USE NOW · RESET RUNWAY") and an
  explanation sentence under the Add account title ("Each account gets its own isolated folder & login while
  sharing…"), the pattern the owner ticked.
- Accounts opens with a bright teal stacked chart unfolded above the rows; at 1280 it pushes the second account
  below the fold.

**Must change before it could win**
- Halve the density: one Start (the hero), lanes without buttons, one filter row, plain card titles, the PROPOSED
  tag in grey.
- Inter for everything but the request log; teal and amber plus one negative, nothing else.
- A linear time axis (or two linear tracks, 5-hour and week) with run-out labels that never cover a bar.
- Remove the test control; first run resets the shell's counts.

## Across the three
- The one thing a winning design here must have: one landing order on the fold, the sentence's three answers in
  three distinct, differently weighted places, with exactly one primary Start.
- The one thing it must not have: a tagline, a numbered lecture title or an explanation line in the H1 slot; the
  biggest type is the answer or nothing.
- Parts worth combining:
  - A's answer strip and lane board (the shell without a sidebar, the "writing" rule, the dimmed quiet lane)
  - B's ledger columns (5-hour and weekly room with the reset under each bar, exhausted first, not-read dashed) as
    A's Room ladder and as the Accounts rows
  - C's year clock drawing with its per-computer legend
  - C's route bar as the first line of A's follow drawer
  - A's Add account step 1 tiles with C's "Supports:" wording
- Ranking: A > B > C. A is the only one that already looks like a shipped product and cures all four things the
  owner ticked; B is cleaner on type but spends its hierarchy on slogans and has no life; C has the boldest
  instrument and the worst noise.
