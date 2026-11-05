# WP-6 report: the direct connection, UI

Branch `feature/peers-ui`, four commits on top of `26ec971` (`feaf00a` the UI, `cd20a21` the e2e tests, `976e12f` the chip
fix, `a9528e3` a popover CSS fix). No push. Only `app/ui/**`, `e2e/app.e2e.mjs` (appended tests plus two flipped assertions) and `README.md` (one
Features bullet) were edited. No Rust was touched.

## What was built, by file

- `app/ui/computers.js`: `heardOn(d, now)` (connected, or heard within three minutes) and `transportLine(c, d)` (the line a
  peer's card shows: "direct · mDNS · connected", "direct · <address> · connected", "relayed through <name>", "not reachable
  · folder only" for a paired peer not heard from, else "folder · heard 40 s ago" / "folder · not heard from yet");
  `fmtCode` ("482915" → "482 915"); the pairing dialog `pairDialog(tab)` with its `pair` and `pair-tab` actions: **Show a
  code** calls `computers_pair_start`, shows the six digits large with a countdown to `expires_at`, the host hint
  ("On another network, enter this address with the code: <host_hint>"), Cancel (closing the dialog while a code is live
  calls `computers_pair_cancel`), "The code expired." with Show a new code at the end; the `computers-pairing` event
  (`{ok, peer?, error?}`) turns the body into "Paired with <name>" and closes the dialog after 1.5 s (or shows the error);
  with no dialog open a successful pairing is a toast, and the page catches up either way. **Enter a code**: one input,
  digits only, six at most, formatted as it is typed, an optional Address (`name:port`), Pair calls `computers_pair_join
  {code, host?}`; a code shorter than six digits is refused in the dialog without a call; the op's error text is shown in
  the dialog (`[data-testid=pair-error]`) and Pair can be tried again; on success the dialog closes, "Paired with <name>"
  (the peer with the newest `paired_at` in the returned status), the page redraws and the new card is scrolled into view.
  The chip's dot is teal when `connected` or heard within three minutes. The one-minute status poll became a 10 s tick
  that reads the status every 10 s while `settings.direct_connection` is on or any peer is `connected` (else once a
  minute, as before) and dispatches a `presence` event with the status. `computersStatus()` now redraws the chip with
  every fresh read (see "Decisions").
- `app/ui/views.js` (Settings › Computers): the **Direct connection** row's switch is live (`computers_set
  {direct_connection}` through the existing `computers-set` action, checked from `settings.direct_connection`), the
  "phase 2" pill is gone, the row gets an ⓘ popover that ends with "Listens on port <port> Change…" (`port` from
  `computers_status`; "chosen at first use" when absent); Change… opens a small dialog (`listen-port` action) whose Save
  calls `computers_set {listen_port}` (0 = pick a free one; 0–65535 checked in the dialog). **Pair a computer…** opens
  the dialog. A peer's **card** gains the transport line (`[data-testid=transport]`) with an ⓘ popover holding Paired
  <date> (or "Not paired: known through the folder only"), Last pull <ago>, the Address and Relayed through when set;
  below it an Address field with Save (`computers_set_peer {id, address}`, paired peers only) and Forget
  (`computers_forget {id}` after a confirm). The card's status line reads "connected" for a connected peer. The 10 s
  `presence` event redraws the Computers section in place, unless a dialog is open or a field in the section has focus.
  The **CLI reference** page gets a Computers group: `computers status | now | set | pair | join <code> | peers | forget
  <id or name> | serve --seconds <n>`.
- `app/ui/home.js`: the Running now panel is drawn by one function, `runningNow(running, pcs, now)`; on the `presence`
  event, on the Overview with no dialog open, `live` is read and only that panel is replaced in place (no page redraw,
  no flicker; the routes and tiles are left as they are).
- `app/ui/app.css`: appended to the "Across computers" block: `.dev .f` (the address row on a card), `.dev .st.tr`
  (the transport line lets its popover out), the popover's font and inline bold words, `.paircode` (the six digits),
  `.linkb` (the Change… link in the popover).
- `app/ui/guide.js`: two settings steps, on the Direct connection switch and on Pair a computer.
- `README.md`: a new Features bullet "Direct connection, on screen" after the "Computers" bullet (WP-5 owns the
  Computers paragraph itself, so it was left alone; the two are adjacent, see "For the merge").

## Gated on WP-5 (built to the contract, untestable here)

Everything that needs the phase 2 ops: `computers_pair_start` → `{code, expires_at, host_hint, port}`,
`computers_pair_cancel`, `computers_pair_join {code, host?}` → the `computers_status` shape, `computers_forget {id}`,
`computers_set_peer {id, address?, name?}`, `computers_set {direct_connection, listen_port}`, the `computers-pairing`
event `{ok, peer?, error?}`, and the new fields read from `computers_status`: `port` (this computer) and, per peer,
`address, path ("mdns"|"address"|"relay"|""), connected, last_pull, paired_at, relayed_by, direct`. On this branch
phase 1's `computers_set` accepts `direct_connection` and `listen_port` silently (it ignores them), so the switch and the
port dialog run without an error but do not stick; every other op answers "unknown operation", which the dialog shows
as its error text (that is what the ungated test checks). The gated e2e test (`HRM_E2E_PEERS=1`) has not been run
against a backend.

## Decisions where the contract or the mock is silent

- **Transport line order**: `connected` wins ("direct · mDNS" when `path` is not "address"; the address when it is);
  then `path == "relay"` with `relayed_by`; then a paired (`direct`) peer not heard within three minutes reads "not
  reachable · folder only"; everything else is the folder with when it was heard.
- **Address and Save** only on paired peers (`direct`); a folder-only peer has nothing to connect to. **Forget** is on
  every peer card, since `computers_forget` deletes the row either way.
- **The listen port** lives in the Direct connection row's ⓘ popover, as the brief says; Change… there opens a small
  dialog rather than editing inside the popover (a hover popover is a poor place for a text field).
- **Which peer was just paired**: `computers_pair_join` returns the whole status, so the dialog names the peer with the
  newest `paired_at`.
- **The 10 s poll** reads `computers_status` (cheap) and dispatches `presence`; pages decide what to redraw. The
  Overview reads `live` and swaps only the Running now panel; the route dots in the WebGL topology are not redrawn
  ("nothing new to draw"). Settings redraws its Computers section unless a field there has focus.
- **The chip fix** (`976e12f`): the chip was drawn from a status cached up to five seconds earlier, so right after the
  folder was chosen the Settings page's own fresh read left the chip hidden. Phase 1's gated test ("the chip shows with
  sync on") failed on the base commit too (checked by building the base `app/ui` and running that test alone). Now every
  `computersStatus()` read redraws the chip. With that, phase 1's `HRM_E2E_SYNC=1` test passes on this branch.
- **The WP-4 ungated test** asserted the switch and Pair disabled and a "phase 2" pill; those two lines are flipped
  (enabled, no pill). Nothing else in earlier tests changed.
- The gated pairing test reads the code and the host hint from `hrm computers pair`'s stdout with two patterns (six
  digits with an optional space; `name:port`), since the contract fixes what is printed but not its layout; after
  `pair` ends it starts `hrm computers serve --seconds 150` in the second data folder so this computer can connect, and
  the second folder gets its own `HRM_PROFILES_HOME` with a seeded Claude session, indexed by `hrm init` and
  `hrm sessions` there before pairing.
- Line endings: the UI and e2e files are CRLF in the working copy (LF in the index through autocrlf, as before); the
  README's working copy kept its existing endings (it was CRLF, as WP-4 found).

## Tests

`cargo build --workspace` then `cd e2e && node app.e2e.mjs`, full suite, with the final UI:

```
43 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\peers-ui\e2e\.work\shots
```

Phase 1's gated test, alone, after the chip fix (`HRM_E2E_SYNC=1 HRM_E2E_ONLY="second computer in the sync folder"`):

```
ok   computers: a second computer in the sync folder reaches Settings, the chip, the Overview, Sessions and Projects
1 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\peers-ui\e2e\.work\shots
```

`cargo test --workspace` (no Rust touched):

```
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.93s
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.51s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.29s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.81s
```

(plus five empty `0 passed` targets). `cargo clippy --workspace` prints 13 pre-existing warnings in `hrm-core`,
`hrm-router` and `hrm` (`sort_by_key`, a large `Err` variant, a complex type and the like); none in files this branch
touches, and none new.

Screenshots worth looking at (in `e2e/.work/shots` after a run): `41-pair-show-code.png` (the dialog, Show a code, the
op's error in place of the code), `41b-pair-enter-code.png` (Enter a code: "482 915", the address, the op's error, Pair
enabled again), `19-settings-computers.png` (the Direct connection row live, no pill), and from the phase 1 gated run
`19b-settings-two-computers.png` (the chip with both computers, the LAPTOP card; its transport line, popover and Forget
sit below the fold of that shot and were checked with a temporary extra screenshot, not kept). The pairing screens
(`42*`) only come from the WP-5-gated test.

## For the merge

- `app/ui/core.js`, `app/ui/lib.js`, `app/ui/index.html`, `app/ui/app.js`, `app/ui/accounts.js`: **not touched**.
- `app/ui/computers.js`: the header comment and the import line (7–8); `heardOn` and `transportLine` after `nameOf`
  (26–35); one expression in `drawChip` (`heardOn`); `drawChip(c)` in `computersStatus`; the tick and the whole pairing
  block replace the old one-line `setInterval` at the end of the file.
- `app/ui/views.js`: the import at line 7; in `drawComputers` the card (lines 739–749: `stale`, `heard`, `facts` lose
  "over <transport>", `when`, `peer`, and `${peer}` at the card's end) and the Direct connection row (line 760); the
  four actions and the `presence` listener after `computers-now` (783–802); the Computers group in `CLI` (896–898).
- `app/ui/home.js`: `runningNow` and the `presence` listener before `view('home'` (318–327); the page body uses
  `${runningNow(running, pcs, now)}` (366) in place of the inline panel.
- `app/ui/guide.js`: two lines in `TOURS.settings` after the Computers step (72–73).
- `app/ui/app.css`: ten lines appended inside the "Across computers" block after `.dev .bar i.warn` (1265–1274).
- `README.md`: six lines inserted after the "Computers" bullet (114–119). WP-5 edits the bullet just above; a merge
  of both touches adjacent lines, so expect a trivial conflict there.
- `e2e/app.e2e.mjs`: two assertions flipped in the WP-4 ungated test (1262–1263); the WP-6 block appended before
  `exitCode = …` (1413–1525). The gated test needs `HRM_E2E_PEERS=1` and `hrm computers pair | serve` from WP-5.
- The e2e folder needs `npm ci` in `e2e/` on a fresh worktree (`playwright-core`); the suite wipes `e2e/.work` at
  start, so never write a log there.
