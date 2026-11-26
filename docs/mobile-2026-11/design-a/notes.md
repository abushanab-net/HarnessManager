# Designer A: Calibre (checkpoint 1 and the full design)

Design read: a companion app for one technical owner who checks it in short glances (on a sofa, in a queue, at a desk away
from the computer) to see which account has room and to answer a waiting session. Operate mode: scanability and native
expectations first; the brand lives in the details. Dials: variance 5, motion 4, density 5.

Context note: the impeccable skill asks for a PRODUCT.md interview first. This run is a delegated brief with no way to ask
questions and with write access to this folder only, so the brief (`../brief/design-brief.md`), `review-1.html` and
`data.json` stand in for PRODUCT.md. Nothing was written outside `design-a/`.

## Three candidate directions

### 1. Calibre (picked)
The phone app reads like the dial of a well-made watch. Limits are about time: a window has room, and a reset erases what
is left. A watch already has a mature visual language for exactly that: chapter rings with minute ticks, subdials, a
power-reserve indicator, a hand that marks "now". The surfaces are enamel and paper, the ink is a deep navy, and the only
loud colour is a vermilion "seconds hand" kept for things that need you. Editorial serif for titles and readings, a calm
grotesk for everything you operate, mono only for commands and paths.

- Strength: the metaphor is functional. The reset clock, the harvest window and the countdown all become dials that
  show time and room in one shape, which is what the owner is deciding on.
- Risk: drifting into skeuomorphic watch kitsch (fake metal, bevels). Rule: the watch shows up in geometry and
  typography (ticks, hands, numerals), never in fake materials.

### 2. Field ledger
A bound notebook: ruled columns, a ledger of accounts with running balances, rubber-stamp badges, a ribbon bookmark for
the account to use next. Typewriter labels, warm paper.

- Strength: very warm, the numbers read like bookkeeping, honest about estimates with pencil-style "≈" marks.
- Why not: ruled lines and stamps are decoration unless they carry data, and the most important element (time left before
  a reset or before a request closes) has no natural shape in a ledger. It also risks the beige-paper look every
  "crafted" app ships with.

### 3. Almanac
A small daily broadsheet: a masthead with the date, a lead story ("Use claude-Resal-Dev01 first"), columns, footnotes for
the age of each reading, a weather-style forecast for limits.

- Strength: strong editorial hierarchy, the forecast metaphor fits "≈ runs out Saturday".
- Why not: newspapers are for reading, and two of the three jobs (find the session that needs you, answer it) are
  operating jobs. A broadsheet layout fights 44 pt targets and one-handed use.

## Why Calibre

It is the one where the metaphor does work. The owner's question is "how much room, and until when?". A dial answers
both in one glance, and the same component scales from the Home widget to the Account detail reset clock, to the
countdown on a permission request, to the XP power reserve, to lock-screen circular widgets later (which are already
round). It keeps the game layer adult: a power reserve and a date wheel feel like instruments, not toys.

## Key decisions so far

- **Colour roles are strict.** Ink for readings. Vermilion only for "needs you" (the one soft pulse in the app). Lume
  green (named after watch lume) only for harvest, XP and positive room. Amber only for low or stale readings. Each
  semantic colour always comes with an icon or words, so colour is never the only signal.
- **Light is "enamel", dark is "night dial".** Dark is a deep blue-black where the lume colours glow, the way a watch
  reads at night. Light uses soft tinted shadows for paper on enamel; dark uses hairline borders, because shadows vanish.
- **Type:** Newsreader (editorial serif with optical sizes) for titles and big readings, Hanken Grotesk for UI, JetBrains
  Mono only for commands, paths and code. Tabular lining figures everywhere a number can change.
- **Honest time:** countdowns tick calmly in ink, never turn red, and always say what happens at zero ("it waits in the
  terminal"). Every reading carries its age.
- **Use next picks claude-Resal-Dev01**, from the real data: 64% of its week is erased Saturday 18:58, the soonest large
  loss. samykabu-teksa has more room (75%) but keeps it until Monday. This is the harvest logic the game layer rewards.
- **Forecasts are ranges.** The Dev01 forecast shows "between running out Saturday afternoon and ≈ 30% left", because
  its usage is all from the last two days. A single number would be false precision.
- **Shape:** cards 16, controls 14, chips and badges full pill, sheets 28. Dials are always circles, never rounded squares.

## Invented for the mockups (not in data.json)

Request texts and their timers, the running-session count (7), XP (1,240), level (Planner, 3 of 4), streak (12, best 21),
badges, last week's recap figures, the XP log entries. The XP log is built from real resets in the data (the accounts whose
week reset in the last two days).

## Flutter mapping (outline; full version after review)

- `ThemeData` built from a `CalibreTokens` `ThemeExtension` (colours per role, light and dark), `TextTheme` from
  `google_fonts` (Newsreader, Hanken Grotesk, JetBrains Mono) with `FontFeature.tabularFigures()` on readings.
- Dials as `CustomPainter` (ticks, arcs, hands) animated with `TweenAnimationBuilder` / implicit animations; springs
  from `flutter_physics` or `SpringSimulation`. Reduce Motion via `MediaQuery.disableAnimations`.
- Reorderable board: `ReorderableListView` with a custom proxy decorator for the settle.
- Charts are small enough to paint by hand; `fl_chart` only if the project detail needs interactive charts.

## What to test with users (first list)

- Can the owner say which account to use and why in under two seconds from Home?
- Does the reset clock read correctly without the legend (time on the outside, room on the inside)?
- Does the calm countdown still create enough urgency for a 60 s question?
- Does "harvest" make sense as a word without onboarding?

## Checkpoint 1, revision 1 (art-director review, 25 Nov 2026)

The direction, type, palette, single vermilion, lume for XP, dial language and honesty rules stay. What changed:

1. **Earned delight, still calm.** Every moment runs once, about 600 ms, and lands in its end state under Reduce Motion
   (checked with `shots/rm-check.mjs`).
   - *Harvest:* a banner drops in ("Harvested · claude-Resal-Dev01's week reset with 4% unused"), the Home power-reserve
     gauge winds from 1,240 to 1,280, a lume tick lands at the new mark, and +40 rises and settles beside the XP while it
     counts up. Shown as a preview of Saturday 18:58, if Dev01 is used first.
   - *Badges* are engraved medallions: knurled rim, bead ring, enamel field, embossed icon, a lume jewel when earned, the
     name engraved around the rim at detail size. Locked badges are dashed with a lume progress arc on the rim and the
     progress in words. Tapping one opens a detail sheet (earned: date and the real event; locked: progress and what is left).
   - *Level-up:* the reserve winds to full, one sweep of lume ticks, then "Strategist" sets in. No confetti; one light
     haptic in the build.
2. **Home game header** is a slim strip (48 pt instead of about 90): a small power-reserve gauge, the level and XP, and
   the streak as a numeral in a date-wheel window that rolls from 11 to 12 on open. Needs you rises about 60 pt.
3. **Use next** reads "64% erased Sat 18:58" in the serif, with one muted line: "The soonest loss. teksa keeps 75% to Monday."
4. **Reset clock** dropped the weekday letters and the legend. Markers are labelled on the dial: Reset Sat 18:58 (lume),
   ≈ could run out Sat 15:00 (amber), Now Wed 21:22, and the bezel says "98 h gone" and "70 h to go". The centre still
   carries room left (64%), so time against room stays the reading.
5. **Permission request** uses the old gap for decision context: the session, what it changed this turn, and Claude's last
   line before it asked. The countdown moved into the action bar, next to the buttons, with what happens at zero.
6. **Presentation:** under each pair of live phones there is a full-length static artboard of the whole scroll in light and
   dark. A Moments section shows the harvest, the level-up and both badge-detail states with Replay buttons. The weekly
   recap is also rendered as a 1080 × 1350 share image (light and dark), previewed at 40% and saved full size as
   `shots/recap-1080x1350-*.png`.

Still flagged by the impeccable detector and kept on purpose: the request-card specimen inside the design-system card
(a specimen, not app UI), the light spring on settle and on the moments (the brief asks for springs; the overshoot is
about 6%), and a navy card shadow on the light phones that the detector reads against the dark page theme.


---

# Full phase (after review 2, 26 Nov 2026)

The owner picked Calibre for every screen. The full design keeps the checkpoint's direction, tokens, dials and honesty rules,
and adds what the review asked for: fun without noise, a glanceable first screen, and Needs you reachable from every tab.

## Where everything is

| Page | Contains |
|---|---|
| `index.html` | Cover, the full design system (colour, type, components, motion, spacing, radii, elevation, iconography, the Needs-you pill, switches, banners, list rows, tab bar, sheet), and the page directory |
| `onboarding.html` | S1 Welcome, S2 Scan, S3 Enter code, S4 Approve on your computer, S5 Connected, S6 Notification prompt |
| `requests.html` | S7 Home, the Needs-you entry on all five tabs plus the empty state, S8 Requests, S9 Permission, S10 Question and its expired state |
| `sessions.html` | S11 Sessions, S12 Session detail (with Control, and without it), S13 Stop sheet, delivered and delivery-unknown outcomes |
| `accounts.html` | S14 Accounts, S15 Account detail (revised), S16 Project detail, S17 Computers |
| `you.html` | The You tab, S18 Progress (revised), S19 Customize Home, S20 Settings and privacy, S21 Notification settings, the earned moments, the 1080 × 1350 recap image |
| `states.html` | Empty / first-sync Home, S22 a/b/c (computer asleep, hub unreachable, phone offline), S23 a/b (access removed, update needed) |
| `platforms.html` | Android adaptations of Home, Permission and the Stop sheet, each beside its iOS version; S24 desktop Pair a phone and Phones list (four frames) |
| `widgets.html` | Two variations of every widget family, light and dark, with the WidgetKit / ActivityKit / Glance constraints |
| `prototype.html` | One phone, five clickable journeys with a step list that follows you |

Every screen is a live phone in light and dark. The top bar's **Full scroll** unrolls every phone to its full length
(`?full` in the URL does the same, for screenshots). Shared code: `calibre.css` (tokens and components), `calibre.js` (dial
primitives, Home, Permission, Account detail, Progress, the moments, page mounting and the prototype router) and one
`screens-*.js` per area. The pages are generated from `tools/body-*.html` with `node tools/page.mjs` so the head and the
script list stay identical; the generated `.html` files open from disk with no build step.

## Revisions to the approved screens

- **Account detail (S15)** now answers in the first 844 pt: the reset clock is 300 pt (its labels kept at 11.5 pt), then a
  decision card (Use next, “64% erased Sat 18:58 if unused”, harvest window, Remind me), then the three windows as one compact
  table. Forecast, 7 days and Sessions moved into tabs below; the page went from about 2.4 screens to 1.2.
- **Progress (S18)**: the level card gained a Next row (the one harvest that moves the reserve, with Open), the streak is one
  row with the date wheel turning 11 → 12, and Badges, XP log and Recap are tabs. Level and next step sit above the fold.
- **Home (S7)**: the request button became the Needs-you pill, Use next is one tap target, and its muted line was corrected to
  the data: samykabu-teksa keeps 75% to **Tuesday** 09:58 (132.6 h from Wed 21:22), not Monday as the checkpoint said.
- **Permission (S9)**: unchanged, except the receipt's tick now lands after the ring closes.

## Per-screen decisions (new screens)

- **S1 Welcome.** The app icon (a night dial with a lume arc) sits inside the Calibre dial, which fills once: the first thing
  the app teaches is the shape it uses for every limit. Two scan choices: a computer nearby or a hub invitation. The version
  the computer needs (1.14) is said up front.
- **S2 Scan.** The camera is dark in both themes, like the system camera. The frame has minute ticks on its edges. Computers
  advertising a pairing (mDNS) appear in the sheet; Type the code instead is always visible; camera access is asked here.
- **S3 Enter code.** Two groups of three cells, like the desktop shows them; the computer's own limits (2 min, 3 tries) drain
  on a small dial; the nearby computer is preselected, host:port covers a VPN.
- **S4 Approve.** Three matching words large in the serif. Grants are shown as *asked*, not as switches: the computer decides.
  The wait is a draining countdown (how long the computer keeps pairing open), never a spinner.
- **S5 Connected.** The same lume seal as an answered request; the first-sync numbers count up once; LAPTOP is offered with
  Ask for access, MINI waits because it sleeps.
- **S6 Notifications.** The illustration is the real private notification (“A session needs you”). Not now is a real choice.
- **Needs you on every tab.** One pill in the large-title bar: signal wash, the count, and the soonest countdown. It moves into
  the compact nav bar on pushed screens. With nothing waiting it becomes a quiet tray button, so the bar never jumps.
- **S8 Requests.** Open requests are cards with a vermilion edge, the full command or question, the options a question offers,
  and the time against its real limit (“4:12 of 5:00”). Resolved ones say who answered: you, the terminal, the timeout.
- **S10 Question.** The tool's options exactly as offered (radio rows with descriptions, plus Write your own); 60 s because
  this phone has Control. At zero it shows “Back in the terminal”: nothing was sent, here is the next step.
- **S11 Sessions.** State is a word plus a mark (pulse, filled, ring, text only); rows needing you sort first; filters are chips.
- **S12 Session detail.** Your prompts in ink bubbles, Claude in the serif, steps folded with diff counts. The waiting request
  answers in place and closes into a one-line receipt. Follow starts the Live Activity. A session not started in Harness
  Manager's terminal shows a disabled composer with the reason, never a hidden one. A running step says “now”, no spinner.
- **S13 Stop.** States exactly what is sent (Ctrl+C, once) and that the session stays open. Ink, not red (red is never used and
  vermilion is kept for Needs you). Two outcomes: Interrupt delivered (receipt, route, time) and Delivery unknown (amber,
  “reading the session again; Stop is not sent twice”).
- **S14 Accounts.** Grouped by what to do (Use next, Low, Room to spare, No limit window) instead of a sort menu. The dial is the
  week; other windows are small bars. Low is amber with an icon and the word. No window shows a dash.
- **S16 Project.** Tokens, ≈ cost at list price, running, in one strip; the 30-day bars show the project's real start (hairlines
  before 11 Nov, not hidden); by model; the v1.1 drill-down is announced, not faked.
- **S17 Computers.** The route is drawn (phone to computer direct, or through the hub); grants as chips; MINI asleep shows its
  age with actions off; “Known, not granted” has an honest empty state instead of an invented computer.
- **You tab.** A slim level card on top (reserve, level, XP, date wheel) then short grouped lists in the platform pattern.
- **S19 Customize Home.** S / M / L inline on every row; the gallery previews each widget in its real shape; one catalogue for
  the board and the system Home and Lock Screens.
- **S20 Settings.** Face ID to approve is on and locked while any computer grants Control, and says why. Notification detail
  Hidden and widget detail Redacted by default. Access is per computer and changed only there. Disconnect says what it erases.
- **S21 Notifications.** One switch per event; thresholds are inline links to sheets; requests can break quiet hours (on);
  a test notification reports the last delivery.
- **Empty / first sync.** Live things (sessions, requests) arrive first; Use next waits for every limit so the pick is never
  made on half the data; skeletons match final shapes and do not shimmer (nothing loops).
- **S22.** One banner pattern for the three ways data gets old; readings turn ink-2, stale dials turn grey, an amber chip gives
  the age, estimates gain ≈. Needs you becomes “unknown”, not 0, when the hub or the network is down.
- **S23.** Says who removed the phone and when, that the cache is erased, and offers LAPTOP. Update needed names the version
  it runs (1.13.2) and the one it needs, with three steps.
- **S24 (desktop).** In the desktop app's own Chrono-Runway tokens. Pair a phone… is a separate button from Invite a computer.
  The dialog shows the used code and the approval side by side; Control is off unless ticked. The Phones list shows grants,
  last contact, expiry and Revoke.

## Fun without noise: what was added

Each runs once, about 400–600 ms, lands in its end state under Reduce Motion, and is never tied to approvals or spend:
the tick that lands when a request is answered (S9, S10, S12 inline, the Stop receipt), the date wheel turning on Home, You
and Progress, the first-sync count on Connected, the receipt rise. The harvest wind-up, level-up and medallions from the
checkpoint stay. The vermilion pulse remains the only loop.

## Android adaptations

Same screen bodies (one widget tree); the scaffold is Material 3 (revised after review 3, see below). In the prototype code
`android()` swaps the scaffold around the shared bodies: chrome, top app bar, NavigationBar, FAB, biometrics. That is how the
Flutter side should work too: a platform scaffold builder around shared body widgets.

## Flutter mapping

**Theme structure**
- `CalibreTokens extends ThemeExtension<CalibreTokens>`: colour roles (`bg`, `paper`, `sunk`, `ink`, `ink2`, `ink3`, `rule`,
  `signal`, `signalWash`, `lume`, `lumeWash`, `amber`, `amberWash`, `onInk`), one `light` and one `dark` const, `lerp` for the
  system theme switch. Semantic only: no widget asks for “green”.
- `CalibreType`: `TextTheme` from `google_fonts` (Newsreader for display/readings with optical size, Hanken Grotesk for UI,
  JetBrains Mono for commands), with `FontFeature.tabularFigures()` on every changing number. Sizes from the type scale in
  `index.html`; all of them scale with `MediaQuery.textScaler` (clamped at 1.6 on the dials' centre numbers only).
- `CalibreShape` (radii 16 / 14 / pill / 28) and `CalibreMotion` (`press` 150 ms, `state` 250 ms, `settle` 420 ms spring,
  `fill` 600 ms ease-out `Cubic(.22, 1, .36, 1)`), both as extensions too.
- `ThemeData` built per platform: `ThemeData(platform: …)` picks `CupertinoPageTransitionsBuilder` (iOS back-swipe) or
  `PredictiveBackPageTransitionsBuilder` (Android); `NavigationBar` vs a custom Cupertino-style tab bar; `showModalBottomSheet`
  with Calibre shape on both.

**Components → widgets**
- Dials, the reset clock, the power reserve, countdowns and medallions: `CustomPainter` (one `DialPainter` with ticks, track,
  arc, hand and labels), animated with `TweenAnimationBuilder` or an `AnimationController`; countdowns repaint from a single
  `Ticker` per screen, not one timer per widget.
- The board: `ReorderableListView` / `SliverReorderableList` with a `proxyDecorator` for the lift and the settle spring.
- Sheets: `showModalBottomSheet` (Material) and `CupertinoSheetRoute` where available; the badge and Stop sheets share content.
- Haptics: `HapticFeedback.lightImpact` on settle and on approve, `selectionClick` on the date wheel. One file decides them.
- Reduce Motion: `MediaQuery.disableAnimationsOf(context)` routes every Calibre animation to its end state.

**Packages**
- Motion: `flutter_animate` for the entrance settles and the one-shot moments (with `.animate(autoPlay: !reduceMotion)`);
  `SpringSimulation` from the framework for the settle; no Lottie or Rive (nothing here needs authored animation).
- Charts: none for the dials (custom painters). `fl_chart` only for the project bars if they become interactive in v1.1;
  the 7- and 30-day bars are simple enough to paint.
- Biometrics `local_auth`; secure storage `flutter_secure_storage` (keys excluded from backup); camera `mobile_scanner`;
  push `firebase_messaging` (Android) and APNs via a small plugin; native widgets via `home_widget` (shared app-group /
  DataStore cache) with the widget UIs written natively: SwiftUI WidgetKit + ActivityKit, Kotlin Glance.

## Widget constraints designed for

- **iOS Home Screen** (`systemSmall` 170, `systemMedium` 364 × 170, `systemLarge` 364 × 382 pt): no free animation, only the
  system's content transition when the timeline changes; live countdowns with `Text(timerInterval:)` and
  `ProgressView(timerInterval:)`. Timeline entries at each reset, and a reload on push. Tinted / clear modes (iOS 18+) use
  `widgetAccentable` on the dial arc. Buttons could run App Intents, but every action needs Face ID, so widgets open the app.
- **Lock Screen** (`accessoryCircular`, `accessoryRectangular`, `accessoryInline`): vibrant, one tint at varying opacity, so
  dials separate track and arc by opacity; semantic colours are lost there by design and the words carry the meaning.
- **Live Activity / Dynamic Island**: up to 8 h active plus 4 h on the Lock Screen, 160 pt maximum height, push-updated.
  Compact leading/trailing hold one glyph and one timer; minimal is a 37 pt circle shared with other activities; expanded is the
  only place with a sentence. Started by Follow (S12) or, if the owner allows, by an open request.
- **Privacy**: widgets and activities read a redacted encrypted cache; titles, commands and options appear only when Settings ›
  Widget detail allows it (variation A2 of the expanded island shows that case).
- **Android Glance**: RemoteViews underneath, so dials are rendered to bitmaps by the app on each update; `SizeMode.Responsive`
  with three layouts per widget; WorkManager refresh at each reset and on push; `Chronometer` for countdowns.
- **Ongoing notification**: a classic ongoing notification (A1) or an Android 16 `ProgressStyle` Live Update promoted to a
  status-bar chip (A2). Removed when the session ends; marked stale if contact is lost.
- **Staleness**: zero on a widget countdown means “refresh due”, never “limit back”; a reading older than an hour shows its age.

## What to test with users

1. **Two-second test (Home and the medium widget):** “Which account should you use now, and why?” Pass: the right account and
   the reset time, unprompted, in under 2 s.
2. **Needs-you pill:** from the Accounts tab, “Is anything waiting for you?” Do people see the pill, read the timer as time left
   for the soonest request, and find the inbox in one tap? Does the pulse feel urgent enough without feeling alarming?
3. **Question at 60 s:** give a 60 s question; measure the answer time and whether the calm ink countdown still prompts
   action. Then let one expire: do people understand nothing was sent and where it went?
4. **Grants as asked (S4):** do people expect to toggle grants on the phone? Does “Your computer has the final say” settle it?
5. **Stop:** after Delivery unknown, do people press Stop again? (They should not need to; the screen says it re-reads.)
6. **Account detail at one screen:** with the forecast in a tab, do people still find “could run out Sat 15:00” on the clock?
7. **Harvest and XP:** does “harvest” make sense without onboarding, and does +40 XP feel earned rather than gamey? Does anyone
   feel pushed to burn tokens to earn XP (the design must never reward spend)?
8. **Lock Screen widgets in monochrome:** can people tell 11% (low) from 64% without the amber, from the number and the arc?
9. **Android:** do Android users find back (predictive back) and the sheet dismissal natural with the Calibre look?
10. **Accessibility:** run the S8, S9 and S14 tasks at the largest Dynamic Type / font scale and with VoiceOver / TalkBack.

## Not finished, or done differently

- The large-text frames and the filter and threshold sheets, missing after the full phase, were added in review 3 (below).
- **Desktop S24** is drawn as four frames (dialog and list, dark and light) instead of one, because the dialog covered the
  Phones list that the brief also asks for.
- Invented for the mockups, beyond the checkpoint list: the question's options and Claude's last line on Bunyan, the resolved
  requests, the Codex transcript on LAPTOP, the project's daily split and model split, device names (iPhone 17, Pixel 9),
  the Harness Manager version on each computer (1.14.0, with DEVELOPMENTPC at 1.13.2 in S23 b only).
- Skills: design-taste-frontend's own scope excludes native mobile, so only its copy, contrast, state and motion rules were
  applied; impeccable's checks were run at checkpoint and the same rules were kept (no new detector run in this phase).

---

# Review 3 revisions (26 Nov 2026)

The judge kept the set and asked for six changes. All six are done; fresh shots are in `shots/` (`sheet-*.png`, `wg-*.png`,
`sheets-new.png`, `perm-ctx*.png`, `wg-ruler.png`, `wg-la1.png`, `wg-n1.png`).

1. **Android is Material 3, not iOS with a new status bar** (`platforms.html`). Edge-to-edge system bars with a soft top scrim
   and the gesture handle over the navigation bar. A **large** top app bar on Home (date and title left, search and overflow
   right), a **medium** one on Permission (arrow and overflow on top, headline below, collapses to small), a **small** one over the
   session behind the Stop sheet. The **NavigationBar**: 80 dp, a surface-container tint, a 64 × 32 active-indicator pill, filled
   icon, labels always shown. Needs you is an **extended FAB** on every tab (the iOS pill's job in Android's idiom). Stadium
   Filled and Outlined buttons. A **state layer** drawn on the first request row (pressed: 10% ink with the ripple from the touch
   point; hover 8%, focus 10%, drag 16%). The Stop **modal bottom sheet** has a 32 × 4 drag handle, 28 dp corners, a 32% scrim,
   a left-aligned headline, a stadium FilledButton and a text button. A new **predictive-back** frame: the request shrinks and
   follows the thumb, Home shows behind under a scrim, the edge arrow appears. Calibre type, colours and dials are unchanged.
2. **Permission: the session reads on one line.** The cause was a CSS name clash: the widget gallery's `.ctx` (a flex row) also
   styled the Permission context card, putting the session block and Claude's quote side by side. The gallery class is now
   `.wctx`. The session title runs full width with its age and files under it, and the quote full width below, on iOS and Android.
3. **Lock-screen privacy by default.** Live Activity A1 now defaults to the push alert's words: “Harness Manager · Needs you”,
   “A session is waiting for you · 4:12 left”, “Followed session”, “Unlock for details”, with no title, project or computer. The
   detailed version sits beside it, labelled **Lock Screen detail: on**, a new switch in Settings › Privacy (off by default).
   Android A1 shows the **public version** a secure lock screen gets (VISIBILITY_PRIVATE with setPublicVersion: “A session needs
   you · Unlock to see which one”, no actions) next to the full notification after unlock. A2 of both was already private.
   The Dynamic Island shows only while the phone is unlocked, so its detail follows the same rule as the app.
4. **Medium A2 week ruler: no collisions, 11 pt minimum.** Day labels sit above the ruler; the dots sit on it (smaller, sized by
   the loss); each label hangs on a leader line in one of two rows (main to the left of its leader, resal one row lower, Dev01 and
   teksa on the first row), so Fri to Sun never overlap. Labels are 11.5 pt, the harvest label 13 pt bold in lume, the days 11 pt.
5. **Added:** Home and Permission at **AX5**, the largest accessibility size (body 53 pt), light and dark (`states.html`, “The
   largest text size”): one column, everything wraps, the countdown leads each request, the dial caps its number at 58 pt, and on
   Permission the sticky bar keeps only the time left and two stacked full-width buttons. The **Sessions filter sheet**
   (`sessions.html`, S11 · sheet) and the **Limit-low threshold sheet** (`you.html`, S21 · sheet), light and dark.
6. **Text size in dense lists.** Every phone and widget was measured in the browser after the change: the smallest text anywhere
   is now 11 pt (tab-bar labels, the streak caption, widget labels), and the Sessions, Accounts and Home Needs-you rows start at
   12 pt (computer and age, plan, windows, time limits). Raised from 9.5 to 10.5 pt up to 11: tab labels, streak days, dial labels,
   lock-screen gauge labels, ruler days, the medallion's engraved rim text. Raised from 11.5 to 12: the secondary data lines in
   those lists, badge progress, the request “of 5:00” labels, the 7-day bar labels, the ladder and the computer stats. Only the
   drawn system keyboard on S3 keeps the system's own letter size.
