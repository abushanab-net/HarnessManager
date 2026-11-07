# Lazy user: re-score, round 1 (revised prototypes)

Same user, same protocol as `critique-lazy-user.md`: ten seconds on each Overview at 1536 wide, Harness dark; then
three tasks by clicking (start on the account with the most room; what runs on SAMYMSI; add a second Codex account),
clicks counted; then the first-run state; then what I would never use. Owner's note taken: "codex-main is out of its
5-hour window" is a valid thing for the app to say, so I do not hold it against a design.

## Design A (Claude) · 9/10 (was 9)
**What got easier**
- Add account: Email now says "optional" and a Codex account went through with only a name; the "Please fill out
  this field" block is gone.
- First run, "2 tools found": the Accounts page now says "Limits appear after the first read." on both rows and
  forecasts nothing; the contradiction I saw in round 1 is gone. The Overview shows the two tools with a "Sign in"
  each and one "Add account"; I know what to do.
- The Room list no longer says "Read now" on opencode-main and agy-main; it says "no limits · usage only" and "login
  expired". I understood both without clicking.

**What is still a hunt**
- Start on the account with the most room: 1 click, toast "Terminal opened on service-account-resal". Unchanged.
- What runs on SAMYMSI: 0 clicks; the column is named. Unchanged. The DEVELOPMENTPC column now folds after four
  sessions behind "+2 more · 2 writing" (1 click, then "Show fewer"); fine.
- Add a second Codex account: still 7 clicks plus a name. Accounts (1) → Add account (2) → Codex tile (3) → Next (4)
  → Next (5) → "Create and log in" (6) → "A terminal window is open: finish the sign-in to Codex there" → Done (7).
  Three steps for what B and C do on one screen.
- First run, Overview: an empty "Running now · nothing yet" box now sits above the "Two tools found" card, so the thing
  I should click is second on the page.

**Anything new that confused me**
- The Room list grew a four-line legend at its foot ("ring and date: the week · bar: the window with the least room
  · ≈ runs out before its reset · exhausted or logged out · not read"). I did not read it; its presence says the
  rings need explaining.
- A tiny "WEEK" caption under every ring; I did not know what else it would have been.

**What I would never use** (unchanged): the tool chips and the computer filter above the columns; the Readouts strip;
the year clock and Latest replies below the fold; the tray popup's "Open" button.

## Design B (Codex) · 9/10 (was 7)
**What got easier**
- The headline now says the answer: "codex-main is out of its 5-hour window. Use service-account-resal now ·
  codex-main 5-hour resets in 42m." That is the whole visit in one line, and the Accounts page repeats it
  ("codex-main is out; use service-account-resal.").
- "Start a session ↗" starts: 1 click, toast "Launch preview: service-account-resal · recent folder HarnessManager";
  the dialog moved behind "Options…" next to it.
- "Running now 6" lists each computer by name with its sessions as rows ("SAMYMSI · elsewhere · sanduq-api ·
  codex-main · waiting"): SAMYMSI at 0 clicks, no Follow needed.
- The four unlabeled counters top right are gone.
- First run: "No accounts connected." with "Add your first account" (was "Prepare"); "Preview detected tools" now
  opens a plain "4 installed tools found" list with "Use detected profiles →", no sync dialog in the way.
- Add a second Codex account: Accounts (1) → Add account (2) → Codex (3; "Routed · Not supported by Codex" greyed,
  email optional, folder "~/.codex-<name>" shown) → Create and log in (4) → "Complete sample sign-in" (5); a toast
  then says "Added codex-second · sample account. Limits have not been read."

**What is still a hunt**
- Anything beyond the headline is read from the table (two bars and a reset time per row). I stop at the headline, so
  this costs me nothing now, but the table is still the biggest thing on the page.
- Needs attention, Activity and Latest replies are still collapsed "+" rows below the fold; found by scrolling.
- "Resets first 18m · sabushanab-resal" card: I still do not know what it wants me to do.

**Anything new that confused me**
- My first try at the name "codex-work" was refused with "That name already has an account. Choose another name."
  The prototype had kept the account I created in round 1 (it remembers between visits). A second name went through.
- The sign-in dialog asks for a "Signed-in email" pre-filled with you@example.com; I would not know whether to change
  it.

**What I would never use** (unchanged): "Details ↗" on the codex-main card; "Options…"; "Usage at a glance"; the
"Room ▾" show-switch under the table.

## Design C (OpenCode) · 7/10 (was 6)
**What got easier**
- "Running now" names the computers: chips "All (18) · DEVELOPMENTPC 12 · SAMYMSI 4 · Samys-MacBook-Pro 2" and each
  row carries its computer ("edge-gateway · SAMYMSI · samykabu-teksa"). SAMYMSI at 0 clicks for one row, 1 click on
  the "SAMYMSI 4" chip for its list; no guessing from a coloured dot.
- Start on the account with the most room: 1 click, green bar "Started new terminal session on service-account-resal
  (DEVELOPMENTPC)". Unchanged and still the clearest confirmation of the three.
- Add a second Codex account: Accounts (1) → + Add account (2) → Codex tile (3) → "Create & sign in (terminal)…" (4);
  email now optional. The new row shows "— plan limits not read (not read yet)" and a green bar says "limits not read
  (—) until first refresh": the invented 100% ring from round 1 is gone.
- Empty state: "No AI coding tool found on DEVELOPMENTPC", six "Prepare X account" tiles and "+ Add first account";
  the paragraph and the "Load Owner's 8 Accounts Demo" button are gone.
- First-run Overview: four main profiles with "— limits not read" and a Start each, "Add another account", and the
  sync box with "Not now — this computer only" first. I know what to do.
- The purple "PROPOSED · MODE…" tag shrank to "Proposed: Prefer expiring 5h"; the hero reads "Start on
  service-account-resal — 98% 5h & 100% week left" with the button beside it.

**What is still a hunt**
- The middle of the page is still "Reset Horizon — Capacity & Linear 7-Day Reset Timeline": now a "Capacity (% left)"
  bars column *and* a day-by-day timeline with dashed tails, a four-item legend and four view switches. I will not
  learn it; it is the largest thing on the screen and answers nothing I asked.
- "SAMYMSI 4" chip, three rows in the card, no scroll and no "+1 more"; I still do not know what the fourth is.
- First-run Accounts page: every row says "plan limits not read (not read yet)" while two of them also say "Week runs
  out Thu 03:38 at this pace" and "Will likely run out of its week Sun at about 00:56", and the headline says
  "claude-main has the most room: ready to start." I do not trust a forecast on a limit that was never read.
- The "Selected route: DEVELOPMENTPC + SAMYMSI → codex-main (73% wk used) → Codex v0.167.0 · first reply ≈ 0.9s"
  strip, "Esc resets selection" and "Inspect codex-main ↗" are still on the first screen.

**Anything new that confused me**
- Each account row now has two capacity bars on the left and two timeline bars on the right that show the same
  windows; I read the percentages twice before deciding they were the same thing.
- "Runs out first" box lists "claude-main · fable 62% left" and "claude-main 96% left" as two rows; I did not know
  why one account appears twice.

**What I would never use** (unchanged): the Reset Horizon table and its switches; the route strip and Inspect; "Use
87% before Today 16:…" advice; "Folder…" next to Start; "Simulate 3s Redraw"; the tray popup.

## Across the three (updated)
- The one thing a winning design here must have: the answer in words at the top (which account is out, which to
  use, with the Start button beside it) and the computers listed by name with their sessions.
- The one thing it must not have: a table or timeline I must read before the page answers me.
- Parts worth combining:
  - B's one-line headline ("codex-main is out of its 5-hour window. Use service-account-resal now …")
  - A's per-computer session columns
  - B's Add account page (one screen, unsupported kind greyed, email optional, 4 clicks)
  - C's green confirmation bar after Start and after Add
  - A's Room list rows for unusable accounts ("no limits · usage only", "login expired")
- Ranking: A > B > C. A and B both got me my answers with no reading; A wins on the many-times-a-day glance (three
  big boxes, named columns), B wins on the once-a-month add-account (4 clicks to A's 7); C fixed its names and its
  honesty but still puts a timeline between me and the answer.
