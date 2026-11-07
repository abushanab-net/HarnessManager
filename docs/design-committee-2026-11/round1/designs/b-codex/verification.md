# Verification — Designer B / Capacity ledger

Re-verified locally on 7 November 2026 using headless Microsoft Edge 156.0.4314.58 and the repository’s existing playwright-core. Open `index.html` directly from disk. No build, package installation, server or internet connection is needed to use the prototype. Keep its repository-relative font path intact.

## Screenshots reviewed

| Logical app frame | Harness dark | Harness light |
|---|---|---|
| 1280 × 720 | Overview, Accounts, Add at 720px, empty, 12 accounts, tray | Overview, Accounts, Add at 720px, empty, 12 accounts, tray |
| 1536 × 864 | Overview, Accounts, Add at 720px, empty, 12 accounts, tray | Overview, Accounts, Add at 720px, empty, 12 accounts, tray |
| 1920 × 1080 | Overview, Accounts, Add at 720px, empty, 12 accounts, tray | Overview, Accounts, Add at 720px, empty, 12 accounts, tray |

The 45px prototype toolbar is outside the app frame. `screenshots/` contains 37 refreshed images: the 36 combinations above and an expanded chart capture. At 1280, all eight account rows, their 5-hour/weekly reset times, the recommendation, the earliest reset and all six directly followable project/account rows under three computers were above the app’s fold. No main-area horizontal overflow was detected at any requested frame. Twelve accounts extend the ledger vertically; the live rail stays sticky during scrolling. The default subscription Add form, sync disclosure, CLI line and footer fit a real 720px browser height without body scrolling. Longer or expanded forms retain a fixed header/footer and a scrolling body. At 1280 the first Accounts row begins around 312px.

## Automated assertions passed

- JavaScript syntax: `node --check` on `app.js` and `data.js`.
- All requested frames/themes: eight account rows, live rail inside app bounds, no horizontal content overflow, actual local Inter font face loaded.
- Text tokens: ink, muted, teal, amber and negative on their relevant surfaces at least 4.5:1. This is a token contrast check, not a complete assistive-technology audit.
- Overview ↔ Accounts, tool filters, local-computer filter, search, Ctrl K, Escape, focus selection and keyboard modal focus wrapping.
- Six tool choices and current capability restrictions; Subscription, API key and Routed sample creation; duplicate account rejection; mismatched sign-in email rejection.
- Empty onboarding, first account only, zero live sessions on first run, detected main profiles and the three sync choices.
- Launch preview, follow session, account tabs, main-profile delete protection, CSV download, 371-cell year clock, computer selection retained across a view update, and request log.
- No tokens-per-second text on Overview, including expanded latest replies. Speed remains in the request log dialog.
- Theme and Still remembered after reload; system reduced-motion disables route animation.
- No uncaught page exceptions or console errors (including browser validation diagnostics).


- Findings replace all three slogans; the forecast column names its window; None, —, No forecast and n/a have distinct meanings.
- Reset/state labels are at least 11px at 1280; helper CSS text has a 10px floor; all tool filter labels stay visible.
- Public 12-account switch, routed daily-cap dollars/percentage/reset, zero-dollar cap exhaustion, and pay-per-use without invented subscription windows.
- Per-account rings on Accounts and tray, official icons and both reset times in tray, and a visible Room / Used setting.
- One-click default Start with an asserted confirmation; Options preserves the advanced launch flow; all six project Follow actions resolve to their own sessions.
- Real quota-changing redraws preserve animation phase, selected account, focused controls, modal form values, disclosure state and scroll across repeated rebuilds.
- The OS reduced-motion preference never writes the Still setting and clearing the OS preference restores motion when Still is off.
- Data-bound seven-day spark values equal the account daily series; stacked chart has seven distinct hues, a named legend, zero-based daily scale and value tooltips.
- Detected tools are reviewed before sync; Not now is the initial sync choice; the tour has per-target Next/Skip steps.

Machine-readable results are in `verification-results.json`. Re-run from the repository root:

```powershell
node docs/design-committee-2026-11/round1/designs/b-codex/verify.mjs
```

The retained verification script uses the existing `e2e/node_modules/playwright-core`; it writes screenshots and results here, keeps its Edge profile/downloads in this folder’s `tmp/`, and removes `tmp/` after closing the browser. It does not start Harness Manager.

## Review boundaries

The default fixture retains the brief’s eight named accounts, four active tools and three named computers; the public 12-account state adds a capped routed account, an API-key account and two subscriptions. Unavailable quota readings remain —. No backend feature was proposed. Login, terminal launch, refresh, folder operations and sync are clearly simulated; the CSV export is a real local download containing fixture data. Navigation outside this round is represented by styled destination placeholders with preserved feature descriptions.

The desktop app, Rust build, application E2E, OS tray behavior, real authentication, real peer sync, native WebView2/WKWebView integration and Safari/macOS were not run or verified. The brief forbids running the app or writing outside this folder; application E2E does both. No application source, lockfile, dependency, git index or commit was changed.
