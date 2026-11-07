# Lazy user: critique, round 1

Protocol: each Overview opened at 1536 wide, Harness dark, looked at for ten seconds from one screenshot; then three
tasks by clicking the page's own controls (start a session on the account with the most room; find what runs on
SAMYMSI; add a second Codex account), clicks counted; then the first-run switch; then what I would never touch.
I use Claude Code daily and Codex sometimes, two accounts, never saw this app, read no paragraphs.

## Design A (Claude) · score 9/10
**What I got in ten seconds**
- Three boxes across the top, one answer each: "Running now 10 sessions · 6 here · 4 elsewhere", "Runs out first
  codex-main · ≈ out Sun 00:56", "Use next service-account-resal" with a teal "Start service-account-resal" button
  right in the box. That is my whole visit.
- Under them, one column per computer with its name on top (DEVELOPMENTPC "here", SAMYMSI, Samys-MacBook-Pro) and the
  sessions inside: account, project, how long, "wrote 4 s ago". I did not have to read a legend to know SAMYMSI has
  three things running and the MacBook has one asleep.
- The "Room" list on the right: one ring per account, big percent, a reset time. The two grey dotted ones with "Read
  now" and "login expired" told me which accounts are not usable without me reading anything.
- I did not get: what "2 of 3 computers alive" changes for me; the difference between the ring percent and the
  "5-hour 98% left" line under it.

**What I had to hunt for (with click counts)**
- Start on the account with the most room: 1 click. The button in the "Use next" box; a toast said "Terminal opened
  on service-account-resal". Nothing to think about.
- What runs on SAMYMSI: 0 clicks. The column is named and on screen.
- Add a second Codex account: 6 clicks plus typing. Accounts tab (1) → Add account (2) → Codex tile (3) → Next (4) →
  Subscription is preselected, Next (5) → name, then "Create and log in" (6) stopped me with "Please fill out this
  field" on Email. I had not typed an email because I did not know which of my two accounts I was about to log into;
  the app should ask after login, not before. The tool tiles do show "Subscription · API key · Routed" per tool, so I
  never wondered which kind Codex takes.
- First run, "2 tools found": the Overview says "Two tools found on DEVELOPMENTPC" with claude-main and codex-main
  rows and a "Sign in" button each; I know what to do. But the Accounts page in the same state shows "limits never
  read" and, two lines lower, "Week runs out ≈ Sun 00:56" on the same account; I would not trust an app that forecasts
  a week it says it has never read.
- First run, nothing installed: "No AI coding tool found" + one "Add account" button; clear. The "Sync with your other
  computers?" box with three choices sits beside it on both first-run screens; I would click "Not now" without
  reading the other two.

**What I would never use**
- The tool filter chips and "All computers / This computer" above the columns; the columns already separate the
  computers.
- The Speed column (≈ 155 t/s) and First reply in Latest replies; I do not know what a good number is.
- The year clock; I would look at it once.
- The Readouts strip (Tokens / Cost / Cached with dot grids), which only appeared after I clicked a filter chip; I did
  not notice it was missing before.

**Must change before it could win**
- Email optional in Add account (or asked after login); the three wizard steps on one page so a Codex account is
  Accounts → Add → Codex → Create (4 clicks).
- In the first-run state, no "Week runs out" line on an account whose limits were never read.
- Drop the Speed column from the Overview's Latest replies.

## Design B (Codex) · score 7/10
**What I got in ten seconds**
- A table of eight accounts, two bars each (5-hour, weekly), a percent and a reset time, a red "Exhausted" on
  codex-main at the top. I got "codex-main is dead" without reading.
- A teal "USE NOW / MOST ROOM service-account-resal" card with a "Start a session" button; got "use that one".
- A "Running now 6 · 3 here" card listing DEVELOPMENTPC 3, SAMYMSI 1 ("sanduq-api · codex-main · Follow"),
  Samys-MacBook-Pro 2. Got it, but it is the smallest text on the page.
- I did not get: the headline "Room to keep going." (it is a mood, not an answer); the four big numbers top right
  "01 02 03 02" (the labels Exhausted / Close / Ready / Not read are tiny and I read them only on the second look);
  "Resets first 18m" (resets what, and does it help me?).

**What I had to hunt for (with click counts)**
- Start on the account with the most room: 2 clicks. "Start a session" opened a dialog (account preselected, a
  folder, "Extra arguments", a "Skip permission prompts" box) and I had to click "Preview launch". The dialog is
  pre-filled, so I clicked through it, but a Start button should start.
- What runs on SAMYMSI: 0 clicks to see "1 · sanduq-api · codex-main"; 1 click on "Follow" opened a card with the
  model, 54 m running and the latest reply. Fine, but the card listed only project names per computer, not the
  sessions, so I guessed from the one name.
- Add a second Codex account: 4 clicks plus a name. Accounts (1) → Add account (2) → Codex tile (3): "Routed" greyed
  to "Not supported by Codex", email marked optional, folder preview "~/.codex-work-design" → "Create and log in" (4)
  → a "Sign in / Codex" dialog with one more button (5). Clean; the only flow of the three that did not ask me for
  anything I did not have.
- First run: one big "Make room for your work." with "Prepare your first account" and "Install a tool"; I would click
  the teal one, although "Prepare" made me pause (I want to "add" an account). The case where tools are already
  installed is hidden behind a small link "Preview detected tools", and clicking it threw a "Bring your computers
  together" sync dialog with a shared-folder path already filled in before showing me the tools; I cancelled it.
- Needs attention, Activity and Latest replies are collapsed "+" rows below the fold; I only found them by scrolling.

**What I would never use**
- "Resets first 18m" card; "Details ↗" on the codex-main card; "Manage accounts ↗" under the table.
- The four counters top right.
- "Extra arguments" and "Skip permission prompts" in the Start dialog.
- "Usage at a glance" (Tokens / At API list prices / Cached) and its Today / 7d / 30d / All switch.

**Must change before it could win**
- The headline must say the answer ("service-account-resal has the most room; codex-main is out"), not "Room to keep
  going."
- "Start a session" starts; the dialog becomes an "Options…" link next to it.
- Running now: the computers' sessions as rows with account + project, same size as the table, not a footnote.
- "Preview detected tools" shows the tools; the sync question comes after, with "Not now" first.

## Design C (OpenCode) · score 6/10
**What I got in ten seconds**
- "Start on service-account-resal — 98% 5h & 100% week left" with a teal "Start service-account-resal" button, top
  left; got "use that one".
- "codex-main will likely run out of its week ≈ Sun 00:56" in the amber box; got "that one is dying".
- A "Running now · 12 live · 6 elsewhere" card with three rows, all "DEV-PC"; I did not see SAMYMSI anywhere in the
  first ten seconds. Three coloured dot chips "12 · 4 · 2" sat in the card's corner with no names.
- I did not get: the whole middle of the page, a table titled "Reset Horizon — Every Account Window Towards Reset"
  with columns "NOW 15:42 · +1.5h · +5h Window · Sat (+24h) · Sun (≈ codex out) · Tue 15:46 · +7d Week" and two bars
  per row with dashed tails and a legend of four line styles. I would not learn this; it is a diagram. A purple tag
  "PROPOSED · MODE: MOST ROOM + NEXT RESET" in the hero means nothing to me. Most text is small monospace.

**What I had to hunt for (with click counts)**
- Start on the account with the most room: 1 click; a green bar said "Started new terminal session on
  service-account-resal (DEVELOPMENTPC)". Good.
- What runs on SAMYMSI: 1 click, but a guess. I clicked the amber "4" chip because amber was the second computer dot
  in the top bar; only after the click did the rows say "SAMYMSI". The chip says 4, the card shows 3 rows with no
  sign of a fourth. The "Sessions 18" nav item was my other guess; I did not take it.
- Add a second Codex account: 4 clicks plus a name. Accounts (1) → "+ Add account" (2) → Codex tile (3), each tile says
  "Supports: Subscription · API key · Routed", email pre-filled with my address → "Create & sign in (terminal)…" (4).
  Then the new "codex-work-design" row appeared with a 100% ring, "5h ↻ 5 h · Wk ↻ 7 d · ≈ 90% left at reset" before I
  had signed in to anything. An account that has never been read should not show numbers; I stopped trusting the
  other rings a little.
- First run: "Your default tool logins are ready. Add a second account or configure multi-computer sync." with four
  rows and a Start each; I know what to do. Nothing installed: a welcome card with a three-line paragraph I skipped,
  six "Prepare X account" buttons, "+ Add first account", and a "Load Owner's 8 Accounts Demo" button inside the app
  frame that a stranger would click and be confused by.
- Needs attention, readouts and the year clock are below the fold next to a five-row Latest replies table; found by
  scrolling, not needed.

**What I would never use**
- The Reset Horizon table and its "5h + Week / 5-hour only / Weekly only / Rings: % left" switches and legend.
- The "SELECTED ROUTE: DEVELOPMENTPC + SAMYMSI → codex-main (73% wk used) → Codex v0.167.0 → OpenAI Direct first
  reply ≈ 0.9s" strip, "Press Esc for finding route", "Inspect codex-main ↗".
- "Harvest 87% before Today 17:30" advice lines; "Folder…" next to Start; the "Test nothing" button in Needs attention;
  "Simulate 3s Redraw".
- The tray popup (two columns of rings anchored at the bottom of the sidebar, covering the page).

**Must change before it could win**
- Running now lists computers by name (tabs or sections "DEVELOPMENTPC · SAMYMSI · MacBook"), never a coloured dot
  I have to guess.
- The Reset Horizon table folds away by default; the first screen is the three answer boxes and the running list.
- A freshly added account shows "—" until its first read.
- Drop the PROPOSED tag, the route strip and the demo button from what a user sees; the empty state without the
  paragraph.

## Across the three
- The one thing a winning design here must have: three plain boxes at the top that each say one answer (use this
  account + Start, this one runs out, this is running where), with the computers named.
- The one thing it must not have: a timeline or diagram I have to learn before the page answers me.
- Parts worth combining:
  - A's three answer boxes and per-computer session columns
  - B's Add account page (one screen, unsupported kind greyed, email optional)
  - A's "Read now" / "login expired" rings for accounts that cannot be used
  - C's green confirmation bar after Start
  - B's tray preview (a plain two-column table of percentages)
- Ranking: A > B > C. A answered all three questions with no reading and one click; B made me read a table and click
  through a dialog; C hid the laptop behind a coloured dot and put a timeline between me and the answer.
