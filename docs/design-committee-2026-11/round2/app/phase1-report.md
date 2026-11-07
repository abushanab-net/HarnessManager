# Round 2 · Phase 1 handoff

Status: complete. The selected Chrono-Runway identity was retained. Work stayed inside this app directory; no staging or commit was performed.

## Open defects repaired

| Re-score defect | Result |
|---|---|
| Incorrect tool/kind matrix | Subscription on all six tools; API key on Claude Code, Codex and Pi; Routed on Claude Code and Pi. Unsupported choices are disabled. OpenCode and Oh My Pi fixtures are subscription-only; the routed cap fixture is Pi. |
| Subscription descriptions promised nonexistent windows | Usage-only tools now say no plan limits are read. Only Claude/Codex subscription fixtures carry plan windows. |
| NOW and +1d labels overprinted | Ruler uses NOW / +2d / +4d / +6d / +7d with separate, readable tick positions. |
| Routed/cap gate overprinted its route | Cap captions are separate rows without a vendor-window gate drawn through text. The valid Pi cap fixture appears in the 12-account state. |
| Five-hour resets were under 3% slivers | Each five-hour row has its own clearly labelled 0–5h zoom; the weekly row retains the 0–7d scale. |
| Minimum widths distorted intervals | Reset and exhaustion fractions have no artificial minimum. Forecasts are drawn only when exhaustion precedes reset. |
| Legend called time span “Measured” | It now says “Time until reset”; capacity stays in its own percentage column. |
| Tray clipped its second column | Bounded grid columns, wrapping names, 540px preview width, all 8/12 accounts, no horizontal overflow. Ring clicks inspect the account. |
| Tiny type / crowded lead / brand truncation | All visible text is ≥11px; the lead is shortened; account names stay readable; the rail brand line wraps without losing the count. |
| Add keyboard legend | Removed. Keyboard behavior is preserved. |
| Running chip said four but rendered three | All 18 sessions are retained in the list; SAMYMSI renders all four. The initial visible rows represent each computer and prioritize streaming work. Names, accounts and computer labels remain separate. |
| Duplicate risk account names obscured the window | Every risk chip names its window, including per-model weekly windows. |
| Unknown capacity became “most room” / “ready” | Recommendations require measured, non-exhausted capacity and a valid login. A new unread profile does not acquire a capacity recommendation. |
| First-run clones retained history / forecasts | Limits, usage, models, sparks, open-session counts, cap and advice are cleared. Fresh Accounts has no forecast, chart history or model history. |
| Setup counts leaked into empty state | MCP, Skills, updates, sessions, projects, cost and live status are zero in fresh presets. |
| Harvest silently introduced a product rule | The short-track label explicitly tags “Proposed: harvest”; the deck preference remains Proposed. |
| No 720-tall fold | The width switch sets exact 1280×720, 1536×864 and 1920×1080 frames. The studio sits outside that height; main owns its scroll. Container queries follow the frame even inside a wider browser. |
| Whole-app innerHTML redraw | Template output is reconciled into keyed regions: studio, rail, topbar, notice, page, tray and dialog. Existing matching elements survive; fields patch previews on keystrokes. |
| State restore incomplete | Focus, caret/input selection, text selection, open dialog, main/list/dialog scroll, selected account, recent-folder choice and live animation object/phase are checked across redraw. Selectors distinguish expiring choices and message actions. |
| Add accepted blank / duplicate profiles and fake credentials | Names, duplicates, email, key presence, endpoint scheme, route choice and pasted-code presence are validated. No fake key/code is prefilled. New profiles stay unread in the current preset. |
| Account settings were readonly/incomplete | Registered account page has Overview, Sessions, Projects and Settings; label/email, API key replacement preview, endpoint/model and route cap controls work. Keys are cleared after the preview. Main-profile hide has no folder deletion choice; non-main removal has confirmation. |
| Mock actions claimed real success | Launch, sign-in, refresh, folder/sync/window and credential interactions explicitly say Preview. The reauthentication dialog selects the actual tool and never claims vendor verification. CSV is a real local download with quoted editable labels. |
| Copied assets failed to load | Relative local font paths were repaired for the round2/app depth. Zero console errors include missing-resource checks. |

## Registry and system handoff

`account.js` registers the lead's account page through `window.HM.pages.account`. `pages-a.js` and `pages-b.js` are deliberately empty, loaded extension slots; other designers need not edit the shell. Every rail destination routes by page id; unregistered destinations use the shared placeholder. Existing pinned-project folding remains available separately.

The five-line API:

```js
window.HM.pages[id] = { title, render: state => html, mount };
Object.assign(window.HM.data, { pageFixtures: [] });
window.HM.navigate(id, { sessionId: 's1', projectId: 'HarnessManager' });
window.HM.redraw();
window.HM.esc(value);
```

`mount(root,state)` is optional, runs once per page entry and may return cleanup. Use root event delegation, stable control ids/data-key values, and state-owned drafts. `design-language.md` supplies both-theme colour roles, exact type/space/radius values, all shared component classes and markup, truth/voice/motion/keyboard rules, extension examples and verification steps.

## Verification

- `node docs/design-committee-2026-11/round2/app/verify.mjs`: 34 behavior checks and 84 frame screenshots. Every capture owns a distinct Edge user-data directory below `tmp/`, has a 60-second deadline, and closes its owned Edge process afterward.
- Captured **1280×720, 1536×864, 1920×1080**, each in **dark and light**: Overview, lower Overview, Accounts, Account detail, all three additional account tabs, Add account, first run, first-run Accounts, empty, 12 accounts, tray with 8 and 12 accounts.
- Every image was reviewed in the linked contact sheets; critical 1280 images were inspected at full size. Found and repaired the first-run row layout, copied font paths, clipped tray, account-stat fold and fractional studio export bounds before final export.
- Zero page/console errors, no main/tray horizontal overflow, 11px minimum visible text, exact frame dimensions. `verification.json` records the checks, capture matrix and source SHA-256 values.
- `node docs/design-committee-2026-11/round2/app/contrast.mjs`: no computed text contrast failures in either theme on Overview (including content below its fold). Gradient artwork/SVG icon colour is not represented by this DOM text check.
- JS syntax checked with `node --check`. `python docs/design-committee-2026-11/round2/app/review.py` asserts all 84 PNG dimensions and rebuilds the contact sheets/review page.
- Impeccable's detector ran once. Inter, the live-session pulse and semantic exhaustion hatching were retained as the pinned Design C language; the incidental update-dot glow was removed.

## What remains and assumptions

No phase-1 work remains. Designers A/B still own the other registered-page implementations; their slots currently show placeholders. The default account detail fixture is service-account-resal; additional settings forms are exercised by the behavior checks. The mock data is an offline fixture anchored to its supplied reference NOW, not live account telemetry.

The 5-hour zoom intentionally has a different labelled scale from the week. At 720px, the complete deck and first reset lanes fit; remaining lanes and lower telemetry require scroll. The tray is a 540px design preview, not a claim that a native OS popup was changed.

Production build/test runners were not run: the supplied round-1 rule forbids running the real app, and those runners write outside the delegated app directory. Native Tauri/WebView2, macOS/WKWebView, physical display scaling, real terminal/vendor/credential/disk operations and production acceptance remain unverified. No backend, package dependency or production UI file was changed. Temporary Edge profiles and scratch repair scripts were removed after verification.
