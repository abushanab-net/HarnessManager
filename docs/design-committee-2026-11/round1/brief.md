# Design committee, round 1: the brief for the designers

You are one of three senior product designers (UX/UI, SaaS products, 3D and animated interfaces, WebGL sites) asked to
redesign Harness Manager's first screen. Three designs are made in parallel by three designers who never see each
other's work; a committee of critics then reviews all three, and the owner picks one as the base and names parts to
take from the others. Your job is one complete, distinct, buildable design that makes the owner say "wow" and the lazy
user say "I got my answer".

## The product, in one paragraph
Harness Manager is a desktop app (Windows and macOS; Tauri with a WebView2/WKWebView window; plain HTML, CSS and
vanilla JavaScript, no framework) for people who run several AI coding tools (Claude Code, Codex, OpenCode,
Antigravity, Pi, Oh My Pi) on several accounts and several computers. Each account lives in its own folder with its
own login. The app reads each account's limits (a 5-hour window, a weekly window, sometimes per-model windows), its
usage and cost, which sessions are running where, which projects they run in, and it starts sessions on the account
with the most room. It also manages MCP servers, skills, system prompts, API-key routing and tool updates, but those
pages are not in this round. It lives in the system tray and is opened many times a day for a glance.

## What the owner said (verbatim where it matters)
- The app today "looks crowded and disorganised, especially the Overview behind the side panel". They ticked: the
  readouts under the brand (tokens, cost, cached), the limit rings at the foot of the sidebar, the explanation paragraph
  under every heading, too many monospace numbers everywhere.
- First ten seconds after opening: "looks at the limits, find out what is running and where".
- Pages opened most: Overview, Projects, Sessions (to pin projects). Almost never: Accounts, Settings ("only when
  needed").
- The one sentence the Overview must answer: **"what is running, what account is exhausted, which account we should
  use because it would reset soon."**
- Jobs ranked for the Overview (1 = must be visible without scrolling; 2 = visible or one click away; 3 = may live on
  its own page or behind a disclosure):
  1. which account has room now and which is about to run out · what is running now, on which computer, follow one ·
     when each limit resets
  2. start a session on the right account from here · cost and tokens today/this week and the trend · which computers
     are alive · which tool talks to which account
  3. things that need attention · the latest replies and their speed · activity over the year
- The lazy user: "a developer colleague who uses one of these tools but has never seen the app". The app will be
  released publicly, so strangers will use it.
- Freedom: you may drop or replace the Horizon Dial, remove the sidebar entirely (a top nav, a rail, a command
  palette), change the navigation structure, drop the finding-as-headline pattern, change the fonts, change the
  Harness palette, and put features behind menus or disclosure. Nothing the app does may be lost (see inventory.md).
- Themes in this round: Harness light and dark only (VektraGrid is derived later from the winner).
- Screens: 1920 wide at 100%, 1536 at 125%, 1280 at 150% (a laptop), and macOS. The window is often side by side with
  a terminal, so it must degrade gracefully when narrow.
- Wow: "let the three designs differ on this, I decide after". Motion, depth and 3D are allowed; each must mean
  something; a setting turns motion off and reduced-motion is honoured.
- Mood in three words: **precise, live, future.**
- From the earlier redesign the owner still loves: the year clock, the readouts' dot-grid trends, the limit circles.
  Keep the ideas if they serve your design; redraw them as you see fit.
- Adding an account: the three kinds (subscription, API key, routed) confuse, and it is not obvious which tool supports
  which kind; otherwise "it works, only make it look like the rest". Design both the first run (no accounts yet) and
  adding one more to an existing set. Designs must hold up to 12 accounts comfortably (the owner has 8 across 4 tools
  on 3 computers).

## Your angle, and how the three designs are kept different
Each of you chooses your own angle; the clerk only makes sure the three differ. To guarantee that, each designer leads
with a different part of the owner's sentence as the hero of the Overview. Your lead is in your task message. The
other two parts must still be answered on the Overview (they are rank 1), but your lead is what the eye meets first and
what the design is built around. Write `angle.md` before you build: your angle in one line, your hero and why, the
references you chose (see references.md; add your own), what you removed from the first screen and where it went,
and what in your design is motion or depth and what it means.

## Deliverable: a clickable prototype that opens from disk
Write everything into your own folder (given in your task message) and nothing outside it. The folder holds:
- `index.html`: the prototype. One page that opens with `file://` in Edge, Chrome and Safari with no build step, no
  package manager and no server. Plain HTML, CSS and JavaScript; ES modules are fine only if they load from the same
  folder (use classic scripts to be safe). A CDN `<script>` is allowed only for three.js or GSAP if your design truly
  needs it; everything else is yours. Keep it to a few files.
- A theme switch (Harness light / dark) visible on the page and remembered in localStorage; a screen-size switch
  (1280 / 1536 / 1920 wide) that frames the app at that width so the owner can check each.
- Working: navigation between the Overview and the Accounts page (other nav items may show a "not in this round"
  placeholder in your design's style); hover, focus and selection states; the Add account flow from the Accounts page
  and from the first-run empty state (a toggle on the page switches the prototype between "8 accounts" and "first
  run, nothing yet"); motion if your design has it, with a Still toggle.
- `angle.md` as above, and `data.js`: your mock data (8 accounts on 4 tools and 3 computers, realistic values; see
  `../../../overview-redesign-2026-11/data.js` for the shapes and values the app really has; name accounts like the
  owner does: claude-main, codex-main, opencode-main, agy-main, sabushanab-resal, service-account-resal,
  samykabu-teksa, samykabu-personal; computers DEVELOPMENTPC, SAMYMSI, Samys-MacBook-Pro).
- Keyboard: the nav and every action reachable by Tab, Enter and Escape; contrast at least WCAG AA for text.

## Assets you must use
- The logo: `docs/logo.svg` (inline it). The Harness mark appears in every theme.
- The tools' official icons: `app/ui/icons/{claude,codex,opencode,agy,pi,omp}.svg` (inline them; never a letter, a
  dot or a generic glyph in place of a tool's icon). Computers are coloured dots or whatever your design chooses, but
  not tool icons.
- Fonts available offline: `app/ui/fonts/` has Inter, JetBrains Mono and EB Garamond (regular, italic) as woff2, with
  the `@font-face` rules in `app/ui/fonts.css`. You may change the fonts to any you can ship as a file in your folder
  or load from Google Fonts; say so in angle.md.
- Palette today (Harness dark): background #0B1418, panel #10202A, line #203540, ink #E6EEF1, muted #8FA6AE, accent
  teal #3FD6CF, warning #E0A84C, negative #F07A6A; Harness light: background #F5F8F9, accent #0E7E88. You may change
  it; keep teal somewhere in the family so the brand is recognisable.

## Rules that are not up for debate
1. Every number is honest: measured (solid), estimated (≈, or dashed), not read (—) stay distinguishable.
2. No tokens-per-second anywhere on the Overview.
3. Nothing in inventory.md is lost. Hidden is fine; gone is not. Say where each thing went.
4. No invented features. If your design needs a capability the app lacks, mark it "proposed" in angle.md and on the
   screen with a small tag; the critics will price it.
5. The page re-renders every few seconds while sessions write; your design must not depend on state that a redraw
   would lose, and motion must not restart on every redraw.
6. The lazy user reads no explanations. If a thing needs a paragraph to be understood, redesign the thing.
7. Write only inside your folder. Do not touch app/ui, do not run the app, do not commit.

## What the critics will score (so you know the target)
Craft (hierarchy, type, colour, motion with meaning, brand), clarity (the owner's sentence answered in five seconds
without reading; every job ranked 1 above the fold at 1280 wide), honesty of every instrument (does it encode something
a number would not say), completeness against inventory.md, buildability in a vanilla-JS WebView2 app that redraws
every few seconds, and the lazy user's verdict: what they got in ten seconds, what they had to hunt for, what they
would never use.

## Files to read before you start
- `docs/design-committee-2026-11/round1/inventory.md` (what must survive)
- `docs/design-committee-2026-11/round1/references.md` (inspiration to choose from)
- `docs/design-committee-2026-11/shots/*.png` (the app today, on the owner's real data; light and dark)
- `docs/overview-redesign-2026-11/data.js` (real data shapes and realistic values)
- `app/ui/index.html`, `app/ui/home.js`, `app/ui/accounts.js` (what the app does, if the inventory leaves a doubt;
  do not copy their look)
