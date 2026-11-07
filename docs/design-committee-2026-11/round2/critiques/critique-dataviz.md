# Data-viz critic: critique, round 2

Lens: instruments. Every page of `round2/app/index.html` was rendered in headless Edge (playwright-core over CDP on a
private port) at 1280×720, 1536×864 and 1920×1080, dark and light, Still; each page's switches, dialogs, empty state
(Designer B's studio "Empty"/"Error" views, the base presets `firstrun`/`empty`), the 12-account preset, the ring
left/used toggle, the tray, the session modes (All / Changes / Failed), the Projects detail view and the routing
"two providers + route" fixture. Every B page opens its first-visit tour dialog on load; it was dismissed before each
capture. Screenshots live only in my scratchpad; nothing in the app folder was touched.

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score 7/10

**Still open from round 1**
- None of the round-1 instrument defects survive: the 5-hour row has its own labelled 0–5 h track, the ruler is
  linear with ticks NOW · +2d · +4d · +6d · +7d that do not touch, the legend reads "Time until reset", forecast dashes
  and the striped gap appear only on claude-main and codex-main, the tray at 12 shows all twelve with no horizontal
  scroll, the routed lane no longer draws a gate over its text, the capability matrix is right.

**What works**
- The 5-hour zoom row ("5h · 0–5h zoom", gate at `resetMs / 5h`) is readable at every width and encodes one thing:
  verified service-account-resal 4 h 08 m sits at ~82% of the track, codex-main 1 h 14 m at ~25%. The "Proposed:
  harvest" tag sits in the caption, not on the mark.
- The week row is to scale (0–7 d) and the starvation gap is a true length: claude-main's dashed amber run to "≈ Out
  Thu 03:38" then the red stripes to the gate at 6 d 14 h; healthy rows draw one solid span and nothing else.
- The ring/bar left-used toggle flips the ring, both capacity bars and the column headers together ("ACCOUNT & WEEK
  RING (USED)", "CAPACITY (% USED)"); the ring's colour rule stays one rule (amber = a window forecasts out, coral =
  exhausted or login expired, dashed grey "—" = not read).
- Capacity bars (5h, Wk) are fixed-length, zero-based "% left" with the number beside them; the Accounts rows use the
  same encoding with a reset countdown per window, so the two pages agree.
- The tray popup: two columns, ring + week reset per account, "Daily cap · $35.20 left" for a routed account,
  dashed "—" + "No plan limits" for usage-only tools; all 12 fit, names unclipped, click opens detail.
- Year clock: wedge cells on a five-step ramp, busiest day findable, per-computer legend gives 53-week totals.
- Latest replies carries no tokens-per-second; the "first reply is approximate (≈)" note and ≈ on every value.
- Honest zero/unread states: first run shows "— limits not read" rows and no forecasts; the empty Overview offers the
  six tools; Needs attention's "Not read (—)" for agy-main.

**What fails**
- Account detail › Limits bars encode **used share** (5-hour "42% left" draws a 58% bar; Week "96% left" draws 4%),
  while the lanes, Accounts rows and tray draw the same bar shape as **% left**. The only cue is the 11 px caption
  "Bars: used share · dashed mark: ≈ level at reset". One shape, two opposite meanings, one click apart (1280 and
  1920, both themes).
- Account detail contradicts itself on screen: header "7.51B tokens · 7 days · $2,795.31", then "Sessions · 7 days —",
  "Busiest project — · No usage this week", "Tokens per day · 30 days: No usage yet", "By model: No usage yet", and
  five "Open now" rows that are all "— Folder not read · PID —". The page says both "7.51B" and "no usage".
- Accounts › "Tokens per day, by account" with 12 accounts: the legend lists 7 names while the columns stack 12
  accounts' colours; codex-resal, claude-audit, pi-openrouter and omp-sandbox have segments but no legend entry (1280
  dark). The chart also has no value axis in either state; per-day totals are tooltip-only.
- Accounts rows › "7-day pace" sparkline draws a dash for a day with no data (service-account-resal: six dashes, one
  bar). A dash is the app's "not read" mark; here it means "zero" or "before the account existed". Two meanings.
- The tray ring mixes quantities: claude-audit's ring reads "14%" (week left) in coral because its 5-hour window is
  exhausted; the popup has no legend and no 5-hour figure, so a coral 14% reads as "week nearly gone".
- Year-clock computer chips truncate at 1280 ("DEVELOP", "Samys-M") in both themes.
- The "ACTION" column header on the Horizon sits over an empty column at rest (Start appears on hover/focus): a
  labelled column of nothing on every capture.
- Readouts dot grids (3 × 8): the cost grid is all full while cost fell $256; the grid encodes a level ramp, not the
  period's shape; the delta text beside it already says everything the grid can.

**Must change**
- One bar encoding app-wide. Either the detail page draws % left like everywhere else (with the dashed "≈ level at
  reset" marker on that scale) or its bars get a different shape (a thin track with the used fill drawn from the
  right) so the reader cannot mistake one for the other.
- Account detail must derive its tiles and charts from the same series as its header; "—" only where a stat was not
  read, never beside a number that was.
- Legend = every account in the chart; a value axis (or the daily total on each column) on the stacked chart.
- Sparkline: an empty slot for no data (no dash); "—" stays reserved for not read.
- Tray ring: colour from the same window as its number, or a second small figure for the 5-hour window.
- Chips that fit at 1280 (abbreviate to the dot + full name on hover rather than cutting the name).

**Inventory misses** (for the clarity judge)
- Account detail › Overview tile "At list prices" lacks the "subscription: not billed per token / billed per token"
  variant for API-key accounts (only subscription seen); Limits panel has no "window has reset" / "no window open"
  states in the fixture.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Dispatch deck cards (3) | Text: use next, runs out first, running now | Yes | Rank-1 answers in words with ≈, read-age and window names |
| Week ring (lane, row, tray) | Week left or used; colour = state rule | Yes | One renderer, one rule, toggle flips all |
| Capacity bars 5h / Wk | % left (or used), fixed length | Yes | Zero-based, labelled, same on Accounts |
| 5h zoom row | Time to 5-hour reset on a 0–5 h scale | Yes | Readable, labelled, independent of the 7-day scale |
| Week timeline row | Time to reset; dashed forecast + striped gap only when a run-out exists | Yes | True lengths on a linear axis |
| Horizon ruler | NOW · +2d … +7d | Yes | Linear, no collisions |
| Selected-route strip | Computer → account → tool, first reply ≈, N open | Yes | Text, no mark |
| Readouts dot grids (3) | Level ramp per period | No | Redundant with the delta; the cost grid shows "full" while cost fell |
| Year clock | 53 × 7 cells, five-step ramp, busiest day | Yes | Readable at 1280; chips truncate |
| Needs attention list | Items with window names and times | Yes | Text |
| Accounts stacked chart | Day × account tokens, colour per account | Partly | Legend incomplete at 12; no axis |
| Accounts By model bars | Tokens per model, equal scale, cost | Yes | Zero-based, numbers at the end |
| Accounts row window bars | % left per window + reset | Yes | Same encoding as the lanes |
| Accounts 7-day sparkline | Daily tokens, 7 bars | Partly | Dash for no-data collides with "—" |
| Account detail limit bars | Used share + ≈ marker at reset | No as drawn | Opposite encoding to every other bar; caption-only cue |
| Account detail tiles (5) | Tokens, cost, p95, sessions, busiest | Partly | Honest marks; values contradict the header |
| Tray ring grid | Week left % + reset, cap $, "—" | Partly | All 12 fit; colour and number from different windows |

## Sessions · 7/10 (Designer A)

**What works**
- The running-now group is one list with status dot (teal = open), title, "msgs · path", account, tool icon, computer
  dot + name, tokens, "live"/"running on X", one action; no chart pretends to summarise it. "18 sessions · 38.94M
  tokens" on the group head is the only aggregate and it is correct against the rail's "Live now 18".
- Rail project dots carry one meaning (teal = has a live session) and the counts beside them say the rest.
- Tag chips get a fixed colour by list position (as inventoried); the tag count sits in the chip.
- 18 live sessions, 115 total, 22 projects, 3 computers all render at 1280 with the group scrolling inside `main`;
  light theme keeps every mark legible.

**What fails**
- At 1280 (both themes) the account column truncates ("service-acco…", "samykabu-te…", "opencode-m…") and the
  when/where cell truncates "running on SA…", "running on Sa…": the computer, a rank-1 fact, is cut while the
  "msgs · path" line keeps full width.
- Colour collisions: the live status dot is teal and the DEVELOPMENTPC computer dot is teal in the same row; the
  "infra" tag is coral (reserved for exhausted/failure), "release" is violet (reserved for computer identity),
  "design" is emerald (reserved for spare capacity near a reset). A reader scanning for red sees a tag.
- The nav badge "⁞⁞⁞⁞ 18" draws a dot grid beside the number it encodes; the grid adds nothing.
- The Sessions page draws a second sidebar (project rail) inside the page; the design language says pages do not
  duplicate a sidebar. It is inventoried, but at 1280 it costs 190 px that the truncated columns needed.

**Must change**
- Give the account and where columns a minimum width before the path line; truncate the path, not the computer.
- Tag palette from six hues that are none of teal/amber/coral/emerald/violet, or desaturated versions with a text
  label (they already have one).
- Drop the nav dot grid; the number is the instrument.

**Inventory misses**
- No "Show all N sessions" row under a group, no paging foot ("‹ 1–50 of 142 ›") seen with 115 sessions; no
  "archived" pill or red failed-command dot in the fixture; the About panel is only in the one-project view (correct).

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Status dot | Open now (teal) / failed (red) | Yes | One meaning per colour within the row, but collides with the computer dot |
| Computer dot + name | Which computer | Yes | Name is text; dot repeats the Overview's palette |
| Tag chips | Tag identity by position colour | Partly | Colours reuse state hues |
| Rail project dots + counts | Has live session; session count | Yes | Count is text, dot is one bit |
| Group head "N sessions · tokens" | Aggregate | Yes | Text |
| Nav live dot grid | Live count | No | Number beside it |

## Session · 8/10 (Designer A)

**What works**
- Five tiles (Tokens 4.82M, Read from cache 76%, First reply p50 1.1 s, Failed commands 0, Files changed 5) are
  numbers with no gauge; "—" is reserved for "no timed requests" (not seen in this fixture but the design has it).
- Steps: ✓/✗/… glyph, tool, target in mono, meta at the right ("+7 −8", "448 ms", "running now"); a click opens the
  output or the coloured diff. The Changes mode is a flat list of only the file-changing steps with their +/−; Failed
  says "Nothing failed."
- The header's cost carries ≈ ("$1.79 ≈"); the "LIVE · following" pill and the hidden "↓ Latest" button behave as
  inventoried.
- Light theme keeps the diff colours and the glyphs legible; 1920 just widens the chat.

**What fails**
- The right-hand step column mixes two quantities in one right-aligned column: durations ("2.2 s", "448 ms") and
  diff counts ("+14 −3"). A reader scanning down cannot sum or compare; the inventory's meta is one string, but the
  design should at least separate them visually.
- First reply p50 on this page is "1.1 s" for a claude-main session while claude-main's Account detail says "First
  reply p95 — · No timed replies": the two pages disagree on whether this account has timed replies.
- The "+7 −8" counts have no header; the tile "Files changed 5" and the conversation head "5 changed files" and
  "62 steps" repeat each other three times above the fold.

**Must change**
- Two columns at the step's right edge (duration | diff) or a fixed-width "diff" column with a header.
- One source for timed replies across Session and Account detail.

**Inventory misses**
- "Show 1,990 more steps" fold, the "YOU · 14:02" prompt bubble timestamps and the Resume-here dialog were not
  exercised by the fixture session; Export as Markdown is a button only.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Stat tiles (5) | Measured session totals | Yes | Numbers; "—" convention available |
| Step glyphs ✓ ✗ … | Done / failed / running | Yes | One glyph per state, text beside |
| Diff counts "+a −b" | Lines changed | Yes | Text; shares a column with durations |
| Diff view (green/red lines) | Added / removed | Yes | Standard, both themes |
| LIVE pill | Following a live session | Yes | Text |

## Projects · 7/10 (Designer A)

**What works**
- "Where the tokens and the cost went": the per-row share track is the same length for every row with a zero
  baseline, the chosen metric's column is emboldened, and the "16 other projects" row closes the sum to 100%
  (21 + 13 + 11 + 11 + 9 + 36). The Tokens / Cost / Sessions switch relabels the track and the share column.
- Five tiles: the Running-now tile names its projects and opens the live list; "$14.48 a day ≈" carries the estimate
  mark; "70% priced" appears on cards under 95% priced (spark-serve).
- Computer dots on cards (teal/amber/violet) plus "not cloned here" in amber text; names on hover.
- 21 projects at 1280 are cards that scroll inside `main`; the 12-account preset changes nothing here (correct).

**What fails**
- Every project card's area sparkline is drawn on its own y-scale: HarnessManager (58.68M tokens) and spark-serve
  (7.53M) both peak at the full height of the box. Rows cannot be compared, which is the one thing a column of
  sparklines is for (1280 and 1920, both themes).
- The stacked share bar above the table encodes the same shares as the per-row tracks and the "Share" column: three
  encodings of one number. The bar's five teal shades are not tellable apart past the second segment.
- The "Running now" tile reads "18" beside "Projects 21" while the heading says "12 have a session open now": the
  tile counts sessions, its neighbours count projects, and nothing on the tile says which.
- The sparkline has no period label on the card ("7 days" appears only in the About panel of the detail view) and
  no baseline mark.

**Must change**
- One y-scale for every card sparkline in the current sort (or a small "max 58.7M" tick), and the period in the
  card.
- Remove the stacked bar; keep the per-row tracks with their swatches.
- Tile label "Sessions running now 18" or "12 projects · 18 sessions".

**Inventory misses**
- No paging foot ("‹ 1 2 3 › · 1–10 of 34 · 10 / 25 / 50 a page") with 21 cards; the "flash once on change" of a
  tile is code, not visible in a still capture.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Five tiles | Totals and the busiest project | Yes | Numbers; the Running-now label is ambiguous |
| Stacked share bar | Top-5 + others share | No | Triplicates the row tracks and the % column |
| Per-row share track | Share of the chosen metric | Yes | Equal length, zero baseline, swatch + name |
| Card area sparkline | 7-day tokens | No as drawn | Each card its own scale; no period label |
| Card computer dots | Which computers have the repo | Yes | Names on hover, "not cloned here" text |
| About panel (detail) | Project totals, 7-day area, accounts with share % | Yes | Text + one chart with a period label |

## MCP servers · 8/10 (Designer B)

**What works**
- The matrix is the right instrument: one row per server, one checkbox column per tool with the tool's icon and the
  count of ticked servers under it (11 · 2 · 4 · 3 · 3 · 4 = 27 = "27 entries written" in the subhead).
- Rows carry the transport tag, the command in mono, the description; the "2 left alone" pill is amber (attention),
  not red.
- 11 rows at 1280 scroll inside the panel with a pager "1–11 of 11 · Rows 25"; the 1920 layout spreads the six
  columns without widening `main`. Light theme keeps the checkbox fill readable.
- Empty state ("No MCP servers yet · Add a server, or import from your tools") and the error state render.

**What fails**
- A "left alone" entry is not marked in the matrix: the cell is a plain ✓ or empty, so the two left-alone entries the
  heading counts cannot be found in the grid; only the Info dialog says. A third cell state (ticked but not written)
  needs its own mark.
- The "Off" tag (no tool ticked) and the hover "Claude Code: N servers" header are not demonstrable in the fixture;
  every row has a tick.

**Must change**
- A distinct cell mark for "left alone" (an outlined box with a dash, amber) with the count in the column head.

**Inventory misses**
- Edit page (Form / JSON switch, argument and env rows) not reached from the Edit button in this render; search
  hiding rows not exercised.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Tool × server checkbox matrix | Written / not written | Yes | Counts in the head sum to the subhead |
| Column counts | Servers ticked per tool | Yes | Follows the ticks |
| "2 left alone" pill | Hand-made entries not taken over | Partly | Not locatable in the grid |
| Transport tag | stdio / http / sse | Yes | Text |

## Skills · 8/10 (Designer B)

**What works**
- Same matrix as MCP with a header checkbox per tool that is checked (255), indeterminate (Antigravity 4, drawn as a
  dash) or empty (Pi 0, Oh My Pi 0); the count sits beside the box. 258 skills survive through "1–25 of 255" paging
  with the matrix scrolling inside its panel at 1280.
- "Update" tag beside the name; Source column as text; the "Not managed · 3" panel below with its own Manage buttons;
  heading finding "3 skill folders need managing" matches the panel.
- Empty state renders ("No skills yet…").

**What fails**
- "Installed 258" tab versus "255 managed · 3 not managed · 1–25 of 255": three numbers for the same store in one
  header; the tab counts folders, the pager counts rows.
- Discover tab not rendered in this pass (no instrument expected there).

**Must change**
- The tab count and the pager count agree, or the tab says "Installed 255 · 3 unmanaged".

**Inventory misses**
- Discover (repositories, Install into chips) and the Install / Import dialogs were not opened.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Tool × skill checkbox matrix | Linked / not linked | Yes | Header tri-state with counts |
| Header tri-state checkbox | All / some / none linked | Yes | Dash for indeterminate |
| Update tag | Newer source version | Yes | Text |

## System prompts · 8/10 (Designer B)

**What works**
- No chart is needed and none is drawn: a tab per tool, a list with "Live" (grey) and "Enabled" (teal) pills, the
  live file in a code block, the path in mono.
- Empty state: "No prompt file yet · (the file is empty)".

**What fails**
- "Live" and "Enabled" are the two states a reader must tell apart and the Live pill is grey text on a grey chip at
  1280 dark; it reads as disabled.

**Must change**
- Give "Live" the same weight as "Enabled" (an outlined teal pill) or spell "in the file now".

**Inventory misses**
- The editor view (title field, Use this prompt, Delete, Save) was not opened; "Writes to <path> · every Codex
  account follows it" shown.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Live / Enabled pills | Which prompt is in the file | Partly | Live pill too quiet |

## API keys & routing · 7/10 (Designer B)

**What works**
- The route chain is the right drawing: provider chips joined by →, each with its state in text ("Serving", "Paused ·
  retry 4m" in amber); "failover · used by pi-lab" beside the id.
- Providers table: Key "Saved" (teal pill), Breaker "Closed" (teal) / "Open · 5 failures" (coral), Requests · 24h and
  First chunk p95 right-aligned. Recent requests: "—" for the failed request's first chunk and tokens.
- Four router stats as plain numbers; the empty state sets them all to 0 with "0 routes · 0 accounts use it".

**What fails**
- The page contradicts the Overview: with the default fixture the router is "stopped · 0 routes · 0 accounts use
  it", while the Overview lane shows opencode-main as "Routed · cheap-fast" with "$11.20 / $25 spent" and the
  12-account preset adds pi-openrouter on cheap-fast. The studio fixture loads a different route ("work-cheap", used
  by "pi-lab") that exists on no other page.
- The stat tiles carry no state colour: Failovers 3, Errors 2, Paused 1 are white; the inventory asks for amber /
  red / red, and the design language asks for a text mark beside colour, so these should at least read "2 errors" in
  coral with the word.
- Recent requests: status "503" is plain; the inventory says a non-200 status is red.

**Must change**
- One routing fixture shared with `data.js` (cheap-fast, used by opencode-main and pi-openrouter) so the page agrees
  with the Overview and Accounts.
- State colour plus word on Failovers / Errors / Paused and on non-200 statuses.

**Inventory misses**
- Add provider / Add route dialogs not opened; "hm-<id>--… models" aggregate chip state and "default" state not in
  the fixture.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Router stat tiles (4) | Requests, failovers, errors, paused | Partly | Numbers; no state mark on the bad ones |
| Route chain chips | Provider order and state | Yes | Text state, amber for paused |
| Breaker / Key pills | Closed/open, saved/missing | Yes | Colour + word |
| Recent requests table | Status, first chunk, time, tokens | Partly | 503 not marked |

## Apps · 8/10 (Designer B)

**What works**
- A table, not tiles: Installed / Newest / Installed by / Action / Show; the second-copy sub-row in mono with
  "second copy, not on PATH first"; the warning bar for the duplicate Claude Code with its Remove action.
- "Up to date" as outlined tags, "Reinstall 1.5.0" as the one primary; the Harness Manager card's "Update
  available" pill in amber.
- Empty state lists six not-installed tools with "—" in Installed and Installed by, and Install… / Website actions.

**What fails**
- Pi's Newest "1.5.0" beside Installed "0.84.3" is not marked (inventory: amber when newer); the heading says "1 tool
  has an update" but the only mark in the row is the primary button.

**Must change**
- Newer version in amber with "newer" in the cell's title.

**Inventory misses**
- "could not check" / "?" version states not in the fixture; Install ▾ menu of methods not opened.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Version columns | Installed vs newest | Partly | Newer not marked |
| Up to date / Update tags | Row state | Yes | Text |
| Show checkbox | Tool visible on other pages | Yes | Native control |

## CLI reference · 9/10 (Designer B)

**What works**
- Static text; monospace only for commands; seven groups as cards; "Start here" examples with their meanings in a
  second column. Nothing to encode and nothing invented. 1280 light and dark both clean.

**What fails**
- Nothing in this lens.

**Must change**
- Nothing.

**Inventory misses**
- None seen (the Computers group was below the first fold and not scrolled in this pass).

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| None | — | — | A reference page; correct to have no chart |

## Settings · 8/10 (Designer B)

**What works**
- The Computers cards are the one place a bar is right: "1,204 of 1,204 entries published" full teal; SAMYMSI
  "924 of 924 applied here" full teal; Samys-MacBook-Pro "296 of 302 applied here" in amber with "heard 4m ago" in
  amber. The number is beside the bar, the baseline is zero, the amber means behind-and-stale and is also said in
  words.
- Section chips at the top, "N settings" per card, "Limits shown as Left / Used" and "Moving routes" present.
- Backups list with sizes, the archive line "37 archived · 8.1 MB · Today 03:00 · sync folder: 37 (on) · folder: 0
  (off)" as text.

**What fails**
- The "This one" pill is emerald-outlined while emerald means spare capacity near a reset elsewhere in the app; the
  computer's own colour dot (teal) sits beside it. Minor, but it is the only emerald on the page.

**Must change**
- "This one" as a plain outlined pill.

**Inventory misses**
- Pair / Join / Invite / Recovery-key dialogs and the Restore / Welcome-back dialog were not opened; the Direct
  connection "Listen port" dialog not opened.

| Instrument | Encodes | Earns its place | Why |
|---|---|---|---|
| Computer progress bars | Entries published/applied of total | Yes | Zero-based, number beside, amber = behind |
| Presence text + colour dot | Connected / heard N ago | Yes | Word plus dot |
| "This one" pill | This computer | Partly | Emerald collides |

## Across the app

- **Is it one app?** Visually yes: one rail, one topbar, `.panel-card` everywhere, the same chips and pills, Inter
  with tabular figures, mono only in paths and commands. The three hands show in the instruments, not the chrome:
  the base draws bars as "% left", the lead's own Account detail draws them as "used share", Designer A draws area
  sparklines each on its own scale, Designer B draws progress bars as "applied of total". Four bar meanings in one
  app. They also show in colour: A's tag palette and B's "This one" pill borrow coral, violet and emerald that the
  language reserves for state and computer identity; the live dot and the DEVELOPMENTPC dot are both teal in the same
  row on Sessions, Projects and the Overview's Running-now list. And in data: the Overview, Accounts, Session and
  Routing pages do not share one fixture (routed accounts exist on three pages and not on the fourth; a session shows
  a 1.1 s first reply the account page says it never timed; a detail page shows 7.51B tokens and "no usage").
  To unify: one `renderBar(kind)` with two shapes (room bar, progress bar) and one caption rule; one tag palette
  outside the state hues; one data module every page reads.
- **Three worst defects, ranked**
  1. Account detail limit bars encode used share while every other bar of the same shape encodes % left, caption
     only; and the same page shows "7.51B tokens" above "No usage yet". The page the lead owns is the least honest.
  2. Projects card sparklines on per-card y-scales (rows cannot be compared) plus a stacked share bar that triplicates
     the share numbers.
  3. Cross-page data disagreement: Routing says "0 routes · router stopped" while the Overview routes opencode-main
     through cheap-fast; with the fixture loaded it shows a route no other page knows. Behind it, Sessions at 1280
     truncating the account and "running on SA…" (the where) while keeping the file path.
- **Ranking, best to worst:** CLI reference > Skills > MCP servers > Session > Settings > Apps > Overview/Accounts
  (base) > System prompts > Sessions > Projects > API keys & routing > Account detail (scored inside the base).
