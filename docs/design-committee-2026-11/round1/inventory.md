# Feature inventory: what the shell, the Overview, Accounts and Add account do today

A design may move, fold, rename or hide any of these behind a menu or a disclosure. It may not lose one. If your design
drops something, say in `angle.md` where it went. "Measured" means the app read it; "estimated" (≈) means a forecast;
"not read" (—) means the app has not fetched it yet. The design must keep those three states distinguishable.

## The shell (always on screen)
- Brand: the Harness Manager mark + name, the computer's name, the account count.
- Search (Ctrl K): finds an account, a session or a setting; on list pages it filters that list.
- Top bar status: `N live · M elsewhere` (sessions running on this computer / on other computers; a click opens the
  live sessions); the computers chip (one coloured dot per computer, teal when heard from within three minutes; a click
  opens Settings › Computers); freshness text `Limits read 14 m ago · usage just now`; the `?` tour of this page; a
  refresh button (re-reads usage and limits and says what it did).
- Navigation: Overview (shows today's cost), Sessions (live count as a dot grid), Projects (count; a fold with the
  pinned projects, each with its live count), Accounts (count), the Setup group (the number of tools with an update;
  folds; a page inside it opens the fold): MCP servers (count), Skills (count), System prompts, API keys & routing,
  Apps (updates); then CLI reference, Settings (a dot when a newer Harness Manager exists), the version.
- Readouts (added 7 Nov, free to move or drop): a range switch Today / 7 d / 30 d / All; Tokens, Cost (at API list
  prices), Cached % each with a dot-grid trend and a delta against the period before.
- Limit rings (added 7 Nov, free to move or drop): one ring per account with plan limits, showing what is left of its
  week (or what is used, a setting), the account's tool icon and name, a bar under it showing how far the week has run
  towards its reset; a grey ring with "—" for an account whose limits were never read.
- Tray: the app keeps running in the tray when the window closes; a click on the tray icon shows the limits in a
  small always-on-top popup; the popup and the window share the design.
- Four themes: Harness and VektraGrid × light and dark (round 1: Harness light and dark only).
- Notices and toasts (a one-line error bar; a short confirmation after an action).
- First visit of a page: a short walkthrough (tour) can be started from `?`.

## The Overview
- The finding: one sentence naming the account that will run out first and when
  ("codex-main will likely run out of its week Sun at about 00:56, 2 d 14 h before it resets"), with its basis
  (measured 73% used, resets Tue 15:46, read 19 m ago; the run-out is an estimate from today's pace).
- The scene (today a half-circle "Horizon Dial", free to replace): the computers, every account and every tool as
  bodies, the routes between them (which tool is talking to which account, from which computer), live particles on a
  route that is in use (a setting turns motion off, and the system's reduced-motion is honoured), the selected route's
  readout (week used %, ≈ run-out), the tool's first-reply time. Hover/focus/click selects; Escape returns to the
  finding's route. No tokens-per-second anywhere on the Overview. Harness mark in every theme.
- Filters: a chip per shown tool (All, Claude Code, Codex, OpenCode, Antigravity, Pi, Oh My Pi), and
  All computers / This computer.
- `Start <account>`: starts a session on the account with the most room (opens a terminal).
- Activity, 53 weeks: a year clock (53 weeks × 7 days as a radial heat map of daily tokens, month letters around it,
  the busiest day in the centre), with a per-computer switch.
- Needs attention: a list of open items (an account about to run out, a tool with an update, a failing login, a paused
  provider) each with its time and a one-line reason; "nothing" when empty.
- Latest replies: a table (time, account, model, computer, tokens, speed, first reply) of the newest replies in the
  range, with a count ("10,472 today · speed and first reply are approximate") and a Request log switch that opens
  the full log (same columns, every number column right-aligned).
- The page re-renders every few seconds while sessions write; the design must survive that (no state lost, no jump).

## Accounts
- The finding: the account with the most room ("service-account-resal has the most room: 2% of its 5-hour limit and 0%
  of its week used"); the basis: 8 accounts across 4 tools, each in its own folder with its own login, sorted by room
  left, usage at API list prices.
- Actions: range 7 d / 14 d / 30 d; Export CSV; Start in a folder… (pick a recent folder of an account, skip
  permission prompts, extra arguments); Add account.
- Tokens per day, by account: a stacked daily chart for the range (foldable).
- Tool filter chips (All + each tool), and the sort/read note.
- One group per tool (foldable): the tool's icon and name, N logged in, N consoles open, next reset, tokens, cost.
- One row per account: a ring (what is left), name and label, email, plan chip (Max 20x, Pro, Team…), added date;
  every limit window the tool has (5-hour, week, week per model…) as a bar with % left and the time to its reset
  (measured; "—" when not read); a 7-day sparkline; tokens; cost at list prices; "read 2 m ago"; a refresh (reads the
  limits live); Start; a ⋯ menu (open folder, rename, profile settings, sync now, show window, remove from the list,
  remove with its folder); an advice line ("Most room of any account: start here next." / "Week runs out Thu 03:38
  at this pace."); N open sessions.
- By model: a panel of usage per model (foldable).
- Account detail (a click on a row): tabs Overview (limits, tokens per day 30 days, by model, login and folder facts,
  open now, recent sessions), Sessions, Projects, Settings (email, label, API key, endpoint, cost cap, remove).
- Main profiles: the tools' own default logins (~/.claude, ~/.codex…) appear as accounts automatically; they cannot
  be removed, only hidden from the list.

## Which limit windows exist, per tool (mock data must follow this)
- Claude Code accounts: a 5-hour window, a weekly window, and often a second weekly window per model (e.g. "Week · Opus").
- Codex accounts: a 5-hour session window and a weekly window (the app reads what Codex reports). An exhausted 5-hour
  window on a Codex account is a valid scenario.
- OpenCode, Antigravity, Pi, Oh My Pi: no plan limits are read; they show usage only ("—" / not read / no limits).
- Routed and API-key accounts: no plan windows; a routed account may have a spend cap.

## Add account
- Tool: one of Claude Code, Codex, OpenCode, Antigravity, Pi, Oh My Pi (only installed tools can be started, but an
  account can be prepared for any).
- Kind: Subscription (every tool: you sign in in the browser or a terminal), API key (Claude Code, Codex and Pi
  only), Routed (Claude Code and Pi only: an account that sends its requests through a route on the local router to a
  provider such as OpenRouter or DeepSeek). Antigravity, OpenCode and Oh My Pi take a subscription only. Source:
  `Caps` in crates/core/src/harness/*.rs. The design must make it obvious which kinds a tool supports.
- Name (becomes the folder `~\.claude-<name>`); Email (subscription: checked after login so a wrong account is
  caught); API key + optional endpoint + optional model (API key); Route (from the routes on the routing page) +
  optional model (routed).
- Facts the dialog shows today: what is kept in sync from the main profile (CLAUDE.md, rules, skills, plugins, MCP
  servers) and what is the account's own (login, history, usage); the CLI equivalent (`hrm add <name> …`).
- Create and log in: Claude: sign-in inside the app (a link opens the browser, the code is pasted back); other tools:
  sign-in in a terminal window; then the plan and the limits appear on the row. API-key and routed accounts are
  simply added.
- First run on a new computer: the tools found on the computer and their main profiles are listed as accounts; a
  first-run dialog asks whether to sync with other computers (a shared folder, a hub you host, or not now).
- The empty state (no tools installed, no accounts) must be designed too: what the page says and offers.
