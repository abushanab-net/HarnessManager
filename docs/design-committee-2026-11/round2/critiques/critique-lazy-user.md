# Lazy user: critique, round 2

Same user as rounds 1 and 2 re-score: uses Claude Code daily and Codex sometimes, two accounts, reads no paragraphs,
clicks the biggest obvious thing. Protocol per page at 1536 wide, Harness dark: ten seconds on one screenshot, then
the page's task by clicking, clicks counted; then the first-run state; then what I would never use. Score = how fast
I got my answer and how little I had to think. Every page rendered and every dialog below was opened by me.

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score 8/10
**Still open from round 1**
- The Reset Horizon table still owns the middle of the Overview: every account has two capacity bars on the left and
  two "5h · 0–5h zoom / Week · 0–7d" timeline bars on the right, plus a four-item legend and four view switches. I
  read the same percentage twice and learned nothing from the timeline. The three answer boxes above it are all I use.
- "Proposed: Prefer expiring 5h" tag in the hero and "5h · 0–5h zoom · Proposed: harvest" inside a row: words I do
  not know.

**What I got in ten seconds (Overview)**
- "Use service-account-resal" with a teal Start beside it; "codex-main will likely run out of its week ≈ Sun 00:56";
  "Running now · DEVELOPMENTPC 12 · SAMYMSI 4 · Samys-MacBook-Pro 2" with rows naming the computer. All three
  answers, no reading.

**What I had to hunt for (with click counts)**
- Start on the account with most room: 1 click → "Launch preview: service-account-resal · D:\Projects\resal\sanduq-api".
- The limits popup: 1 click on "Tray limits popup" at the foot of the sidebar; eight rings with "Use next" on top. Found
  instantly because it is labelled.
- Add a second Codex account: Accounts (1) → + Add account (2) → Codex tile (3; "Routed · Unsupported" greyed, email
  optional) → "Create & sign in (terminal)…" (4). New row says "limits not read (—) until first refresh". Good.
- Account detail: 1 click on a row; "Start service-account-resal", limits, login state. I did not need it.
- Nothing I was stuck on in the base.

**What fails**
- The green notice bar sticks across pages: "Launch preview: service-account-resal…" from the Overview was still on
  top of Sessions and the Session page until I clicked Dismiss (1536 dark).
- Account detail: "Bars: used share · dashed mark: ≈ level at reset." legend under the limits; a legend means the bars
  need explaining.

**What I would never use**
- Reset Horizon and its "5h + Week / 5-hour only / Weekly only / Rings: % left" switches; "Folder…" next to Start;
  "Simulate 3s Redraw"; the "Tokens per day, by account" fold and "By model" on Accounts; "Export CSV".

**Must change**
- Fold the Reset Horizon away by default (a "Timeline" disclosure); the first screen is the three boxes and the account
  list with one bar per window.
- Notices clear when I change page.

## Sessions · 9/10 (Designer A)
**What I got in ten seconds**: "18 sessions are running now; open one to follow it live." and a list with account,
tool, computer and a Follow button on every running row. Computers as chips in the filter row: All computers · This
computer · SAMYMSI · Samys-MacBook-Pro.
**What I had to hunt for**: SAMYMSI chip (1) → "4 sessions are running now", every row says SAMYMSI → Follow (2) →
the Session page. Two clicks, no thinking.
**What fails**: the older rows say "not cloned here" on the right where I expected a Follow or Open; I did not know what
it wanted me to do about it. The left rail (Pinned, Recent, Tags) repeats the Projects page in small type.
**What I would never use**: Group by Project / Date / Account / Tool / Tag; the Tags block; "Every project, with costs →".
**Inventory misses**: none I would notice.
**Must change**: "not cloned here" becomes a button or disappears; nothing else.

## Session · 8/10 (Designer A)
**What I got in ten seconds**: title, "LIVE · following", "ran on SAMYMSI", five number tiles, and the conversation
scrolled to the bottom where the last line is "… Glob **/*.css running now". The last thing the agent did: 0 clicks.
**What I had to hunt for**: "Resume here" (1) → a dialog with "Last ran on SAMYMSI", "Opens in this computer's clone of
edge-gateway" and two sentences I skipped → "Resume here" (2) → "Preview: would resume here as a new session continued
from SAMYMSI; no terminal was opened." Two clicks; the dialog is the only paragraph I met on the page.
**What fails**: the five tiles (Tokens, Read from cache, First reply p50, Failed commands, Files changed) sit above the
conversation and push the transcript down; only "Failed commands 0" tells me anything. The transcript text talks about
`settle()` and `hrm sessions` under a title about Rust proxy filters; sample data, but it made me doubt I had the right
session.
**What I would never use**: First reply p50; the All / Changes / Failed switch; the ⋯ menu.
**Inventory misses**: none I would notice.
**Must change**: tiles to one line under the title; the resume dialog to one sentence plus the button.

## Projects · 7/10 (Designer A)
**What I got in ten seconds**: "21 projects ran in the last 7 days; 12 have a session open now." and a cost table.
**What I had to hunt for**: pin Bunyan. Bunyan is in the "Where the tokens and the cost went" table, which has no pin.
The cards below are paged ten at a time with "‹ 1 3 ›" chips at the very bottom; Bunyan is on page 2. Scroll to the
foot, "›" (1), scroll, find the card, click a tiny pin glyph with no label at its right edge (2) → "Pinned to the
sidebar". I guessed the glyph; a lazy user who does not guess gives up at the table. On the Sessions page the same pin
sits beside "Bunyan 14" in the rail and is 1 click, so the Projects page is the harder road to its own feature.
**What fails**: the table rows and the cards are two lists of the same projects; the page chips show "1 3" and I did
not know where 2 went; the "Active" filter is on by default and I did not know what it hides.
**What I would never use**: the share bar; Cost / Tokens / Sessions switches; the sparkline on every card; rename and
tags.
**Inventory misses**: none I would notice.
**Must change**: a search box or the name typed in Ctrl K jumps to the card; the pin is a labelled "Pin" button on the
card and on the table row; one list, not two.

## MCP servers · 8/10 (Designer B)
**What I got in ten seconds**: a grid, one row per server, one checkbox per tool, counts under the tool icons. I
understood "tick the tool" without reading. Headline "11 servers shared. 2 entries left alone." I did not know what
"left alone" means.
**What I had to hunt for**: a tour dialog ("Add an MCP server · 1 of 1 · Add once, then tick the tools…") opened on
arrival and sat over the grid until I pressed Done (1). Then codegraph × Codex checkbox (2) → "Preview: Saved codegraph
for Codex." One real click.
**What fails**: the tour on arrival; "2 left alone · Handwritten entries stay until you take them over." is a sentence I
had to read twice.
**What I would never use**: Info chips; "Copy as JSON"; Import from apps; the Rows 25 selector.
**Inventory misses**: none I would notice.
**Must change**: no tour on arrival (the ? button is enough); "left alone" becomes "2 written by hand, not managed".

## Skills · 6/10 (Designer B)
**What I got in ten seconds**: "3 skill folders need managing." (managing what?), a 255-row grid like MCP's. archify is
the first row and its Claude Code box is ticked: answer in 0 clicks. Typing "archify" in the search box filtered the
grid to one row, also fine.
**What I had to hunt for**: "Install skill" (1) → a dialog whose Source field says "GitHub tree link or folder"; I
pasted a GitHub tree link and clicked Install skill (2) → red "Enter an absolute folder path." Stuck. The dialog asks
for a link and refuses it. I did not finish the task.
**What fails**: the install rejection above (1536 dark); the headline and the "Not managed · 3 / Manage all" box
mean nothing to me; the "Update" chip beside archify is the only update cue and it is grey.
**What I would never use**: Source dropdown; "Link every filtered skill to …" bulk ticks in the header; Check for
updates; Import from a tool.
**Inventory misses**: installing from a GitHub link, which the field itself offers, does not work in the prototype.
**Must change**: Install accepts the GitHub link it asks for (or the field says "Folder" only); the headline says what
is wrong in my words ("3 skill folders are not in the store").

## System prompts · 8/10 (Designer B)
**What I got in ten seconds**: "CLAUDE.md is the file Claude Code reads.", a tool tab per tool, the live file on the
right, two saved versions on the left. Nothing to do; I understood it and left.
**What fails**: nothing I hit. "Careful changes · Enabled" next to "The file now · Live" made me wonder which one is
in force.
**What I would never use**: all of it, in practice; I edit CLAUDE.md in my editor.
**Inventory misses**: none I would notice.
**Must change**: one word that says which version is in force.

## API keys & routing · 7/10 (Designer B)
**What I got in ten seconds**: "The local router is stopped." and a "Start router" button; no providers, no routes.
**What I had to hunt for**: a tour ("Providers · 1 of 3") blocked the page on arrival (1 to skip). "Add provider" (2)
→ a dialog with Id, Name, Format, Base URL, Models, Key. I typed id, name and key and clicked Add → "Enter an HTTP or
HTTPS endpoint." The Base URL field shows https://openrouter.ai/api/v1 as a placeholder but is empty, so I had to
know DeepSeek's URL. Filled it, Add provider (3) → "Provider added in preview". "Start router" (4) → "Router started";
headline became "0 requests. 0 provider paused." Four clicks, one field I had to know, two hints I had to read.
**What fails**: the tour; Base URL not pre-filled from the picked provider; "Format" and "Models (comma separated)" are
questions I cannot answer.
**What I would never use**: Add route; Failovers / Errors / Paused providers tiles; Recent requests.
**Inventory misses**: none I would notice.
**Must change**: pick a provider from a list (OpenRouter, DeepSeek, Custom) that fills the URL and format; Key is the
only field I type; no tour on arrival.

## Apps · 8/10 (Designer B)
**What I got in ten seconds**: "1 tool has an update.", a table with Installed and Newest versions, and one teal button
on the Pi row. Clear.
**What I had to hunt for**: the teal button says "Reinstall 1.5.0", not Update; I paused on "reinstall" (why would I?)
then clicked it (1) → "Update Pi: 0 consoles are running this tool. An update may not replace files in use. Preview the
terminal command?" with buttons Cancel / Update (2) → "Pi update simulated"; headline became "Your tools are up to
date." Two clicks.
**What fails**: the button says Reinstall, the dialog says Update, the question asks about a preview; three words for
one action. "Claude Code has two copies" amber bar with "Remove winget copy…": I did not know whether that was
dangerous, so I left it.
**What I would never use**: the Show checkboxes; Copy path; the Harness Manager box at the bottom.
**Inventory misses**: none I would notice.
**Must change**: the button says "Update to 1.5.0"; the dialog says "Update Pi to 1.5.0? Nothing is running it." and
one button.

## CLI reference · 8/10 (Designer B)
**What I got in ten seconds**: "Every desktop action has an hrm command." and a Start here box with four commands. I
would copy `hrm list` and leave.
**What fails**: nothing I hit; it is a long static list.
**What I would never use**: the lower boxes.
**Inventory misses**: none I would notice.
**Must change**: nothing.

## Settings · 8/10 (Designer B)
**What I got in ten seconds**: section chips across the top (Appearance, Window, Updates, Limits, …) and plain rows with
a control on the right. Readable without reading.
**What I had to hunt for**: a tour ("What each setting means · 1 of 6") blocked the page on arrival (1 to close). "Start
with Windows" is under Window and terminal and was already unchecked in the sample, so my task was done at 0 clicks;
clicking it turned it on ("Would start in the tray at Windows sign-in") and I clicked it back. Light: "Light and dark ·
System / Light / Dark", 1 click → whole app went light, "Saved theme." Overview in light is as readable as dark.
**What fails**: the tour; the i buttons after every label are an invitation to read; "Theme: Harness / VektraGrid" and
"Light and dark" are two rows for what I think of as one choice.
**What I would never use**: Log, Maintenance, Backups, Computers sections; "Listen port"; "Pair a computer".
**Inventory misses**: none I would notice.
**Must change**: no tour on arrival; Theme and Light/dark on one row.

## First run (Overview, Sessions, MCP servers in the "First Run" state)
- Overview: "Limits have not been read yet.", four main profiles each with a Start, "+ Add another account", and the
  sync box with "Not now — this computer only" first. I know what to do: click Start on claude-main.
- Sessions: "No sessions yet. Sessions appear here as your tools write their logs." + New session. Clear.
- MCP servers: "No MCP servers yet." + "Import from apps" and "Add MCP server". Clear.
- The sidebar in first run still shows "Setup · 0 updates" in amber and "Apps · 0 updates" in amber; amber for zero
  made me look for a problem that is not there.

## Across the app
- Is it one app? Mostly. Designer B's pages (MCP, Skills, Routing, Apps, Settings) open a modal tour on arrival; the
  base and Designer A's pages do not, so half the app greets me with a dialog and half does not. B's pages also carry
  a second "Prototype · Ready / Empty / Loading / Error" bar above the frame that the other pages lack. Designer A's
  pages use a left rail inside the page (Sessions) and paged cards (Projects); the base uses one list. Headlines are the
  same voice everywhere ("N things are X.") which helps. Buttons and chips look the same across the three hands; the
  dialogs do not: B's say "Preview · local state only" on a line of their own, the base's do not. Unify: no tours on
  arrival anywhere; one dialog header; one empty-state style.
- The three worst defects, ranked:
  1. Skills › Install skill asks for "GitHub tree link or folder" and then refuses the GitHub link ("Enter an absolute
     folder path."): the task cannot be finished.
  2. Modal tours on arrival at MCP servers, API keys & routing and Settings: every first visit starts with a dialog I
     must dismiss before I can click anything.
  3. Overview's Reset Horizon timeline still fills the middle of the first screen (open since round 1); and on
     Projects, pinning is an unlabelled glyph on a paged card list, two scrolls away.
- Ranking of the twelve pages from best to worst for me: Sessions > Session > Overview/shell > Apps > MCP servers >
  Settings > Accounts/Account detail > CLI reference > System prompts > Projects > API keys & routing > Skills.
