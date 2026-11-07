# Data-viz critic: re-score, round 1

Method: the three revised prototypes were rendered again in headless Edge (playwright-core over CDP) at 1280 and 1920
dark, 1280 light, Still and Live, Overview and Accounts, tray popups, and the 12-account state each page now offers
(A: "12 accounts" data switch; B: Data › 12 accounts; C: `preset=12`). Every "fixed now" claim below was checked on
screen, and bar encodings were re-read in each prototype's code where a picture alone could not settle them. Owner's
fact applied: a Codex account has a 5-hour window as well as a weekly one; all three now show both for codex-main.
(Another critic's run was killing every Edge process on port 9455 during this session, so my renders used a private
Edge on port 9471; nothing else changed in the method.)

## Design A (Claude) · score 8/10 (was 6)

**Instruments now honest (verified on screen)**
- Ladder bar: now % left of the window with the least room, same shape and meaning as the Accounts rows, the follow
  drawer and the tray (service-account-resal "5-hour 98% left" draws a 98% bar; samykabu-teksa 54% draws 54%;
  codex-main "Week 27% left" draws 27% in amber with "≈ out Sun 00:56"). The reset countdown stands beside it.
- Ring: captioned "WEEK" under every ring, header "ring: week left", and a legend at the ladder's foot ("ring and date:
  the week · bar: the window with the least room · ≈ runs out before its reset · exhausted or logged out · — not
  read"). The colour rule is one rule now: claude-main 72% is amber (its Opus week forecasts out), sabushanab-resal
  87% is teal, samykabu-work (12-account state) is red with "5-hour exhausted"; the same colours appear on the
  Accounts rings and the tray.
- Latest replies on the Overview: the Speed column is gone; "first reply is approximate" only.
- Readouts: one row of 14 dots per readout; no collision with "Cached" at 1280 in either theme.
- "Running now" cell: the three unlabeled computer dots are gone.
- 12 accounts: the ladder scrolls inside its own panel at 1280 × 720 (8 rows visible, the rest by scroll), the tray
  popup lists all 12 with a scroll, the answer strip gains a red "Exhausted now · samykabu-work · 5-hour 100% used ·
  back in 1 h 12 m".

**Still misleading or decorative**
- The dashed grey ring with "—" now carries two states: "not read" (legend) and "no limits · usage only"
  (opencode-main, agy-main: "OpenCode has no limits"). One mark, two meanings; the text disambiguates, the mark does
  not.
- The dot grids remain a 14-dot opacity strip whose only readable message is "rising" or "flat"; the delta text beside
  them says the same. Harmless now, still redundant.

**New defect**
- 12-account state at 1280 dark and light: the ladder list gets a horizontal scrollbar and clips the forecast text
  ("≈ out Sun 08:1" on claude-main); the clipped text is the one estimate the row carries.

## Design B (Codex) · score 9/10 (was 7)

**Instruments now honest (verified on screen)**
- Run-out column: "—" is reserved for not read; ready accounts show "None", a capped routed account shows "No
  forecast", a pay-per-use account "n/a"; the legend reads "Measured · ≈ Forecast · Not read · None: no run-out before
  reset". The forecast names its window ("≈ Weekly 1d 7h") in neutral ink with a dashed underline, so amber now means
  pressure only (14%, 8%, "Close").
- Dot grids removed; "Usage at a glance" is three values with their deltas.
- Tokens per day, by account: a colour per account, a legend, "Daily tokens · 0–9.4M · oldest to newest", zero
  baseline; the eight segments are tellable apart at 1280 dark.
- Counters (01/02/03/02) removed; the State column alone carries the four states.
- Year clock: near-background to full teal ramp; the busiest cell is outlined; "53 weeks · daily tokens" replaces the
  slogan.
- Rings: every Accounts row has a ring, the tray has ring + tool icon + 5-hour and week % with resets + Start, and the
  Accounts toolbar states the convention ("rings: % left (week / daily cap)").
- 12 accounts: the ledger adds a capped row ("Daily cap: $13.80 left / $25.00 · 55.2% · resets in 8h 18m", state
  "Capped") and a pay-per-use row ("Pay per use · no plan windows", "n/a"); nothing is drawn as a fake window; the
  live rail stays beside the ledger; the tray lists all 12 with a scroll.
- The rail now names each project's account and its writing/waiting state under a named computer; "4 writing" in the
  header.

**Still misleading or decorative**
- The traveller marks survive as a 1-px "—" after "here" / "elsewhere" in the rail header; they encode nothing a
  reader can see while Still.
- The year clock keeps a two-line explanation ("Brighter = more tokens. The outlined cell is the busiest day."); the
  outlined cell and the ramp should carry that alone.
- The capped account's daily-cap bar sits in the "Weekly room" column; the label says daily cap, the column header says
  weekly.

**New defect**
- None found in the instruments.

## Design C (OpenCode) · score 6/10 (was 5)

**Instruments now honest (verified on screen and in `views.js`)**
- The time axis is linear: NOW · Sat (+1d) · Sun (+2d) … +7d at equal spacing, `linearPct(ms) = ms / 7 d`; reset gates
  and run-out boxes are placed by the same function.
- Capacity is its own column: fixed-length, zero-based "% left" bars for 5h and Wk, identical to B's ledger in
  encoding; the ring is week left (or used by the switch) and is coloured by one rule (amber where a forecast runs
  out, red where a window is exhausted, dashed grey "—" where not read).
- Dashed "≈ pace forecast" and the red hatched gap appear only for claude-main and codex-main (the two accounts with a
  run-out before reset); healthy accounts draw one solid bar to the gate. The starvation gap is a real length on a
  real axis now ("≈ Out Thu 03:38 (−1 d 2 h)" then the red span to "Wk ↻ 6 d 14 h").
- Year clock: crisp wedge cells on a five-step ramp; the busiest day is findable on the ring; the legend beside it
  gives 53-week totals per computer, the clock's own period.
- "Speed (cadence)" is gone from Latest replies.
- The tray popup renders all 12 accounts (scrollable).
- In-bar beads are gone; the live dot sits by "N open".

**Still misleading or decorative**
- The 5-hour track on a 7-day axis is a sliver of under 3% ("5h ↻ 4 h 08 m" is drawn as a 20-px stub at 1920, 12 px
  at 1280); every account's 5-hour gate looks the same, so the timeline cannot show the rank-1 job "which 5-hour
  window resets first". The capacity column and the text column do that job; the timeline's 5-hour row is decoration.
- The healthy weekly bar is a solid teal span labelled "Measured" in the legend; its length is time to reset, not a
  measurement of room. Honest on a time axis, mislabelled in the legend.
- Per lane there are now more marks than before (ring, two capacity bars, two timeline rows with gates, two text
  columns); the timeline earns its width for two of eight accounts. The owner's clutter complaint is answered least by
  this design.

**New defects**
- 1280, both themes, 8 and 12 accounts: the axis labels "NOW 15:42" and "Sat (+1d)" overprint each other
  ("NOW 15:4Sat(+1d)").
- The routed account's lane (opencode-main) draws the "Cap ↻ 8 h 18 m" gate label over its own text at 1280, 1536 and
  1920 ("Ro|Cap ↻ 8 h 18 m|ast (DeepSeek V4 → OpenRouter)").
- The tray popup's second column is clipped ("sabusha…", "claude-ma…", "codex-ma…") and the popup shows a horizontal
  scrollbar at 1280 and 1536, with 8 and with 12 accounts.

## Across the three (updated)
- The one thing a winning design here must have: two aligned, zero-based "% left" bars per account (5-hour, week)
  with the reset countdown under each, exhausted first, not-read dashed, estimates named by window with ≈; B has it,
  A's ladder and Accounts rows now match it, C has added it as its capacity column.
- The one thing it must not have: a track whose length a reader will take for room when it is time (C's weekly
  timeline bar, labelled "Measured"), or one mark for two states (A's "—" ring for both not-read and no-limits).
- Parts worth combining:
  - B's capacity ledger, run-out column vocabulary ("None / No forecast / n/a / —") and capped and pay-per-use rows.
  - A's board (lane per computer, write pulse, "+N more · M writing") and A's ladder legend wording.
  - C's starvation gap (dashed forecast to "≈ Out", red span to the reset gate) as an optional expansion of a
    forecast row, not a permanent column.
  - C's year clock (wedge cells, five-step ramp, 53-week per-computer totals) or A's (square cells).
  - B's tray (ring + both windows + resets + Start) as the tray popup.
- Ranking: B > A > C. B's instruments are now all honest and the fewest; A fixed every encoding fault and keeps the
  best live view; C fixed the axis and the bar encoding but its timeline still spends most of its width on marks the
  capacity column already answers, and it brought three new layout defects.

## Instrument table: rows that changed

| Design | Instrument | Encodes | Earns its place | Why (now) |
|---|---|---|---|---|
| A | Running-now computer dots | removed | n/a | Gone; the text says "2 of 3 computers alive" |
| A | Ladder ring | Week left % (arc); one colour rule (amber forecast out, red exhausted/logged out) | Yes | Captioned "WEEK", legend at the foot, same rule on Accounts and tray |
| A | Ladder bar | % left of the window with the least room; reset countdown beside | Yes | Same shape and meaning everywhere; was elapsed fraction |
| A | Readouts dot grids (3) | 14 values as one opacity row | Marginal | No collision at 1280; still redundant with the delta |
| A | Latest replies table | Time, account, model, computer, tokens, first reply | Yes | Speed column removed from the Overview |
| A | Ladder at 12 accounts | Scrolls in its own panel | Partly | Scrolls; horizontal overflow clips the forecast text at 1280 |
| A | Dashed "—" ring | Not read, and also "no limits" | Partly | Two states on one mark; text disambiguates |
| B | ≈ Run-out column | Forecast by window; None / No forecast / n/a; — only for not read | Yes | Was ambiguous "—" |
| B | State counters | removed | n/a | Gone |
| B | Usage dot grids | removed | n/a | Gone; values and deltas remain |
| B | Year clock | 53 × 7 cells, full ramp, busiest cell outlined | Yes | Busiest day findable on the ring |
| B | Accounts stacked chart | Day × account, colour per account, legend, 0–9.4M scale | Yes | Was one hue, no legend |
| B | Accounts row ring | Week (or daily cap) left % | Yes | Added; same helper as inspector and tray |
| B | Tray popup | Ring, tool icon, 5-hour and week % with resets, Start | Yes | Was a bare table |
| B | Ledger capped / pay-per-use rows | Dollar cap left and %, reset; "no plan windows" | Yes | No fake windows; cap bar sits under a "Weekly room" header |
| B | Live rail | Project · account · writing/waiting per named computer | Yes | Text answers "what is running where"; traveller marks vestigial |
| C | Reset Horizon axis | Linear 0–7 d, computed ticks | Yes | Was two unmarked zones; tick labels collide at 1280 |
| C | Capacity column (5h, Wk bars) | % left, fixed length, zero baseline | Yes | New; B's encoding |
| C | Weekly timeline bar | Time to reset (solid), forecast (dashed) and starvation gap (red) only when a run-out exists | Partly | Honest lengths; legend calls the solid span "Measured" |
| C | 5-hour timeline row | Time to 5-hour reset on a 7-day axis | No | Under 3% of the track; all gates look alike; the capacity column does the job |
| C | Flow beads | removed | n/a | Replaced by a live dot by "N open" |
| C | Year clock | 371 wedge cells, five-step ramp; 53-week totals per computer | Yes | Was a glow; now readable at 1280 |
| C | Latest replies "cadence" | removed | n/a | Gone |
| C | Tray popup | Ring + week reset per account, all 12 | Partly | Lists every account; second column clipped with a horizontal scrollbar |
| C | Routed lane (timeline) | Cap reset gate over text | No | Gate label overprints the lane text at every width |
