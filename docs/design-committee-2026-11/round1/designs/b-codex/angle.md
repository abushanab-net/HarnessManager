# Capacity ledger — Designer B

**Angle:** Every account's room, on one aligned instrument; the next move is always beside it.

The hero is a capacity ledger: fixed columns for 5-hour and weekly room with each window’s reset beneath it, exhausted first, unknown separate. A novice can compare all eight accounts without decoding a scene. Measured room is solid; estimates carry ≈ and dashed marks; a missing reading is —, never zero. Tool/account/computer routes become explicit text in a live rail and in account detail. The ledger grows to twelve rows without horizontal scroll; at 1280 the eight-account example and all rank-one answers fit a 720px viewport. Twelve rows extend vertically, with live and recommendation rails staying visible.

## Direction and references

Linear's disclosure discipline, Grafana's aligned instruments, and Raycast's keyboard access, as described in the supplied references.md. These are conceptual references, not claims about their current websites; no external assets or UI copied. A bespoke native HTML/CSS system uses midnight green, mineral surfaces, teal room, amber pressure and vermilion exhaustion. Inter is referenced offline from ../../../../../app/ui/fonts/Inter-latin.woff2. Numbers are tabular Inter, not a wall of monospace. The supplied logo and all six official tool SVGs are embedded inline in index.html. No library, CDN, build step or network requirement.

## Five deliberate changes

1. Remove the sidebar's gauges and metrics; elevate every account's paired windows into the Overview ledger.
2. Replace the Horizon scene with literal tool → account → computer routes and a selected-account inspector.
3. Separate measured exhaustion from forecast pressure, with an explicit legend and read timestamps.
4. Place “Use now”, “Resets first” and followable live projects beside the limits, above the fold.
5. Make account creation tool-first: unsupported connection kinds stay visible, disabled and explained; login is a separate, clearly simulated step.

## Inventory destination (capabilities preserved)

| Existing capability | Destination / behavior |
|---|---|
| Brand, current computer, account count | Shell masthead; retained in empty state. |
| Search Ctrl K; list filtering; accounts, sessions, settings | Shell search; filters current ledger/accounts; Ctrl K opens a keyboard-accessible command dialog. |
| Local/elsewhere live count | Top status button → live-session dialog; individual project Follow in live rail. |
| Computer dots and freshness | Top computer button → Computers dialog; three-minute status rule; limits/usage freshness retained. |
| Refresh, errors, confirmations, page tour | Shell refresh; role=status toast, inline form errors; ? opens page tour. |
| Overview cost, Sessions dot grid, Projects count and pinned fold | Compact left navigation; pinned project disclosure includes live counts. |
| Setup update count and auto-open fold | Setup disclosure; MCP 11, Skills 255, prompts, routing, Apps update. These pages are styled out-of-round destinations. |
| CLI, Settings/new-version dot, version | Navigation foot; settings placeholder lists preserved settings. |
| Readouts/ranges/deltas/dot-grid trends | Overview below ledger: Today / 7d / 30d / All, values and deltas, with API list-price cost labelled. Decorative dot grids were dropped under the inventory’s explicit permission; Accounts sparks use the real daily fixture series. |
| Limit rings / elapsed reset bar / used-vs-left setting | Ledger replaces duplicated sidebar rings; selected-account inspector shows a weekly ring and elapsed-week track. Accounts and tray restore one small weekly/daily-cap ring per account; the ledger exposes Room / Used and the Accounts toolbar names the ring convention. |
| Tray and popup | “Tray preview” control opens compact matching capacity popup. Closing real window/tray lifecycle remains implementation behavior, not simulated OS integration. |
| Harness light/dark, VektraGrid | Harness themes implemented; VektraGrid explicitly deferred per round-one brief. |
| Finding and basis | Actual exception and next move are the headings; the selected-account inspector supplies the basis: measured usage, read time, reset, ≈ forecast and its today's-pace basis; exhaustion and pressure summary above ledger. |
| Computers, every account/tool, route selection, live motion, first-reply readout | Ledger plus live rail; selecting a row opens route inspector with computer chips and ≈ first reply. Hover/focus highlights; Escape restores codex finding selection. Pi/Oh My Pi are in filters and Add flow, with no accounts/running routes in fixture. |
| Tool and local/all-computer filters | Ledger/Accounts toolbar. No fabricated remote data or missing account concealed as zero. |
| Start best-room account | Use-now card starts the recent-folder preview in one click; Options… holds folder/account/arguments/permissions. Launch remains clearly simulated. |
| 53-week year clock / device filter / busiest day | Activity disclosure below ledger; full 371-cell radial heatmap with month labels and per-computer selector. |
| Needs attention with reason/time/empty | Overview disclosure; low limits, expired login, paused provider and app update fixtures. Empty state says nothing to review. |
| Latest replies and request log | Overview disclosure shows time/account/model/computer/tokens/≈ first reply; speed is intentionally only in Request log, avoiding any tokens-per-second on Overview. Log contains all columns, numeric columns right-aligned. |
| Repeated redraws | State stays outside data rendering: stable selection, theme, width, filters, disclosure state and dialog form. Data tick increments an actual fixture quota and calls the full renderer. Stable control keys, scroll/disclosure restoration and pause-aware animation delay preserve interaction through redraws; modal forms stay mounted. No connected production polling is fabricated. |
| Accounts finding, sort/read basis | Compact Accounts finding names the exception and next account; grouped by tool, room-sorted within groups; individual folder/login facts in detail. |
| 7/14/30d, export CSV, start in folder, Add | Accounts toolbar. Export creates mock-data CSV locally; ranges recompute fixture daily chart. Start has recent folder, skip-permission control and extra arguments. |
| Stacked tokens/day and by-model panels | Accounts foldable chart and model breakdown; daily fixture totals, distinct stable account hues, a named legend and zero-based daily-token scale, clearly sample data. |
| Tool groups: logged in/consoles/reset/tokens/cost | Group summaries, foldable, with official icons. |
| Rows: labels/email/plan/added/limits/per-model/read/spark/tokens/cost/refresh/start/advice/open | Small weekly/cap rings, data-bound seven-day sparks, full Accounts rows and account detail dialog. Ledger stays purposefully compact. |
| Row menu: folder/rename/profile/sync/show/hide/remove/delete-folder | Account detail menu, with explicit confirmation for mock removal. Main profiles offer Hide only; deletion is disabled for main profiles. No real folders touched. |
| Detail Overview/Sessions/Projects/Settings | Account dialog tabs; overview contains 30d tokens, model windows, models, login/folder, open and recent sessions; editable settings include email/label/key/endpoint/cost cap/removal as appropriate. |
| Default tool profiles | Four named main profiles; non-removable, hideable; shown automatically in detected-tools onboarding variant. |
| Add six tools, installed state, capability matrix | Tool chooser includes six inline icons. Claude supports all kinds; Codex subscription/key; Pi all kinds; OpenCode/Antigravity/Oh My Pi subscription only, verified against harness Caps. Pi/Oh My Pi not installed in fixture: prepare allowed; launch disallowed. |
| Subscription email/name/folder, key/endpoint/model, route/model | Conditional form, name validation and collision protection, optional model except Pi non-subscription (required by existing launch implementation). Route choices match routing fixture. |
| Synced vs own facts and CLI | Add flow disclosure and live CLI preview. |
| Claude browser/code, others terminal login; plan/limits after login | Explicit simulated login step; Claude browser-link/code UI, other tools terminal step; completion adds row. Newly created limits remain — until read; the prototype never invents a measured quota. |
| First-run detected profiles and sync choices | Empty screen offers Add your first account and an Apps destination. Preview detected tools first reviews four tools/main profiles; explicit continuation then offers Not now (selected), shared folder or self-hosted hub, with no folder prefilled. |

## Motion and accessibility

Per-project writing/waiting states and a writing count make activity literal; a quiet travelling mark on each writing computer route denotes activity; it does not encode speed or quota consumption. Ring/rail depth is a one-pixel highlight indicating selection, not fictitious spatial data. No WebGL: the comparison task benefits from alignment, and vanilla CSS is enough. Still toggle pauses routes without reconstructing DOM. System reduced motion is transient and independent of the saved Still preference. Neutral ≈ and dashed marks encode forecasts; amber encodes pressure. The redundant leading-zero counters were removed. Native buttons, inputs, disclosures and modal dialogs provide Tab/Enter/Escape behavior; a small focus loop keeps keyboard focus inside open dialogs. Text, status labels and patterns accompany color. Focus outlines remain visible in both themes.

## Assumptions, proposals and limits

This is a deterministic fixture at Friday 6 Nov 2026, 15:42, not connected telemetry. Data shapes follow the supplied reference but names/computers follow the brief. Production capabilities come from the current harness Caps, which override an older routed-OpenCode reference fixture. The 8-account sample keeps the original four tools. The Data switch also exposes 12 accounts, including a routed Claude daily cap and a pay-per-use Codex account. Daily budget and plan quota are separate quantities; a cap never becomes a fake 5-hour/weekly window. Two of the four tools with fixture accounts expose no quota API; their room/reset stays unavailable, explicitly explained in their inspector. No new backend capability is proposed. “Resets first” is a sort of existing reset timestamps, not a promise that a different weekly constraint will clear. Simulated login/start/refresh/sync are labelled prototype behavior; they never invoke terminals or external services. The screen-width switch frames logical 1280×720, 1536×864 and 1920×1080 app surfaces and scales down to fit; browser outer controls are not part of app space.

## Verification

Re-verified on 7 November 2026 in headless Edge 156.0.4314.58: 1280/1536/1920 × Harness dark/light, with 37 refreshed screenshots (Overview, Accounts, first run, real-720px-height Add, 12 accounts, tray, plus the expanded daily chart). All eight ledger rows, individual window resets, recommendation, reset card and six directly followable projects fit the 1280×720 app frame. The first Accounts row begins around 312px. Critical reset/state/legend text is at least 11px; CSS helper text has a 10px floor.

The retained Playwright suite passes full quota-changing redraw phase/focus/selection/form/disclosure/scroll checks, OS-motion versus saved-Still checks, capped and zero-cap states, actual-series spark values, chart legend/hues, SVG nav, six direct session destinations, one-click Start, 720px Add fit, and the previous capability/validation/onboarding/export/contrast checks. Both page exceptions and console errors are empty. See rebuttal.md, verification.md and verification-results.json. No temporary folder remains.

App-wide E2E would launch the app and write outside the allowed folder, so this task’s explicit constraints exclude it; the folder-local suite validates the changed standalone prototype. No Rust/application source changed and no build, app launch or commit was performed. Native app integration, production polling performance, OS tray/terminal/auth/sync acceptance and Safari/macOS belong to the selected implementation round. No new backend capability is proposed.
