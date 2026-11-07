# Stage 3 report: the approved design in app/ui

7 November 2026. Nothing is committed; the owner commits. The e2e suite was run against `cargo build` (debug) with
`node e2e/app.e2e.mjs`; the real-data screenshots come from `target\debug\HarnessManager.exe` on the owner's data with
`HRM_DEVTOOLS_PORT=9333`, driven by playwright-core from e2e/node_modules (the installed app kept running untouched).

## The suite

| pass | result | note |
|---|---|---|
| 1 | 43 passed, 4 failed | stale selectors in four tests (below) |
| 2 | 46 passed, 1 failed | the appearance test's measurements still named the old pages' classes |
| 3 | **47 passed, 0 failed** | |
| 4 | **47 passed, 0 failed** | green twice in a row |
| 5 | **47 passed, 0 failed** | after the CLI heading change, one more full pass |

50 tests are defined; 3 skip unless their environment is set (`HRM_E2E_SYNC`: a second computer in the sync folder;
`HRM_E2E_PEERS`: pairing with a second computer; `HRM_E2E_HUB`: joining a hub with the admin token). No `[window]`
console error in any pass.

The four failures were all assertions describing the old layout, fixed in the test (the app followed the reference):
- *the window follows the approved design*: the Session page's All / Changes / Failed modes are `[data-tour=modes] button`
  (filter chips, as drawn), not the old `.seg3 a`.
- *appearance: dark theme…*: the Sessions page is drawn in `.a-grp` group cards, not `.panel-card`, so the card count
  takes both; the `.a-srow` padding at the test's 1280 width is `6px 10px` (the reference's own narrow rule, which the
  app applies as a container query); the MCP, Skills and System prompts measurements now name the reference's classes
  (`.b-matrix`, `.b-toolbar`, `.b-prompt-grid`) instead of `.mcpmain`, `.sklist`, `.pgrid`.
- *hub: the card is live…*: the first-run sync choice puts its "Selected" pill inside the card title's `<b>` exactly as the
  reference does, so the test reads the title's own text node.
- *backups and archive*: the Sessions row's computer cell is `.a-pc`.

The two test couplings named in the brief ("backups: back up" relies on the skill "skills: install" creates; "add a
Claude/API/Codex" and "Start opens" start on #accounts) are untouched and pass in suite order.

## What stage 3 changed

- **app/ui/app.css** (3333 → 2483 lines). The stage-1 copy of Designer A's `a-` block (lines 2316–2561) is gone:
  pages-sessions.css supersedes it and the 44 rules it had that pages-sessions.css lacks (`.a-menu`, `.a-dialog`,
  `.a-chips`, `.a-stackbar`, `.l1`–`.l5`…) matched nothing in the JS. The legacy block (lines 2715–3333) is reduced to
  the 25 rules something still draws: the computers chip (`.pcs`, `.cdots`), the pager's `.btn` and `.seg3`
  (lib.js, used by MCP, Skills, Projects), `toolPill`'s `.tool`, the tour's buttons, the Add account dialog's
  `.clibox`, `.num`, `tr.paged`, `.fold`, and the new `svg.split-bar`. Deletion was rule by rule, not by comment anchor:
  a script split every selector list, kept a selector only if every class in it appears in a class attribute, a
  classList call or a querySelector string of app/ui/*.js|html, and the result was read before building. The alias
  block of old token names (`--s`, `--m`, `--line`…) keeps the 16 names the surviving rules read; 49 are gone.
- **app/ui/lib.js**: `meter`, `bars`, `timeline` and `diffHtml` (the carriers of the last inline `style=` attributes)
  plus `avatar`, `initials`, `hue`, `lvl`, `dayShort`, `resetText` had no caller after stage 2 and are deleted.
- **app/ui/accounts.js**: the old `sessionRow` (`.srow`) had no caller (sessions.js draws its own) and is deleted.
- **app/ui/computers.js**: `splitBar()` draws an `<svg class="split-bar">` of `<rect width="N%">` in the computers'
  colours instead of `<i style="width:…">`, like the other bars. It still has no caller (kept as the brief asked).
- **app/ui/guide.js**: the tour targets on the new pages: Sessions tags → `[data-act=tag-edit]`, Session modes →
  `[data-tour=modes]`, System prompts → `[data-testid=prompt-tabs]`; the Settings "Save" step is gone (settings save as
  they change, there is no Save button; the ⓘ step keeps `[data-testid=tip-term]`, which exists). Every other step's
  selector was checked against the page modules; `kpi-tokens` is built as `kpi-${key}` and resolves.
- **app/ui/cli.js**: the heading is the reference's ("Every desktop action has an hrm command." · "Add --json for scripts ·
  static reference"); the chip row of group jumps, which the reference does not have, is gone with its action.
- **e2e/app.e2e.mjs**: the selector and measurement edits above and the CLI heading in the sections test.

No `on*=` attribute and no `style=` attribute remain anywhere in app/ui (grep over *.js and *.html).

## Files changed across the three stages (git status)

Modified: `.gitignore` (adds `.delegate/`), `app/ui/accounts.js`, `app/ui/app.css`, `app/ui/app.js`, `app/ui/apps.js`,
`app/ui/computers.js`, `app/ui/core.js`, `app/ui/guide.js`, `app/ui/home.js`, `app/ui/index.html`, `app/ui/lib.js`,
`app/ui/limits.html`, `app/ui/mcp.js`, `app/ui/popup.js`, `app/ui/restore.js`, `app/ui/resume.js`, `app/ui/skills.js`,
`e2e/app.e2e.mjs`. Deleted: `app/ui/views.js`. New: `app/ui/cli.js`, `app/ui/projects.js`, `app/ui/prompts.js`,
`app/ui/routing.js`, `app/ui/sessions.js`, `app/ui/settings.js`, `app/ui/pages-sessions.css`, `app/ui/pages-setup.css`,
`app/ui/pages-system.css`, `docs/design-committee-2026-11/` (the whole committee record, including this folder).
Net over app/ui, e2e and .gitignore: 19 tracked files, +3932 / −3447 lines, plus the untracked modules.
Line endings: the working copy is CRLF under autocrlf; every edited file is uniformly CRLF (checked).

## The app on the owner's data against the reference

120 screenshots in `docs/design-committee-2026-11/implementation/shots/<page>-<size>-<brand>-<theme>.png`: 15 pages
(overview, sessions, session, projects, accounts, account, add-account, mcp, skills, prompts, routing, apps, cli,
settings, tray) × 1536×864 and 1280×720 × Harness/VektraGrid × light/dark, each at 1 px per CSS px. Every one was
looked at (the 1536 Harness-light set at full size, the rest on 2×2 contact sheets) next to the reference's
`round2/app/screenshots*/`. `console-errors.json` beside the folder is empty: no console error on any page in any
theme. Themes were set by writing `html[data-brand]`/`[data-theme]`, not through Settings, so the owner's settings
are unchanged; the tour marks were saved and restored.

What matches: the rail (192 px, nav counts, Setup fold, Pinned, the tray trigger), the top bar, the Overview's
three-card deck and Reset Horizon lanes, Accounts with its tool cards and per-window bars, Account detail's five stats
and Limits/Login cards, the Add account dialog (640 wide, tool and kind grids, CLI line), the Sessions rail and group
cards, the Session stats and conversation, Projects' tiles and share table and cards, the MCP and Skills matrices with
sticky headers, System prompts' list and live file, Routing's router/routes/providers/requests cards, Apps' tables,
CLI's cards, Settings' sections and the tray popup, in all four themes; VektraGrid's EB Garamond findings and
teal/plum/sand roles read correctly in light and dark.

Differences from the reference, and why:
- Real data, not the fixture: the owner has no routes (Routing shows the reference's empty state), no saved prompts,
  one Codex account, and long delegated-task titles that truncate with an ellipsis as the reference's rows do.
- The Overview's 5-hour window appears only on accounts whose 5-hour window is open (claude-main and codex-main show
  "Week · 0–7d" alone); the reference's fixture had every window open.
- The top bar's computers chip is set in the mono face (the stage-1 shell kept the old `.pcs` rule); the reference
  sets it in Inter. One rule in app.css if the owner wants it changed.
- The Session page's user bubble shows a whole delegated prompt (real content); the reference's bubbles were short.
- At 1280 the Session page's actions wrap to two rows, as the reference's own 1280 shot does.

## Observed on real data, not a design matter

- In one of the eight theme passes at 1280 (Harness dark), the Sessions page stayed on its "Reading sessions…"
  placeholder for more than 45 s and the Session and Projects pages queued behind it: core.js renders pages one after
  another (`chain.then(renderNow)`), so a slow `sessions` read on the owner's 341 sessions, while live sessions were
  being written, held every later page. It recovered on its own (Accounts and the rest drew normally in the same
  pass). The Rust side was out of scope for this work; worth a look at the `sessions` op's time with live writers.

## The critics' fixes still open

The three stage-2 reports were not in the repository or the committee record (`docs/design-committee-2026-11` holds the
brief, the critiques and the prototypes; `.delegate/runs` holds the design-round Codex/OpenCode runs), so this list is
what the stage-1 and stage-2 hand-over named and what the screenshots show against the critiques' "Across the app"
sections:
- Named by the hand-over and done here: the legacy CSS, the stale tour steps, the `splitBar` inline width, the test
  couplings.
- From the critiques, visible as done: no tour opens on arrival (the ? starts it); one popover menu; no "Preview"
  wording; the tray popup is keyboard-reachable (the sidebar test checks it); bars mean % left everywhere including
  Account detail ("Bars: % left · dashed mark: ≈ level at reset"); the Projects share bar is gone; tag hues sit outside
  the state colours; the Reset Horizon "Next reset & pace" column does not wrap mid-value at 1536 or 1280.
- Still visible: the computers chip's mono face (above). The Overview's Running-now live dot and this computer's dot
  are both teal in the same row (dataviz critique), as in the approved reference.
- Not checkable here: the error and conflict states the clarity critic listed for Skills, Routing and Apps (a failed
  install, a paused provider, two copies of a tool) do not occur on the owner's data today.

## Risks

- The CSS pruning is mechanical and the suite plus the 120 screenshots cover every page and dialog the suite reaches,
  but a dialog or state no test opens and no screenshot shows could have lost a rule. The deleted rules' classes
  appear in no class attribute, classList call or selector string in app/ui, so the exposure is a class built from a
  variable; the only such pattern found (`c${colorIdx}` on computer dots) is styled in the shell block.
- `splitBar` has no caller; its SVG form and the `.split-bar` rules are unexercised.
- The `sessions` read stall above is the one behaviour seen on real data that no test reproduces.
- The three env-gated tests (sync folder, pairing, hub) were not run.
