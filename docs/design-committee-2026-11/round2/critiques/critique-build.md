# Build: critique, round 2

Method: the whole-app prototype (`round2/app/index.html`) in headless Edge over CDP on port 9454, every page opened through
`window.HM.navigate`, 1280×720 dark and light and 1920×1080 dark, Live and Still. Per page: DOM nodes, running animations,
main-thread cost over 4 s idle at 150 % DPR (`Performance.getMetrics`), `HM.redraw` calls per 4 s, a keystroke in the first page
input (or the global search), and the prototype's own "Simulate 3s Redraw" with scroll, focus, an open dialog and the running
animation's computed opacity checked before and two frames after. Something on this machine kills every headless Edge every
10–25 s (two of mine on different ports died in the same instant; another critic's survived), so each measurement ran as a short
segment on a fresh browser; 89 launches, no measurement lost. Zero console errors on any page.

| 1280×720 dark, 8 accounts | nodes in page | animations Live / Still | idle 4 s at 150 %: ms, redraws | 10 redraws at 150 % | keystroke kept | redraw: scroll / focus / dialog / animation | inline `style=` |
|---|---|---|---|---|---|---|---|
| Overview | 1,385 | 7 pulse / 0 | 4 ms, 0 | 168 ms | yes | kept / kept / – / 0.66 → 0.62 | 183 |
| Accounts | 569 | 0 | 2 ms, 0 | 71 ms | yes | kept / kept / – / – | 306 |
| Account detail | 235 | 0 | 2 ms, 0 | 43 ms | yes | kept / **moved to the rail item** / – / – | 39 |
| Sessions (A) | 1,590 | 18 pulse / 0 | 26 ms, 1 (3 s tick) | 133 ms | yes | kept / into its dialog / New session kept / 0.42 → 0.40 | 100 |
| Session (A) | 593 | 1 pulse / 0 | 31 ms, 2 (2 s tick) | 89 ms | yes | chat scroller / kept / – / 0.43 → 0.46 | 1 |
| Projects (A) | 475 | 0 | 2 ms, 0 | 85 ms | yes | kept / kept / dialog + typed draft + focus kept / – | 29 |
| MCP servers (B) | 390 | 0 | 2 ms, 0 | 60 ms | yes (after the tour) | kept / kept / – / – | 0 |
| Skills (B) | 746 (25 of 255 paged) | 0 | 2 ms, 0 | 117 ms | yes; filter "asc" → 23 rows | kept / kept / – / – | 0 |
| System prompts (B) | 72 | 0 | 1 ms, 0 | 32 ms | yes | – / kept / – / – | 0 |
| API keys & routing (B) | 45 | 0 | 2 ms, 0 | 29 ms | yes | kept / kept / – / – | 0 |
| Apps (B) | 213 | 0 | 1 ms, 0 | 40 ms | yes | kept / kept / – / – | 0 |
| CLI reference (B) | 170 | 0 | 2 ms, 0 | 33 ms | – (no input) | kept / no control to focus / – / – | 0 |
| Settings (B) | 531 | 0 | 2 ms, 0 | – | yes; "zz" in Registry path survived a redraw with focus | kept / kept / – / – | 3 |

Still: 0 running animations on every page and 0 tick redraws (A's intervals check `state.motion`). Idle in Still: 1–2 ms per 4 s on
every page. No page scrolls horizontally at 1280 or 1920.

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) · score 8/10

**Still open from round 1**
- Inline styles: 183 `style=""` attributes on the Overview, 306 on Accounts, 39 on Account detail (B's seven pages have none). Still
  the main obstacle to VektraGrid.
- Tokens are still named by hue (`--teal`, `--amber`, `--coral`, `--emerald`, `--violet`) where `app.css` names roles
  (`--ac`, `--warn`, `--neg`, `--r2`); a second brand is a rename of every use, not a new block.
- The Overview's redraw is the heaviest in the app (168 ms for ten at 150 %, 17 ms each), because the whole app string is rebuilt
  and diffed on every change; acceptable, but it is the budget every page inherits.

**What works**
- The render model is now sound and measured. `renderApp()` builds one HTML string and `patchNode()` reconciles it into seven
  `data-region` roots, keying children by `id`, `data-region` or an attribute selector; existing nodes survive, so `main#page-root`
  keeps its scroll (150 → 150 on Overview, Accounts and Apps when measured without my own focus call), the focused control keeps
  focus and caret, Projects' open dialog keeps a typed draft and focus, and the pulse's opacity changes ≤ 0.04 across the redraw.
- Keystrokes patch, not re-render: `updateField` writes two preview nodes; typing in the global search on every page and in Settings'
  text field kept focus and value, and a redraw while the field held "zz" kept both.
- Idle cost is nil on the base pages (2–4 ms per 4 s at 150 %) and Still stops everything.
- The page registry is five lines (`HM.pages[id] = { title, render(state), mount(root, state) }`, `HM.navigate`, `HM.redraw`,
  `HM.esc`, `HM.data`); `mount` runs once per entry and its cleanup on leaving (A's timers are cleared when the page changes:
  0 redraws on any other page). Routing by id works for all eleven registered pages; `account.js` uses the same registry.
- The 720-tall frame is modelled: `main#page-root` is the scroller (`scrollHeight` 1,831 vs `clientHeight` 637 on Overview),
  `@container app (max-width: 1360px)` follows the frame, and the `--space-*`/`--radius-*` tokens the design language documents
  are defined (`styles.css` line 2116; computed `--space-2: 8px`, `--radius-control: 6px`).
- The data the base reads is the app's: `detail`/`account_stats` (p95, p50, replies, top project share), `limits_extra`,
  `usage` by model, caps; the Account page's "Started with CLAUDE_CONFIG_DIR" and the Add matrix follow `caps`.

**What fails**
- Focus restore is keyed by an attribute selector (`tag[data-action=…][data-page=…]`), not an id. On Account detail (1280 dark)
  the focused "← Accounts" chip (`data-action="nav-page" data-page="accounts"`) came back as focus on the rail's Accounts item,
  the first element matching the same selector. Any two controls with the same data attributes collide; the fix is ids on
  focusable controls or a `data-key`, which B's rows already carry and A's partly.
- Thirteen inline `oninput="window.HM_APP.updateField(…)"` handlers in `modals.js` and one in `account.js` (A has seven more). The
  app's Tauri CSP is `default-src 'self'` with no `script-src 'unsafe-inline'`, so inline handler attributes do not run inside
  WebView2; today's `app/ui` has one. Every such handler becomes delegated `input` listening in the port.
- `document.querySelector('.workspace').inert = !!STATE.modal` makes the whole workspace inert while a lead dialog is open: right
  for the lead's modals, but B's native `<dialog>`s live outside that rule and A's dialogs inside the page, so three dialog systems
  coexist (see Across the app).
- The tray popup is a 540 px preview inside the frame; `limits.html`/`popup.js` is a separate window and gets no shared component
  from this prototype beyond `renderRingSvg`.

**Must change**
- Key every focusable control with an id or `data-key` and restore by that; drop attribute-selector keys.
- Replace inline event attributes with delegated listeners before the port (CSP).
- Move the 500-odd inline styles on Overview/Accounts into classes and rename the colour tokens to roles.

## Sessions · 8/10 (Designer A)

**What works**
- Faithful to the inventory's row: `aSessions[0]` has exactly the 24 fields of the `sessions` row (`project_key`, `device_name`,
  `open_elsewhere`, `resumable` true-or-reason, `continued_from`, `source_path`, `tool_calls`, `failed`…); groups are computed
  from `project_key`, pages are 50 (`PER = 50`), the left rail's pinned/recent/tags are derived.
- Redraw-safe: rows carry `data-key`, scroll kept (150 → 150), the 18 pulses continued across the redraw, the New session dialog
  stayed open. The 3 s tick returns early when Still, a dialog, a menu or a rename is open.
- Empty state is computed ("No sessions yet. Sessions appear here as your tools write their logs." + Add account).
- `pages-a.css` is fully `a-` prefixed; no class collides with `styles.css` or `pages-b.css`.

**What fails**
- 1,590 page nodes with 18 live pulses: 26 ms per 4 s and one full-app reconcile every 3 s. In the app this tick must become the
  `refreshed` event, which already fires only while sessions write; keep the `mount`/cleanup shape, drop the interval.
- 100 inline styles in `pages-a.js` (the lead's language is classes).
- Seven inline `onchange="window.HM.aInput(…)"` handlers (CSP, as above); `H.aInput`/`H.aToggleTag` are extra globals on `HM`.
- `.l1`–`.l5` in `pages-a.css` are unprefixed with `[data-theme=light]` overrides: harmless today, a collision waiting for the next
  page that uses `.l1`.

**Must change**
- Replace the interval with the app's `refreshed` event and the inline handlers with delegation; prefix `.l1`–`.l5`.

**Inventory misses** (build view): none of the data lines is missing; the 50-row paging, grouping modes, tags, computer filter and
"Every project, with costs" link are present.

## Session · 8/10 (Designer A)

**What works**
- The chat is its own scroller (`main` does not scroll: 672/672), follow-at-end is kept in `A(state).session.follow` and re-applied
  after each tick; the 2 s tick appends a step to the transcript model and redraws (31 ms per 4 s, 2 redraws, continuous pulse).
- Transcript shape matches `transcript`: turns with `prompt`, `prompt_at`, `answer[]`, `answer_pos[]`, steps with `kind`, `tool`,
  `input`, `status`, `duration_ms`, `result`, `change {path, added, removed}`; stats match `session_stats` (`hit`, `p50`); export as
  Markdown is a real download; Resume uses `settings.terminal`.
- One inline style on the page; 593 nodes.

**What fails**
- A whole-app reconcile every 2 s while a session writes is the costliest steady state in the app (31 ms per 4 s at 150 %, ≈ 0.8 %
  of a core); today's `views.js` polls `file_stamp` and appends to the chat instead. Port the append, not the redraw.
- The "Resume in Windows Terminal ▾" split button offers a per-launch terminal choice the inventory does not list (the app has one
  `terminal` setting); untagged.
- Same inline-handler and `HM` global notes as Sessions.

**Must change**
- Keep the transcript model but drive it from `file_stamp` and append in place; tag or drop the terminal picker.

**Inventory misses**: "thousands of steps shows its first and last until asked" is drawn as a folding row; "resume here" for a
remote session is a dialog (`resume-here`). Nothing structural missing.

## Projects · 8/10 (Designer A)

**What works**
- The only page where a dialog with a typed draft, its focus and the page scroll all survived the redraw (`a-new-folder` kept its
  value and focus).
- Derived tiles (tokens, cost, projects, running, busiest) and the share table come from `aSessions`; `series[]`, `devices[]`,
  `here`, `pinned`, `tags[]` match `projects`; pin, rename, merge and tags are dialogs that mutate the fixture.
- A `MutationObserver` flashes a tile that changed; it watches only `#page-root` and is disconnected on leave.

**What fails**
- 29 inline styles; the rename input is an inline `onchange`.
- The project-open-in-Sessions handoff is a `setTimeout(0)` in `mount` that navigates away when `state.projectId` is set: a hidden
  routing rule in a page file.

**Must change**
- Make the Projects → Sessions handoff a `navigate('sessions', { project })` call at the click, not a mount side effect.

**Inventory misses**: none seen (cost per project, pin, rename, tabs, open in Sessions).

## MCP servers · 8/10 (Designer B)

**What works**
- Data exactly as `items.mcp` (`name, transport, command, args, env, url, headers, description, homepage`), `targets`, `sync_last`,
  `mcp_last_import`; the matrix is a table with a bulk column per tool, "left alone" entries counted; empty state computed
  ("No MCP servers yet. 0 entries written · 6 tools · not synced yet").
- Zero inline styles; every class `b-` prefixed; redraw kept scroll and focus; typing works once the first-visit tour is dismissed.
- Dialogs are native `<dialog>` with `showModal()`, a draft object, their own `redrawDialog()` that restores focus/selection/scroll,
  and an `Escape`/backdrop close.

**What fails**
- The page's dialogs, tour, "Ready / Empty / Loading / Error" studio and notices live outside the registry: `pages-b.js` appends
  `#b-studio` before `#app-root` (the frame top moves from 64 px to 100 px on B's pages), appends `<dialog>` to `document.body`,
  and registers five document-level capture listeners (`click`, `change`, `input`, `toggle`, `keydown`) that
  `stopImmediatePropagation` ahead of the lead's handlers. That is a second app bolted beside the first, not a page in it.
- `syncStudio` runs a `MutationObserver` on all of `#app-root` (childList, subtree, attributes) and on every mutation rewrites the
  lead's rail counts and hides tool chips and groups on the lead's Accounts page by editing their DOM. It fires on every patch,
  including A's 2 s and 3 s ticks. It will not port and it is why the rail's "MCP servers 11" and "Skills 255" come from B's fixture,
  not the lead's state.
- `pages-b.css` uses `:has()` (lines 31–33, 103) and `text-wrap: balance`: fine in WebView2; `:has()` needs Safari 15.4+ and
  `text-wrap` falls back harmlessly. `||=` needs Safari 14+. Nothing blocking, but the only newer syntax in the app comes from here.

**Must change**
- Register dialogs, notices and tour through one shell API (the lead's `data-region="dialog"` or a shared native-dialog helper)
  and delete the observer: counts and hidden tools are lead state.

**Inventory misses**: edit-as-JSON mode is not visible in the add/edit dialog (the inventory and e2e `mc-json` expect it).

## Skills · 8/10 (Designer B)

**What works**
- 255 skills paged at 25 (746 nodes, 117 ms per ten redraws), source filter, "Not managed · 3" with Manage, update badges from
  `skill_check`, Installed/Discover tabs, `skill_repos` and `discover` shapes as the inventory; the search filters rows (23 after
  "asc").
- Empty state computed; Still/Live irrelevant (no animation); zero inline styles.

**What fails**
- Same shell-level hacks as MCP (observer, capture listeners, body dialogs); the first-visit tour opens a modal on entry, so the
  global search is inert until it is dismissed (that is the app's behaviour too, but the tour is B's own `localStorage` keys
  `hm.b.tour.*`, not the app's `hm.tour.*`).

**Must change**
- As MCP. Share the tour with `guide.js`'s keys.

**Inventory misses**: Discover's per-repository errors are in the fixture but I did not see an error row rendered.

## System prompts · 8/10 (Designer B)

**What works**
- `items.prompts` and `prompt_live` as specified; per-tool file path and live body; versions list; drafts kept in `promptDrafts`
  so a redraw does not lose an edit.

**What fails**
- 72 nodes and 32 ms per ten redraws: the lightest page, nothing to fault on cost; the shell hacks apply.

**Must change**: as MCP.

**Inventory misses**: none seen.

## API keys & routing · 8/10 (Designer B)

**What works**
- `routing` shape complete (`stats.providers.p95_first`, `keys`, `breakers`, `routed_profiles`, `log`); a populated fixture
  (`bRoutingPopulated`) exists beside the stopped default; providers/routes dialogs; Start router.

**What fails**
- The default state is the stopped, empty router, so the page's real density (routes, log table, paused providers) is only seen
  after switching fixtures; 45 nodes measured.

**Must change**: as MCP; make the populated fixture the default for review.

**Inventory misses**: none seen.

## Apps · 8/10 (Designer B)

**What works**
- `apps` shape complete (`copies`, `install`, `update`, `download`, `auto_updates`, `latest.versions`), second-copy banner with
  Remove, Show checkboxes writing `hidden_tools`, self-update.
- Scroll and focus kept across the redraw; empty state computed ("No tools installed yet. 0 installed · 6 not installed").

**What fails**
- "Installed by: native · updates itself / npm" is derived from `copies[].kind` and `auto_updates`; fine, but the wording is new.

**Must change**: as MCP.

**Inventory misses**: none seen.

## CLI reference · 9/10 (Designer B)

Static groups of commands, 170 nodes, no state, no handlers to port; the only page with nothing to focus (my redraw check found no
control, which is correct). Implement as drawn.

## Settings · 7/10 (Designer B)

**What works**
- Every `state.settings` key of the inventory is in the fixture (28 keys), plus `computers_status` with `hub`, peers, `transcripts_status`,
  `backups`, `archive_status`, `restore_preview`; nine `<details>` sections remembered in `B().folds`; the Registry path field kept
  "zz" and focus across a redraw; Computers, Backups, Restore, pair and hub dialogs exist (`H.designerB.openHub/openPair/openRestore`).

**What fails**
- 3,729 px tall at 1280 (5.5 screens) with all sections open by default; the section nav is the only way across it.
- The "?" tour and the top bar's Computers chip are hijacked by a capture `click` listener that `stopImmediatePropagation`s the
  lead's `open-modal` action when the page is Settings or a tour exists: the shell's own controls behave differently per page.
- `#b-studio` state switches (Ready/Empty/Loading/Error) are B's private studio, not the lead's preset; the two studios disagree
  (the lead's "Empty (0)" preset and B's "Empty" view are different states).
- Largest e2e surface in the app (computers, pairing, hub, backups, restore, updates, terminal: about twelve tests), all selector
  rewrites because every control is `data-b`/`data-v` instead of `data-testid`.

**Must change**
- Fold all but Appearance by default or make the section nav sticky; remove the capture-listener hijacks; one studio.

**Inventory misses**: the direct-connection pairing tabs (show a code / enter a code) and the hub "invite" flow are present as
dialogs; the ring-key dialog I did not open. The "Welcome back" restore path is a dialog, not a first-run page.

## Across the app

- **Is it one app?** Not yet in code; nearly in look. The three hands show in: (1) **dialogs**: the lead renders modals into
  `data-region="dialog"` and makes the workspace `inert`; A renders dialogs inside the page from `A(state).dialog`; B appends native
  `<dialog>`s to `body` with its own draft/focus/scroll restore. (2) **events**: the lead uses `data-action` delegation plus 13 inline
  `on*=` attributes; A uses `data-a` delegation plus 7 inline `window.HM.aInput` handlers; B uses `data-b`/`data-v` with five
  document-level capture listeners. (3) **state**: the lead's flat `STATE` (45 keys) with `state.a` and `state.b` sub-objects, and
  B's deep-copied fixture models per view. (4) **styling**: the lead 183–306 inline styles per page, A 1–100, B 0; classes
  unprefixed / `a-` / `b-`; one shared class (`.plan-pill`, which B extends with `.b-*` tones, benign); A's unprefixed `.l1`–`.l5`.
  (5) **timers**: A's `setInterval`s where the app has the `refreshed` event. (6) **globals**: `window.HM`, `HM_DATA`, `HM_VIEWS`,
  `HM_APP`, plus `HM.aInput`, `HM.aToggleTag`, `HM.editAccount`, `HM.saveAccount`, `HM.designerB`. To unify: one dialog helper on
  the registry, one `data-act` delegation (the app's), ids on focusable controls, classes instead of inline styles, tokens by role,
  and the observer gone.
- **The three worst defects**, ranked: (1) `pages-b.js`'s `MutationObserver` and capture listeners editing the lead's rail, Accounts
  page and top-bar actions from outside the registry, so B's pages cannot be lifted into `app/ui` without rewriting their shell
  glue; (2) inline event-handler attributes (21 across the lead and A) that the app's CSP (`default-src 'self'`, no
  `script-src 'unsafe-inline'`) blocks in WebView2; (3) focus restore keyed by attribute selectors, which on Account detail moved
  focus from the page's back link to the rail item, and the 500-odd inline styles on the lead's two main pages that block the
  VektraGrid themes.
- **Ranking, best to worst**: CLI > Projects > Sessions > Session > Apps > Prompts > MCP > Skills > Routing > Overview > Accounts >
  Account detail > Settings — one line: A's pages port almost as written, B's need their shell glue removed, the lead's own pages
  carry the inline styles and the CSP risk, and Settings is the biggest surface with the most hijacks.

## WebView2 / WKWebView

No blocking feature. `inert` (Safari 15.5+), `<dialog>.showModal` (15.4+), `:has()` in `pages-b.css` (15.4+), `text-wrap: balance`
(fallback), `||=`/`??=` (14+), `queueMicrotask`, `CSS.escape`, container queries (16+; the app already requires them). The one
real risk is the CSP above: inline `on*=` attributes silently never run in the app window.

## E2E impact per page

Selectors in the prototype are `data-action`/`data-a`/`data-b`, never `data-testid`; every page test needs selector edits. Beyond that:

| Page | Tests touched | Rewritten (structure) | Selector edits |
|---|---|---|---|
| Shell + Overview + Accounts + Account + Add | overview (198), window-follows-design (649), measurements (703), sections (737), accounts list, add/sign-in ×4, detail tabs, start in folder | 4 (the dial, rings and the 256 px sidebar assertions die; rail 192 px, new type scale) | 8 |
| Sessions | grouped by repository (411), tags (567), OpenCode/Antigravity (601), index and watcher (1831) | 0 | 4 |
| Session | reading a session (502), live messages (529), thousands of steps (619), export (634), resume here (1556) | 2 (the `chat` markup: steps, answers, folding) | 3 |
| Projects | projects (447), computers on Projects (1314 part) | 0 | 2 |
| MCP servers | 823 | 1 (form/JSON modes, import report) | – |
| Skills | 894, 910, 950 | 0 | 3 |
| System prompts | 996 | 0 | 1 |
| Routing | 789 | 0 | 1 |
| Apps | 981, 1130 | 0 | 2 |
| CLI | – | 0 | 0 |
| Settings | 1009, 1027, 1047, 1148, 1173, 1253, 1268, 1309, 1314, 1443, 1483, 1617, 1655, 1735 | 1 (the walkthrough test vs B's tour) | 13 |

## Cost table

Assumptions: one engineer who knows `app/ui`; each page becomes a `view()` in its own module as today; the lead's `patchNode`
reconciler replaces `sheet.innerHTML` once, in `core.js`, and every page renders a string (so A's and B's pages port their
templates, not their event glue); A's timers become the `refreshed` event; B's dialogs move to the app's dialog root; backend
untouched (every field used is in `inventory-pages.md`'s Data lines, with the three untagged items above); four themes by renaming
tokens to roles; e2e updated to pass; QA at 1920/100 %, 1536/125 %, 1280/150 % and macOS WebKit.

| Page | Engineer-days | What the days buy | Biggest risk |
|---|---|---|---|
| Shell: reconciler, registry, state restore, dialogs, CSP cleanup | 3 | `patchNode` into `core.js`'s render chain, ids/`data-key` on controls, one dialog helper, delegated inputs, `warmPage` cache retired | Focus-restore collisions; the `refreshed` event driving a full reconcile |
| Overview | 2.5 | deck, horizon with 5 h zoom, lower panels, inline styles to classes | Label placement on the 7-day track at 1280 |
| Accounts | 1 | groups, rows, chart, 306 inline styles to classes | – |
| Account detail | 1 | five stat tiles from `account_stats`, limits, facts, four tabs, settings form | – |
| Add account + tray | 1.25 | wizard with the `caps` matrix, sign-in step; ring list in `limits.html` | Tray is a separate window |
| Sessions | 1.5 | rail, groups, paging, tags, filters; tick → event | 1,590 nodes with 18 pulses on each redraw |
| Session | 2 | chat with steps/answers/folding, follow, append from `file_stamp`, export, resume dialogs | Replacing the 2 s full redraw with an append |
| Projects | 1.25 | tiles, share table, cards, pin/rename/merge/tags dialogs | – |
| MCP servers | 1.25 | matrix with bulk, add/edit form + JSON, import report, info | Shell glue removal |
| Skills | 1.5 | installed matrix, pager, sources, not-managed, Discover, updates | – |
| System prompts | 0.75 | per-tool file, versions, drafts | – |
| API keys & routing | 1 | providers/routes dialogs, start/stop, log, breakers | – |
| Apps | 0.75 | table, copies, show/hide, self-update | – |
| CLI reference | 0.25 | static | – |
| Settings | 2 | nine sections, computers (folder/hub/direct), backups, restore, updates, terminal | Hijacked shell controls; height |
| Themes and tokens | 1 | roles instead of hues, VektraGrid ×2 | – |
| E2E | 3.5 | 8 structural rewrites, ≈ 37 selector passes | – |
| QA | 1.5 | three widths, two themes, WebKit | – |
| **Total** | **27 (25 to 30)** | | |
