# Designer C — Chrono-Runway (Time & Reset-First Architecture)

## Angle in one line
**Rate limits are perishable time windows, not static fuel tanks: organize Harness Manager around a live temporal runway toward every account's next 5-hour and weekly reset so the best account to start on now—and the one about to run dry before its reset—are unmistakable in two seconds.**

---

## Hero & Why
### What the eye meets first
The hero of the Overview is **The Temporal Dispatch & Reset Horizon**:
1. **Top-Left Primary Dispatch Card ("Best Bet Now · Reset Runway")**: Answers *"which account we should use because it would reset soon / has the most room"* immediately. It highlights **`service-account-resal`** (Claude Code · Max 5x, **98% 5h left** resetting in **4h 08m**, **100% week left** resetting in **4d 7h**) with an immediate **`▶ Start service-account-resal`** button right beside a **Harvest Before Reset** callout for **`sabushanab-resal`** (**87% left** on a 5-hour window that resets in **2h 19m** — free capacity that evaporates if unused before `18:01`).
2. **Center & Right Companion Cards Above the Fold**:
   - **Exhaustion Forecast (`codex-main`)**: Answers *"what account is exhausted / about to run out"* — **`codex-main`** has **27% week left** (73% used, measured) and is projected to hit `0%` on **`≈ Sun 00:56`**, **`2d 14h` before its `Tue 15:46` reset**, alongside `claude-main` (`≈ Thu 03:38`).
   - **Live Sessions (`12 live · 6 elsewhere`)**: Answers *"what is running now, on which computer, follow one"* right above the fold, showing active project runs across `DEVELOPMENTPC`, `SAMYMSI`, and `Samys-MacBook-Pro` with one-click **`Follow`** actions.
3. **The Reset Horizon Timeline (Centerpiece Above the Fold at 1280px–1920px)**:
   Replaces the half-circle "Horizon Dial" with a precision **dual-scale temporal runway** (`NOW` → `+1h` → `+5h` → `+24h` → `+3d` → `+7d`). Every account occupies a horizontal lane showing:
   - Its **limit ring** (`% left` or `% used`, toggleable) and **official tool icon**.
   - Its **5-hour window bar** and **weekly (plus per-model) window bar** plotted directly against the time axis toward their exact **Reset Gates (`↻`)**.
   - Its **forecasted depletion trajectory (`≈` dashed vector)**: when an account's pace will exhaust its quota before the reset gate (`codex-main` on `Sun 00:56`), the starvation gap between `≈ run-out` and `↻ reset` is shaded in coral/amber hatching so the deficit duration (`2d 14h gap`) is read spatially, not parsed from a paragraph.
   - **Direct `▶ Start` action** on every lane, plus an interactive **Route & Computer Inspector** at the bottom of the timeline showing the live path (`Computer → Account → Tool → Provider/Router`, first-reply latency `0.9s`, and active session count).

### Why this solves the owner's critique
The current app scatters the same question across three places: a prose headline with an explanatory paragraph underneath, a cramped 2×4 grid of limit rings at the bottom of the sidebar with truncated names (`service-acc...`, `samykabu-t...`), and a half-empty semicircular dial whose node labels collide. By turning **time-to-reset** into the horizontal coordinate of the main canvas, the user never has to cross-reference `% left` against `resets Tue 15:46` in their head.

---

## Visual References Chosen
From `references.md` and studio practice:
1. **Linear (`linear.app`)**: Progressive disclosure, disciplined hairline borders (`1px solid`), restrained typography where sans-serif (`Inter`) carries hierarchy and monospace (`JetBrains Mono`) is reserved strictly for aligned timestamps, countdowns, and tabular metrics.
2. **Vercel Dashboard (`vercel.com`)**: High-signal summary cards above a structured telemetry canvas; clear distinction between primary action and secondary inspection.
3. **Mission Control / Aerospace Operations (`open-design.ai` & COSMOS)**: Temporal countdown bars, reset gates (`T-2h 19m`), solid vs. dashed forecast trajectories (`measured` vs. `≈ estimated`), and subtle live pulse beads along active execution lanes.
4. **Owner's Favorites Preserved & Redrawn**:
   - **The 53-week Year Clock**: Kept as a crisp radial heatmap SVG with month markers around the perimeter, Monday inside, busiest day (`Sep 30 · 4.79B`) in the center, and the per-computer switch (`All`, `DEVELOPMENTPC`, `SAMYMSI`, `Samys-MacBook-Pro`).
   - **Dot-Grid Trends**: Kept inside the **Period Readouts Strip** (`Today / 7d / 30d / All` for Tokens, Cost at list prices, and Cached %), moved out of the cramped sidebar into a spacious horizontal telemetry deck.
   - **Limit Circles (Rings)**: Kept for every account—integrated into each account's timeline lane, each Accounts page row, and the **System Tray Popup** preview.

---

## Typography, Fonts & Palette
- **Fonts**: Uses the repository's offline variable `woff2` fonts (`../../../../../app/ui/fonts/Inter-latin.woff2` and `../../../../../app/ui/fonts/JetBrainsMono-latin.woff2`) via relative `@font-face` rules so `index.html` works 100% offline over `file://` with zero external requests.
  - **Inter** (`400–700`) for all UI labels, account names, navigation, and findings (fixing the owner's complaint of *"too many monospace numbers everywhere"*).
  - **JetBrains Mono** (`500–600`, `font-variant-numeric: tabular-nums`) strictly for countdown timers (`04h 08m`), percentages (`98%`), and right-aligned log tables.
- **Palette (Harness Dark & Harness Light)**:
  - **Harness Dark**: Canvas `#091115`, Elevated Panel `#0E1B22`, Card Surface `#12232C`, Hairline `#1E333E`, Ink `#E8F1F4`, Muted `#8CA4AE`, Brand Teal `#3FD6CF` (Harvestable / Healthy Runway), Amber Warning `#E5A93C` (Pace Deficit / Expiring Soon), Coral Negative `#F27263` (Exhaustion Gap / Needs Login).
  - **Harness Light**: Canvas `#F3F7F8`, Elevated Panel `#FFFFFF`, Card Surface `#F8FBFC`, Hairline `#D5E2E6`, Ink `#0D1F26`, Muted `#566F79`, Brand Teal `#0C7A84`, Amber Warning `#B87309`, Coral Negative `#C93B2B`.
  - Both themes exceed WCAG AA contrast ratios across all text and badge pairs.

---

## What Moved or Was Hidden from the First Screen (Inventory Accounting)
Nothing in `inventory.md` is lost. Every single item is present and interactive:

| Inventory Item | Where it was today | Where it lives in Designer C (`c-opencode`) |
|---|---|---|
| **Sidebar Readouts** (`Today/7d/30d/All` · Tokens, Cost, Cached % + dot-grid trends) | Crammed under the brand in the left sidebar | Moved to the **Telemetry & Activity Row** on the Overview (spacious 3-card dot-grid readout strip with range switcher) |
| **Sidebar Limit Rings** (8 rings at the foot of the sidebar) | Cramped 2-column grid at the bottom of the sidebar | Integrated directly into **every account's Reset Horizon lane** (with a `% left / % used` toggle) AND accessible anytime via the **Tray Popup button (`Tray limits`)** in the top bar |
| **Explanatory paragraphs under headings** | 2-line gray explanation below every section title | **Removed** per Rule 6 ("The lazy user reads no explanations"); replaced by self-evident visual legend pills (`Solid = Measured`, `≈ Dashed = Pace Forecast`, `— = Not Read`) |
| **Horizon Dial (Computer → Account → Tool → Provider routes)** | Large half-circle taking 60% of the Overview | Replaced by the **Reset Horizon Timeline** + integrated **Live Route & Latency Bar** at the bottom of the hero card (clicking or hovering any account lane highlights its Computer → Account → Tool → Provider/Router path and first-reply time; `Escape` resets to the primary finding route) |
| **Setup sub-navigation** (`MCP servers 11`, `Skills 255`, `System prompts`, `API keys & routing`, `Apps · 1 update`) | Always-expanded list pushing sidebar items down | Collapsible **Setup** disclosure group in the left rail with the `1 update` badge visible when collapsed; clicking opens the fold and shows the styled Round-1 placeholder view |
| **Pinned Projects in sidebar** | Nested under Projects | Expandable fold on **Projects (`23`)** in the left rail showing pinned projects (`HarnessManager · 4 live`, `sanduq-api · 3 live`, `resal-wiki · 2 live`, `dgx-serve · 1 live`) |
| **Tray Popup** | OS system tray popup | Interactive **`Tray Popup`** button in the top bar opens the exact always-on-top mini limit-rings & reset countdown popover right inside the prototype |
| **First-visit Walkthrough (`?`)** | Top-right `?` button | Interactive **`?` Tour** popover stepping through the 3 core regions of the current page |
| **Latest Replies & Full Request Log** | Bottom of Overview | Compact **Latest Replies** table at the bottom of Overview (`10,756 today · speed and first reply are approximate`, no tokens/s on Overview per Rule 2!) with a **`Request log`** switch that opens the full right-aligned modal/drawer log |

---

## Motion, Depth & Redraw Resilience
- **What moves and why**:
  1. **Active Session Flow Beads**: Accounts with live sessions (`claude-main`, `codex-main`, `service-account-resal`, `opencode-main`) emit subtle, GPU-composited (`transform: translate3d`) telemetry pulses along their timeline track from the active computer dot toward the reset gate. Speed is steady and calm, encoding *"currently executing"* rather than decorative noise.
  2. **Now-Line Temporal Heartbeat**: The vertical `NOW` cursor on the Reset Horizon has a soft breathing halo indicating real-time telemetry freshness.
- **Still Toggle & Reduced Motion**:
  - A prominent **`Motion: Live / Still`** switch in the prototype studio bar (and respected automatically via `prefers-reduced-motion: reduce`) freezes all CSS/SVG animations immediately via `[data-motion="still"]`.
- **Surviving 3-Second Live Redraws (Rule 5)**:
  - All interactive state (`selectedAccountId`, `activePage`, `toolFilter`, `computerFilter`, `windowMode`, `showUsedPct`, `rangeFilter`, `expandedGroups`, `openModal`) lives in a persistent top-level `STATE` store independent of DOM replacement.
  - A **`Simulate 3s Redraw`** button (and optional live tick) updates token counts and timestamps in place via `data-bind` / keyed updates without resetting hover, focus, scroll position, or CSS animation phase.

---

## Items Marked "Proposed" (Rule 4)
We did not invent backend capabilities; everything relies on data the app already collects (`windows`, `used`, `reset`, `out`, `running`, `computers`). One convenience affordance is marked with a small **`PROPOSED`** tag on screen:
1. **`Auto-pick expiring window` option in `Start` (`PROPOSED`)**: When starting a session from the Overview hero, an optional toggle lets `Start` prefer an account whose **5-hour window resets within 2 hours and still has >50% room** (`sabushanab-resal`) before falling back to the account with the highest weekly room (`service-account-resal`). Priced at ~15 lines of JS in `home.js` using existing `windows` data.

---

## Reasonable Assumptions Made
1. **8 Owner Accounts & 3 Computers**: Modeled faithfully after the owner's real screenshots (`home-dark.png`, `accounts-dark.png`) and `brief.md`: `service-account-resal`, `claude-main`, `samykabu-personal`, `sabushanab-resal`, `samykabu-teksa` (Claude Code); `codex-main` (Codex); `opencode-main` (OpenCode, routed); `agy-main` (Antigravity, unread/needs login); plus a demo toggle in the prototype bar to test **8 accounts (Owner's setup)**, **12 accounts (Scale stress-test)**, and **First run (0 accounts / empty state)**.
2. **No `tokens/s` on Overview (Rule 2)**: In the `Latest replies` table on the Overview, the `Speed` column shows qualitative/relative pace or is only shown numerically inside the full **Request Log** modal, keeping the Overview 100% free of tokens-per-second while preserving the column in the full Request Log.
