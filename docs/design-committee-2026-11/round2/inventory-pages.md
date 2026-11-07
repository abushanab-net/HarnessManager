# Feature inventory, round 2: every page the round-1 inventory did not cover

Read with `../round1/inventory.md`, which covers the shell, the Overview, Accounts and Add account and sets the rules:
a design may move, fold, rename or hide any item behind a menu or a disclosure; it may not lose one, and if it drops
something it says in `angle.md` where it went. "Measured" means the app read it; "estimated" (≈) is a forecast or a
price at API list prices; "not read" (—) means the app has not fetched it yet. Every page below is taken from the
renderer that draws it (`app/ui/*.js`), its rendered strings and `data-act` / `data-testid` attributes, and from the
assertions in `e2e/app.e2e.mjs`. The last bullet of each section, **Data**, names the fields the renderer reads, so
mock data can carry the same names.

Shared conventions every page uses (design them once):
- Every list page redraws in place when the backend says something changed (the `refreshed` event from the folder
  watcher and the background import; the live tick of a session; presence from other computers). Scroll position,
  focus, an open dialog and a half-typed field must survive a redraw; a dialog left open makes the page catch up when
  it closes.
- Loading states today: a crumb and an h1 such as "Reading sessions…", "Reading projects…", "Reading each tool's
  version…", "Opening the session…"; the Sessions page shows its last drawing while fresh data loads.
- A failed page draw shows "Something went wrong", the error text, and a "Try again" button.
- Tools are the six harnesses (Claude Code, Codex, OpenCode, Antigravity, Pi, Oh My Pi); a tool hidden with
  Apps › Show leaves every page (Accounts, MCP servers, Skills, System prompts, the sidebar rings).
- Dialogs: a head with the title and ✕; Escape closes; a click outside closes; the first field is focused; some show a
  keyboard legend ("Enter pair · Esc cancel", "Tab next field · Esc cancel").
- Confirmations before anything destructive, with a danger button (Remove, Delete, Forget, Leave) or a primary one when
  nothing is lost (Restore, Replace, Update).
- ⋯ menus open below their button (above when near the bottom); danger items are last and red.

## Sessions list (`#sessions`, `app/ui/views.js`)
- Every filter is a link: the state lives in the hash (`group`, `days`, `live`, `project`, `q`, `tag`, `pc`, `page`,
  `harness`, `profile`), so Back works and a filter can be bookmarked. The search, the computer and every filter are
  answered by the backend index (FTS5), not by hiding rows.
- Head, all projects: crumb "Sessions · 7 days · 3 computers" (days only when not All; computers only with sync on);
  the finding: "**2 sessions are** running now; open one to follow it live." or "Every tool's sessions in one place,
  from Antigravity, OpenCode, Claude Code and Codex." (the tools seen in the list); the basis: "87 sessions in 7 days
  across 12 projects. Tokens are counted from the logs; costs are at list prices, so ≈."; a count readout
  ("87 sessions · 7 days · 12 projects"); **New session** (primary).
- Head, one project (`project=` set): "← Projects"; the project's name (rename pencil; a double-click on the name
  renames in place: Enter saves, Esc cancels, blur saves), the pin button, its tag chips with the tag editor; the
  repository or folder path in mono; stats: **$cost** at list prices, **tokens**, **N sessions**, **N accounts**,
  **N running**; a ⋯ menu (Pin / Unpin, Rename project, Merge into another project, Reset name; "Open its sessions"
  when not already there); **New session** in this folder (only when the project has a local root).
- The rail (left panel, all-projects view only): "Live now" with the running count; "All projects" with the total;
  "Pinned" (each pinned project); "Recent" (or "Projects" when nothing is pinned) with the first 8 and "N more…" to
  show the rest; "Every project, with costs →" (to Projects); "Tags" with every tag as a coloured chip and its count.
  Each project item: a dot (teal when it has a live session), the name, its session count or "2 live". The selected
  item is marked.
- The About panel (one-project view, only when the project spans several folders or accounts): "This project" ($cost,
  tokens, sessions); an area chart of the range ("7 days" / "Activity"); "Running" (each live session, a link);
  "Accounts" (each with its session count and share %); "Folders" (up to 6, "N more").
- Filter bar: active chips with ✕ for "Running now", a tag, and a search “word”; **Group by** Project / Date / Account
  / Tool / Tag (Project is hidden inside a project); range **7d / 30d / All**; with sync on, a computer segment
  **All computers / This computer / <each other computer>**; inside a project, **Accounts** chips (All + each account)
  and **Tags** chips (All + each tag of the project and its sessions); the note "Search above searches every session".
- "Running now": every open session sits in a group of its own at the top, whatever the grouping, never inside a fold
  (unless the Running-now filter is on, when the groups hold them).
- A group: its name (a tag chip when grouped by tag; "No folder" for sessions without one; "untagged" when grouped by
  tag), the repository root in mono or the remote, a ⋯ project menu (project grouping only), "N sessions · tokens ·
  $cost" (cost in project grouping), **New session** (project grouping, local root), a fold button ▾/▸ (folded groups
  are remembered per grouping while the app runs). Below the rows: "Show all N sessions" when the page gave the
  group fewer rows than it has (fetches that group in full).
- A session row: a status dot (teal = open now; red = a command failed, with "N failed commands" on hover); the title
  ("Untitled session" when none) with an "archived" pill when archived; a mono line "continued from LAPTOP · 14 msgs ·
  Projects\platform-gitops" (the last two path segments; full path on hover) and the tag chips (the tag button with
  the count, up to two chips, "+N" for the rest); the account; the tool pill with its icon; with sync on, a Computer
  cell (the computer's dot and name); tokens; when ("live", "running on LAPTOP", "Today 14:02", "Yesterday 09:10", a
  date); one action: **Follow** (open here), **Resume** (closed, local), **Resume here** (from another computer and
  resumable), or the reason it cannot be ("not cloned here", "on LAPTOP"; the full reason on hover), nothing when
  archived.
- Search (the top bar, Ctrl K): a pause of 250 ms sends the words to the index; the hash gets `q=`; a chip shows it;
  clearing the field restores the list. Typing on another page and pressing Enter opens Sessions with that search.
- Paging: 50 rows a page across every group; "‹ 1–50 of 142 ›" at the foot; a group's "Show all" reaches beyond the
  page. The index test seeds 60 sessions and expects 50 on the first page and the rest on the next.
- Live: a seeded session file appears on the page within two seconds without a click; the page keeps its scroll.
- Empty: "No sessions match." (filters or search); the whole app empty (no tools, no sessions) must be designed too.
- New session dialog ("New session" or "New session in <project>"): Folder (fixed from the project, else "No folder
  chosen" + **Choose folder…**); Account chips (tool icon + label; only accounts that can start: logged in or not a
  subscription; the project's latest account is picked first); **Skip permission prompts** with the tool's own flag
  shown (`--dangerously-skip-permissions` for Claude Code, `--dangerously-bypass-approvals-and-sandbox` for Codex;
  hidden for other tools) and its warning; **Start session** (disabled until a folder and an account are chosen);
  empty: "No account can start yet: add one, or log one in, on Accounts."
- Tag editor ("Tags of <name>"): a field "Type a tag and press Enter to add it" (lower-cased, spaces become dashes);
  the hint "Tags are yours: free text, shared across tools and accounts, never written into the tools' files." (+ for
  a project: "A project's tag reaches every session of it in filters."); every known tag as a checkbox with its chip
  and count; "No tags yet."; Cancel / **Save**; toast "Tagged release, client-x" or "Tags cleared". A tag's colour is
  its position in the tag list (6 colours), so it never changes.
- Rename / Merge dialog (from the menu when the name is not on screen): Project (name + key), New name or "Merge into
  key" with the hint "The key of the project to merge into, as shown under its name".
- Data: `sessions` → `{ groups: [{ key, name, root, remote, total, tokens, sessions: [row] }], total }`; a row:
  `id, profile, harness, title, cwd, messages, prompts, tool_calls, failed, tokens, model, open, archived, started_at,
  last_active_at, tags[], source_path, project_key, project_name, device, device_name, open_elsewhere,
  resumable (true | "reason: detail"), continued_from`. `projects` → `[{ key, name, root, remote, pinned, sessions,
  tokens, cost, live, accounts[], tags[], series[], last_active_at, priced_share, devices: [{ id, name, sessions }],
  here }]`. `tag_list` → `[{ tag, sessions, projects }]`. `computers_status` → `{ mode, folder, this: { id, name },
  peers: [{ id, name, … }] }`.

## Session (`#session?profile=&id=[&device=]`, `app/ui/views.js`, dialogs in `resume.js`)
- Opening: "Opening the session…" only for a different session; a redraw of the shown one keeps it on screen until
  the new drawing is ready (the live test asserts the page never blanks). A session from another computer: toast
  "Fetching from the folder…" the first time (the transcript is pulled from the sync folder and kept in the data
  folder).
- Head: "← Sessions · <project or folder name>"; the title; a meta row: "● LIVE" pill (adds "· following" while
  following), "archived" pill, "<account> · <tool>", the tag chips with the editor, the model in mono, "ran on
  LAPTOP" (remote) or "continued from LAPTOP", "started <when> · N turns", "tokens · $cost" (cost only when priced).
- Actions: **Export as Markdown** (local sessions; a save dialog with "<title>.md"; the file has `# title`, `## You`,
  each step as `` `Bash` command (failed) ``, the answers); **Resume in Windows Terminal** / **Resume in a console
  window** / **Resume in Terminal** (macOS) as a split button whose caret opens "Open sessions in Windows Terminal /
  in a console window" (saves the Terminal setting); for a remote session **Resume here** (primary) or the reason it
  cannot be; for an archived one "archived: it can be read, not resumed"; a ⋯ menu: "Open the log file folder".
- Tiles (measured from the log): **Tokens**; **Read from cache** % ("—" without timed requests); **First reply p50**
  in seconds ("—"); **Failed commands** (red when > 0); **Files changed**.
- Conversation panel head: "Conversation · 12 turns · 340 steps · 3 changed files · 1 failed"; a segment **All /
  Changes / Failed**.
- The chat (All): each turn in order: the prompt bubble on the right ("YOU · 14:02"), the steps where they happened,
  the reply bubble on the left ("CLAUDE CODE · 14:03 · claude-opus-5-5", rendered as Markdown); replies with no step
  between them share one bubble. Thinking steps are left out.
- A step: ✓ / ✗ / … (done / failed / running or pending), the tool name, what it worked on in mono (full text on
  hover), and its meta: "+12 −3" for a change, "failed · 1.2 s", "1.2 s", or "running now". A click unfolds it to
  its output (`pre`; "No output.") or its diff (added and removed lines coloured). Unfolded steps stay unfolded
  through live redraws.
- A long run of steps shows its first 5 and last 5 with "Show 1,990 more steps" between (the test seeds a Codex turn
  of 2,000 steps and expects 11 items, then all 2,000 after the click). In Changes and Failed the single flat list
  shows 30 + 10 before asking.
- Changes: only the steps that changed files ("This session changed no files."); Failed: only the failed steps
  ("Nothing failed."); no messages at all: "This session has no messages yet."
- Following a live session: it opens at the end and follows as its tool writes (the log file's size and time are
  checked every 2 s; a change redraws in place, never while a dialog is open). Scrolling up stops following and keeps
  the place; a "↓ Latest · follow live" button shows whenever the reader is not at the end (plain "↓ Latest" on a
  closed session) and jumps back.
- Resume-here dialog: the title; "<project> · <account> · <tool>"; **Last ran on** LAPTOP · when; **Opens in** this
  computer's clone of the repository (or "this computer's clone of <project>" when unknown); "Opens as a new session
  that continues from the LAPTOP transcript; the LAPTOP copy stays as it is."; "Only the conversation comes across:
  files changed since, attachments and running shells do not. Pull the branch first."; Cancel / **Resume here**;
  afterwards the copy is a session of this computer marked "continued from LAPTOP".
- Data: `transcript` → `{ meta: { title, profile, harness, model, cwd, project_name, open, archived, started_at,
  prompts, tokens, tags[], source_path, device_name, continued_from, resumable }, turns: [{ prompt, prompt_at,
  answer: [md…], answer_pos[], answer_at, model, steps: [{ kind, tool, input, status: done|error|running|pending,
  duration_ms, result, error, change: { path, added, removed, diff } }] }] }`; `session_stats` → `{ cost, requests,
  hit, p50 }`; `file_stamp` for the live tick.

## Projects (`#projects`, `app/ui/views.js`)
- State in the hash (`days`, `sort`, `metric`, `per`, `page`) through replaceState; a choice redraws only its part
  (the test asserts the tiles are the same DOM nodes after a sort, a metric change or a pin).
- Head: crumb "Projects · 7 days"; the finding "12 projects ran in the last 7 days; **2** have a session open now.";
  the basis "A project is a folder your tools ran in: click one for its sessions; pin, tag or rename it from its icons.
  Cost is at API list prices, so ≈."; range **7d / 30d / All**; sort **Active / Cost / Tokens / Name**.
- Five tiles: **Tokens · 7 days** (N sessions); **At list prices** ($ total; "$x a day" or "no priced model yet");
  **Projects** (N accounts); **Running now** (a click opens the live sessions; the names of the running projects or
  "nothing running"); **Busiest** (the name; "62% of tokens · 58% of cost"). A number that changed since the last draw
  flashes once.
- "Where the tokens and the cost went": a segment **Tokens / Cost / Sessions**; a stacked bar of the top five (six
  colours) plus "others"; a header row and one row per top project: swatch, name (a link to its sessions), a share
  track, Cost, Tokens, Sessions (the chosen metric highlighted), share %; "N other projects" summed. Hidden when there
  are no projects.
- One card per project (a link to its sessions; marked when a session is live): the name (rename pencil; double-click
  renames in place), the remote or folder in mono, the tag chips with the editor; an area sparkline of the range;
  **$cost** with "tokens · 80% priced" when under 95 % of its tokens have a price; "N sessions · M accounts"; "2 live"
  or the last activity; with sync on, a dot per computer that has the repository (names and counts on hover) and
  "not cloned here" when this one does not; actions: pin, New session here (local root only), ⋯ (Open its sessions,
  Pin / Unpin, Rename project, Merge into another project, Reset name).
- Pins: the pin icon, the ⋯ menu and the sidebar ✕ all pin and unpin; the sidebar shows at most five pinned projects
  under Projects, each with its live grid or session count; the choice is saved in settings.
- Empty: "No sessions in this range yet."
- Paging: over 10 rows, "‹ 1 2 3 › · 1–10 of 34 · 10 / 25 / 50 a page".
- Data: `projects` as on the Sessions page (`key, name, root, remote, pinned, sessions, tokens, cost, live, accounts[],
  tags[], series[], last_active_at, priced_share, devices[], here`); `project_pin`, `project_alias { key, name |
  merge_into }`.

## Account detail (`#account?name=&tab=`, `app/ui/accounts.js`)
- Head: a big avatar of the label's initials; "← Accounts"; the tool's icon and the account's label; a line with the
  tool, the email in mono, the plan pill (teal plan name or "Logged in" / red "Not logged in" / iris "Routed" / amber
  "API key"), "org · role", "· main profile", "2 consoles open in platform-gitops."; actions: **Start in a folder…**
  (the dialog of round 1), **Start <label>** (primary; disabled for a subscription not logged in), ⋯ (Start in a
  folder…, Log in again, Open folder, Profile settings, Remove account…).
- Tabs: **Overview**, **Sessions**, **Projects**, **Profile settings**.
- Overview tiles: **Tokens · 7 days** (x% read from cache); **At list prices** ("subscription: not billed per token" /
  "billed per token"); **First reply p95** ("p50 1.1 s · 412 replies" / "no timed replies"); **Sessions · 7 days**
  ("2 running now" / "none running now"); **Busiest project** (share %; the name / "no usage this week"). "—" when
  the stats are not read.
- Limits panel: "read 2 m ago" / "not read yet" / "<Tool> has no limits to read" · "mark = level at reset at this
  pace"; one bar per limit window: its label, "63% left" or "Used up", "resets in 2 h 10 m" / "resets Thu 15:46" /
  "window has reset" / "no window open"; the bar is the used share with a marker at the estimated level at the reset;
  a note "Runs out Thu 03:38 at this pace." / "Lasts until the reset at this pace." / "Plenty left."; empty: "Limits
  show up after this account runs on this computer." (subscription) / "API keys are billed per use and have no plan
  limits."
- Tokens per day, 30 days: a bar chart (today marked; first, middle and "today" labels; a tooltip per day) with
  "tokens · $cost at list prices" in the head; "No usage yet."
- By model, 30 days: a row per model (name, bar, tokens, cost or "—"); "No usage yet."
- Login and folder (facts): Login "valid" / "expired" / "not logged in" / "key saved" / "no key"; Folder (a link that
  opens it); Started with (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `XDG_CONFIG_HOME`, "its own login" for Antigravity,
  `PI_CODING_AGENT_DIR`); for a Claude account that is not the main profile: "Follows the main profile: CLAUDE.md,
  rules, skills, MCP" and "Own: login, history, usage"; for a Claude subscription: "Live limits on (reads the saved
  login) / off"; CLI (`hrm …` for this account); **Log in again** (subscriptions).
- Open now (when consoles run): each with its folder, "PID 1234 · since 14:02", **Show window** (toast "That console
  is in another window; switch to it from the taskbar." when it cannot).
- Recent sessions: the five newest (dot live / failed, title, project, "live" or when) with "All N" to the Sessions
  tab; "No sessions in the last 30 days."
- Sessions tab: the session rows of the Sessions page; same empty text. Projects tab: a table Project / Key / Tokens,
  7 days; "No usage this week."
- Profile settings: **Name** (the label; "Its folder and the command line keep the id <name>") + Save; **Expected
  email** ("Checked after each login, so you notice a wrong account") + Save; API-key accounts: **API key** (paste a
  new key; "Kept in Windows Credential Manager, never in a file") + Replace key, **Endpoint and model** (Base URL,
  Model; "Leave empty to use the vendor's own API and default model") + Save; routed accounts: **Daily spend cap** ($;
  "At the cap it stops answering for it until midnight. Empty: no cap."; shown on the Accounts row as "$5.00 daily
  cap" with what is spent today) + Save; Claude non-main: **Shared with the main profile** + Sync now; **Remove
  account** (main: "Stops managing the tool's own profile. Its folder and login are never deleted."; else "Removes it
  from Harness Manager. Its folder stays unless you choose to delete it.") → the Remove dialog (main: no folder
  choice, "Find tools in Settings adds it back"; else the checkbox "Also delete its folder") → back to Accounts.
- Sign-in dialogs: Claude in the app (Status line; "Open the sign-in page" with the URL; a Code field "Paste the code
  or callback URL" + Send; Cancel sign-in; Done; a wrong email is an error state); other tools in a terminal, with a
  dialog that follows every 2 s ("A terminal window opened with the sign-in. Finish it there…" → "Signed in as
  …" + Done). Closing the dialog cancels a waiting sign-in.
- Data: `detail` → `{ view: profile, sessions: [row], projects: [[key, name, tokens]], cli }`; a profile: `name, label,
  harness, harness_label, kind (subscription|api_key|routed), main, home, created_at, identity { logged_in,
  needs_login, email, name, plan, org, role }, expected_email, email_mismatch, quota { tiers: [{ id (session|week|
  week-<model>), label, used (%), resets_at }], fetched_at }, caps { quota, api_key, routable, mcp, skills, prompts },
  running: [{ pid, cwd, started_at, session_id }], base_url, model, route, daily_cap_usd`; `account_stats` →
  `{ week: { tokens, hit, cost, p95, p50, replies, top_project: { name, share } }, per_day[], days[], cost30 }`;
  `limits_extra` → `{ caps: { name: { cap, spent } }, forecast: { "name|tier": { runs_out, at_reset } } }`;
  `usage { days: 30, profile }` → `{ models: [{ key, tokens, cost }] }`.

## MCP servers (`#mcp` and `#mcp-edit`, `app/ui/mcp.js`)
- Head: crumb "MCP servers · checked 3 m ago"; the finding "11 servers are written into 27 places; **2 entries were
  left alone** because a tool's file had them by hand."; the basis "Kept once here and written into every tool you
  tick, in that tool's own format. The servers your tools already have are read in when Harness Manager starts; only
  entries it wrote are ever removed."; **Import from apps**; **Add MCP server** (primary).
- A status line: a pill "in step · checked 3 m ago" (teal) / "2 left alone · checked …" (amber) / "not synced yet";
  "11 servers · 4 tools · 27 entries written".
- The matrix: a row per server and a column per tool that takes MCP servers (the header shows "<Tool>: N servers" on
  hover; the count follows each tick). A row: the name, a transport tag (stdio / http / sse), an "Off" tag when no
  tool is ticked, an ⓘ button, the command line or URL in mono (full on hover), the description; a checkbox per tool;
  an Edit pencil; a ⋯ menu (Copy as JSON, Remove…). Ticking writes the server into that tool's file at once
  (`.claude.json`, Codex `config.toml`, …) and toasts what happened: "Written to 2 places", "Saved", "Skipped: …", or
  the left-alone message with an offer (see Skills) when a tool has its own entry of that name.
- Search (the top bar) hides rows that do not match name or command; "No server matches the search."; the list pages
  at 25 (25 / 50 / 100 a page, "1–25 of 41").
- Empty: "No MCP servers yet. Add one, or import the ones your tools already have."
- Info dialog (the ⓘ): "Connection" (the transport tag and the line), the description, the homepage as a link; "In
  each tool": the tool's icon and name with "written" / "not written" / "left alone", the file it lives in with
  "(mcpServers)"-style part or "(not there yet)", or "not read yet"; a note "Left alone, because a hand-made entry
  with this name is there: <files>. Edit the server here and save to take it over, or remove the tool's own entry.";
  Close / **Edit**.
- Import from apps reads every tool's config again: toast "Imported 3 new servers" / "No new MCP servers found".
- Remove: "Remove <name> from Harness Manager and from every tool it wrote it into?"
- Add / Edit page: a back arrow and "Add MCP server" / "Edit MCP server"; **Name*** (read-only when editing: "The name
  is the key in every tool's config"); **Connection** with a **Form / JSON** switch; Form: **Transport** Local command
  (stdio) / HTTP / SSE; stdio: **Command***, **Arguments** (a row each, ✕, "+ Add argument"), **Environment
  variables** (NAME / value, the value masked with an eye to reveal, ✕, "+ Add variable"); http / sse: **URL***,
  **Headers** (the same rows); JSON: a textarea holding the server as Claude Code writes it (`type`, `command`,
  `args`, `env` | `url`, `headers`) and the hint; leaving JSON with bad text toasts "The JSON is not a server: …" and
  stays; **Write to**: a switch per tool with its icon ("(not installed)" when not); a new server is ticked for Claude
  Code when installed; **Description & links** (a disclosure: "for you only; never written to a tool": Description,
  Homepage or docs); Cancel / **Add server** or **Save**; "Give the server a name" when empty. The draft survives
  redraws (adding a row, switching transport or mode).
- Data: `items` → `{ mcp: [{ name, transport, command, args[], env{}, url, headers{}, description, homepage }],
  targets: [{ kind: "mcp", item, harness, profile }] }`; `sync_last` → `{ at, conflicts, skipped }`;
  `mcp_last_import` → `{ report: [{ harness, label, file, exists, part, kept[] }] }`; `mcp_save { server, replace,
  targets[] }` → a sync report `{ written[], conflicts[], skipped[], error }`.

## Skills (`#skills`, `app/ui/skills.js`)
- Head: crumb "Skills · 255 in the store"; the finding "255 skills, linked into each tool you tick." or "**3 folders
  in the store are not managed yet**; Manage adds them."; the basis names the store folder ("Skills live once, in
  `~\.agents\skills`, and are linked into each tool you tick, so an update reaches all of them.") and repeats the
  not-managed note; actions on Installed: **Check for updates**, **Import from a tool**, **Install skill** (primary);
  on Discover: **Refresh**, **Install skill**.
- Tabs: **Installed** (count) and **Discover**; beside them on Installed: "Source: [all ▾]" (every source seen plus
  "Not managed") · "252 managed · 3 not managed · 2 updates".
- The table: Skill, Source, a column per tool with its icon, the count ticked, and a header checkbox that links or
  unlinks every skill shown in one write (indeterminate when some are ticked; toast "Linked 12 skills into Claude
  Code"), ⋯. A row: the name, an "Update" tag-button when its source has a newer version, the description (clamped,
  full on hover), the source ("anthropics/skills", "Imported", "Store", "Local"; the path on hover), a checkbox per
  tool, ⋯ (Update from its source — GitHub or a local path only —, Open folder, Remove…).
- Search and the source filter hide rows across pages; "No skill matches."; pages at 25 (25 / 50 / 100).
- Empty: "No skills yet. Find some under Discover, install one from a folder or a GitHub link, or import a tool's own
  skills."
- Not managed (a panel under the table): "Not managed · 3 folders in the store this list did not know" with **Manage
  all**; a row per folder: name, description, "linked in Codex, Claude Code" / "not linked", **Manage** (toast "Now
  managed here: loose"; it comes in ticked for the tools already linking to it).
- Discover: a search field "Search skills in these repositories"; the repositories as tags with ✕; "owner/repository
  or a GitHub link" + **Add**; **Install into**: a chip per tool (remembered on this computer; Claude Code to begin
  with) with "remembered for the next install; Installed ticks the rest"; then a section per repository ("anthropics/
  skills · 40 skills", or its error in red): each skill with its name, its path in the repo, its description, and
  **Install** or an "Installed" tag; "No SKILL.md in this repository."; while loading: "Reading 3 repositories… the
  first time downloads each one." (one download per repository).
- Install skill dialog: **Source** (a GitHub tree link or a folder; **Folder…** picker), **Name** (optional; the
  folder name otherwise); toast "Installed pdf; tick the tools it should be in".
- Import from a tool dialog: **From** (a tool's main profile; only tools with skills); "Its skill folders move to the
  shared store and are linked back."; toast "Imported 4: a, b, c, d. Kept your own x: a different one is installed".
- Check for updates: toast "Checking each skill's source…" then "2 updates available: use Update beside the skill" /
  "Every skill with a source to check is up to date"; Update toasts "Updated hello".
- A tool that already has its own copy of a skill (not a link): the tick leaves it alone, toasts, and a notice offers
  "Replace the copy with links" → a confirm ("… moves into `.hrm-backup` beside it, and the shared copy is linked
  there instead. Nothing is deleted.") → toast "Replaced 1 copy; the originals are in .hrm-backup".
- Data: `items` → `{ skills: [{ name, source }], skill_info: { name: { description } }, unmanaged: [{ name,
  description, linked: [harness] }], skill_store, skill_repos: [{ owner, name }], targets: [{ kind: "skill", item,
  harness }] }`; `discover` → `[{ repo, error, skills: [{ name, folder, path, url, description, installed }] }]`;
  `skill_check` → `[{ name, update: true|false|null }]`; `skill_install { source, name, enable[] }` → `{ name, sync }`.

## System prompts (`#prompts`, `app/ui/views.js`)
- Head: "The instructions each tool reads when it starts."; the basis "CLAUDE.md for Claude Code, AGENTS.md for Codex
  and OpenCode, GEMINI.md for Antigravity. Keep several here and choose which one is in the file; what the file held
  before is saved first, so nothing is lost."; **New prompt** (primary).
- A tab per tool that has a prompt file (icon + name; "Codex reads AGENTS.md" on hover). Files: CLAUDE.md (Claude
  Code), AGENTS.md (Codex, OpenCode, Pi, Oh My Pi), GEMINI.md (Antigravity).
- The list (left): "The file now" with its first line (or "not there yet") and a "live" pill; then each saved prompt
  with its title, first line, and an "enabled" pill when it is the one in the file.
- The editor (right, a prompt selected): the title field; "enabled in Codex" pill; "Writes to <path> · every Codex
  account follows it" or "No profile of Codex yet"; the body textarea; the foot: "This prompt is in the file now." /
  "Not in the file yet.", **Delete**, **Show the live file**, **Use this prompt** (hidden when enabled), **Save**.
- The live view (nothing selected): "The file now" with the live pill, the where line, the file's text in a code block
  ("(the file is empty)" / "No profile of this tool yet.").
- Actions: New prompt adds "New prompt" (empty) and selects it; Save toasts "Saved"; Use this prompt saves, then
  writes the file ("Written to the prompt file"); the file's previous text is saved as a prompt first (the test asserts
  the hand-written AGENTS.md survives as a library entry); Delete asks "Delete this prompt from the library? The prompt
  file is not changed."
- Data: `items.prompts` → `[{ id, harness, title, body, enabled }]`; `prompt_live { harness }` → `{ path, body }`.

## API keys & routing (`#routing`, `app/ui/views.js`)
- Head: the finding "The router is up on :15731: 1,204 requests in 24 h; **1 provider is** paused." (or ", 3 errors";
  or "The local router is stopped."); the basis "Keys live only in Windows Credential Manager. The local router sends
  routed accounts to providers, moves to the next one when a provider fails, and pauses one that keeps failing.
  Subscription logins never pass through it."; **Add provider**; **Add route** (primary; disabled until a provider
  exists).
- The router card: a live dot; "Local router · running / stopped"; "127.0.0.1:15731 · token required · 2 routes ·
  3 accounts use it"; **Start router** / **Stop router**; four stats: Requests · 24 h, Failovers (amber when any),
  Errors (red), Paused providers (red).
- Routes (a card each): the id in mono; "failover · used by work-cheap, pi-lab" / "aggregate · used by no account
  yet"; the provider chain as chips joined by →, each with the provider's name and its state: "serving", "standby",
  "default", "hm-<id>--… models" (aggregate), "paused · retry 4 m" (red); a hint per mode ("Tries each provider in
  order; one that fails five times in a row is skipped for a while." / "One model list from all its providers,
  converted between the Anthropic and OpenAI formats."); **Remove** (confirm). Empty: "No routes yet. Add a provider,
  then a route that uses it."
- Providers (a table): Provider (name, id), Format, Base URL, Key ("saved" / red "missing"), Breaker ("closed" teal /
  "open · 5 failures" red), Requests · 24 h, First chunk p95, Remove (confirm "Remove <id> and its saved key?").
  Empty: "No providers yet. Add OpenRouter, DeepSeek or any Anthropic- or OpenAI-compatible endpoint."
- Recent requests through the router (a table): Time, Account, Route → provider, Model, Status (200 teal; other or
  "failed" red), First chunk, Time, Tokens ("—"); "No requests yet."
- Add provider dialog: **Id** (openrouter), **Name**, **Format** (Anthropic Messages / OpenAI Chat Completions),
  **Base URL** ("Anthropic: the root (…/v1/messages is added). OpenAI: up to /v1 (…/chat/completions is added)."),
  **Models** ("Comma separated. The first is used when a request names a model this provider lacks."), **Key**
  (masked; "Kept in Windows Credential Manager"); toast "Provider added".
- Add route dialog: **Id** (cheap), **Mode** ("Failover: try providers in order" / "One model list across
  providers"), **Providers** as checkboxes in order (the first ticked; "Checked in this order; failover tries them
  top to bottom."); toast "Route added".
- A routed account is added on Accounts (kind Routed, a route from this page) and may carry a daily spend cap (its
  Profile settings).
- Data: `routing` → `{ running, port, now, stats: { requests, failovers, errors, providers: { id: { requests,
  p95_first } } }, routes: [{ id, mode: failover|aggregate, providers: [id], default_provider }], providers: [{ id,
  name, format: anthropic|openai, base_url }], keys: { id: bool }, breakers: { id: { state: open|closed, failures,
  retry_at } }, routed_profiles: [{ name, route }], log: [{ ts, profile, route, provider, model, status, first_ms,
  ms, input, output }] }`.

## Apps (`#apps`, `app/ui/apps.js`)
- Head: crumb "Apps · checked 14:02"; "The tools on this computer, every copy of each, and the newest version its
  vendor offers."; "How each was installed; installs and updates open in a terminal so you can see what they do.";
  "Checked 14:02" and **Check for updates** (toast "Asking each vendor for the newest versions…"; a failed vendor is
  a toast "Could not check codex: …").
- A warning bar per tool installed twice: "Claude Code is installed 2 times. Your terminals run the npm copy (2.1.300);
  the winget copy in <path> is 2.0.9 / is the same version / could not say its version. Keep one." with **Remove the
  winget copy**.
- The table: Tool, Installed, Newest, Installed by, (action), Show. A row: the tool's icon and name; its path in mono
  (or "Not installed" / "Not installed; get it from its website"); the installed version ("?" when unreadable, "—"
  when absent); the newest version (amber when newer; "could not check" with the error on hover; "—"); how it was
  installed ("native · updates itself", "npm", "Homebrew", "winget", "Scoop", "Chocolatey", "manual"); the action:
  **Install ▾** (a menu of methods, each with its command) or **Website**, **Update** (primary when newer; "Reinstall
  2.1.0" for a native installer; "Up to date" when current), a ⋯ (the update label, "Reinstall with the vendor's
  installer", "Open its folder", "Copy its path", "Copy the update command"); a **Show** checkbox ("Show Claude Code
  on Accounts, MCP servers, Skills and System prompts"; off removes the tool from those pages).
- A second copy of a tool is a sub-row: its path "· second copy, not on PATH first", version, kind, **Remove…**
  (confirm: "The winget copy at <path> is removed in a terminal window. The copy your terminals run stays.").
- Groups: the installed tools, then "Not installed · N". Empty: "None of the tools is installed yet: install one
  below."
- The Harness Manager card: "1.7.0 · 1.8.0 is available: Settings › Updates installs it" / "· the newest"; "It checks
  at launch and every hour, and installs signed updates only."; a pill "update available" / "up to date" / "not
  checked yet".
- Update with consoles open: a confirm "2 consoles are running Claude Code. The update may not be able to replace
  files they use. Update anyway?"; then toast "Running in a new terminal: npm install -g …"; the page watches the
  version for two minutes and toasts "Claude Code is now 2.1.301". The sidebar's "N updates" beside Setup / Apps is
  counted against the versions installed now, so an applied update leaves it.
- Data: `apps` → `{ apps: [{ id, label, path, version, auto_updates, download, copies: [{ path, version, kind,
  runs }], install: [{ id, label, command }], update: { id, label, command } }], latest: { at, versions: { id },
  errors: { id } }, updates }`; `self_latest` → `{ version, notes, current }`; `settings.hidden_tools[]`.

## CLI reference (`#cli`, `app/ui/views.js`)
- "`hrm` does everything this window does."; "Add `--json` to any command for scripts."
- An example block: `hrm list` (accounts, plans, limits, tokens, open consoles), `hrm start work-max --cwd D:\Projects\
  my-app` (a new terminal, in that folder), `hrm quota work-max --live` (limits straight from Anthropic),
  `hrm sessions --group project --json` (every tool's sessions, for scripts).
- Seven groups, each command with a one-line meaning: **Accounts** (list, show, add, login, remove, key); **Run**
  (run, start, resume, open); **Usage and limits** (usage, overview, requests, quota); **Sessions and projects**
  (sessions, session, projects, tag, tags); **Shared setup** (sync, mcp, skills, prompts); **Routing and setup**
  (provider, route, router, settings, init, import); **Computers** (computers status / now / set, hub join / invite /
  status / leave, ring show | enter, computers pair / join / peers / forget / serve).
- Static: no state, no actions, no empty state. Monospace for commands only.

## Settings (`#settings`, `app/ui/views.js`; Computers in `computers.js`; Restore in `restore.js`)
- Head: "Saved as you change them, for this computer."; "Keys and logins are never in these files or in backups."
- Every section is a card that folds ("N settings" in its head; open by default; its fields stay on the page while
  folded). Every row: a title, an ⓘ whose tooltip (hover or keyboard focus) explains the setting and names its
  default, a one-line description, and the control. Switches, segments and dropdowns save at once ("Saved"); typed
  fields have a button beside them. Sections in order: Appearance, Window and terminal, Updates, Usage and limits,
  Connections, Diagnostic log, Maintenance, Computers, Backups and archive.
- **Appearance**: Theme (Harness / VektraGrid; default Harness); Light and dark (System / Light / Dark; default
  System); Limits shown as (Left / Used; default Left; changes every ring and bar in the app); Moving routes (NEW;
  "Animate the routes"; default on; the system's reduced motion also stills them).
- **Window and terminal** (Windows; "Window" on macOS): Close to the tray ("Keep running in the tray"; default on);
  Launch at sign-in ("Start with Windows"; default off; toasts "Harness Manager starts in the tray when you sign in" /
  "It no longer starts at sign-in"); Terminal (Windows Terminal / Console; default Windows Terminal).
- **Updates**: Harness Manager ("Version 1.7.0. New versions are looked for at launch and every hour." + **Check for
  updates**, or "Version 1.8.0 is available; you have 1.7.0. <release notes>" + **Update and restart** → "Updating…";
  an offer that turned out stale toasts "Harness Manager 1.7.0 is the newest version" and withdraws; a dot on the
  Settings nav item while an update is offered); New versions of your tools ("Check at launch"; default off).
- **Usage and limits**: Refresh (1 / 5 / 15 min; default 5); Live Claude limits ("Ask Anthropic for current
  limits"; default off; the long explanation of what it does and never does).
- **Connections**: Router port (a number + **Change**; default 15731; 127.0.0.1 only); Account list (the path of
  profiles.json + **Move…**; the default path as placeholder; a OneDrive folder shares the accounts between
  computers, each keeping its own keys and logins).
- **Diagnostic log**: Keep a log (default on); Level (Errors / Warnings / Everything; default Warnings); Log file +
  **Open log**.
- **Maintenance**: Find tools again (**Find tools**; toast "Added codex-main" / "No new tools found"); Old launcher
  (**Import** from claude-accounts.json; toast "Imported N accounts and M keys; skipped K"); Tours (**Show the tours
  again**; toast "Each page will show its tour again" and the current page's tour starts). Note: a "Link copies to
  the main profile" action exists (`relink`) and is named in a toast, but no row draws it today; the design may give
  it a place here.
- **Computers**: This computer (its name + **Rename**; default the OS computer name); How your computers sync
  (two cards, one chosen at most): **A shared folder** ("OneDrive, Google Drive, an SMB share or a NAS. Nothing to
  run; the service carries the files."; the folder path or "No folder chosen yet"; **Choose…** / **Change…**;
  **Stop syncing** when on; choosing a folder that holds history opens Welcome back) and **A hub you host** ("Your VPS
  with Docker, behind Caddy. Works from any network, no VPN needed, and gives live presence everywhere. Only your
  computers can read it: everything but their names is sealed with your ring key."; a "live" / "joined" pill; the
  URL or "Not joined yet"; **Join with a code…** or, when joined, **Invite a computer…** and **Stop syncing**; a
  status line "joined · 2 computers · live / polling" and, in red, "no ring key: enter the recovery key below");
  Recovery key (only with a hub or a ring key: **Show again** behind a confirm "Anyone who reads it can read your
  hub." and **Enter recovery key…**, primary when the key is missing); Carry transcripts (default on; the tooltip
  adds "Carried 12 · 3.4 MB · 1 skipped" and the limits: a transcript over 20 MB is not carried, the folder holds up
  to 200 MB); Presence ("Share what runs here"; default on; every minute); Direct connection ("Connect directly";
  default off; the tooltip shows "Listens on port 51234" with **Change…** → a Listen-port dialog where 0 picks a free
  port) and **Pair a computer…**; a line "3 computers · synced through a folder · last round 2 m ago" (or "sync is
  off: only this computer"), a red sync error when there is one, **Sync now** (toast "Synced with 2 other computers" /
  "Sync round: <error>"); then a card per computer: "this one" pill, its colour dot and name, "Windows · 1.7.0 ·
  writing now / connected / heard 40 s ago / not heard from yet" (amber when not heard within three minutes), a
  progress bar ("1,204 of 1,204 entries published" here; "… applied here" for a peer; amber when behind and stale),
  "12 projects here · 3 accounts · 1 running" (or "1,204 journal entries" / "nothing written yet"); for a peer: how
  it is reached now ("direct · mDNS · connected", "direct · laptop:7777 · connected", "direct · rendezvous ·
  connected", "relayed through DESK", "hub · connected", "folder · heard 40 s ago", "not reachable · folder only")
  with an ⓘ (Paired <date> / Not paired: known through the folder only; Last pull; Address; Relayed through), an
  address field "name:port" + **Save** (paired peers), **Forget** (confirm: "… drops its certificate; what was merged
  from it stays. Pair again to reconnect."). The cards follow presence every 10 s unless one is being edited.
- Computers dialogs: **Pair a computer** with two tabs: *Show a code* ("On the other computer, open Settings ›
  Computers › Pair a computer… › Enter a code, and type" a six-digit code "482 915", "Good for 1:58", the address to
  use from another network; "The code expired." + Show a new code; "Paired with LAPTOP." then it closes) and *Enter
  a code* (Code, digits only, a space after three; Address "name:port" for another network; errors inline: "Enter the
  six digits shown on the other computer." or the operation's own text; Pair can be tried again); **Join a hub**
  (Address https://…, Code, Admin token for the first computer; "The address starts with https:// …"; "Enter the six
  digits from Invite a computer…, or the admin token for the first computer."; the hub's own error inline); **Invite
  a computer** (a code good for ten minutes for one computer, the address, "it will also ask for the recovery key"
  when not paired); **Your recovery key** (8 groups of 4; "I saved it in my password manager"; shown once at the first
  join); **Enter the recovery key** (typed or pasted; case, spaces and dashes ignored); **Add this computer to your
  others** (first run, once: the two cards and "Not now").
- **Backups and archive**: Data folder (the path + **Open**); Automatic backups (Off / every 6 / 12 / 24 hours /
  week + Keep 3 / 5 / 10 / 20 / 50; default 24 h, 10; the tooltip lists what a backup holds and that keys and logins
  never are); Also in the sync folder ("Copy backups there"; default on; "once a sync folder is chosen"); Include
  transcripts (default off); the Backups list: "Backups · 4 · 12.3 MB", **Import…** (a zip; confirm "Everything in
  <file> is put back…"), **Export…** (a zip; toast "Exported: keep it somewhere safe; it holds no keys or logins"),
  **Back up now** (label "Manual"); a row per backup (date and time, label, size, rename, delete with confirm,
  **Restore** with confirm "… The state now is kept as a backup first. API keys and logins are not touched." → toast
  "Restored, with 1 skill folder put back. The state before is kept as a backup named Before restore."); "No backups
  yet."; "In the sync folder · N · every computer's" rows (label · computer, size, **Restore…**); Archive sessions
  older than (Never / 30 days / 90 days / 1 year + a custom number of days; default 30); Archive location (a switch
  for the sync folder "(none chosen yet)", a folder "No folder chosen" + **Choose…** / **Change…** / **None**);
  Archive now ("37 archived · 8.1 MB · sync folder: 37 · folder: 0 (off) · last run Today 03:00" / "Nothing archived
  yet." + **Archive now** → toast "3 sessions moved to the archive" / "Nothing archived: <why>"); Restore (**Restore…**
  → "Restore from…": the sync folder, each backup here, each backup in the sync folder, "A folder…", "A zip file…" →
  the Restore dialog).
- Restore / Welcome back dialog: "This backup is from DESK, up to 6 Nov 2026 14:02." or "This folder has the history
  of 2 computers, including DESK, up to …"; "Merge it into this computer?" / "Restore it into this installation?";
  the counts "3.4 M tokens across 1,204 requests, 87 sessions, 3 accounts' limit history, and 37 archived
  transcripts."; "Keys and logins are not in it: each account asks you to log in again when first used."; "Nothing is
  counted twice: what this computer already has stays as it is."; a line per computer ("DESK · 2 requests ·
  1 sessions"); the path; buttons Cancel / Start fresh, **Restore** / **Restore as a new computer**, and **Restore and
  continue as DESK** when the source names a computer; toast "Restored: 0 requests, 0 sessions and 0 archived
  transcripts added; this computer continues as DESK"; "The folder holds no history yet" / "Nothing to restore in it".
- Data: `state.settings` → `brand, theme, quota_display, moving_routes, close_to_tray, terminal,
  check_updates_at_launch, refresh_minutes, router_port, registry_path, log_enabled, log_level, hidden_tools[],
  pinned_projects[], backup_hours, backup_keep, backup_in_sync_folder, backup_transcripts, archive_days,
  archive_in_sync_folder, archive_dir, sync_mode (off|folder|hub), sync_folder, sync_transcripts, sync_presence,
  direct_connection, device_name, device_id`; `state.autostart, live_quota, version, os, registry_path`;
  `computers_status` → `{ mode, folder, port, last_run, error, this: { id, name, last_seq, published_seq, projects,
  accounts, running[] }, peers: [{ id, name, os, app_version, connected, last_seen, last_seq, applied_seq, direct,
  paired_at, last_pull, address, path (mdns|address|rendezvous|relay), relayed_by, transport (hub|folder), projects,
  accounts, running[] }], hub: { url, devices, ws, ring, ring_shown, joined } }`; `transcripts_status` → `{ carried,
  bytes, skipped[] }`; `backups` → `{ data, dir, list: [{ name, label, at, size }], in_sync_folder: [{ label, at,
  size, path, device_name }] }`; `archive_status` → `{ archived, bytes, locations: [{ kind, sessions, on }], last:
  { at } }`; `restore_preview` → `{ empty, source: folder|zip, path, newest, tokens, requests, sessions, accounts,
  archived, this: { name }, computers: [{ id, name, requests, sessions }] }`; `self_update_check` → `{ version, notes,
  current }`.

## The tours (first-visit walkthrough, `app/ui/guide.js`)
- On a page's first visit (per computer; not while a dialog is open) a tour starts: one part of the page is
  spotlighted (a rounded cut-out) with a popover beside it (below, else above, else to the right; centred when no
  part): "2 of 6", a title, a sentence, **Skip all tours** (or **Close** on the last step), **Back**, **Next** /
  **Done**. Keyboard: → next, ← back, Esc skips. Finishing marks that page; skipping turns every page's tour off;
  the `?` in the top bar replays the current page's tour; Settings › Show the tours again turns them all back on.
  A step whose part is not on the page (because of the state or because the selector is stale) is skipped.
- Steps today, per page (title: gist): **home** Welcome; Every page; The finding; Key numbers; Tool and computers;
  Live routes; Needs attention; Limits; Every model call; Today. **accounts** Add an account; This week; Tokens per
  day; Your accounts, by tool; Limits; Start; More; The account with the most room. **account** Overview, Sessions,
  Projects, Settings; Start in a folder; More. **projects** Every project at a glance; One card per project.
  **sessions** Projects, pinned and recent; Tags; Find a session; Grouped by project. **session** Changes and Failed;
  Conversation; Resume; Export. **routing** Providers; Routes; The router. **mcp** Add an MCP server. **skills**
  Install a skill. **prompts** One list per tool; Versions. **settings** What each setting means; Save; Computers;
  Direct connection; Pair a computer; Tours. No tours exist for Apps, CLI reference or the MCP edit page.
- The design decides where the popover and the spotlight sit in its layout, and what the first-visit experience of an
  empty app is.

## Notices and toasts (`app/ui/core.js`)
- A toast (`role=status`, bottom of the window): one line; stays 3.5 s or 60 ms a character, whichever is longer;
  an error toast is red and stays at least 7 s; a new toast replaces the old. Every action's success text is a toast
  ("Saved", "Pinned to the sidebar", "Opened a terminal for work-max", "Written to 2 places", "Router started",
  "Exported", "Copied", "Resumed in a new terminal"…), and every failed call's error text too.
- A notice (`role=alert`, a bar that stays until dismissed with ×; the next failure replaces it): every error from a
  failed call, and the skills' left-alone case with an action button inside it ("Replace the copy with links").
- Other persistent signals: the Settings nav dot for a newer Harness Manager (toast "Harness Manager 1.8.0 is
  available. Settings > Updates installs it." once per version); "N updates" beside Setup and Apps; a sync error in
  red on the Computers line; the first-run toast "Found 3 installed tools and imported 2 accounts from your
  PowerShell launcher."
- The refresh button's toast: "12 new records." / "Usage is up to date.", then "Limits of work-max: <error>" when a
  live read failed, then "Claude limits are the copies Claude Code last saved, which can be hours old. Turn on Live
  Claude limits in Settings to read current ones" when Live is off and a Claude account's limits are over 10 minutes
  old.
- A page that failed to draw: "Something went wrong", the error, **Try again**.

## Ctrl K search (the top-bar field, `app/ui/app.js` and each list page)
- Ctrl K focuses the field (placeholder "Find an account, session or setting"; a `Ctrl K` key cap inside it). There
  is no results dropdown today: the field filters the page it is on.
- On Accounts: filters the rows live by name, label or email ("No account matches the search." in an empty tool
  group). On Sessions: after a 250 ms pause the words go to the index; a chip “word ✕” shows; the hash carries `q`.
  On MCP servers and Skills: hides non-matching rows and re-pages ("No server matches the search." / "No skill
  matches."). On any other page: Enter opens Sessions with the words as the search and empties the field.
- Settings are not searched today although the placeholder says so; a design may add it or fix the placeholder.
- Data: the pages' own lists; `sessions { filter: { query } }`.

## The tray popup (`app/ui/limits.html` + `popup.js`; `app/src-tauri/src/main.rs`)
- The tray icon (tooltip "Harness Manager"): a left click opens the limits popup, a second click or losing focus
  closes it; a double-click opens the window; the right-click menu is "Open Harness Manager" and "Quit". Closing the
  window keeps the app in the tray while Close to the tray is on; "Start with Windows" launches it hidden there.
- The popup: a frameless, non-resizable, always-on-top window (not in the taskbar), 372 px wide, three rings a row
  and a row per three shown accounts (20 + rows × 104 px tall; sized once at the first open, so an account added
  later scrolls until the app restarts); placed above a bottom tray or below a top one, centred on the click and
  kept within the screen. It follows the app's theme (brand and light / dark).
- The content is exactly the sidebar's limit rings (`drawNavLimits`): one per account of a shown tool, in account
  order: a ring of the week used with the % left in the middle (or % used with the setting), a bar under it filling
  as the week runs towards its reset, the tool's icon and the account's label; amber when ≥ 80 % used; a grey dotted
  ring with "—" when limits were never read ("limits not read yet"); a red dashed ring with "—" for a subscription
  not logged in ("sign in to read its limits"); "∞" for an API-key account ("API key, no limit"); the hover title
  "work-max · week resets Tue 15:46 (2 d left)". A click on a ring opens the main window at that account's detail.
- It redraws after every background refresh (the `refreshed` event). No actions, no empty state beyond "no accounts"
  (nothing is drawn; the design should say what an empty popup shows).
- Data: `state.profiles` (as the Accounts page: `name, label, harness, kind, identity.logged_in, identity.needs_login,
  quota.tiers[{ id, label, used, resets_at }]`), `settings.quota_display`, `settings.hidden_tools`.
