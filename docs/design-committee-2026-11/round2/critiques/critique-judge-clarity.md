# Judge 2 (clarity): critique, round 2

Method: `round2/app/index.html` opened from disk in headless Edge via playwright-core (port 9452, own scratchpad),
every page rendered at 1280×720, 1536×864 and 1920×1080 in Harness dark and light through the page's own switches,
8 / 12 / first-run / empty presets where they apply, every action clicked, every dialog opened, Tab and Escape pressed
in each dialog, text contrast and minimum font size computed from the rendered DOM, reduced-motion checked in a
separate context, screenshots looked at. Capability matrix checked against the owner's: API key = Claude Code,
Codex, Pi; Routed = Claude Code, Pi; the rest Subscription only. Nothing was edited.

Whole-app facts that hold on every page: zero text nodes under 4.5:1 in either theme; smallest text 11 px; zero
console or page errors; Still and `prefers-reduced-motion` leave zero running animations; the rail reaches all
twelve pages; the official tool SVGs and the Harness mark are used throughout.

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score 8/10

**Still open from round 1**
- None of my round-1 items is still open. Verified on screen: the capability matrix (Claude Code Sub · API key ·
  Routed; Codex Sub · API key; Pi Sub · API key · Routed; OpenCode, Antigravity, Oh My Pi Sub only; unsupported kinds
  greyed with native `disabled`, "Unsupported" and the reason "Routed accounts are supported by Claude Code and Pi
  only."); the ruler reads NOW · +2d · +4d · +6d · +7d with no overprint at 1280; the routed/cap row is its own
  caption line; every 5-hour window has a labelled "5h · 0–5h zoom" track; the legend says "Time until reset"; the
  Harvest rule is tagged "Proposed: harvest" on the lane and "Proposed: Prefer expiring 5h" on the deck; Running now
  lists all 18 sessions with named computer pills (DEVELOPMENTPC 12 · SAMYMSI 4 · Samys-MacBook-Pro 2); every risk
  chip names its window; the tray holds 8 and 12 accounts in two columns with no clipping; dialogs trap Tab and
  Escape returns focus to the opener; first run and empty presets are zeroed ("0 live · 0 elsewhere", "1 computer",
  "Limits not read", $0.00, MCP 0, Skills 0, 0 updates).

**What works**
- The owner's sentence is answered in the deck without reading a paragraph at 1280 dark and light: "Use
  service-account-resal" with Start; "codex-main will likely run out of its week ≈ Sun 00:56, 2 d 14 h before reset."
  with "Measured 73% used (27% left), resets Tue 15:46, read 18s ago · pace forecast ≈."; "Running now · 12 here ·
  6 elsewhere" with project, computer, account and Follow per row.
- Honest marks everywhere: measured solid, ≈ on every forecast and list price, — for not read ("— login expired",
  "— limits not read"), "No plan limits · Usage only" for OpenCode and Antigravity instead of a fake ring; the
  12-account state carries a true exhausted-now case ("claude-audit is exhausted now (0% 5-hour left, resets in 42 m)")
  and a money cap ("pi-openrouter · Daily cap · $35.20 left").
- Accounts page is complete: the finding as the head ("service-account-resal has the most room: 2% of its 5-hour
  limit and 0% of its week used."), one-line basis, 7d/14d/30d, Export CSV (real download + notice), Start in a
  folder… (account, recent folder, skip-permissions with the real flags, extra arguments), Add account, the daily
  chart and By model folded, tool chips, groups with logged in / consoles / next reset / tokens / cost, rows with
  ring, every window with % left and ↻ reset, the forecast line, 7-day pace bars, tokens, list cost, read-ago,
  refresh, Start, ⋯ with the seven actions, open count. First-run Accounts says "Limits have not been read yet."
  with dashed pace bars and "never".
- Account detail has the four tabs and everything the inventory lists: five tiles, Limits with "Bars: used share ·
  dashed mark: ≈ level at reset.", Login and folder facts (Started with `CLAUDE_CONFIG_DIR`, CLI line, "Follows
  main… Own… Live limits off"), Log in again (a sign-in dialog with "Open the sign-in page (preview)", the URL, a
  code field, Send, Cancel sign-in, Done), tokens per day 30 days, by model, Open now with Show window, Recent
  sessions with All N; Settings has Name, Expected email, Shared with the main profile + Sync now, Daily spend cap
  on the routed account, Remove account → "Also delete its folder", and on a main profile "Hide main profile" with
  no folder choice.
- Add account validates (name pattern, duplicate, email, key, endpoint scheme, route), the Claude subscription path
  shows the code step, "Verify code & finish" creates the profile unread ("Created preview profile claude-judge-x
  (~\.claude-judge-x) — limits not read (—) until first refresh.") and Escape restores focus to "+ Add account".
- The tray popup (8, 12, empty: "No accounts configured") shows ring, tool icon, name and ↻ reset per account, a
  "Use next" line, "No plan limits" for usage-only tools and the daily cap for the routed one; a ring click opens
  the account.

**What fails**
- Tray popup is not reachable by keyboard: after "Tray limits popup" is pressed, focus stays on the trigger and Tab
  goes to CLI reference and Settings in the rail, never into the popup; opened by mouse, Tab from inside walks out
  to the page's Follow buttons and chips; Escape closes it but leaves focus on whatever was last tabbed (1280 dark).
- Two lead dialogs open with focus on `body`: the Request log and the `?` tour (focus=BODY on open; the only
  focusable element afterwards is Close). Design-language rule: "Opening focuses the first control."
- Shared notice state leaks into dialogs: after visiting System prompts and saving a prompt, opening Add account on
  Accounts shows "Preview: Saved prompt." in the dialog's own error strip (1280 dark, screenshot
  `add-oauth-step`). The strip is for this dialog's validation; it is showing another page's toast.
- Accounts search with no match: typing "zzzq" in Ctrl K empties the page under the chips with no "No account
  matches the search." line (1280 dark). The inventory names that text.
- The Add-account Claude sign-in step has no "Open the sign-in page" link, only "Paste a code or callback URL below";
  the Log-in-again dialog on Account detail has the link and the URL. The first-run stranger has nowhere to click to
  get a code.
- The Accounts row ⋯ opens a centred modal titled "Account Actions — service-account-resal" rather than a menu under
  the button (inventory: "⋯ menus open below their button"); Designer A's and B's pages use popovers, so the same
  glyph behaves three ways in one app.
- The Settings "Limits shown as Left / Used" control and the Overview's "Rings: % left/used" toggle are two controls
  for one setting; I could not confirm they move together.

**Must change**
- Move focus into the tray popup on open, trap Tab there, and return focus to the trigger on Escape; focus the first
  control of the Request log and tour dialogs.
- Scope the notice/toast to its owner: clear it on page change and never render it in a dialog's error slot.
- Show "No account matches the search." when the Ctrl K filter empties the Accounts page.
- Add the "Open the sign-in page" link (with the URL) to the Add-account code step.
- Make the row ⋯ a popover under the button, the same component A and B use.

**Inventory misses (base)**
- Accounts: the "No account matches the search." empty text (inventory, Ctrl K section).
- Add account: the link that opens the browser for the Claude code (inventory: "a link opens the browser, the code is
  pasted back").
- Account detail › Profile settings for an API-key account (API key + Replace key, Endpoint and model) — no API-key
  account is in either fixture, so not seen; routed cap and main-profile hide are seen.
- Tray: the "∞ · API key, no limit" ring and the red dashed "sign in to read its limits" ring are not distinguishable
  from "No plan limits" (agy-main, login expired, shows the same grey "—" as OpenCode); the hover title
  "<account> · week resets … (2 d left)" not seen.
- ⋯ menu placement (below the button) — replaced by a modal.
- Everything else in round1/inventory.md was found on screen (shell, Overview, Accounts, Add account, first run,
  empty, 12 accounts, tray).

## Sessions · 8/10 (Designer A)

**What works**
- The finding is the head and changes with the data: "18 sessions are running now; open one to follow it live."
  (all), "2 sessions are …" (tag filter), basis "115 sessions · 7 days · 7 groups · 3 computers · tokens from the
  logs · cost ≈ list prices"; New session is the one primary.
- The rail: Live now (count), All projects, Pinned with live counts and pin icons, Recent with counts and "8 more…",
  "Every project, with costs →", Tags with counts; the selected item is marked.
- Filter bar: active chip with ✕ ("sanduq ×"), Group by Project / Date / Account / Tool / Tag (Project hidden inside
  a project), 7d / 30d / All, All computers / This computer / SAMYMSI / Samys-MacBook-Pro, "Search above searches
  every session"; inside a project the Accounts and Tags chips.
- Running now always sits at the top as its own group; groups show root in mono, ⋯, "N sessions · tokens · $cost",
  New session, fold; "Show all 25 sessions" under a trimmed group.
- Rows carry every cell: status dot, title, mono line ("99 msgs · resal\sanduq-api", "continued from
  Samys-MacBook-Pro · 210 msgs · …"), tag chips, account, tool icon, computer dot + name, tokens, when, and one
  action: Follow, Resume, Resume here, or "not cloned here".
- Search: "wiki" from any page (Enter on CLI reference opened Sessions with the chip), "No sessions match." on a
  miss; New session dialog (Folder select, account chips, "Skip permission prompts --dangerously-skip-permissions"
  with its warning, Start session disabled until chosen); Tags dialog with the hint and every tag as a checkbox;
  Resume-here dialog word for word from the inventory; project head with pencil, pin, tags, repo path, five stats,
  ⋯ (Open its sessions / Unpin / Rename / Merge / Reset name), in-place rename (input focused), Merge dialog; the
  About panel with This project, 7 days chart, Running, Accounts with share. Empty: "No sessions yet." + Add account.

**What fails**
- The rail's "All projects" count changes with a filter: 113 unfiltered, 223 with the sanduq tag chip on (1280 dark,
  `sessions-rail-project` vs `sessions-showall`). A total that grows when you filter is not trusted again.
- No paging foot: with 113 sessions there is no "‹ 1–50 of 142 ›"; only per-group "Show all N". The inventory's
  50-a-page foot is missing.
- No archived session anywhere in the fixture: the "archived" pill, the no-action state and "it can be read, not
  resumed" are never shown.
- No row with a red failed-command dot ("N failed commands" on hover) in the list, although session s2 has one
  failed command on its page.
- About panel: "Folders (up to 6, N more)" is not drawn for a multi-folder project.
- The tool is an icon only in rows; a stranger must hover to learn Pi from Oh My Pi.

**Must change**
- Fix the All-projects count under filters; add the 50-a-page foot.
- Seed one archived and one failed row so the states are designed, not promised; draw the Folders list in About.

**Inventory misses**
- Paging foot "‹ 1–50 of N ›"; archived pill and non-resumable reason; failed-command red dot; About › Folders;
  tool name on the pill (icon only). Double-click rename and the live two-second append could not be tested.

## Session · 8/10 (Designer A)

**What works**
- Head: "← Sessions · HarnessManager", the title, "LIVE · following" pill, "claude-main · Claude Code", tag chip
  with editor, model in mono, "started Today 14:30 · 12 turns", "4.85M tokens · $1.80 ≈"; Export as Markdown; Resume
  in Windows Terminal as a split button whose caret menu holds "Open sessions in Windows Terminal / in a console
  window"; ⋯ "Open the log file folder".
- Five tiles (Tokens, Read from cache, First reply p50, Failed commands red when 1, Files changed); Conversation head
  "12 turns · 64 steps · 5 changed files · 0 failed" with All / Changes / Failed; bubbles "YOU · 14:02" right and
  "CLAUDE CODE · 16:48 · claude-opus-4-6" left with Markdown; steps ✓ / … with tool, mono input, "+14 −3", "1.8 s",
  "running now"; a click unfolds a step; the 2,026-step session shows first and last 5 with "Show more steps" and
  expands; "Nothing failed." in Failed; "↓ Latest · follow live" appears when scrolled up; the conversation scrolls
  inside its panel so the head and tiles stay put at 1280×720.

**What fails**
- The "↓ Latest · follow live" button could not be clicked by the automation within 4 s (it is present; the click
  timed out twice) — check it is not covered by the scroller or re-created on each tick.
- A session from another computer (meta "ran on LAPTOP", "Resume here" as the primary, or the reason it cannot) was
  not reachable: clicking a remote row opened the Resume-here dialog instead of the session page.
- No archived session page ("archived: it can be read, not resumed"), no "This session has no messages yet.", no
  "Fetching from the folder…" toast in the fixture.

**Must change**
- Let a remote row open its session page (the Resume-here button is the action, the row is the link) and draw the
  remote and archived heads; make ↓ Latest a stable, clickable node.

**Inventory misses**
- Remote session head and Resume-here-primary state on the page; archived state; "no messages yet"; the first-fetch
  toast. Thinking steps omitted could not be verified.

## Projects · 8/10 (Designer A)

**What works**
- Finding as head: "23 projects ran in all time; 12 have a session open now." basis "All time · 3 computers · cost ≈
  at list prices"; 7d / 30d / All; Active / Cost / Tokens / Name.
- Five tiles exactly as listed (Tokens · all time with N sessions, At list prices with "$4.37 a day ≈", Projects with
  8 accounts, Running now with the names, Busiest with "18% of tokens · 13% of cost").
- "Where the tokens and the cost went": Tokens / Cost / Sessions segment, stacked bar, swatch rows with share track,
  Cost ≈, Tokens, Sessions, Share, "18 other projects".
- Cards: name with pencil, remote or folder in mono, tag chips with editor, area sparkline, "$35.17 · 139.7M tokens ·
  77% priced", "70 sessions · 4 accounts", "3 live" or last activity, computer dots and "not cloned here", pin, New
  session, ⋯ (the five items); pin toggles with "✓ Pinned to the sidebar"; rename in place focuses the input; per
  page 10 / 25 / 50 and page numbers. Empty: "No sessions in this range yet." with one line and Add account.

**What fails**
- The "✓ Pinned to the sidebar" notice survives a switch to the empty preset and sits above "No sessions in this
  range yet." (1280 dark) — the same stale-notice leak as the shell.
- The tile numbers' flash on change and the computer-dot hover names could not be verified.

**Must change**
- Clear the notice on data/preset change; otherwise implement as drawn.

**Inventory misses**
- None seen. Flash-on-change and hover names untested.

## MCP servers · 9/10 (Designer B)

**What works**
- Head: "11 servers shared. 2 entries left alone." basis "27 entries written · 6 tools · checked 3m ago"; status
  pill "2 left alone" with "Handwritten entries stay until you take them over."; Import from apps ("No new MCP
  servers in the fixture."), Add MCP server primary.
- Matrix: a column per tool with icon and count, rows with name, transport tag, Info, command in mono, description,
  a checkbox per tool, Edit, ⋯ (Copy as JSON, Remove…); a tick writes and says "Saved blender for Claude Code.";
  Info dialog with Connection · stdio, the line, "In each tool" with Written / Not written and the file and part;
  Remove confirm; Add / Edit dialog with Name*, Form / JSON, Transport, Command*, Arguments and Environment rows,
  Write to with icons and "(not installed)", "Description & links · never written to a tool" disclosure, Cancel /
  Add server; empty name → "Give the server a name using letters, digits, dots, dashes or underscores."; search
  "No server matches the search."; pager "1–11 of 11 · Rows 25"; empty "No MCP servers yet."; loading and
  "Something went wrong. / Try again" views; 1920 stretches cleanly.

**What fails**
- The "Off" tag (no tool ticked) and the left-alone offer ("Left alone, because a hand-made entry… Edit the server
  here and save to take it over") never appear: no row in the fixture is untouched and no tick hits a hand-made
  entry.
- The page tour is a one-step centred modal ("1 of 1 · Add once, then tick the tools…") without the spotlight the
  Settings tour has.
- Add / Edit is a dialog, not the page with a back arrow — allowed, but at 1280 the dialog scrolls inside itself with
  the footer out of sight until you scroll.

**Must change**
- Seed one untouched server and one left-alone conflict so the Off tag and the take-over note are drawn.

**Inventory misses**
- "Off" tag; the left-alone note in the Info dialog and the tick-time offer; per-tool "not read yet"; the bad-JSON
  toast ("The JSON is not a server: …") not verified.

## Skills · 8/10 (Designer B)

**What works**
- Head "3 skill folders need managing." / "258 skills share one store."; basis "258 in store · 255 managed · 3 not
  managed · 2 updates"; Check for updates / Import from a tool / Install skill; Installed 258 / Discover tabs; the
  store path in mono; Source select; "Not managed · 3".
- Table with Skill, Source, a column per tool with count and header checkbox (indeterminate shown), rows with the
  Update tag-button, clamped description, source, ticks, ⋯ (Update from source, Open folder, Remove…); the header
  tick writes once ("Unlinked 255 filtered skills from Claude Code."); Not managed panel with Manage all / Manage →
  "Now managed here: client-release, local-audit, team-notes"; Discover with search field, repo tags with ✕,
  owner/repo + Add, Install into chips "Remembered for the next install", per-repo sections with path, description,
  Install / Installed; Install skill and Import from a tool dialogs as listed; "No skill matches."; "No skills yet."
  with the three ways in.

**What fails**
- The copy-conflict flow (a tool with its own copy → notice "Replace the copy with links" → confirm about
  `.hrm-backup` → "Replaced 1 copy…") is not in the fixture; ticking never hits it.
- Discover's loading ("Reading 3 repositories…"), a repository error in red, and "No SKILL.md in this repository."
  are not shown.
- At 1280 the Linked-tools table scrolls inside a fixed box while the page also scrolls (two scrollbars at
  `skills-1280-dark-scroll1`); the Not managed panel is below the fold on first paint although the head names it.
- One click on a header checkbox unlinks 255 skills with no confirmation; the inventory allows the single write, but
  a 255-place change deserves the same confirm the Remove gets.

**Must change**
- Seed one copy conflict and one failing repository; let the table grow with the page instead of scrolling inside it.

**Inventory misses**
- Replace-copy notice / confirm / toast; Discover loading, error and no-SKILL.md texts; "Updated hello" toast not
  verified; "Checking each skill's source…" then the result — only the result was seen.

## System prompts · 9/10 (Designer B)

**What works**
- Head "CLAUDE.md is the file Claude Code reads." (GEMINI.md for Antigravity and so on per tab), basis "2 saved
  versions · previous file saved before replacement"; New prompt primary; a tab per tool with its icon, all six.
- Left list: "The file now · Live" with its first line, each saved prompt with first line and "Enabled" pill; the
  live view shows "Every Claude Code account follows this file", the path in mono and the file text ("(the file is
  empty)" / "No profile of Claude Code yet" when empty).
- Editor: Title, the where-line, path, body, foot "This prompt is in the file now." / "Not in the file yet.",
  Delete… (confirm "Delete this prompt from the library? The prompt file is not changed."), Show live file, Use this
  prompt (hidden on the enabled one), Save ("Saved prompt."); New prompt appends "New prompt · Empty" and focuses the
  title. Empty state: "No prompt file yet." / "not there yet" / "No saved prompts."

**What fails**
- In the live view, Tab leaves the page after the two list cards: the file text block is not focusable or scrollable
  from the keyboard.
- "Use this prompt" → "Written to the prompt file" and the hand-written file surviving as a library entry were not
  exercised (the fixture's enabled prompt was selected); "Codex reads AGENTS.md" on hover not verified.

**Must change**
- Make the live file block keyboard-scrollable (tabindex) — then implement as drawn.

**Inventory misses**
- None seen; the two unverified actions above.

## API keys & routing · 8/10 (Designer B)

**What works**
- Head "The local router is stopped." / "1,204 requests. 1 provider paused."; basis "127.0.0.1:15731 · token
  required · subscription logins stay outside the router"; Add provider; Add route natively disabled until a
  provider exists.
- Router card with status, four stats, "1 routes · 1 accounts use it", Start / Stop router with an honest notice
  ("Router started in preview; no process changed."); Routes as cards (id mono, "failover · used by pi-lab", chips
  OpenRouter Serving → DeepSeek "Paused · retry 4m", "How failover works" disclosure, Remove… confirm); Providers
  table (name/id, format + endpoint, Key Saved, Breaker Closed / "Open · 5 failures", Requests 24h, First chunk
  p95, Remove…); Recent requests (Time, Account, Route → provider, Model, Status 200 / 503, First chunk, Time,
  Tokens —); Add provider (Id, Name, Format, Base URL with the hint, Models with the hint, Key with "Production:
  Windows Credential Manager. This preview discards the key."); Add route (Id, Mode, Providers with #1 order and
  the hint); the three empty texts.

**What fails**
- The stats carry no state: Failovers 3 is not amber, Errors 2 and Paused providers 1 are not red (1280 dark and
  light, `routing-populated`); the inventory marks all three.
- The populated state exists only behind a prototype "Two providers + route" button in Designer B's own fixture bar;
  the default fixture is empty.
- The aggregate mode (chip "hm-<id>--… models", its hint) and the red "missing" key state are not shown.

**Must change**
- Mark the three stats (amber / red with a word, not colour alone); seed an aggregate route and a missing key.

**Inventory misses**
- Aggregate route and hint; Key "missing"; "Remove <id> and its saved key?" wording not captured; "3 errors"
  head variant not seen.

## Apps · 8/10 (Designer B)

**What works**
- Head "1 tool has an update." basis "5 installed · 1 not installed · checked 15:42"; Check for updates; the
  two-copies bar with the exact sentence and "Remove winget copy…" (confirm: "The winget copy at … is removed in a
  terminal. The copy your terminals run stays."); table Tool / Installed / Newest / Installed by / Action / Show
  with icon, path in mono, versions, "native · updates itself", "Up to date", "Reinstall 1.5.0" primary on Pi,
  ⋯ (Update, Reinstall with vendor installer, Open folder, Copy path, Copy update command), the second-copy sub-row
  with Remove…, Not installed · 1 with Install… (npm / Vendor installer with the command and "Use method"), the
  Show checkbox, the Harness Manager card ("1.8.3 · 1.8.4 available · Update available · Settings › Updates" button,
  "Checks at launch and hourly · signed updates only").

**What fails**
- "Newest" is not amber when newer (Pi 1.5.0 is plain white at 1280 dark); the one update the head announces is
  not marked in the column.
- "could not check" with the error on hover, "?" for an unreadable version, "Website" for a tool without an
  installer, and the update-with-consoles-open confirm are not in the fixture; the Show-off effect on other pages
  was not verified.

**Must change**
- Mark the newer version (amber + the word); seed a could-not-check vendor.

**Inventory misses**
- "could not check" / "?" / "Website" states; the consoles-open confirm and "Claude Code is now 2.1.301" watch.

## CLI reference · 9/10 (Designer B)

**What works**
- Head "Every desktop action has an hrm command." basis "Add --json for scripts · static reference"; the Start-here
  block with the four examples and their meanings; seven groups (Accounts, Run, Usage and limits, Sessions and
  projects, Shared setup, Routing and setup, Computers) with every command the inventory lists and a one-line
  meaning; monospace only on commands; no state, no actions, the same page in the empty preset; fits 1920 in two
  columns and 1280 in two columns with the Computers group full width.

**What fails**
- Nothing against the inventory. Commands are plain text (no copy affordance) — not required.

**Must change**
- Implement as drawn.

**Inventory misses**
- None.

## Settings · 9/10 (Designer B)

**What works**
- Head "Settings stay on this computer." basis "Saved as you change them · keys and logins never enter backups";
  section jump chips; every section a folding card with "N settings"; every row with title, ⓘ (title "About Theme",
  a `role=tooltip` on focus: "The look of this window. Default: Harness; VektraGrid is outside this round."),
  one-line description and control; switches save at once ("Saved brand.").
- All nine sections with their rows: Appearance (Theme, Light and dark, Limits shown as, Moving routes), Window and
  terminal, Updates (1.8.3 · 1.8.4 available + "Update and restart…" confirm; Check at launch), Usage and limits,
  Connections (Router port + Change, profiles.json path + Move…), Diagnostic log, Maintenance (Find tools, Old
  launcher Import…, Show the tours again, Link copies to the main profile), Computers (This computer + Rename, the
  two sync cards with "Selected", Change… / Stop syncing… (Leave confirm), Join with a code…, Carry transcripts with
  "Carried 12 · 3.4 MB · 1 skipped", Presence, Direct connection + Pair a computer… + Listen port…, "3 computers ·
  Synced through a folder · last round 2m ago" + Sync now + First-run choices…, a card per computer with This one,
  OS · version · state, progress bar, counts, "direct · mdns · connected ⓘ", "relayed through SAMYMSI", Address +
  Save, Forget… confirm, amber "heard 4m ago"), Backups and archive (Data folder + Open, Automatic backups + Keep,
  Also in the sync folder, Include transcripts, Backups · 2 · 6.5 MB with Import… / Export… / Back up now, rows with
  Rename / Restore… / Delete…, In the sync folder · 1, Archive sessions older than + custom, Archive location, "37
  archived · 8.1 MB · Today 03:00" + Archive now + Restore… → the picker → "Restore / Welcome back" with the counts,
  the three buttons and "Restore and continue as SAMYMSI").
- Pair a computer (Show a code "482 915 · Good for 1:58", the address, Show a new code / Enter a code), Join a hub,
  Listen port, Choose shared folder, "Add this computer to your others" (first run) — each traps Tab, opens on its
  first field and Escape returns to its opener; the six-step tour spotlights the Theme ⓘ.

**What fails**
- The hub-only rows (Recovery key "Show again" behind its confirm, "Enter recovery key…", "Invite a computer…",
  "joined · 2 computers · live / polling") are not reachable: the fixture syncs through a folder and Join does not
  flip it, so they were never seen.
- "Change" on Router port and "Import…" on Old launcher did not respond to the automated click within 4 s at 1280
  (both exist; the Update-and-restart confirm opened instead when the page scrolled) — verify their hit areas.
- The sync-error red line and "sync is off: only this computer" variant are not shown.

**Must change**
- Add a hub-joined fixture so the recovery-key and invite rows are drawn; otherwise implement as drawn.

**Inventory misses**
- Recovery key (Show again / Enter recovery key…), Invite a computer…, hub status line; sync error line; the
  "sync off" line; the Launch-at-sign-in and Find-tools toasts not captured.

## Across the app

- Is it one app? Largely yes: one rail and top bar, one token set, the same chips, pills, tables, dialogs and
  notice bar on every page, 11 px floor and AA contrast everywhere. The three hands show in five places:
  1. Row ⋯ menus: the lead's is a centred modal ("Account Actions"), A's a popover menu (`role=menu`) under the
     button, B's a `<details>` popover under the button. Unify on one popover component.
  2. Dialog subtitles: B writes "Preview · local state only" under every dialog title and prefixes every notice with
     "Preview:"; the lead writes "Sign-in preview · no vendor connection" inline and "Refresh preview:"; A writes
     "Preview: would resume…". One voice, and none of it in the shipped app.
  3. Tours: the lead's is a one-step centred modal ("1 of 1"); B's is a multi-step spotlight tour with Skip all /
     Next; A's pages have `data-tour` anchors but no tour starts. Unify on B's and give Sessions, Session and
     Projects their steps.
  4. Notice ownership: the notice bar is one global string; it persists across presets ("Pinned to the sidebar"
     above the empty Projects page) and renders inside the Add-account dialog's error strip ("Preview: Saved
     prompt."). Make it page-scoped and keep dialog errors separate.
  5. Prototype chrome: B's pages add a second fixture bar (Ready / Empty / Loading / Error, "Two providers + route")
     above the lead's studio, outside the frame — fold it into the one studio so the owner has one set of switches.
- The three worst defects, ranked:
  1. The tray popup cannot be reached or left by keyboard (focus never enters it; Tab from inside walks out; Escape
     drops focus on the page), and the Request log and tour open with focus on `body` — the shell's own
     accessibility, inherited by every page.
  2. The global notice leaks: another page's toast appears inside the Add-account dialog and a stale toast survives a
     data change — a state-ownership bug in the shell that will show up on every page once data redraws every few
     seconds.
  3. Counts a user reads first are not trustworthy on two pages: the Sessions rail's "All projects" grows from 113
     to 223 when a tag filter is on, and Accounts goes blank with no "No account matches" when the search misses.
- Ranking, best to worst: CLI reference > Settings > System prompts > MCP servers > Overview > Accounts (with Add
  account and detail) > Projects > Session > Sessions > Skills > API keys & routing > Apps — the static and
  settings pages are complete and honest; the base is clear but has the keyboard and notice bugs; A's pages are
  rich but miss their archived / remote / paging states; Skills, Routing and Apps are complete in layout and thin in
  the error and conflict states the inventory names.
