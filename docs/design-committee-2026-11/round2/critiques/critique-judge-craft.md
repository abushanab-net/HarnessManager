# Judge 1, craft: critique, round 2

Rendered from disk in headless Edge 156 through playwright-core (Edge refuses `--remote-debugging-port` on this
machine, so the same msedge.exe is driven over the pipe transport; each page got its own browser because something
here kills headless Edge every ~90 s). Every page at 1280×720, 1536×864 and 1920×1080, dark and light, through the
prototype's own switches; dialogs, menus, tabs, the tray, scrolled states, the 8-account, 12-account, first-run and
empty presets; a DOM audit of every fold (type sizes, monospace, primary buttons, overflow) and a Still check
(`document.getAnimations()`: 7, 18 and 1 running pulses in Live on Overview, Sessions and Session; 0 in Still).
Nothing was edited.

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score 8/10
**Still open from round 1**
- Fixed and seen: linear 0–7d axis with NOW/+2d/+4d/+6d/+7d and no collisions; the 5-hour window has its own
  "5h · 0–5h zoom" track; the legend says "Time until reset"; the tray is two columns with wrapping names and all 8
  (and 12) accounts; "Harvest" is tagged Proposed on its lane and on the hero; the capability matrix matches the
  owner's (Claude Code and Pi three kinds, Codex Subscription · API key, the other three Subscription); no text under
  11 px anywhere (DOM audit: zero on every page); first-run Accounts says "Limits have not been read yet." with "—"
  rings and no forecast; Setup counts are 0 in the empty preset; the frame is a true 720-tall fold.
- Not fixed: the "Next reset & pace" column wraps its values mid-number at 1536 and 1920 ("5h ↻ 4 h 08 m · Wk ↻ 4 d 7
  / h", "Wk ↻ 6 d / 14 h") and the hero's fact line wraps "5h ↻ 4 h 08 / m (Today 19:50)" at 1280 light; a column
  that exists to be read at a glance breaks its own units.

**What works**
- The dispatch deck is the strongest read in the app: three cards, three colours by meaning (teal use-next, coral
  runs-out, neutral running), one primary Start, and at 1920 it fills the width without a void.
- The Reset Horizon lanes are now one instrument: ring, two capacity bars, two time tracks, one readout, hover-only
  Start; the hatched coral gap on codex-main reads as "runs out before reset" without a legend lookup.
- Accounts: the finding in 18/700 with the account name teal, every window bar labelled, "read 2m ago", advice line
  in colour by meaning (coral for codex-main's forecast); the 12-account preset holds.
- Account detail is quiet and complete: five readout tiles, the limits panel naming its encoding ("Bars: used
  share · dashed mark: ≈ level at reset"), facts grid with mono only on the folder and CLI line.
- Light theme is a true sibling: same hierarchy, AA ink, the coral and amber darkened; the tray popup matches the
  lanes' rings exactly.

**What fails**
- Overview, all widths: the "?" tour is a centred card titled "Quick Walkthrough — Chrono-Runway Overview" holding
  three numbered paragraphs ("1. Top Dispatch Deck: Answers which account…"). That is the explanation paragraph the
  owner ticked, now behind a button, and it names the design direction to the user.
- Accounts and Account detail: the ⋯ opens a modal titled "Account Actions — service-account-resal" with six
  full-width buttons and a blurred page behind it. Everywhere else in the app (Projects, Session) ⋯ is a popover
  under its button; the inventory says menus open below their button.
- Add account: the step labels are numbered ("1. Choose AI Coding Tool", "2. Account Kind for Claude Code") and the
  profile-name label carries the folder in mono inside the label; design-language.md itself forbids numbered labels.
- Accounts, first-run preset at 1280: the "Provider subscription" plan pill wraps to two lines inside its box and
  "Added Nov 1" wraps beside it; the pill is the only one in the app that breaks.
- Overview at 1536 dark: the "Running now" header's "12 here · 6 elsewhere" sits 11 px under the card title, the
  only card in the deck whose eyebrow and count share a line with different weights.
- The deck's hero "Use service-account-resal" (18/700) is the same size as the Accounts finding, so the Overview's
  one answer is no larger than any other page's heading; the lead's own scale says the dispatch finding may be 18,
  but at 1920 the card has room for 22.

**Must change**
- `white-space: nowrap` (or a wider column) for the Next reset & pace values and the hero fact line at every width.
- Replace the Overview tour card with the step tour the Settings pages use (spotlight, one sentence, Next), and
  strip the design-direction name from it.
- Make the ⋯ on Accounts and Account detail the same popover A uses on Projects; drop the numbered step labels.

## Sessions · 8/10 (Designer A)
**What works**
- Reads as C: the finding in 22/700, facts line, chips, panel cards, hairlines; the left rail (Live now, All
  projects, Pinned with live counts, Recent, Tags) is in the rail's own vocabulary.
- "Running now" as the first group with its count and tokens in the header; rows in one weight hierarchy: title
  700, meta muted, account, tool icon with name at 1536+, computer dot with name, tokens, "live" in teal, one
  neutral Follow per row.
- The one-project view (back link, name with rename pencil, pin, tag, path in mono, five stats, Accounts and Tags
  chip rows, About panel with the area sparkline) is a complete second head without a second style.
- New session and Tags dialogs share the lead's modal: head, ✕, fields, Cancel + primary right.
- Empty and first-run states are one card with the true zero and one action.

**What fails**
- All widths: the H1 colours "18 sessions are" amber. In this language amber means pressure or a forecast; here it
  is decoration on a healthy count (Projects does the same with "12").
- 1280 dark: account names truncate in the rows ("service-acco…", "sabushanab-…", "opencode-m…") while the tool
  column drops its name; the language says long names wrap, never shrink or clip.
- The meta line "127 msgs · abushanab-net\HarnessManager" is entirely JetBrains Mono (9 mono leaves per fold); only
  the path qualifies.
- New session dialog: the Folder control is a native select rendered in monospace ("No folder chosen"), the only
  mono form control in the app; its hint reads "Preview: Choose folder… opens the system picker in the app", a
  prototype note inside product copy.
- The tag chips use six hues (teal, violet, amber, emerald, coral, blue) inside a rail that otherwise reserves
  coral and amber for state; "infra" in coral reads as a failure.

**Must change**
- Plain ink for the H1 count; wrap not clip at 1280; mono only on the path; a normal select for Folder.

**Inventory misses**: paging footer ("‹ 1–50 of 142 ›") not seen at 1536 with 115 sessions; "Show all N sessions"
not seen under a group.

## Session · 9/10 (Designer A)
**What works**
- The best page in the app. Head: back link with the project, title 22/700, one meta row (LIVE · following pill in
  teal, account · tool, tag, model in mono, started, turns, tokens · $≈), actions right with one split button.
- Five readout tiles in the lead's `.account-stat` style; the conversation panel head carries the counts and the
  All / Changes / Failed segment in `.filter-chip`.
- Steps are a disciplined log: ✓ in teal, tool name in 600, target in mono (a path, so allowed), duration or
  "+12 −3" right-aligned; an opened step shows a real diff with coral and emerald lines; "running now" on the last.
- Bubbles: user right in teal-dim, assistant left on card surface with the model in the eyebrow; "↓ Latest · follow
  live" is a teal pill at the panel's foot.
- The caret menu and the ⋯ menu are true popovers anchored under their buttons.

**What fails**
- 1920: step rows run the full 1,700 px, so a 2-second duration sits 1,400 px from its tool name; the log wants a
  max measure or the duration beside the target.
- 1280: the meta row wraps "4.82M tokens · $1.79 ≈" onto a third line under the pill row.

**Must change**
- Cap the conversation column (or move durations next to the target) at 1920.

**Inventory misses**: "Show 1,990 more steps" fold not seen (fixture has 62 steps); Export dialog not seen (the
button exists); Resume-here dialog not reachable from this local session.

## Projects · 8/10 (Designer A)
**What works**
- Finding + facts + range and sort chips on one line; five tiles in the lead's readout style; the "Where the tokens
  and the cost went" table with a segmented stacked bar, swatches, share tracks and right-aligned numbers is the
  cleanest table in the app.
- Cards: name 700, remote in mono, tag chips, area sparkline, $ and tokens right, sessions · accounts, "3 live" with
  computer dots, "not cloned here" in amber with its dot; the pager at the foot ("‹ 1 2 3 › · 1–10 of 21 · 10 25 50
  a page") is in chips.
- The ⋯ popover (Open its sessions, Unpin, Rename, Merge, Reset name in coral last) is the reference menu for the
  whole app.

**What fails**
- All widths: "12" in the H1 is amber (decorative colour, see Sessions).
- The three card actions (pin, play, ⋯) are 12 px glyphs with no hover affordance at the card's far right; the
  "New session here" play glyph is the only triangle icon in the app and is not the `.btn-lane-start` word button
  the language gives for row starts.
- Tile values are 20 px while the Session tiles are 20 px and the Overview readouts 18 px; the Projects fold uses
  nine type sizes (DOM audit) against five on the Setup pages.
- Empty state: "A project is a folder your tools ran in." is an explanation line under the empty heading.

**Must change**
- Plain H1 count; word buttons (Pin, Start, ⋯) or at least 16 px icons with labels on hover; one tile scale.

**Inventory misses**: double-click rename-in-place not tested; the "flash once" on a changed tile not observed.

## MCP servers · 8/10 (Designer B)
**What works**
- Finding "11 servers shared. 2 entries left alone." with the amber "2 left alone" chip and its one-line reason;
  Import and Add as secondary + primary.
- The matrix is C-flavoured: tool icons with names and per-tool counts as column heads, name 700 + transport chip +
  Info chip, command in mono (allowed), description muted, checkboxes in teal, Edit + ⋯ right.
- Info dialog: connection, description, "In each tool" with icon, Written / Not written pills and the target file
  in mono; Close + Edit.
- Add/Edit form: Name, Form / JSON segment, Transport, Command, Arguments, Environment variables, Write to with
  icons and "(not installed)", Description disclosure; fits 864.

**What fails**
- On first entry the page is hidden behind a tour modal ("Add an MCP server · Preview · local state only · 1 of 1 ·
  Add once, then tick…"): a centred generic dialog with no spotlight, not positioned beside the part it names, and
  carrying prototype wording. Every B page does this; A's pages have no tour at all; the lead's Overview tour is a
  third design.
- Every B dialog's first line is "Preview · local state only" in muted type under the title (Add MCP server, Info).
- The "Info" chip looks like the transport tag beside it (same shape, same weight), so one of two identical pills
  is a button.
- Dialog footers are left-aligned (Cancel, Add server) while the lead's and A's are right-aligned.

**Must change**
- Remove "Preview · local state only" from product dialogs; right-align footers; make Info an icon button.

**Inventory misses**: row ⋯ (Copy as JSON, Remove…) not opened; "No server matches the search." not seen.

## Skills · 8/10 (Designer B)
**What works**
- Finding "3 skill folders need managing." leads with the exception; Installed 258 / Discover tabs in filter chips;
  the store path in mono; Source select and "Not managed · 3" chip right.
- Header checkboxes with counts per tool (255 ✓, 4 indeterminate, 0 ☐) are the one place in the app where a
  bulk action is drawn honestly; Update tag-buttons beside names; ⋯ right.
- The Not managed panel under the table with Manage all and per-row Manage; Discover with repository tags, "Install
  into" icon chips and "Remembered for the next install", Installed tags in teal-dim.
- Import from a tool and Install skill dialogs are short and in the lead's form components.

**What fails**
- The same tour modal on entry and "Preview · local state only" in every dialog.
- Install skill's "Folder…" is a full-width secondary button under the Source field, and Import's "Import" is a
  full-width primary: two button widths that exist nowhere else.
- At 1920 the Skill column is 420 px wide for 15-character names and the description runs under it in 12 px; the
  table would read better with the description in its own column.

**Must change**
- Normal-width buttons in dialogs; the Preview line out.

**Inventory misses**: the left-alone notice with "Replace the copy with links" not seen; "No skill matches." not
seen.

## System prompts · 8/10 (Designer B)
**What works**
- The closest page to the lead's own: tool tabs with icons, the left list of "The file now" (Live pill) and saved
  prompts (Enabled pill), the editor with Title, path in mono, body, and a foot that states "This prompt is in the
  file now." before Delete… / Show live file / Save.
- The Codex tab shows a different file and a "Preserve this handwritten instruction" body: the hand-written file
  survives, visibly.
- Empty preset: "No prompt file yet." with "not there yet" and "(the file is empty)" rather than invented content.

**What fails**
- Editor: the "Enabled in Claude Code" pill is stretched to the full 1,000 px editor width (1536 dark, prompt
  selected), a status pill drawn as a bar.
- The tour modal on entry ("One list per tool · 1 of 2").
- The live view's code block is a 420 px tall empty box for three lines of text at 1536; the panel does not size to
  its content.

**Must change**
- Pill at its natural width; code block sized to content with a minimum.

**Inventory misses**: "Use this prompt" confirmation toast and the Delete confirm not seen.

## API keys & routing · 7/10 (Designer B)
**What works**
- When populated (via the page's own fixture action): finding "1,204 requests. 1 provider paused.", the router card
  with four stats, the route card with its provider chain as cards joined by → and "Paused · retry 4m" in amber,
  the Providers table with Saved / Closed in teal and "Open · 5 failures" in coral, the Recent requests table with
  "—" for unread tokens. All in the lead's components.
- Add provider is a correct form (Id, Name, Format, Base URL with its hint, Models, Key) in `.form-field`.

**What fails**
- In the 8-account preset the page opens as "The local router is stopped." with 0/0/0/0 and three empty cards; the
  owner's data state is only reachable by a "routing-fixture" control. The page the committee sees is the empty one.
- The tour modal on entry ("Providers · 1 of 3"); "Preview · local state only" in Add provider; "Production:
  Windows Credential Manager. This preview discards the key." under the Key field.
- Empty cards are 140 px tall with a bold "No routes yet." and a muted sentence, three in a column: the emptiest
  fold in the app at 1536 and 1920.
- Dialog footer left-aligned; "Add route" is a teal primary while disabled (it reads as enabled at 1280).

**Must change**
- Ship the populated state as the 8-account preset; stack the three empty cards into one; a visibly disabled Add
  route; footers right.

**Inventory misses**: Add route dialog not opened (its button is disabled with no providers); provider Remove
confirm not seen.

## Apps · 8/10 (Designer B)
**What works**
- Finding "1 tool has an update." and facts; the two-copies warning as an amber `.notice-bar` with "Remove winget
  copy…" on its right; the second copy as an indented sub-row in mono with Remove….
- Table: icon + name, path in mono, Installed, Newest (amber when newer), Installed by, Action, Show checkbox; "Not
  installed · 1" group; the Harness Manager card with "Update available" pill and "Settings › Updates" link.
- The Install dialog lists methods with their commands in mono and a "Use method" per row.
- The empty preset is honest: six Not installed rows with Install… or Website, "checked —".

**What fails**
- "Up to date" is drawn as a teal-dim chip in the Action column, the same shape as the "Reinstall 1.5.0" primary
  beside it, so a status looks like a button in a column of buttons.
- Check for updates shows a top notice "✓ Preview: Vendor-version fixtures checked; 1 update available." with the
  word Preview in product copy.
- The tour is absent on this page (per inventory) but the page still opens the same modal pattern for Install; the
  Install dialog's rows are 48 px tall with no hover state.

**Must change**
- Status as text, not a chip, in the Action column; drop "Preview:" from the notice.

**Inventory misses**: Update-with-consoles-open confirm not seen; ⋯ menu not opened.

## CLI reference · 7/10 (Designer B)
**What works**
- Finding "Every desktop action has an hrm command." with "--json" in mono in the facts line; a "Start here" block
  and seven group cards in a two-column grid; commands in mono teal at 600, meanings in Inter: the only page where
  mono is the content and it stays disciplined (23 mono leaves, all commands).

**What fails**
- 1920 light and dark: in "Start here" the command sits at x=223 and its meaning at x=1,065; 840 px of nothing
  between a command and what it does. In the group cards the gap is 300–400 px. The eye cannot pair them.
- 1280: the group cards' meaning column wraps to two lines ("Accounts, plans, limits, tokens and open / consoles")
  while the command column has 100 px to spare.
- The Computers card at the foot is full-width while every other group is half-width, so the grid ends on an odd
  card.

**Must change**
- A fixed command column (max-content) with the meaning starting 24 px after it; a max width for the page.

**Inventory misses**: none; the seven groups and examples are present.

## Settings · 8/10 (Designer B)
**What works**
- Finding + facts, a chip row that jumps to sections, each section a `.panel-card` with "N settings" and a fold
  caret; each row: title 600, ⓘ icon button, one-line description, control right (segments in filter chips,
  checkboxes with labels, inputs with Save).
- Computers: the two transport cards with "Selected" pill, Carry transcripts / Presence / Direct connection rows,
  the "3 computers · Synced through a folder · last round 2m ago" line, then three computer cards with a coloured
  dot, OS · version · state (amber "heard 4m ago"), a progress bar, counts, reach line with ⓘ, Address + Save +
  Forget. The Pair dialog's six digits in 32 px teal are the one moment of theatre in the app and earn it.
- Backups: Automatic backups segment + Keep, the backups list with Rename / Restore… / Delete…, the archive rows,
  Restore and Update-and-restart confirms in the lead's dialog with the danger button last.

**What fails**
- The tour modal on entry ("What each setting means · 1 of 6 · Focus an info button…"), and step 2 ("Save") draws a
  teal outline around the Harness/VektraGrid segment behind a centred dialog: the spotlight and the text are not
  together.
- The "Keep" select is 450 px wide for a two-digit value; the Custom days input is 360 px; native selects and inputs
  are not sized to their content anywhere on the page.
- Row controls begin at x=694 at 1280, leaving a 250 px gap between every description and its control; at 1920 the
  gap is 600 px.
- "Preview · local state only" in Join a hub, Listen port, Forget, Update and restart; "simulated code · no peer
  connection" under the pairing code.

**Must change**
- Content-sized selects and inputs; controls start after the description column, not at mid-page; the Preview
  lines out.

**Inventory misses**: Recovery key dialogs (Show again / Enter) not reachable in the folder-sync state; the Welcome
back / Restore-from dialog not opened; Invite a computer not reachable without a hub.

## Across the app
- **Is it one app?** Mostly, and more than I expected: the rail, topbar, page head (finding + facts + actions),
  `.panel-card`, chips, tables, rings, teal primary and both themes are shared by all three hands, and the DOM audit
  finds no page with text under 11 px, no horizontal overflow, and monospace only on paths, commands and logs
  except A's session meta. The three hands show in five places, and each is cheap to unify:
  1. **Tours.** B: a centred "1 of N" modal with "Preview · local state only", opening on first entry to every Setup
     page before the page is seen; the lead: a three-paragraph "Quick Walkthrough" card on Overview; A: none.
     Unify on one step tour with a spotlight and a one-line popover beside the part, started from "?" and on first
     visit; no design-direction names, no Preview labels.
  2. **Menus.** A's ⋯ is a popover under its button (Projects, Session); the lead's ⋯ on Accounts and Account detail
     is a modal titled "Account Actions — name" with six full-width buttons. Unify on A's popover.
  3. **Dialog footers.** Lead and A: Cancel + primary right-aligned; B: left-aligned, with occasional full-width
     primaries (Import, Save, Folder…). Unify on right-aligned, natural width.
  4. **Colour in the finding.** Lead: teal for the account name, coral/amber for the exhausted part, by meaning; A:
     amber on a count ("18 sessions are", "12"); B: plain. Unify on the lead's rule: colour only where it means
     room, pressure or failure.
  5. **"Preview" wording.** B prints it as the first line of every dialog and in notices; A prints it as a hint under
     the Folder field; the lead prints none. One prototype label in the studio bar, none in the product.
- **The three worst defects, ranked**
  1. The B-page tour: five pages open behind a generic centred modal carrying "Preview · local state only" and no
     spotlight; the owner's first sight of MCP servers, Skills, System prompts, Routing and Settings is a dialog, and
     the lead's own Overview tour is a wall of explanation. (All widths, both themes.)
  2. The base's Reset Horizon "Next reset & pace" column and hero fact line wrap mid-value ("4 h 08 / m", "4 d 7 /
     h") at 1536 and 1920, and the first-run "Provider subscription" plan pill wraps at 1280: the flagship
     instrument breaks its own numbers.
  3. API keys & routing ships as the empty router in the 8-account preset, with three tall empty cards; the populated
     state (which is good) is hidden behind a fixture control the owner will not find.
- **Ranking, best to worst:** Session > Sessions > Skills > MCP servers > Overview > Accounts > Apps > Account detail
  > Settings > Projects > System prompts > CLI reference > API keys & routing. Session is the one page I would
  implement as drawn; Routing and CLI need their layouts redrawn before they read as this app.
