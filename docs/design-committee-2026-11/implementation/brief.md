# Implementing the approved design in app/ui

## The decision
On 7 November 2026 the owner approved every page of the round-2 prototype ("Good: implement as drawn, with the
critics' fixes") and chose "Approve: implement in app/ui with the critics' fixes folded in".

## The spec, in order of authority
1. `docs/design-committee-2026-11/round2/app/` is the reference design and the spec: index.html (shell), styles.css,
   views.js (Overview, Accounts, Add account flow in modals.js), account.js (Account detail), pages-a.js + pages-a.css
   (Sessions, Session, Projects), pages-b.js + pages-b.css (MCP servers, Skills, System prompts, API keys & routing,
   Apps, CLI reference, Settings), data.js (the mock data: shows which fields each page expects). Open it from disk to
   see any page at 1280×720, 1536×864 and 1920×1080 in both themes. **Owner rule: the reference is the spec; implement
   it exactly, never improvise a layout or a widget.** Where the reference and the app's real data disagree, keep the
   reference's layout and feed it the real field.
2. `round2/app/design-language.md`: tokens, components, class names, rules.
3. The critics' fixes: `round2/critiques/critique-*.md` (six files; each page has "What fails" and "Must change";
   "Across the app" lists the cross-cutting ones). Every item there is folded into the implementation of its page.
4. `round2/inventory-pages.md` and `round1/inventory.md`: every feature the app has today must survive (hidden is
   fine, lost is not). The real renderers in app/ui are the second source for what each page does and which ops
   (`call('…')`) feed it.
5. Owner rules from earlier rounds: the real logo (docs/logo.svg) and the tools' official icons via `toolIcon`;
   the Harness mark in every theme; no tokens-per-second on the Overview; measured / ≈ / — distinguishable; four
   themes (Harness and VektraGrid × light and dark) on `html[data-brand][data-theme]`.

## How the app is built
Tauri (WebView2 on Windows, WKWebView on macOS) serving app/ui as plain ES modules; `core.js` owns the router
(`view(name, render)`, `go`, `parseHash`, `render`), the sheet, dialogs (`openDialog` / `closeDialog`), toasts,
notices, theme (`applyTheme`), folds and pins; `lib.js` has `ic`, `brandMark`, `toolIcon`, `esc`, `startTour`,
`limitRing`; each page module registers its views. Data comes from Rust through `call(op, args)`; the ops and their
payloads are what the current modules already use (read them; do not change the Rust side in this work).
The CSP is `default-src 'self'; style-src 'self' 'unsafe-inline'`: **no inline `on*=` handlers** (event delegation
with `data-act`, as today). Tauri embeds app/ui at build time: **`cargo build` before `node e2e/app.e2e.mjs`**;
`HRM_E2E_ONLY=<part of a test name>` runs a subset. The e2e suite (50 tests) asserts `data-testid` elements and
three design-measurement tests; keep every existing `data-testid` whose element survives, and when a page's
structure changes, update its tests in the same change (the design-measurement tests are rewritten to the new
measurements from the reference). Never delete code by comment anchors without reading the diff: it once removed
exports and broke 45 tests.

## Stages and ownership (no two agents edit the same file)
**Stage 1, foundation + the base (one agent, first):**
- `app/ui/app.css` rewritten from styles.css + pages-a.css + pages-b.css: role tokens for the four themes (VektraGrid
  derived from the existing app.css mapping: plum #7B1E4B, deep teal #0F6863, sand gold #D4B064, ivory #FAF7F2, deep
  green #0B1F1D; EB Garamond display, Inter text), every inline style of the reference turned into a class, the
  reference's frame-based width switch turned into container queries on the sheet (the reference's own breakpoints).
- `app/ui/index.html` as the reference's shell: rail with its nav, counts, Setup fold, pinned projects, the tray
  popup entry, the top bar (search, live chip, computers chip, freshness, tour, refresh), notice and toast roots.
- `app/ui/core.js`: the reference's keyed-region reconciler and state restore (scroll, focus, caret, open dialog,
  selection, animation phase survive the 'refreshed' redraw and keystrokes), one dialog controller with focus trap and
  restore, one popover menu component, one notice channel that never leaks between pages, the tour as the app's
  existing spotlight tour started from `?` only (never auto-opening a modal), `view()` registry kept.
- `app/ui/limits.html` + `popup.js`: the reference's tray popup, keyboard reachable.
- `app/ui/home.js` (Overview), `accounts.js` (Accounts, Account detail, Add account) as the reference draws them, with
  their critics' fixes (Account detail bars mean % left like every other bar; hero and "Next reset & pace" never wrap
  mid-value; unread accounts never replace the exhausted finding or hide running work; the capability matrix from
  `Caps`; Codex has a 5-hour and a weekly window).
- Split `app/ui/views.js` into `sessions.js` (sessions, session), `projects.js`, `routing.js` (routing, usage),
  `prompts.js`, `settings.js`, `cli.js` as a pure move with no behaviour change, and link per-page CSS files from
  index.html: `pages-sessions.css`, `pages-setup.css`, `pages-system.css` (empty placeholders), so stage 2 owns
  separate files.
- Build, then run the e2e tests for the overview, sidebar, accounts, add/sign-in, account detail, Start, and the three
  design tests; fix until they pass; the other tests may fail until stage 2.
**Stage 2, in parallel (three agents):**
- Agent Sessions: `sessions.js`, `projects.js`, `resume.js`, `restore.js`, `pages-sessions.css` ← pages-a.js/.css.
- Agent Setup: `mcp.js`, `skills.js`, `prompts.js`, `pages-setup.css` ← pages-b.js/.css (mcp, skills, prompts).
- Agent System: `routing.js`, `apps.js`, `cli.js` (from guide.js), `settings.js`, `computers.js`, `pages-system.css`
  ← pages-b.js/.css (routing, apps, cli, settings).
  Each folds in its pages' critics' fixes, keeps every feature in the inventory, updates its tests, builds and runs
  its subset of the e2e suite until it passes.
**Stage 3:** one agent runs the whole suite, fixes the remaining failures without undoing the design, and reports.

## Done means
The app on real data looks like the reference in all four themes at 1280/1536/1920, every e2e test passes, nothing
from the inventories is lost, no inline handlers, no inline styles, and the diff is reviewable (one module per page).
Nothing is committed: the owner commits.
