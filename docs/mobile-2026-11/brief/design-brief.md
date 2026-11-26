# Harness Manager mobile: high-fidelity design brief

You are one of **two designers working independently** on the high-fidelity design of the Harness Manager phone app.
The owner will compare both designs side by side and pick one (or parts of both), so **commit to one bold, distinct,
well-reasoned direction**. Do not look for, read or open the other designer's folder.

## The product

Harness Manager is a desktop app (Windows/macOS/Linux) that manages the owner's AI coding tool accounts: Claude Code,
Codex, OpenCode and Antigravity. It shows plan limits and when they reset, token usage and list-price cost, live
coding sessions on several computers, and lets the owner answer a session's permission requests and questions, reply
to it and stop it. The phone app is its companion. It does three jobs, in this order:

1. **See which account has room** (limits, resets, the account to use next and why).
2. **Find the session that needs you** (a permission request or question waiting, with the time left to answer).
3. **Answer it while the request is still open** (approve/deny, pick an answer, reply one line, Stop).

## Approved scope (the owner kept every item of review 1)

Read `docs/mobile-2026-11/review-1.html` (open it in a browser, or read its HTML) for the low-fidelity wireframes,
the features, the flows and the notes under each screen. **Every screen, widget, notification and game mechanic in it
is approved.** Design all of them at high fidelity:

- S1 Welcome · S2 Scan QR · S3 Enter code · S4 Approve on your computer · S5 Connected · S6 Notification prompt
- S7 Home (a board of movable widget cards) · S8 Requests inbox · S9 Permission request · S10 Question
- S11 Sessions · S12 Session detail (transcript, inline request, reply composer, Stop) · S13 Stop confirmation sheet
- S14 Accounts · S15 Account detail (windows, reset clock, forecast, harvest window) · S16 Project detail
- S17 Computers · S18 Progress (level, XP, streak, badges, weekly recap) · S19 Customize Home (widget board + gallery)
- S20 Settings and privacy · S21 Notification settings · S22 Asleep / unreachable / offline states · S23 Access removed / update needed
- S24 Desktop: "Pair a phone" dialog and the Phones list (one desktop frame, in the desktop app's own style is fine)
- Also: the Weekly recap card (shareable image), a level-up moment, and the empty/first-sync state of Home.

Tabs: **Home, Sessions, Accounts, Computers, You**. Requests are reachable from every tab.

## The owner's decisions

- **Built with Flutter** for iOS and Android together. Your design system becomes a Flutter theme: custom
  rendering is fine, but respect platform conventions where users expect them (iOS back-swipe and large titles,
  Android predictive back and Material bottom sheets, haptics on both). Show at least **three screens in an Android
  adaptation** next to their iOS version.
- **Home-screen widgets, lock-screen widgets and Live Activities are native** (SwiftUI WidgetKit / ActivityKit on iOS,
  Android App Widgets and an ongoing notification). Design them **within those platforms' real constraints**: widget
  families and sizes, no free animation inside home-screen widgets (only system content transitions), lock-screen
  widgets rendered in the system's monochrome/vibrant style, Dynamic Island compact / minimal / expanded layouts.
- Away-from-home access through the owner's hub from day one; push alerts are private ("A session needs you"),
  details fetched after unlock.
- Pairing grants: Summaries and Transcripts on by default; Control (answer, reply, Stop) granted on the computer.
- A **question** waits 60 s for a phone with Control (15 s otherwise); a **permission request** waits up to 300 s.
  Make the time left honest and calm, never alarming.
- **Game layer: balanced.** XP, harvest (using limit room before a reset erases it), streak, levels
  (Connected → Organized → Planner → Strategist), badges with the rule behind each, a weekly recap. Points only for
  saving room or time: **never** for tokens spent, money, approval speed or number of approvals. No leaderboards.
  The whole layer can be switched off.

## Design pillars

1. **Glanceable.** "Can I keep working, and where?" answered in under two seconds from Home and from the widgets.
2. **Gamified, not childish.** The restraint of a premium fitness or finance app: rings, streaks and XP that feel
   earned and calm, not a toy.
3. **Honest numbers.** Every reading shows its age; estimates carry ≈; unknown is a dash, never a zero; a stale
   reading looks stale.
4. **Professional and crafted.** It must look like a senior product designer made it with care, following Apple HIG
   and Material 3 practice: clear hierarchy, consistent spacing scale, real typographic system, considered icons.
5. **Subtle motion.** 150–600 ms, spring or ease-out, purposeful: rings fill on open, the XP bar ticks on a harvest,
   cards settle when reordered, one restrained level-up celebration, a soft pulse only on what needs you. Nothing else
   loops. Everything respects Reduce Motion.
6. **Accessible.** WCAG AA contrast in light and dark, 44 pt / 48 dp touch targets, Dynamic Type / font scaling that
   does not break layouts, colour never the only signal.

## Avoid

- The generic "AI-made" look: purple-blue gradients, glassmorphism everywhere, neon on black, emoji used as icons,
  a grid of identical rounded cards in Inter with a big number on each.
- Copying the desktop app's look one-to-one, or recreating a specific existing app (Apple Fitness, Duolingo,
  Robinhood…). Learn from them; don't clone them.
- Placeholder or lorem text. Use the real data below.

## Real data

`docs/mobile-2026-11/brief/data.json` is a snapshot of the owner's real accounts, limits, projects, recent sessions and
totals. Use these names and numbers. Invent only what is missing (request texts, XP, streak, badges) and keep it
plausible for this data. Account names, project names and computer names are the owner's own.

## What to deliver, in your folder only

A **standalone HTML prototype** that opens from disk (`file://`) with no build step:

- `index.html`: the entry point, with
  1. a cover: the direction's name, a one-paragraph rationale, and its three key ideas;
  2. the **design system**: colour tokens (light and dark), type scale, spacing, radii, elevation, iconography,
     motion tokens (durations and curves) and the core components (cards, rings, bars, chips, buttons, sheets,
     list rows, the request card, the tab bar);
  3. **every screen** above, in iPhone frames (390×844) at high fidelity, light and dark (a toggle is fine), plus the
     three Android adaptations;
  4. a **clickable prototype** mode for the main journeys: Welcome → Scan → Approve → Connected → Home; Home →
     Needs you → Permission → Approve once → receipt; Home → Use next → Account detail; Sessions → Session detail →
     Stop sheet; You → Progress → badge detail;
  5. a **widget gallery** with **at least two distinct variations** of each: iOS small, medium, large home-screen
     widgets; lock-screen circular, rectangular and inline; Live Activity on the lock screen and the Dynamic Island
     (compact, minimal, expanded); Android home widget (resizable) and the ongoing notification. Label each variation
     (A1, A2…) and say in one line what makes it different.
- Any extra `.css` / `.js` / image files it needs, inside your folder. Libraries only from cdnjs.cloudflare.com,
  cdn.jsdelivr.net or unpkg.com with an exact version; fonts from Google Fonts. Everything else inline.
- `notes.md`: the direction, the key decisions and trade-offs, how it maps to Flutter (theme structure, any packages
  you would use for motion or charts), and what you would test with users.

## How to work

- Use your design skills: **design-taste-frontend** (its app-screen run and rules), **impeccable**, and
  **frontend-design** / **ui-ux-pro-max** if you have them. Read their `SKILL.md` and follow them.
- Before drawing, write down three candidate directions in `notes.md` and pick the most distinctive one that still
  serves the pillars.
- **Look at your own work.** Render your pages to screenshots and fix what looks wrong: alignment, clipping,
  overflow, contrast, inconsistent spacing. Playwright is installed:
  `import { chromium } from 'file:///D:/Projects/abushanab-net/HarnessManager-rc-chat/e2e/node_modules/playwright-core/index.mjs'`
  then `chromium.launch({ channel: 'msedge' })`. Keep screenshots inside your folder under `shots/`.
- Write only inside your folder. Do not edit anything else in the repository. Do not commit.

## Checkpoint 1 (do this first, then stop)

The owner wants to see each direction early. **Deliver only this, then stop and report**; you will receive review
feedback and then continue to the full scope above.

- In `index.html`: the cover (direction name, rationale, three key ideas) and a compact design-system strip
  (colour tokens light and dark, type scale, the ring, bar, chip, button and card components, motion tokens).
- **Four screens, at full high fidelity, light and dark**, in iPhone frames (390×844), with their motion working:
  1. **S7 Home**: the widget board, Needs you, Use next with the harvest hook, the game header.
  2. **S9 Permission request**: the command, where it runs, the honest countdown, Approve once / Deny.
  3. **S15 Account detail**: the windows, reset clock, forecast, harvest window, 7-day usage.
  4. **S18 Progress**: level, XP, streak, badges, the weekly recap card.
- `notes.md`: your three candidate directions, the one you picked and why.
- Self-review from screenshots before you report.

## Full phase (after review 2)

The owner compared both checkpoints and picked **Calibre (designer A) as the direction**, preferring it on every screen
(Home, Permission, Account detail, Progress). Split Time (designer B) is not continued as an app direction.

What the owner and the judge want carried into the full phase:

- Calibre stays: Newsreader + Hanken Grotesk + JetBrains Mono, enamel/night dial, ink, one vermilion for what needs you,
  lume green for XP and room, amber for low or stale, the dial language, the honesty rules.
- **Fun without noise.** Calibre's risk is reading calmer than "fun to monitor". Keep the earned moments (harvest wind-up,
  level-up, medallions) and add small rewarding feedback where it is earned (a tick landing when a request is answered,
  the streak date wheel turning), never on approvals or spend.
- **Glanceable first screen.** Split Time's strength was that each screen answered its question without scrolling.
  Account detail and Progress currently run about 2.4 screens: put the decision (room, reset, use next / level and next
  step) in the first 844 pt, push secondary material below, collapse or tab the rest.
- Requests reachable from every tab (a Needs-you pill in the large-title bar or the tab bar), honest countdowns.

### Designer A: everything in the "What to deliver" list above

All 24 screens (S1–S24) plus the recap card, the level-up moment and the empty/first-sync Home, light and dark; the
three Android adaptations next to their iOS versions; the clickable prototype journeys; the widget gallery with **at
least two variations (A1, A2…) of every widget family**; `notes.md` with the Flutter mapping and what to test. Split the
HTML into several pages if one page gets heavy (for example `index.html` cover + system + navigation, `screens.html`,
`prototype.html`, `widgets.html`), sharing `calibre.css` / `calibre.js`. Keep the four approved screens; revise them only
for the glanceability point above.

### Designer B: a second set of widget variations, in Calibre

The owner asked for at least two widget variations **from each designer**. B now designs widgets **inside Calibre's
design system**: read `docs/mobile-2026-11/design-a/` (notes.md, calibre.css, index.html) for tokens, type, dials and
icons, and bring B's own strength (one-glance readability, energetic but calm game signals). Deliver in
`docs/mobile-2026-11/design-b/widgets/`: `widgets.html` with **at least two variations (B1, B2…) of every widget family**
(iOS small, medium, large; lock-screen circular, rectangular, inline; Live Activity lock screen and Dynamic Island
compact / minimal / expanded; Android resizable home widget at three sizes; the ongoing notification), light and dark
(lock-screen ones in the system's monochrome / vibrant tint), each with a one-line "what makes it different", plus
`notes.md` (what each family shows, the WidgetKit / Glance / RemoteViews constraints you designed for, refresh and
staleness rules). Do not repeat A's widget layouts if you happen to see them; make distinct choices.
