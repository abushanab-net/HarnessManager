# Data-viz critic: critique, round 1

Lens: instruments. Every chart, ring, bar, timeline, dot grid, year clock or scene was opened in headless Edge at
1280, 1536 and 1920, light and dark, Still and Live, with the 12-account state where the page offers one (A has no
switch; I appended four accounts to its data at runtime). Encodings were checked against each prototype's own code
where the picture alone could not settle what a mark means.

## Design A (Claude) · score 6/10
**What works**
- The answer strip is three text cells, not three gauges: "Runs out first: codex-main · Week 73% used · ≈ out Sun
  00:56 · resets Tue 15:46 · read 14 m ago" carries measured, estimate and read-age in one line with no instrument to
  decode. Fastest honest answer to "what is exhausted" of the three.
- The board is the best "what is running where" of the round: one lane per computer, the quiet computer dimmed with
  "last heard 26 m ago", and the write pulse (a bar at the card's edge plus "wrote 4 s ago") encodes one variable,
  recency of writing, that the text already states, so the mark is redundant but never misleading.
- Not-read is a dashed grey ring with "—" and a "Read now" button in the ladder, the tray and the Accounts row; a
  not-read window on the Accounts page is a dotted track with "— not read". The three states stay apart without the
  legend.
- The year clock (below the fold) is the most legible of the three: square cells, visible dynamic range, month letters,
  "Monday inside · daily tokens", busiest day in the centre; the current-week gap at 12 o'clock makes "now" findable.
- Accounts rows and the follow drawer use one bar per window with "% left" and the reset countdown; zero baseline,
  equal length per row, amber only where a forecast runs out, "≈ out Sun 08:10" beside the amber bar.

**What fails**
- The ladder's bar lies to a reader at every width and theme: the label beside it says "5-hour 98% left" but the bar is
  about 17% full, because the bar encodes how far the window has run to its reset (4 h 08 m left of 5 h), not the
  98%. Two rows down "54% left" shows a ~75% bar. The same bar shape means "% left" in the follow drawer and on the
  Accounts page. One mark, two encodings; the legend ("measured · ≈ at today's pace · — not read") does not say so.
- The ladder ring's colour follows no single rule: sabushanab-resal's ring is amber at 87% week left while the Accounts
  advice for the same account says "5-hour resets in 22 m: plenty of room after that"; claude-main's ring is teal at
  72% although its Opus week carries "≈ out Sun 08:10" in amber on the Accounts page. Colour is meant to say "at
  today's pace this runs out", but the eye cannot learn that from the ladder.
- "Latest replies" on the Overview has a Speed column reading "≈ 155 t/s" (1280 and 1920, both themes). The brief's
  rule 2: "No tokens-per-second anywhere on the Overview."
- At 1280 the readouts strip collides: the Cost dot grid runs into the "Cached" label ("▪▪▪▪ached"), dark and light.
- The dot-grid trends are 28 dots folded into two rows of 14; the fold hides which dot follows which, so the "trend"
  is read as texture, not as a line. The delta text ("+18% vs before") is doing the whole job.
- "Running now · 10 sessions · ●●●": three unlabeled coloured dots that repeat the computers chip in the top bar. The
  cell's text ("6 here · 4 elsewhere · 2 of 3 computers alive") already says it.
- No 12-account state on the page. With twelve rows injected at 1280 the ladder runs below the fold (nine rings
  visible, three hidden) and is not sticky, so the account with the least room can be off screen while its lane is on
  screen.

**Must change before it could win**
- Make the ladder bar encode what its label says (% left), or change the label to "4 h 08 m of 5 h run" and give the
  elapsed bar a visibly different shape (a thin track under the name, as the current app does); one encoding per shape
  across ladder, drawer, tray and Accounts.
- One colour rule for the ring, written in the legend ("amber: runs out before its reset at today's pace"), applied
  identically on the Overview and Accounts.
- Remove the Speed column from the Overview table (keep it in the Request log).
- Give the readouts strip room at 1280 (drop the dot grids to a single row of 14 or stack the three readouts).
- A 12-account switch, and a ladder that scrolls on its own or compacts its rows at 1280.

## Design B (Codex) · score 7/10
**What works**
- The capacity ledger is the one instrument in the round that answers rank-1 job one in a glance: two aligned bar
  columns, "5-hour room" and "Weekly room", every bar the same length with a zero baseline, "% left" and the reset
  countdown under each, exhausted rows first. Eight accounts compare without reading a number; twelve rows at 1280
  stay aligned and the live rail stays visible (verified with four rows appended).
- Honesty is carried by the marks themselves: a not-read window is a dashed outline track with "—"; an exhausted window
  is an empty track with a red outline; the ≈ forecast is amber text with a dashed underline in the inspector. All
  three survive light and dark at 1280.
- "Resets first · 18m · sabushanab-resal · 5-hour reset · 14% room until then · 87% of its week is still available":
  rank-1 job three (the next reset) as a single number with its basis, no gauge needed.
- The inspector keeps one ring (week left) and labels its second bar "Week elapsed 44%", so the elapsed bar and the
  room ring never pretend to be the same quantity. This is how A's ladder should have done it.
- By-model panel on Accounts: zero-based bars, equal scale, value and cost at the end of each; a plain instrument that
  earns its row.

**What fails**
- "—" means two things in the hero table: in the "≈ Run-out" column it means "no run-out forecast" for ready
  accounts, and two rows lower it means "not read" for opencode-main and agy-main. The brief reserves "—" for not read.
  Seen at every width, both themes.
- The "Usage at a glance" dot grids are decoration: 48 dots whose opacity is a hash of the index (`(i*7+seed)%11/13`),
  not data, so the three grids look like the same static noise under 48.2M, $184.37 and 91%. The owner's "dot-grid
  trends" idea is present in name only.
- "Tokens per day, by account" on Accounts stacks eight accounts in one teal at eight opacities with no legend and no
  y-scale; at 1280 dark the slices are indistinguishable bands. The chart cannot answer "which account burned
  Tuesday".
- Amber carries two meanings: pressure ("Close", "8%", the "Setup · 1 update" badge) and estimate (the legend's
  "≈ Estimated" dashed amber swatch, the forecast underline). A reader cannot tell "amber because forecast" from
  "amber because low".
- The year clock has too little dynamic range: cells sit between two close shades of teal, so the "busiest day Nov 17"
  in the centre cannot be found on the ring itself. The "371 days of building" headline beside it is copy, not data.
- The "01 · 02 · 03 · 02 Exhausted/Close/Ready/Not read" counters repeat the State column in 28-px digits with
  theatrical leading zeros; the ledger is eight rows tall and already shows the same four groups.
- The limit circles the owner loves survive only in the inspector (one ring) and the detail dialog; the tray preview is
  a table of coloured percentages and the Accounts rows have no ring. Honest, but the kept idea is barely kept.

**Must change before it could win**
- Use a blank cell or "none before reset" in the Run-out column; "—" only for not read.
- Dot grids fed by the period's daily series (one row, oldest to newest) or dropped; the delta text stays.
- The stacked chart gets a colour per account and a legend, or becomes small multiples per account.
- Split amber: one hue for pressure, the ≈ glyph plus dashing (no hue) for estimates.
- Widen the year clock's ramp (near-background for quiet days, full teal for the busiest).
- Replace the four counters with nothing; the ledger's grouping and the State column already say it.

## Design C (OpenCode) · score 5/10
**What works**
- The reset gate marks ("5h ↻ 4 h 08 m", "Wk ↻ 4 d 7 h") and the red "≈ Out Sun 00:56 (−2 d 14 h)" box make the
  starvation gap a visible length, which is the one thing a timeline can say that a number cannot. For codex-main and
  claude-main this reads correctly at 1536 and 1920 in both themes.
- "Harvest ★" (a 5-hour window with ≥75% left resetting within two hours, "use before it resets") is the sharpest
  reading of the owner's "which account we should use because it would reset soon" in the round.
- The limit ring is used consistently: week left (or used, by the "Rings: % left" switch) in every lane, in every
  Accounts row and in the tray popup; not-read is a dashed ring with "—" everywhere.
- Routed accounts get their own honest row ("— plan limits not read · $13.80 left of $25 cap · ↻ 00:00") instead of a
  fake empty bar.
- The dot grids (3 × 8, five opacity levels from real per-period series) read as a rising ramp; of the three, this is
  the only dot grid that shows a trend at a glance.

**What fails**
- The Reset Horizon's time axis is not to scale and does not say so: the first 38% of the track is 0–5 h and the rest
  is 5 h–7 d, then the weekly zone is remapped again to 42–85% of the track and the 5-hour gates are clamped to 6–31%
  (`timeToPct`, `gateWkX`, `gate5X` in views.js). The header ticks "NOW · +1.5h · +5h · Sat (+24h) · Sun · Tue 15:46 ·
  +7d" sit at even-looking spacings that correspond to nothing linear. At every width and theme the reader compares
  lengths across a break they cannot see.
- The bars' fill is "% left × distance to the gate", so a bar's length is room multiplied by time: service-account-resal
  (98% left, 4 h 08 m) draws four times longer than claude-main (42% left, 2 h 19 m). The legend labels this fill
  "Measured room". It is neither room nor time.
- For accounts with no run-out forecast, a dashed "≈ pace trajectory" segment is still drawn from the fill end to the
  gate (samykabu-personal, samykabu-teksa, sabushanab-resal); the legend says dashed means an estimate, and there is
  none. The lane is the busiest instrument of the three designs (ring + two bars + gate + dash + red box + moving
  beads + two text columns per account) for a job B answers with two bars.
- The tray popup in the 12-account state shows eight rings; the four added accounts (codex-resal, claude-audit,
  pi-openrouter, omp-sandbox) are missing (1280 dark, `preset=12&tray=1`). With twelve accounts the Horizon is
  twelve lanes of 44 px plus a route strip, so the readouts and the year clock drop two screens down at 1280.
- "Latest replies · Speed (cadence)" shows "Steady stream · Fast burst · Low latency": a categorical the app does not
  measure, invented to dodge the tokens-per-second rule; the inventory column is speed, and the honest move is to omit
  it on the Overview as B did.
- The year clock is drawn as soft dots with a radial glow; at 1280 and 1536 the ring is a teal smudge where neither
  weeks nor days can be counted, and the per-computer legend beside it ("DEV-PC: 1.48B today") is a different period
  from the clock (53 weeks), so the colours imply a breakdown the clock does not show.
- Live mode adds brighter travelling beads inside the 5-hour bars (1536 dark Live); at a glance they read as a second
  marker on the bar (a threshold, a forecast), not as "a session is writing".

**Must change before it could win**
- Either a true linear axis per window (a 0–5 h strip and a 0–7 d strip, each to scale, both labelled) or drop the
  timeline and keep B's two aligned bars with C's gate labels and the red run-out box.
- Bar fill = one variable (room), with time only as the gate position; no dashed segment where there is no forecast.
- Tray popup renders every account; a compact lane height at twelve rows.
- Replace "cadence" with nothing on the Overview; keep tokens/s in the Request log.
- Year clock as cells, not glow; per-computer switch filters the clock rather than listing today's totals beside it.

## Across the three
- The one thing a winning design here must have: two aligned bars per account (5-hour, week), each with "% left" and
  its reset countdown, exhausted first, not-read dashed, with a one-line "use next / resets first" beside them.
- The one thing it must not have: a bar or ring whose length or colour encodes something other than what the label
  beside it says.
- Parts worth combining:
  - B's capacity ledger (bars, states, dashed not-read, run-out column) as the hero instrument.
  - A's board (lane per computer, write pulse, quiet computer dimmed) for "what is running where".
  - C's "Harvest ★" rule and its reset-gate labels ("5h ↻ 1 h 48 m ★") inside B's reset column.
  - A's year clock (square cells, month letters, busiest day in the centre) below the fold.
  - C's ring with the "% left / % used" switch, as the single ring per account in the tray popup.
- Ranking: B > A > C. B answers all three rank-1 jobs from one honest instrument with the fewest marks; A's board is the
  best live view but its ladder bar and ring colour mislead; C's timeline is the boldest idea and the least truthful
  drawing.

## Instrument table

| Design | Instrument | Encodes | Earns its place | Why |
|---|---|---|---|---|
| A | Answer strip (3 cells) | Text: live count, run-out account with basis, use-next account | Yes | Measured/≈/read-age in one line; no decoding |
| A | Running-now computer dots | Computer colour × 3 | No | Repeats the computers chip; replace with nothing |
| A | Board lanes + session cards | Computer → sessions; card = tool, account, project, model, age, usage | Yes | Position answers "where"; the only true "what is running" view |
| A | Write pulse (card edge) | Wrote within 30 s | Yes (marginal) | Redundant with "wrote 4 s ago" but never misleading; stops in Still |
| A | Ladder ring | Week left % (arc), warn state (colour) | Partly | Arc honest; colour rule unreadable (87% amber, 72% teal with a forecast) |
| A | Ladder bar | 5-hour window elapsed fraction | No | Labelled "98% left" beside a 17% bar; same shape means % left elsewhere |
| A | Readouts dot grids (3) | 28 daily values as opacity, two rows | No | Fold breaks the time order; collides with "Cached" at 1280; keep the delta text or one row |
| A | Year clock | 53 weeks × 7 days, tokens as cell shade | Yes | Legible cells, range, month letters, busiest day; per-computer switch |
| A | Latest replies table | Time, account, model, computer, tokens, speed, first reply | No as drawn | "≈ 155 t/s" on the Overview breaks rule 2; drop Speed here |
| A | Accounts stacked chart | Day × account tokens, colour per account | Yes | Legend, colour identity, zero baseline; no y-scale (same as today) |
| A | Accounts row bars + sparkline | % left per window; 7-day tokens | Yes | One encoding, amber only on a forecast, "≈ out" text beside |
| A | Tray popup rings | Week left %, reset day | Yes | Compact, same ring as the ladder, not-read dashed |
| B | Capacity ledger bars (×2 per account) | % left of 5-hour and week; reset countdown; state | Yes | Aligned, zero baseline, dashed not-read, red exhausted; the hero |
| B | ≈ Run-out column | Estimated run-out before reset | Partly | Right place; "—" also means not read two rows down |
| B | State counters (01/02/03/02) | Count per state | No | Repeats the State column in display digits; remove |
| B | Use-now / Resets-first cards | Text: best account; next reset with basis | Yes | Rank-1 jobs two and three without a gauge |
| B | Live rail traveller lines | A session is active on that computer | No | 27 px line with a moving dot; the "3 here" text says it; replace with the live count dot |
| B | Inspector ring + elapsed bar | Week left %; week elapsed % | Yes | Two quantities, two shapes, both labelled |
| B | Usage dot grids (3) | Index hash (not data) | No | Static noise; feed daily series or drop |
| B | Year clock | 53 × 7 cells, tokens as shade | Partly | Correct structure, too little range to find the busiest day |
| B | Accounts stacked chart | Day × account, one hue at 8 opacities | No | No legend, bands indistinguishable; colour per account or small multiples |
| B | Accounts row bars + sparkline | % left per window; 7-day tokens | Yes | Same encoding as the ledger |
| B | By-model bars | Tokens per model, equal scale | Yes | Zero-based, value and cost at the end |
| B | Tray preview table | 5-hour %, week % per account | Yes (plain) | Honest and compact; owner's rings absent |
| C | Hero cards (3) | Text: best account, exhaustion forecast, running now | Yes | Rank-1 answers in words with ≈ and read-age |
| C | Limit ring (lanes, rows, tray) | Week left or used %, switchable | Yes | One rule everywhere; dashed "—" when not read |
| C | Reset Horizon axis | Time to reset on a two-zone, remapped scale | No | Breaks unmarked; ticks not to scale |
| C | Horizon bar fill | % left × distance to gate | No | Labelled "Measured room"; encodes a product of two variables |
| C | Reset gates + run-out box | Reset instant; estimated run-out and gap | Yes | The one spatial fact a timeline adds; keep the labels |
| C | Pace-trajectory dashes | Estimate (legend) | No | Drawn where no forecast exists |
| C | Flow beads | Session active | No | Read as a second marker inside the bar; the "N open" text says it |
| C | Selected-route strip | Computer → account → tool → provider, first reply | Yes | Replaces the dial's route readout as text |
| C | Period readouts dot grids (3) | 24 levels, rising | Yes (marginal) | Readable ramp; still redundant with the delta |
| C | Year clock | Dots with glow, tokens as size/opacity | No | Smudge at 1280/1536; legend beside it is today's totals, not the clock's period |
| C | Latest replies "cadence" | Invented category | No | Not measured by the app; omit on the Overview |
| C | Accounts stacked chart | Day × account, colour per account, legend | Yes | Readable; no y-scale |
| C | Accounts row bars + 7-day bars | % left per window; 7-day pace | Yes | Same encoding as the hero rings' labels |
| C | Tray popup ring grid | Week left %, reset countdown | Partly | Good at 8; shows only 8 of 12 accounts |
