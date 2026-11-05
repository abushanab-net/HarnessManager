# WP-4 report: the UI for computers and the "All computers" views

Branch `feature/sync-ui`, two commits on top of `220133b` (`d5462ce` the UI, `483cbe3` the e2e tests). No push.
Only `app/ui/**`, `e2e/app.e2e.mjs` (appended tests) and `README.md` (one Features bullet) were edited.

## What was built, by file

- `app/ui/computers.js` (new): `computersStatus()` calls `computers_status` and turns an "unknown operation"
  error into the "sync off, no peers" state (`{mode:'off', peers:[], this:{name: state.computer}}`), kept in
  `S.cache.computers`; `syncOn`, `computers`, `nameOf`, `cdot`/`cdots` (a dot per computer in its colour: this one
  teal, then sand, iris, amber, by place in the list), `splitBar` (a thin bar of a value by computer and the largest
  named), `tileSplit` (reads `tiles[*].by_device`, keyed object or array, on the home answer or its `dash`); the
  Overview scope (`scope()`/`setScope()` in `localStorage['hm.scope']`, default `all`, the `scopeSeg()` segment and
  its `scope` action); the top-bar chip `drawChip()` (a dot per computer, teal when `last_seen` is within three
  minutes, "synced N ago" from `last_run`, hidden when `mode` is off), drawn after every page render and once a minute.
- `app/ui/index.html`: one span `#pcs` (`data-testid=computers-chip`, hidden) before `#fresh` in the top bar.
- `app/ui/app.js`: `import './computers.js'`.
- `app/ui/home.js` (Overview): `home` is called with `scope`; the "All computers · This computer" segment sits
  before the period segment when sync is on; each tile gets the split bar and the largest computer in its note (the
  cache-hit tile lists each computer's rate instead of a bar); the Activity grid gets an "All · <each computer>"
  segment when there are peers, reads `activity.days[*].by_device`, and its hover caption carries the split; Live
  routes: a dot per computer on an account card where that account runs (this computer from `profiles[*].running`,
  others from `live.running[*].device`), traffic from another computer only in the sand colour (`'p'` edge), tool
  cards count peers' open sessions, two legend entries; a "Running now" panel (`data-testid=running-now`) lists
  `live.running` with the computer's name. With no peers none of this is drawn: the page is as before.
- `app/ui/accounts.js` (`sessionRow`): with sync on, a Computer cell (dot + name, `.pc`) and the `withpc` class on the
  row; a session with `open_elsewhere` reads "running on <name>" in the time cell and its action cell is left empty.
- `app/ui/views.js`: Sessions calls `sessions` with `scope` (`here` for This computer, else `all`), adds the
  "All computers · This computer · <each peer>" segment (`pc` in the hash; a peer's id filters client-side by
  `device`); Projects fetches the status with the list and each row card ends with a dots cluster of the computers that
  have the repository (`devices[*]`) and "not cloned here" when `here` is false; Settings gets a **Computers**
  section above Backups (`drawComputers`): this computer's name (Rename → `computers_set {device_name}`), the
  chooser (the shared-folder card with the path, Choose…/Change… through `pickFolder` →
  `computers_set {sync_mode:'folder', sync_folder}`, "Stop syncing" → `sync_mode:'off'`; the hub card disabled with a
  "phase 5" pill and a greyed address field), Carry transcripts and Presence switches (`computers_set`), Direct
  connection disabled with a "phase 2" pill and a greyed "Pair a computer…", then a header line (N computers · how
  they sync · last round) with **Sync now** (`computers_now`, toasts the result) and a card per computer (name with
  its dot, OS · version · "writing now" / "heard N ago" / "not heard from yet", a thin bar of `applied_seq` over
  `last_seq` (this one: `published_seq`), and projects/accounts counts when the status carries them). Info popovers
  on the rows as elsewhere.
- `app/ui/app.css`: one block appended at the end ("Across computers"): `.cdot`/`.cdots`, `.pcs` (the chip, in the
  `.udot`/`.count` family), `.ovk .split`, `.tn .cdots`, `.legend .lp`, `.runrow`, `.srow .pc` and the `withpc` grids,
  `.pcs-row`, `.mcards`/`.mcard`, `.devhead`, `.devs`/`.dev`. Everything through the existing variables, so light and
  dark both follow; nothing changes a layout while sync is off.
- `app/ui/guide.js`: a home step on the scope switch (shown only when the switch is on the page) and a settings
  step on the Computers section.
- `README.md`: a Features bullet "Across computers, on screen".

## Gated on WP-3 (built to the contract, untestable here)

Everything that needs `computers_status` to answer with `mode != 'off'`: the chip, the scope segment and the split
bars, the Activity switch, the route dots and Running now, the Computer column and filter, the project dots, the
computer cards for peers, Sync now. Fields read exactly as the contract names them: `computers_status.{mode, folder,
this.{id,name,last_seq,published_seq}, last_run, error, peers[].{id,name,os,app_version,last_seen,last_seq,
applied_seq,transport,running}}`, home `tiles[*].by_device[{id,name,value}]` and `activity.days[{day,tokens,
by_device:{id:tokens}}]`, sessions rows `device, device_name, open_elsewhere`, projects rows `devices[{id,name,
sessions,tokens}], here`, `live.running[*].device`. The e2e test `computers: a second computer in the sync folder…`
runs only with `HRM_E2E_SYNC=1`: it writes `devices/<id>/{device.json, manifest.json, journal/<inc>-1-3.jsonl,
presence.json}` into a temp sync folder, turns the folder on with `computers_set`, clicks Sync now and asserts every
page. It has not been run against a backend.

## Decisions where the plan or the mock is silent

- The Computers section sits **above Backups** (the mock puts it first; the existing section-count test fixes the
  first seven sections, so the brief's "above Backups" placement keeps that test as it is).
- Turning sync off: the mock has no control for it; a small **Stop syncing** button appears in the folder card when
  the folder is on (`computers_set {sync_mode:'off'}`).
- The per-computer **projects and accounts counts** on a card are not in the `computers_status` contract; they show
  when the status carries `projects`/`accounts` numbers, else the card shows running sessions, the transport and the
  journal size. The progress bar is `applied_seq / last_seq` (this computer: `published_seq / last_seq`).
- "Traffic from another computer" on a route: the contract gives no per-computer split of `live.accounts`, so an
  account's route is drawn in the other computer's colour when it runs **only** on another computer.
- Sessions on a **peer** (not "here"): the backend scope has only `all`/`here`, so a chosen peer is filtered on the
  page by `device`, with the group totals recomputed.
- The gated test chooses the folder with `computers_set` instead of seeding `settings.json` at the top of the e2e
  file (the brief asks for appended tests only; seeding at the top would touch line 38, which WP-1 may also merge
  around). The journal chunk's manifest entry uses the bare file name (`<inc>-1-3.jsonl`) for `chunks[].file`.
- The mock's "N active days, M on more than one" under the Activity grid is not built (not in the brief).
- Line endings: `git ls-files --eol` shows every file stored LF with CRLF working copies (autocrlf); the working
  copies of the edited UI/e2e files and the new `computers.js` are CRLF. `README.md` was already CRLF in the
  working copy; untouched.

## Tests

`cargo build --workspace` then `cd e2e && node app.e2e.mjs`, full suite, twice (before and after a one-line width fix):

```
41 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\sync-ui\e2e\.work\shots
```

The two new ungated tests pass (`computers: the Settings section in the off state…`, `computers: the All computers
· This computer choice is remembered…`). No Rust was touched; `cargo test --workspace` after the
final build:

```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s   (x5)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.53s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.96s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.74s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo clippy --workspace` printed no warning or error lines.

Screenshots worth looking at (in `e2e/.work/shots`): `19-settings-computers.png` (the new section, off state),
`17-dark-settings.png` (dark; the section is below the fold), `00-home.png` / `06-sessions.png` / `06d-projects.png`
(unchanged while sync is off). The sync-on screens (`19b`–`19f`) only come from the gated test.

## For the merge

- `app/ui/core.js` and `app/ui/lib.js`: **not touched**.
- `app/ui/index.html` line 37: the `#pcs` span inserted before `#fresh`.
- `app/ui/app.js` line 12: `import './computers.js';`.
- `app/ui/accounts.js`: line 7 (import) and `sessionRow` lines 263–272.
- `app/ui/guide.js`: one line in `TOURS.home` (after "Tool and period") and one in `TOURS.settings` (after "Save").
- `app/ui/views.js`: the import at line 7; `view('sessions')` (the `pc` param, the call, the client filter, the
  segment); `pcDots` before `nameEl` and its use in `drawRows`; `fetchProjects`; the settings template (the Computers
  heading and placeholder before Backups) and `drawComputers(s, st)` before `drawBackups(s)`; the new
  `drawComputers` block and its actions before `kb`.
- `app/ui/home.js`: imports, `st.device`, `heatmap(days, metric, width, pc)`, `kpis(d, ps, risks, home, pcs)`,
  `topology(state, d, pcs)` and its `peers` return, the `COL.p` colour, and the page body.
- `e2e/app.e2e.mjs`: three tests appended under `// ---- across computers (WP-4) ----`, before `exitCode = …`.
  The gated one needs `HRM_E2E_SYNC=1`.
- Hard-coded tool lists (`TOOL` maps) were not changed; WP-1 adds Pi/Oh My Pi there if it needs to.
