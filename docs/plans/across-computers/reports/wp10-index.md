# WP-10: the index as the source, and a folder watcher

Branch `feature/index-reads` from `develop` at 1.6.2, in the worktree `HarnessManager-wt/index`. Not pushed.

## What was built, by file

**`crates/core/src/index.rs` (new).** The only code that walks the tools' folders. `refresh(ctx, Scope::All |
Scope::Paths(..))` runs the harness scans (unchanged; they keep their per-file cache rows), resolves every
session's project, marks what is open here, journals this computer's sessions through the existing
`journal_sessions` (phase 1 rules), reconciles the index rows (written only where the JSON, the project or
`open` differ; removed when the file is gone), writes `project_index`, and records `index:refreshed_at` on a
full walk. `Paths` maps the changed files to the accounts whose session folders contain them and refreshes
those. `mark_open` (the presence tick, once a minute) keeps the `open` column right when a console closes
without a file change. `refresh_if_older` (the CLI), `rebuild` (`--all`), `status` (`hrm index status`).
Four tests: the index answers the scan's list, pages with totals, searches a word of a title (FTS5); a changed
file re-read by `Paths` changes its row and nothing else; a removed file drops its row and swapped fixtures
converge (and a rebuild lands on the same list); the first read of an empty index refreshes.

**`crates/core/src/watch.rs` (new).** The watcher on `notify` 8: every account's session folders
(`session_dirs`, a new `Harness` method implemented for Claude, Codex, Pi/Oh My Pi, OpenCode and
Antigravity), recursive, rebuilt when the folder set changes (checked once a minute, so accounts added by
`hrm init` are picked up). A burst settles 500 ms (3 s at most), then the usage import runs from the cursors
for the touched accounts and `index::refresh(Paths)` follows; the app emits `refreshed` and the page redraws
in place. A folder that cannot be watched is logged once and the periodic walk stays its refresh. A heartbeat
in `index:watcher_at` lets the CLI say whether the watcher is on. One test: a file written under a watched
folder is in the index within two seconds (skipped with a note on a `CI` runner that drops notifications).

**`crates/core/src/db.rs`.** The migration in `Db::open`: promoted columns `id, title, cwd, started_at,
last_active_at, tokens, project_key, project_name, open`; indexes `sessions(device, last_active_at)`,
`sessions(project_key, last_active_at)`, `usage(device, project)`, `usage(day, project)`; `project_index`;
the FTS5 table `sessions_fts(title, cwd, profile)` as external content over `sessions` with insert, delete
and update triggers (`recursive_triggers` on, so `insert or replace` elsewhere stays in step); existing rows
promoted once (`migrated:index`) and the FTS rebuilt. `put_session` became an upsert that keeps the project
and `open` columns the index resolved; `upsert_session` (journal rows, restore, merges) sets every column.
`prefix_end` turns the `like prefix%` scans of the cache into primary-key ranges.

**`crates/core/src/service.rs`.** `sessions`, `sessions_total`, `grouped_sessions`, `projects` and
`transcript` read the table: one SQL over the index rows (this computer's cache rows, the other computers'
rows, the archived table) with the filter in SQL, `per`/`page`, a `total`, and the search as FTS5 prefix words
or a substring of the title or id. `SessionFilter` gained `device` (one other computer), `day` (a date
group's rows), `id`, `per`, `page`; `ProjectGroup` gained `total`. Groups are built from a light query
(no JSON) for their counts and tokens over the whole filter, and the page's rows are placed in them.
`transcript` falls back to a refresh of the account when the id is not in the index yet (a file just
written), then reads again. `alias_project` refreshes the index so renames and merges reach every row.
No harness scan runs on a page open; the first read of an empty index runs one.

**`crates/core/src/projects.rs`.** The filesystem part of `Resolver::resolve` (git root, origin) is
memoized for five minutes across calls: the refresh and the pages resolve the same few hundred folders.

**`crates/core/src/transcripts.rs`.** `roots_here` reads the promoted `cwd` column; `resume_here` refreshes
the account after the copy so the fork is listed at once.

**`crates/core/src/harness/*.rs`.** `session_dirs` per tool; the three file-based scans parse a changed
file now when `parse::PARSE_NOW` is set (the walk and the CLI), and keep 1.6.2's cached-record-plus-
background-parse on a watcher refresh, so a rollout being written is not re-read whole on every burst.

**`app/src-tauri/src/main.rs`.** The five-minute tick runs `index::refresh(All)` after the imports; the
presence loop calls `index::mark_open`; the watcher starts at setup and emits `refreshed`; the `sessions`
op answers `{groups, total}`; `detail` asks for 40 rows.

**`crates/cli/src/main.rs`.** `hrm index status [--json]` and `hrm index refresh [--all] [--json]`;
`hrm sessions` and `hrm projects list` refresh first when the index is older than the refresh interval
(the CLI has no walk of its own).

**`app/ui/views.js`, `app/ui/app.js`.** The Sessions page sends `per` 50, `page`, the search, the computer
and the group; draws from the answer; shows a pager (`page-prev`, `page-next`); a group's "Show all N
sessions" fetches the group's rows (`s-more-<key>`); the top bar search asks the index after a 250 ms
pause and is re-applied on every draw. `refreshed` events coalesce into one redraw per burst.

**`crates/core/examples/timing.rs`.** Times `index::refresh(All)`, a page of 50, its total, the grouped
page and a search.

**`e2e/app.e2e.mjs`.** The seeds that must wait for the index (below) and the new block
`// ---- index and watcher (WP-10) ----`.

**`README.md`.** One paragraph under "How it works".

**`crates/core/Cargo.toml`.** `notify = 8` (default features off, `macos_fsevent`). New crates in the lock and
their licences: `notify` 8.2.1 (CC0-1.0), `notify-types` 2.2.0 (MIT OR Apache-2.0), `inotify` 0.11.5 and
`inotify-sys` 0.1.8 (ISC, Linux only), `kqueue` 1.2.1 and `kqueue-sys` 1.1.2 (MIT, BSD only),
`objc2-core-services` 0.3.2 (Zlib OR Apache-2.0 OR MIT, macOS only); `mio`, `filetime` and `walkdir` were
already in the lock. FTS5 is in the bundled SQLite (`libsqlite3-sys` sets `SQLITE_ENABLE_FTS5` for
`bundled`), so no `rusqlite` feature changed.

## Decisions where the contract was silent

- **Which rows are this computer's.** The scans' per-file cache rows (`<tool>|<account>|<file>`) are the
  index rows for this computer, as the contract lists them; OpenCode's sessions share one database, so their
  key is `opencode|<account>|<id>`, and Antigravity's rows are written by the reconcile (their scans kept no
  cache). This computer's journal rows (`<tool>/<account>/<id>`) stay the outbox mirror and are not read, so a
  session continued from another computer is listed twice as before (theirs and the copy here).
- **Archived sessions** stay in the `archived` table and join the read through a `union all` with
  `json_extract` columns; they are few, and this keeps backups, restores and their counts as they were.
- **The CLI** reads the index and refreshes first when it is older than `refresh_minutes` (the app's walk
  keeps it fresh while the app runs; without the app `hrm sessions` costs what it did).
- **The watcher's refresh** keeps the 1.6.2 handling of a file still being written (the cached record with
  the file's time now, the full parse in the background, one at a time); the walk and the CLI parse now.
- **`alias_project`** runs a full refresh: a rename or merge changes the project of many rows.
- **Group totals with paging:** a page is 50 sessions newest first, grouped; each group shows the whole
  filter's count and tokens, and "Show all" fetches that group's rows through the same op.
- **`open`** is set by every refresh and by the presence tick; the Accounts page's live counts still come
  from the harnesses' `running()` as before.

## Timings (`cargo run -p hrm-core --example timing`, dev build, on a copy of this computer's data)

8 accounts, 8,349 sessions in the index (4,930 Claude Code, 2,582 Codex, 86 Antigravity, 16 OpenCode),
296,000 usage rows, journaling on. "Before" is 1.6.2 on the same copy, second (warm) run; "after" is the
fourth run of this branch.

| Step | Before | After |
|---|---|---|
| `service::sessions`, every row (the CLI's `hrm sessions`) | 2,049 ms (scans + journal + resolve) | 546 ms (one query, 8,349 JSON rows parsed) |
| `service::sessions`, a page of 50, 7 days (the Sessions page) | — (not paged) | 129–168 ms |
| `sessions_total` | — | 22–29 ms |
| `grouped_sessions`, the page of 50 | 2,317 ms (whole list) | 186–225 ms |
| `grouped_sessions` with a search | — (client-side) | 59–76 ms |
| `grouped_sessions`, every row | 2,317 ms | 520 ms |
| `projects` (the Projects page) | 3,181 ms | 432–522 ms (of which the usage cost-per-day query is 300–550 ms, as before) |
| `index::refresh(All)` (the background walk) | — (the pages did this) | 2,187 ms warm; 4,869 ms when the 145 MB rollout being written is re-parsed |
| harness scans, all accounts | 753 ms | 658 ms (unchanged code) |

In the window (debug build, the e2e sandbox) a file written under a watched folder was on the Sessions page
without a reload in 821–848 ms over four runs (`indexed and drawn in 848 ms` in the green run's output).

What is left in a page: parsing the JSON of 50 rows and `decorate` (tags, peers, device names, forks, and
`roots_here` when a row from another computer is on the page). The Projects page's remaining time is the
request-log query it always had.

## Skipped

- Virtualizing the DOM (option 4 of the measurements): with 50 rows a page it is moot.
- The `hub` crate and the router: untouched (they do not list sessions).
- A `changed` event distinct from `refreshed`: the window already redraws on `refreshed`; the watcher
  reuses it with `{"index": report}` in the payload.
- The `scan_cached` two-second cache the brief mentions: it is not in this tree (1.6.2 shipped
  warm pages instead); nothing to remove.

## Tests

`cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace` (only warnings that were
there before: `sort_by_key` suggestions in `dash.rs`, `codex.rs`, `service.rs:384`, `manual_contains` in
`procs.rs`, `chunks_exact` in `secrets.rs`), the window suite with `HRM_E2E_SYNC=1 HRM_E2E_PEERS=1
HRM_E2E_HUB=1`.

Three earlier full runs each had one failure that was not the code: the first run's search race (fixed, see
`views.js`: the typed search is re-applied on each draw) cascaded into every page that filters by the top bar,
and two runs crossed local midnight, where "Pi and Oh My Pi" (a reply dated the new day falls outside the
7-day window computed a second earlier) and "a second computer in the sync folder" (its rows seeded an hour
back are "yesterday" on the Today range) fail on the clock alone; both pass in a run inside one day. The
fourth run, started at 01:02, is the one below.

Verbatim lines:

```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test --workspace
     Running unittests src\main.rs (target\debug\deps\fakeharness-024f9b33a321e0e6.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\main.rs (target\debug\deps\hrm-7c1fc09f15995d22.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\e2e.rs (target\debug\deps\e2e-9b126cb0703db721.exe)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.13s
     Running unittests src\main.rs (target\debug\deps\HarnessManager-e610735b7f65d61b.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\lib.rs (target\debug\deps\hrm_core-3e251c7bc27b6d2d.exe)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.20s
     Running tests\data_safety.rs (target\debug\deps\data_safety-05b59da44ae52e1c.exe)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.52s
     Running tests\pi_sessions.rs (target\debug\deps\pi_sessions-512121cb1509bf81.exe)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.82s
     Running unittests src\lib.rs (target\debug\deps\hrm_hub-4faed9d7f57c3e29.exe)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
     Running unittests src\main.rs (target\debug\deps\hrm_hub-0a283673f13d7185.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\lib.rs (target\debug\deps\hrm_router-9d41594902c80898.exe)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\router_e2e.rs (target\debug\deps\router_e2e-0d66912828e9d2bf.exe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.63s
   Doc-tests hrm_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests hrm_hub
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests hrm_router
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo clippy --workspace
(no warning in index.rs, watch.rs, or the lines this package added; the pre-existing ones are listed above)

$ HRM_E2E_SYNC=1 HRM_E2E_PEERS=1 HRM_E2E_HUB=1 node e2e/app.e2e.mjs   (started 01:02 on 6 Nov 2026)
ok   overview: key numbers, live routes, limits, reply timing, mix, events, health and the request log
ok   sidebar: Setup folds and stays as left, a page inside opens it, the pinned projects fold
ok   accounts list shows the main profiles of installed tools
ok   add a Claude account: in-app sign-in with a pasted code, then plan and limits
ok   closing the sign-in dialog stops the waiting sign-in
ok   Start opens a terminal running the account
ok   add an API-key account
ok   add a Codex account: sign-in opens in a terminal
ok   account detail: overview, sessions, projects and settings tabs
ok   Start in a folder: a recent folder of the account, skip permission prompts, extra arguments
ok   sessions are grouped by repository across Claude Code and Codex
ok   projects: cost per project, pin, rename and tabs in place, and a project opened in Sessions
ok   reading a session: steps, failed command, answer and changes
ok   an open session shows new messages as its tool writes them
ok   tags: on a session and a project, grouped and filtered by tag, and seen by the CLI
ok   OpenCode and Antigravity sessions are listed and readable
ok   a turn with thousands of steps shows its first and last steps until asked
ok   export a session as Markdown
ok   the window follows the Mission Control design in the Harness theme
ok   measurements match the design pages
ok   every page has the sections of its design page, in order
ok   usage lives on Accounts: the chart, each row's tokens and cost, the range, and the old address
ok   routing: add a provider and a route, start and stop the router
ok   MCP servers: add, write to tools, edit as JSON, search, and import from apps
ok   skills: install from a folder and link into Codex
ok   skills: folders in the store that the list lacks show as Not managed, and Manage adds them
ok   skills: Discover lists the skills of each repository and installs one into the store
ok   apps: each tool with its path and version, and Update runs its installer in a terminal
ok   prompts: write a prompt into the tool file, keeping the old content
ok   refresh says what it did, and why Claude limits stay old while Live is off
ok   backups: back up, lose a skill folder, restore it; export to a file; the interval saves on change
ok   appearance: dark theme, limits as used, and a hidden tool left out of the pages
ok   a new Harness Manager: found at launch, offered in Settings > Updates
ok   settings: change the terminal
ok   a walkthrough on a page's first visit, and setting tooltips with their defaults
ok   rename an account: shown everywhere by its new name, the id stays
ok   remove an account and its folder
ok   remove accounts from the list: keep the folder, and a main profile keeps everything
ok   closing the window keeps the app running in the tray
ok   computers: the Settings section in the off state, the chooser, no chip, and the Overview as before
ok   computers: the All computers · This computer choice is remembered on this computer, All by default
ok   computers: a second computer in the sync folder reaches Settings, the chip, the Overview, Sessions and Projects
ok   Pi and Oh My Pi: seeded sessions appear in the window and in hrm sessions and hrm usage
ok   direct connection: the switch without its pill, the dialog with both tabs, the code input, and an unknown operation shown as the error text
ok   direct connection: pairing with a second computer run by hrm computers pair, the card direct and connected, its session after Sync now, Forget
ok   resume here: a remote session is read from the folder and resumed as a fork in the local clone
ok   hub: the card is live, the Join dialog takes a code or the admin token, an unreachable hub is the error text, the first-run choice offers both cards
ok   hub: joining from the card with the admin token, the recovery key once, Invite a computer, a second data folder joining with the code, its session after Sync now
ok   backups and archive: the section, hrm archive now on an old session, Restore… from a backup, Welcome back
     indexed and drawn in 848 ms
ok   index and watcher: a seeded file is on the Sessions page within two seconds, the search finds a word of a title, and the list pages at 50
50 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\index\e2e\.work\shots
```

## Seeds changed in the e2e

- "Pi and Oh My Pi": `hrm('index', 'refresh')` after `hrm('init')` (the new accounts' files into the index
  now; the loop on `hrm sessions` stays).
- "direct connection …" (WP-6): `hrm2('sessions')` became `hrm2('index', 'refresh')` (the second computer's
  index, which its journal carries).
- "the hub …" (WP-9): both `hrm3('sessions')` calls became `hrm3('index', 'refresh')` (before the join, and
  again once sync is on so the sessions are journaled).
- "backups and archive …" (WP-8): `hrm('index', 'refresh')` after `hrm archive now`, before the Sessions
  page is opened (the moved file leaves the index now, not at the app's next walk).
- "sessions are grouped by repository …": the top bar search assertion now expects the hash to carry
  `q=longhorn` and counts the rows present (the index answers; nothing is hidden client-side).
- Seeds that open a session page right after writing its file (`s-live`, `cx-long`) are unchanged:
  `transcript` refreshes the account when the id is not indexed yet.
- New block `// ---- index and watcher (WP-10) ----`: a file written while the Sessions page is open appears
  within two seconds without a reload (`waitFor` at 2,000 ms) and `hrm index status --json` says the watcher
  is on; the top bar search and `hrm sessions --search` find it by a word of its title; 60 more sessions and
  `hrm index refresh` give 50 rows and a next page; a group's "Show all" fetches its rows.

## Merge notes

- The migration is additive (columns with defaults, indexes, one table, one virtual table, triggers) and runs
  once per database at open; the first open after the update also rebuilds the FTS rows and the first walk
  writes the project columns of every row. A fresh install's first launch parses every session file once
  (the index is the source now), the way "All" days used to.
- `sessions_fts` is an external-content FTS5 table keyed by `sessions.rowid`; the code never runs `VACUUM`
  (which could renumber rowids). `hrm index refresh --all` rebuilds it should it ever drift.
- `SessionFilter` and the `sessions` op changed shape (`{groups, total}` instead of an array); the Sessions
  page is the only caller. `ProjectGroup` has `total`.
- `put_session` no longer resets a row's project and `open` columns; `upsert_session` sets every column, so
  rows merged from other computers and restored backups carry them.
- The CRLF/LF split is as in the repository: UI and e2e CRLF in the working copy, everything else LF.
- Three commits on `feature/index-reads`, oldest first: the Rust (index, read path, watcher, app, CLI,
  probe); the Sessions page, the e2e and the README; this report.
