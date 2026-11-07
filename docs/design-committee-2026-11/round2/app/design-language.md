# Chrono-Runway design language · round 2

This is the retained Design C system for an **Operate** desktop app. Repair and extend the dispatch deck, Reset Horizon, capacity column and rail. Do not replace their composition, palette, fonts or voice. `styles.css` is the normative implementation; the examples below use its exact classes. This document and `verification.json` describe the local, offline prototype, not a production integration.

## Colour and depth tokens

Use CSS roles, never copy a raw colour into a page file. Dark and light are selected on the document element. Official SVG artwork keeps its original colours.

| CSS token | Harness dark | Harness light |
|---|---|---|
| `--bg-outer` | `#050a0d` | `#dde6ea` |
| `--bg-canvas` | `#091115` | `#f2f6f8` |
| `--bg-rail` | `#0b151a` | `#e9eff2` |
| `--bg-panel` | `#0e1b22` | `#ffffff` |
| `--bg-card` | `#12232c` | `#f8fbfc` |
| `--bg-card-hover` | `#162b36` | `#eef5f7` |
| `--bg-elevated` | `#172d38` | `#ffffff` |
| `--bg-input` | `#0a1419` | `#f0f5f7` |
| `--bg-glass` | `rgba(14, 27, 34, 0.88)` | `rgba(255, 255, 255, 0.92)` |
| `--line-subtle` | `#182a34` | `#dfe9ed` |
| `--line-default` | `#203642` | `#cddbe1` |
| `--line-strong` | `#2c4a5a` | `#aebec6` |
| `--line-focus` | `#3fd6cf` | `#0b7a82` |
| `--ink-primary` | `#eaf2f5` | `#0d1f26` |
| `--ink-secondary` | `#c0d2d9` | `#223842` |
| `--ink-muted` | `#93acb7` | `#435a64` |
| `--ink-dim` | `#829ca8` | `#526a75` |
| `--teal` | `#3fd6cf` | `#08656c` |
| `--teal-bright` | `#5cebe4` | `#065056` |
| `--teal-dim` | `rgba(63, 214, 207, 0.14)` | `rgba(8, 101, 108, 0.10)` |
| `--teal-border` | `rgba(63, 214, 207, 0.38)` | `rgba(8, 101, 108, 0.36)` |
| `--teal-ink` | `#042222` | `#ffffff` |
| `--emerald` | `#2ce59b` | `#0a6e46` |
| `--emerald-dim` | `rgba(44, 229, 155, 0.14)` | `rgba(10, 110, 70, 0.11)` |
| `--amber` | `#e8ad43` | `#8a4f00` |
| `--amber-dim` | `rgba(232, 173, 67, 0.15)` | `rgba(138, 79, 0, 0.10)` |
| `--amber-border` | `rgba(232, 173, 67, 0.42)` | `rgba(138, 79, 0, 0.35)` |
| `--coral` | `#f27565` | `#b42818` |
| `--coral-dim` | `rgba(242, 117, 101, 0.16)` | `rgba(180, 40, 24, 0.10)` |
| `--coral-border` | `rgba(242, 117, 101, 0.45)` | `rgba(180, 40, 24, 0.35)` |
| `--violet` | `#9e8cfc` | `#4c35b8` |
| `--violet-dim` | `rgba(158, 140, 252, 0.15)` | `rgba(76, 53, 184, 0.10)` |
| `--track-bg` | `#081014` | `#eaf1f4` |
| `--track-grid` | `rgba(140, 164, 174, 0.10)` | `rgba(13, 31, 38, 0.08)` |
| `--starve-stripe` | `repeating-linear-gradient(     -45deg,     rgba(242, 117, 101, 0.24),     rgba(242, 117, 101, 0.24) 5px,     rgba(242, 117, 101, 0.06) 5px,     rgba(242, 117, 101, 0.06) 10px   )` | `repeating-linear-gradient(     -45deg,     rgba(200, 55, 39, 0.18),     rgba(200, 55, 39, 0.18) 5px,     rgba(200, 55, 39, 0.04) 5px,     rgba(200, 55, 39, 0.04) 10px   )` |
| `--shadow-lg` | `0 20px 50px rgba(0, 0, 0, 0.55)` | `0 20px 50px rgba(13, 31, 38, 0.18)` |
| `--shadow-card` | `0 4px 16px rgba(0, 0, 0, 0.25)` | `0 2px 8px rgba(13, 31, 38, 0.05)` |

`--bg-canvas` is the workspace; `--bg-rail` the persistent navigation; `--bg-panel` a section; `--bg-card` a row or nested control; `--bg-input` editable input; `--bg-card-hover` hover. Use primary ink for the finding and values, secondary ink for facts, muted ink for timestamps and labels, dim ink for tertiary metadata. All text remains ≥11 px. Teal means selected/healthy, amber means pressure or a forecast, coral means exhausted/login failure. Emerald is spare capacity near a reset. Violet is reserved for existing computer identity, never another primary action. A colour alone never carries a state.

The stripes in `--starve-stripe` encode the interval between estimated exhaustion and reset; they are not surface decoration. Card elevation is `--shadow-card`, dialog/tray elevation `--shadow-lg`. Do not add glow. Existing pulse signals active sessions; Still and reduced motion disable animations and transitions.

## Type, spacing and shape

Inter variable 400–700 is offline at `../../../../app/ui/fonts/Inter-latin.woff2`. All UI numbers use tabular numerals. JetBrains Mono uses the equivalent local WOFF2 **only for logs, CLI and paths**, via `.mono`. Email, chips, counts, resets and percentages stay Inter.

| Role | Size / weight | Where |
|---|---|---|
| Page finding | 22 / 700 | `.page-heading h1` |
| Accounts finding | 18 / 700 | `.accounts-finding h1` |
| Dispatch finding | 18 / 700; 17 at 1280 | `.lead-headline` |
| Readout value | 20 / 700; 18 at 1280 | `.account-stat > b` |
| Section title | 12.5–13.5 / 600–700 | `.panel-title`, `.horizon-title` |
| Body | 13 / 400–500, line-height 1.4 | default |
| Row and table | 11.5–13 / 400–700 | `.replies-table`, `.acct-row` |
| Secondary label, ring, badge, ruler | 11 minimum / 400–700 | `.muted`, `.plan-pill`, `.axis-tick`, `.ring-val` |

Spacing tokens: `--space-1:4px`, `--space-2:8px`, `--space-3:12px`, `--space-4:16px`, `--space-5:20px`, `--space-6:24px`. Page padding is 12px 16px 20px; sections are separated by 12px; related controls by 4–8px. Dense incumbent rows use 6–10px padding. Do not create a second spacing scale.

Shape tokens: `--radius-sm:4px` (small badges), `--radius-control:6px` (chips and tracks), `--radius-card:10px` (cards), `--radius-dialog:12px` (dialogs). Existing buttons use 7px, small rows 7–8px, tray 11px. Rings/dots are circles; a status pill uses 99px. Borders are 1px of the appropriate line role. Focus is 2px `--line-focus`, 3px offset.

## The frame and fold

The studio controls sit **outside** the modelled app. `.app-frame` is exactly **1280×720**, **1536×864** or **1920×1080**. Its rail is 192px; the remaining workspace contains the topbar and an independently scrolling `main#page-root`. `.rail` scrolls if necessary. Page content must never widen `main`.

Responsive rules are `@container app (max-width:1360px)`, based on the frame, so choosing 1280 in a wider browser produces the same layout. Do not use viewport breakpoints for page content. The dispatch deck remains three columns. Its running list scrolls; the initial rows represent every computer, followed by every other session. The horizon lanes scroll with `main`; the lower readouts, attention, year clock and latest replies remain below them. At the 720 fold the complete deck and the first reset lanes are visible. We do **not** claim all eight or twelve lanes fit without scrolling.

`.page-body` is the standard page root. Start with a finding, then its controls and task data; no greeting, slogan or explanation paragraph. One primary action per fold. Secondary row actions remain neutral; Overview lane Starts appear on hover or keyboard focus. Long names wrap at natural boundaries rather than shrinking text. Tables may use `.table-wrap` when their real data needs horizontal scrolling, but avoid it for ordinary account rows.

## Components (copy the exact class names)

### Page head and actions

```html
<div class="page-body">
  <header class="page-heading">
    <div><h1>Two sessions need attention.</h1><div class="heading-facts">DEVELOPMENTPC · limits read 2m ago</div></div>
    <div class="page-actions"><button class="btn-primary">Start session</button></div>
  </header>
</div>
```

`.btn-primary` is the sole primary; `.btn-secondary` is a normal action, `.icon-btn` a compact action with an accessible name. `.cta-start-lg` belongs only to the Overview lead. `.btn-lane-start` and `.btn-follow` are compact row actions. Buttons carry native `disabled`, never a merely grey class.

```html
<button class="btn-secondary">Export CSV</button>
<button class="icon-btn" aria-label="Refresh limits">↻</button>
<button class="btn-lane-start" data-action="start-account" data-account="codex-main">Start</button>
<button class="btn-follow" data-action="follow-session" data-session="s1">Follow</button>
```

### Cards and the dispatch deck

`.panel-card` is a section, `.panel-head` its title/action line, `.panel-title` its heading. `.accounts-top-bar` is the Accounts finding/actions surface. `.deck-card` is exclusive to the dispatch deck; add `.hero-lead` or `.exhaust-card` for its existing roles. Keep the three jobs and their order.

```html
<section class="panel-card"><div class="panel-head"><h2 class="panel-title">Recent sessions</h2><button class="filter-chip">All 18</button></div><!-- rows --></section>
<section class="dispatch-deck">
  <div class="deck-card hero-lead"><div class="lead-headline">Use service-account-resal</div><button class="cta-start-lg">Start</button></div>
  <div class="deck-card exhaust-card"><div class="exhaust-Finding">codex-main runs out ≈ Sunday.</div></div>
  <div class="deck-card"><h2 class="panel-title">Running now</h2><!-- .live-mini-list --></div>
</section>
```

The lead's existing `.eyebrow-row`/`.eyebrow` names are retained for deck role labels; do not add numbered labels or slogans above other page headings. `.proposed-tag` is a neutral, 11px control/label that includes the word “Proposed”. `.lead-candidates`, `.candidate-pill` and `.active` choose the existing next-account modes, never a new unlabelled algorithm.

```html
<button class="candidate-pill active"><div><b>service-account-resal</b><div class="muted">98% 5h · 100% week left</div></div><span>↻ 4h 08m</span></button>
<span class="proposed-tag">Proposed: prefer expiring 5h</span>
```

### Chips, badges and tabs

`.chip-group` groups controls; `.filter-chip.active` selects a filter; `.plan-pill` labels plan/type; `.status-chip` is shell status; `.mini-chip` is an actionable risk, `.danger` marks failure. Include window names in risk chips when the same account appears twice. `.pc-dot` identifies a computer, with its name in text or a title; it is never a tool icon.

```html
<div class="chip-group" role="group" aria-label="Range"><button class="filter-chip active" aria-pressed="true">7d</button><button class="filter-chip">30d</button></div>
<span class="plan-pill">Max 5x</span>
<button class="mini-chip danger">codex-main · 5-hour · 0% left</button>
<nav class="page-tabs" aria-label="Account tabs"><button class="filter-chip active" aria-current="page" data-action="set-detail-tab" data-tab="overview">Overview</button></nav>
```

### Timeline lanes, capacity, rings and bars

`.horizon-panel` contains `.horizon-header`, `.horizon-filterbar`, `.timeline-grid-head`, `.timeline-lanes` and `.route-inspector-bar`. A `.t-lane.selected` has five columns: `.lane-id`, `.lane-cap-col`, `.lane-runway`, `.lane-readout`, `.lane-action`. Keep capacity independent of reset time.

The ring is **weekly left** (or used when toggled); it is a 30px SVG, `.ring-wrap`/`.ring-val`. `HM_VIEWS.renderRingSvg(account, state.showUsedPct)` is the shared renderer. API keys show ∞ (no plan limit), unread plan capacity shows a dotted — ring. Usage-only tools get an explicit “No plan limits” label beside it.

```js
const ring = window.HM_VIEWS.renderRingSvg(account, state.showUsedPct);
const capacity = window.HM_VIEWS.renderCapacityColumn(account, state.showUsedPct);
const resetTrack = window.HM_VIEWS.renderLinearRunwayTrack(account, state);
```

```html
<div class="t-lane selected" tabindex="0" data-action="select-lane" data-account="codex-main">
  <div class="lane-id"><!-- shared ring --><div class="lane-name-box"><span class="lane-acct-name">codex-main</span><span class="plan-pill">Pro</span></div></div>
  <div class="lane-cap-col"><div class="cap-mini-row"><span class="cap-mini-lbl">5h</span><div class="cap-mini-track"><div class="cap-mini-fill" style="width:52%"></div></div><span class="cap-mini-pct">52%</span></div></div>
  <!-- use the shared track renderer -->
  <div class="lane-readout"><span class="lane-readout-primary">5h ↻ 1h 14m</span><span class="lane-readout-sub warn">≈ Week out Sun 00:56</span></div>
  <div class="lane-action"><button class="btn-lane-start">Start</button></div>
</div>
```

The weekly axis is strictly 0–7d; ticks NOW/+2d/+4d/+6d/+7d have no adjacent word collisions. The short row is explicitly **“5h · 0–5h zoom”**, using `resetMs / (5 hours)`; it is **not on the weekly scale**. `.runway-caption` holds names and countdowns outside the marks; `.runway-track-row` holds the solid `.runway-bar-fill`, `.reset-gate`, estimated `.runway-forecast-dash` and `.runway-starve-gap`. Widths are true fractions; never impose a minimum interval width. The legend says “Time until reset”, not “Measured capacity”. A daily-cap row uses `.cap-runway` with separate caption lines; it has no vendor-window gate. “Harvest” is tagged Proposed.

Generic limit bars use `.win-track`/`.win-fill`. Account detail bars encode **used share**, with `.limit-forecast` for estimated use at reset; their caption states this explicitly. The lane capacity bars encode left or used according to the global toggle. Label the quantity each time.

```html
<div class="account-limit"><div class="account-limit-head"><b>Week</b><span>27% left · resets in 4d</span></div><div class="win-track"><div class="win-fill" style="width:73%"></div><i class="limit-forecast" style="left:calc(100% - 1px)" title="≈ Used up at reset"></i></div><span class="muted">≈ Runs out Sunday at this pace.</span></div>
```

### Lists and tables

`.tool-group-card`/`.tool-group-header` group `.acct-row` accounts. A row's named columns are `.acct-windows-col`, `.spark-bars`, totals and actions. `.live-mini-list` is a labelled, keyboard-focusable scroller; `.live-mini-row` contains `.live-mini-left`, `.live-mini-meta` and `.live-account`. The grid arrangement of this text is scoped to the running list; do not apply it to first-run profile facts.

```html
<div class="live-mini-list" tabindex="0" aria-label="Running sessions; scroll for more"><div class="live-mini-row"><div class="live-mini-left"><i class="pc-dot"></i><!-- official tool SVG --><b>HarnessManager</b><span class="live-mini-meta">DEVELOPMENTPC</span><span class="live-account">claude-main · streaming · 72m</span></div><button class="btn-follow">Follow</button></div></div>
<table class="replies-table"><thead><tr><th>Project</th><th class="num">Tokens</th></tr></thead><tbody><tr><td>HarnessManager</td><td class="num">2.75M</td></tr></tbody></table>
```

`.replies-table` is the general compact table, `.num` aligns numeric cells right. `.mono` is optional only for a log or a path column. `.account-session`/`.session-meta` are larger session rows; `.facts-grid` is a `<dl>` of label/value facts. `.att-item`, `.att-title`, `.att-reason`, `.att-when` belong to Needs attention.

```html
<dl class="facts-grid"><dt>Folder</dt><dd class="mono">D:\Projects\HarnessManager</dd><dt>Login</dt><dd>Valid</dd></dl>
<div class="account-session"><div><b>Review deployment</b><div class="session-meta">resal-wiki · SAMYMSI · live</div></div><button class="btn-follow">Follow</button></div>
<div class="att-item"><div><b class="att-title">Login expired</b><div class="att-reason">agy-main · sign in again</div></div><span class="att-when">Now</span></div>
```

### Readouts, charts and the year clock

`.readout-rows`, `.readout-item`, `.readout-label`, `.readout-val`, `.readout-delta` hold range readouts. Use `HM_VIEWS.renderDotMatrix(levels)` for `.dot-matrix`/`.dot-cell.lv-0` … `.lv-4`. `.stacked-chart-box`, `.stack-col`, `.stack-bars`, `.stack-seg`, `.stack-lbl` are daily stacked columns. `.spark-bars` is the seven-day mini chart. Tooltips name date, quantity and period. Charts must not imply usage in a fresh fixture.

```html
<div class="readout-item"><span class="readout-label">Tokens</span><span class="readout-val">2.75M</span><!-- renderDotMatrix(levels) --><span class="readout-delta">≈ +12%</span></div>
<div class="spark-bars" aria-label="Seven-day tokens"><i style="height:8px"></i><i style="height:18px"></i><i style="height:24px"></i></div>
<div class="account-chart"><div class="stack-col today" title="Today: 2.75M tokens"><div class="stack-bars" style="height:60px"></div><span class="stack-lbl">Today</span></div></div>
```

The retained year clock is a 156×156 SVG, **53 weeks × 7 days**, Monday inside. `.year-clock-wrap`, `.year-clock-svg-box`, `.yc-cell.lv-0` … `.lv-4`, `.year-clock-center` are the exact classes. Month letters are 11px, cells use the retained five-step ramp. Computer values beside it are **53-week totals**, not today's usage. Call the shared renderer; do not draw another radial heatmap.

```html
<div class="year-clock-wrap"><!-- HM_VIEWS.renderYearClockSvg(state.yearClockPc, state.metrics) --><div class="muted">53-week total · daily tokens · Monday inside</div></div>
```

### Dialogs, forms, empty states and notices

The controller owns modal state and focus. `.modal-backdrop` is contained by the modelled frame; `.modal-card` scrolls vertically within `frame height - 40px`. Opening focuses the first control; Tab is trapped; Escape closes and restores the opener. The rail and page are inert while a dialog is open. Keep live dialog inputs keyed with an id. Never change the entire modal DOM on a keystroke.

```html
<div class="modal-backdrop" data-action="close-modal-backdrop"><div class="modal-card" role="dialog" aria-modal="true" aria-label="Add account"><div class="modal-head"><h2>Add account</h2><button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button></div><div class="form-field"><label for="profile-name">Name</label><input id="profile-name" type="text" required /></div><button class="btn-primary">Create account</button></div></div>
```

`.tool-pick-grid`/`.tool-pick-card.active`, `.kind-pick-grid`/`.kind-pick-card.active`, `.sync-facts-grid`, `.form-field`, `.form-grid` and `.settings-row` are form components. A disabled account kind has native `disabled` and text “Unsupported”; the selected kind resets to Subscription when changing to a tool that does not support it. Name and credential validation happen before creation. Never prefill a fake key or auth code. A mock click says Preview; it cannot claim a terminal, file operation, vendor refresh or credential write occurred.

```html
<div class="tool-pick-grid"><button class="tool-pick-card active"><!-- HM.data.LOGOS.codex --><b>Codex</b><span>Subscription · API key</span></button></div>
<div class="kind-pick-grid"><button class="kind-pick-card disabled" disabled><b>Routed</b><span>Unsupported</span></button></div>
<div class="settings-row"><div class="form-field"><label for="profile-label">Name</label><input id="profile-label" /></div><button class="btn-secondary">Save</button></div>
<section class="panel-card empty-state"><h1>No usage yet.</h1><button class="btn-primary">Add account</button></section>
<div class="notice-bar" role="status"><span>Saved profile changes in this preview.</span><button class="filter-chip" data-action="clear-toast">Dismiss</button></div>
```

A toast is the persistent, dismissible `.notice-bar`, never an ephemeral information loss. `.notice-bar.warn` is an error/warning. Empty states show the true zero/unread state and one next action. Use `—` where data has not been read; no guessed 100%, cost history or capacity recommendation. First run uses `.detected-profiles` rows and sync choices; no-tools empty offers preparation for all six tools. Setup counts and updates are zero in both fresh presets.

### Rail and tray

The shell owns `.rail`, `.nav-item.active`, `.nav-item-left`, `.nav-sublist`, `.nav-subitem`, `.nav-badge`, `.rail-bottom`, `.topbar`, `.search-box`, `.status-chip` and `.freshness-label`. Pages do not duplicate a sidebar or topbar. The Harness SVG and official six tool SVGs are `HM.data.LOGOS`; use them unchanged. `.tool-svg` is 14px; `.nav-icon` is a 16px, 1.6px stroke SVG. No substitute letters, emoji, or dots for tools.

```html
<button class="nav-item active" data-action="nav-page" data-page="sessions"><span class="nav-item-left"><!-- stroke SVG --><span>Sessions</span></span><span class="nav-badge">18</span></button>
<aside class="tray-popup" role="dialog" aria-label="Tray limits"><div class="tray-rings-grid"><button class="tray-ring-card" data-action="open-account-detail" data-account="codex-main"><!-- shared ring + official icon --><b>codex-main</b></button></div></aside>
```

The in-prototype tray is 540px, two `minmax(0,1fr)` columns, names wrap, max grid height 352px. It includes all 8/12 profiles, no horizontal scrollbar. Clicking opens account detail. A blank popup says “No accounts configured”. This is a **design preview**, not a claim that the OS tray was resized or verified.

## Rules for truth and interaction

- Measured values have solid marks; all forecasts and approximate costs/latencies carry `≈` or a dashed mark. `—` means unread, zero means measured zero, “No plan limits” means a capability does not exist. No tokens-per-second anywhere on Overview; numeric speed remains in the Request log.
- Claude/Codex subscription profiles can have 5h/week windows; Claude can add per-model weeks. OpenCode, Antigravity, Pi and Oh My Pi have no plan limits to read. API keys/routes have no vendor windows; a route may have a daily spend cap, labelled as money.
- Subscription: all six tools. API key: Claude Code/Codex/Pi. Routed: Claude Code/Pi. All other kind/tool combinations are disabled. Only installed tools can launch; a profile can be prepared for an uninstalled tool.
- Never recommend “most room” without measured eligible capacity; exhausted, login-expired and unread profiles do not qualify. First-run clones clear usage, advice, sessions, models and cap along with their windows.
- The window name accompanies a run-out. Do not merge per-model/week forecasts into an unnamed duplicate account chip. Every count describes the rows behind that count; a complete scrollable list is preferable to an unexplained slice.
- Main profiles can be hidden; their folder/login cannot be deleted. A non-main removal has a confirmation with the optional folder choice. This prototype performs neither disk deletion nor real credential storage.
- All actions are native buttons/links or keyboard-activated labelled elements. Row Enter/Space works; Ctrl/Cmd K focuses search; Escape closes dialog/tray before resetting a route. Modal focus and opener restoration survive redraw. Use `aria-label` for icon-only controls and associate form labels with stable input ids.
- Animations mean active sessions. Keep the animated node alive; never set a fresh animation delay on every render. Still and `prefers-reduced-motion` disable all animation/transition. No particles, pulse or decoration on unread/empty data.
- Escape every external string with `HM.esc(value)` at an HTML boundary. Use `textContent` for keystroke previews. Do not interpolate a label, path, query, endpoint or credential directly into markup.

## Page registry and extension contract

`data.js` initializes `window.HM = { pages: {}, data }`. `views.js` loads shared renderers, `account.js` registers the lead's account page, `pages-a.js` and `pages-b.js` are the extension slots loaded by `index.html`, then `modals.js` starts the shell. Do not edit lead files to add a page: replace your allocated extension slot, registering stable ids. The rail routes by id: `sessions`, `projects`, `mcp`, `skills`, `prompts`, `routing`, `apps`, `cli`, `settings`. `session` is linked from session rows. Unknown ids render the lead's “Page awaiting its round 2 design” placeholder.

The API in five lines:

```js
window.HM.pages[id] = { title, render: state => html, mount };
Object.assign(window.HM.data, { myPageFixtures: [] });
window.HM.navigate(id, { sessionId: 's1', projectId: 'HarnessManager' });
window.HM.redraw(); // patch changed regions, preserving live DOM state
window.HM.esc(value); // escape external text/attributes in render output
```

`render(state)` returns HTML synchronously. Shared state contains `accounts` (active preset + additions − removals), `metrics` (derived truth), `page`, `theme`, `frameWidth`, `motion`, `preset`, `searchQuery`, `sessionId`, `projectId`, `pageTab`, `detailAccountId`, `detailTab`. Page-specific fields belong on this shared state or on a namespaced state object. Never put an input's only copy inside the DOM. Page registration's `title` is descriptive metadata; keep the visible H1 a finding. Built-in placeholder names are `HM.titles`.

`mount(root,state)` is optional, runs once on entry to that page, and can return a cleanup function called on exit. Attach delegated listeners to the stable `root` and keep event decisions based on the current `state`. A data redraw patches children without rerunning mount; do not attach directly to a replaceable row or reset a chart animation in mount. `mount` can create scoped observers and clean them up on exit. No global timers without cleanup.

```js
(function () {
  const H = window.HM;
  // Extend; never replace H.data, edit data.js, or overwrite another designer's fixtures.
  Object.assign(H.data, { projectPage: { rows: [{ id:'hm', name:'HarnessManager' }] } });
  H.pages.projects = {
    title: 'Projects',
    render(state) {
      const selected = state.projects?.selected;
      return `<div class="page-body"><header class="page-heading"><h1>${state.metrics.projectsCount} projects are indexed.</h1></header>
        ${H.data.projectPage.rows.map(p => `<button class="filter-chip ${selected===p.id?'active':''}" data-project-select="${H.esc(p.id)}">${H.esc(p.name)}</button>`).join('')}</div>`;
    },
    mount(root, state) {
      const click = event => {
        const button = event.target.closest('[data-project-select]');
        if (!button) return;
        state.projects ||= {};
        state.projects.selected = button.dataset.projectSelect;
        H.redraw();
      };
      root.addEventListener('click', click);
      return () => root.removeEventListener('click', click);
    }
  };
})();
```

Built-in delegated actions are listed by name in `modals.js`; reuse `data-action="nav-page" data-page="..."`, `open-account-detail`/`data-account`, `follow-session`/`data-session`, `open-start-folder`/`data-account`, `open-add-account`, `start-account`/`data-account`, `set-detail-tab`/`data-tab`, `refresh-single-account`, `toggle-tray`, `clear-toast`. Custom actions should use your own `data-*` selector and a root listener. The controller ignores unknown actions. A nav-page button may supply `data-project`, `data-session` or `data-tab`, which populate `projectId`, `sessionId` and `pageTab`. Follow uses registered `session` when available, with the current preview drawer as fallback.

Keyed patching is per `data-region`: studio, rail, topbar, notice, page, tray, dialog. Controls use ids or `data-action` plus distinguishing data attributes; add stable ids to repeated fields and unique `data-key` values to rows. Keystrokes update state and preview text only. Data redraws keep existing matching elements, input caret/selection, scroll, dialog and animation nodes. A page must not call `innerHTML` on `#app-root`, replace the frame, or reinitialize global state.

## Verification and rebuilding evidence

Open `index.html` directly from disk. No server, package install, frontend build, CDN or network is needed. URL presets: `?w=1280&theme=light&page=accounts`, `preset=8|12|firstrun|empty`, `page=account&account=service-account-resal&tab=overview|sessions|projects|settings`, `modal=add`, `tray=1`, `motion=still`.

1. Check 1280×720, 1536×864 and 1920×1080 in both themes, at actual modelled frame dimensions. Use the width control from a larger viewport too. The studio is outside the exported app screenshot.
2. Check the real data, fresh/empty data, 12 accounts, long names and special characters; all four account tabs; tool/kind compatibility; hide-only main profile. Confirm unknown data never acquires a forecast or usage history.
3. Click every action and complete keyboard-only navigation. Add input typing, search typing, recent-folder selection and selected text must survive `HM.redraw()`. Scroll main/list/tray/dialog, focus a control, redraw; all survive. Check that the pulse DOM node remains identical and animation time progresses.
4. Check Still and reduced motion. Verify 11px text minimum, AA text contrast, no overlapping ruler labels, readable 5h short track, true weekly time positions and no horizontal clipping of main/tray. Look below the fold at the year clock, readouts and latest replies too.
5. Capture the frame, inspect every capture and fix defects before final capture. Require zero `pageerror` and console error events. Export `verification.json`; never label headless Edge checks as macOS WKWebView or live Tauri acceptance.

From the repo root, `node docs/design-committee-2026-11/round2/app/verify.mjs` uses the existing `e2e/node_modules/playwright-core` and installed Edge. `--checks-only` runs behavior checks. Each capture owns a distinct `tmp/edge-*` user-data directory, a 60s total deadline and a closed/killed browser process at completion. The helper writes only under this app directory. Remove `tmp/` after the final run. `verification.json` records the exact captured widths/themes/states and behavior checks. The HTML `review.html` links every screenshot. The production test/build runners start the real app and write outside this allowed directory, so they are deliberately not run for this static prototype task.

`node docs/design-committee-2026-11/round2/app/contrast.mjs` checks computed text colours against composed backgrounds in both themes. Sign-in is an explicit dialog preview; it never marks a vendor login verified. Captures are pinned by source SHA-256 values in `verification.json`.
