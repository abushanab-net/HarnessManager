# Design brief: Harness Manager review, November 2026

Harness Manager is a desktop app (Tauri) for a developer who runs several AI coding tools (Claude Code, Codex, OpenCode,
Antigravity, Pi, Oh My Pi) under several accounts on several computers. It shows each account's limits, what is running,
sessions and projects, and it manages the shared setup (MCP servers, skills, system prompts, routing, the tools themselves).

The owner's one-sentence job for the app: **"what is running, which account is exhausted, and which account should I use
next because it resets soon."**

This brief is for three design options that will sit side by side in a review report. Each option is drawn as static HTML
screens at 1280 × 800, in the Harness dark and light themes. They are mockups: nothing needs to work except looking right.

## What is wrong today (measured on the running app, 1.9.14)

1. **Text is too small.** On the Overview, 333 of 365 text elements are 11 or 11.5 px. The floor in these mockups is 12 px,
   body is 13 px.
2. **Pages are long.** At 1280 × 820 the Overview is 2.6 screens, Sessions 4.6, Settings 5.8, Skills 6.2.
3. **Small targets.** 31 of 84 controls on the Overview are under 24 px on a side. Here every control is at least 28 px tall.
4. **The page title is a sentence.** Each page's h1 is a "finding" ("7 sessions are running now; open one to follow it
   live."). On pages with nothing to find it turns into a slogan ("Every tool's sessions in one place, from…") and the
   page's name appears nowhere but the lit nav item.
5. **The Overview says each fact three times.** Every account row has a capacity column (bars with % left), a lane (time
   until reset) and a text column (next reset and pace), all for the same windows. A long label breaks the row: one tool's
   model-group names wrap to four lines and make a single row 250 px tall; a long plan name overlaps the next column.
6. **Needs attention repeats accounts.** One card per limit window, so one account appears two or three times.
7. **"Use next" ignores the tool.** It recommends one account across all tools, although a person usually wants "which
   Claude Code account" or "which Codex account".
8. **Nav badges mean five things**: today's cost, live sessions as a dot grid, a project count (which changes with the
   page that loaded it: 247 on the Overview, 29 elsewhere), an account count, an update count. None is labelled.
9. **Computers has no page.** Pairing, the hub and sync live in section 8 of 9 on the Settings page, although the top
   bar has a computers chip and sync is a headline feature.
10. **Details need a page change.** Opening an account or a session replaces the list; coming back loses the place.
11. **The primary button can be the wrong one.** On a logged-out account's page the primary "Start" is disabled and the
    action that works, "Log in again", is a secondary button at the bottom of a card.
12. **Search is only a filter.** Ctrl K focuses a box that searches sessions. It cannot start a session, add an account
    or jump to a setting.
13. **Lists of the same thing twice.** Projects shows the same projects in a share table and again as cards; Sessions has
    a project rail that repeats the pinned projects already in the sidebar.

## Rules for every option

- **Frame**: `<div class="app">` is exactly 1280 × 800 and never scrolls as a page (`base.css`). Content inside may be cut
  at the fold on purpose (show what fits), but nothing may overflow sideways or overlap.
- **Files**: write only inside this folder (`docs/project-review-2026-11/mockups/`). One HTML file per screen, named
  `<option>-<screen>.html` (e.g. `a-overview.html`). Each file links `base.css` and `kit.js` from this folder and puts
  its own layout CSS in a `<style>` block. No external requests of any kind, no frameworks, no build step.
- **Tokens only**: colours, radii and type sizes come from the custom properties in `base.css`. No raw hex in page CSS
  (official icon artwork keeps its own colours). Teal = selected or healthy, amber = pressure or a forecast, coral =
  exhausted or a failed login, emerald = spare capacity that resets soon, violet = a computer's identity. A colour never
  carries a state alone: pair it with a word or a shape.
- **Type**: `--fs-title` 20 (page name), `--fs-section` 15, `--fs-body` 13, `--fs-small` 12 (the floor), `--fs-key` 22
  (one key number per card). Inter for everything; JetBrains Mono (`.mono`) only for paths, commands and CLI text.
- **Official tool icons, always**: `<i data-tool="claude|codex|opencode|agy|pi|omp" data-size="16"></i>` (painted by
  `kit.js`). Never a letter, a generic glyph or a coloured dot in place of a tool's icon. The app mark is
  `<i data-logo data-size="26"></i>`. Computers are coloured dots (`.dot`, `.dot.pc2`, `.dot.pc3`) plus the name.
- **Every page has a stable name** (`.title`, top left) and, beside or under it, one status sentence in plain words.
- **One primary button per screen** (`.btn.primary`), and it is the action that works in the state shown.
- **Measured, estimated, not read**: a measured value is solid; an estimate carries "≈" (and a dashed or striped mark);
  a value the app has not read is "—" with the reason ("log in", "no plan limits").
- **Plain words**: no slogans, no explanation paragraphs, no marketing sentences. Sentence case. A label says what the
  thing is; a button says what it does ("Start", "Log in", "Follow", "Resume").
- **Themes**: `<html data-theme="dark">`; `kit.js` reads `?theme=light`. Both must look right.
- **Nothing dropped**: every action listed for a screen below must be reachable in the mockup (visibly, or behind a ⋯
  menu drawn open or implied by a ⋯ button). Do not invent features the app does not have, except those this brief asks for.
- **Screenshots**: `node shoot.mjs <file.html>` writes `<file>-dark.png` and `<file>-light.png` and prints the text sizes
  in use and whether anything overflows. Look at both PNGs (Read tool) and fix what is wrong: overlap, clipped text,
  anything under 12 px, misaligned columns, empty-looking areas. Iterate until the screen would pass a design review.

## Sample data (use exactly this, so the options can be compared)

This computer: **WORKSTATION**. Others: **LAPTOP** (seen 1 m ago), **STUDIO-MAC** (seen 3 h ago). Now: Tuesday 14:05.

| Account | Tool | Plan | 5-hour left | Week left | Other window | State |
|---|---|---|---|---|---|---|
| work-max | Claude Code | Max 20x | 64% · resets in 2 h 10 m | 41% · resets Fri 09:00 (2 d 19 h) | Week · Fable 35% | ≈ runs out Thu 11:20, 21 h before the reset. 2 sessions open |
| claude-main (main) | Claude Code | Max 5x | 100% | 78% · resets in 5 d 2 h | Week · Fable 80% | lasts until the reset. 1 session open (on LAPTOP) |
| client-acme | Claude Code | — | — | — | — | login expired: log in |
| team-ci | Claude Code | API key | no plan limits | — | — | pay per use · ≈ $4.20 today |
| codex-main (main) | Codex | ChatGPT Pro | 90% · resets in 4 h | 0% · resets in 1 h 02 m | — | used up until 15:07 |
| codex-personal | Codex | Plus | 100% | 88% · resets in 6 d | — | lasts. 1 session open. Most room among Codex accounts |
| opencode-main (main) | OpenCode | — | no plan limits | — | — | usage only |
| agy-main (main) | Antigravity | Google AI Pro | 100% · resets in 5 h | 100% · resets in 7 d | — | idle |

Emails (shown small under a name where the screen has room): alex@example.com (Claude accounts), alex@example.org (Codex),
alex@example.net (Antigravity).

Recommended next: for Claude Code **claude-main** (78% week left, lasts until its reset); for Codex **codex-personal**;
across all tools **agy-main** (untouched).

Running now (4): "Longhorn backup target for staging" · platform-gitops · work-max · WORKSTATION · 1 h 12 m · 59.8k tokens;
"Bump ArgoCD and fix the sync waves" · platform-gitops · work-max · WORKSTATION · 24 m · 18.2k;
"Invoice export: totals off by one cent" · billing-api · claude-main · LAPTOP · 41 m · 33.0k;
"Fix the flaky build" · web-console · codex-personal · WORKSTATION · 6 m · 4.1k.

Recent closed sessions: "Rotate the staging certificates" · infra-terraform · claude-main · Today 11:02 · 212k;
"Explain the release script" · docs-site · agy-main · Today 09:40 · 8.4k; "Run the long migration" · billing-api ·
codex-main · Yesterday 18:15 · 96k (1 failed command); "Draft the Q4 runbook" · docs-site · work-max · Yesterday 15:30 · 44k.

Projects: platform-gitops (github.com/example/platform-gitops), billing-api, web-console, docs-site, infra-terraform.
Tags: release, client-x.

Today: 48.2M tokens · ≈ $61.40 at API list prices · 94% read from cache. 7 days: 412M tokens · ≈ $498.

Needs attention (grouped by account): codex-main: week used up, back at 15:07. work-max: week ≈ runs out Thu 11:20,
21 h before its reset. client-acme: login expired. Codex: update 0.169.0 → 0.170.0 available.

Setup counts: 12 MCP servers, 33 skills. App version 1.9.14.

## Navigation (same for every option unless the option says otherwise)

Overview · Sessions · Projects · Accounts · **Computers** (new page) · Setup (MCP servers, Skills, System prompts,
API keys & routing, Apps) · CLI reference · Settings.
A nav badge appears only for something to act on or watch, with its unit: Sessions "4 live", Accounts "2" (need
attention), Setup "1 update". No cost in the nav.
Top bar: a command box "Search or run a command" with `Ctrl K`, then "4 live", the computers chip (three dots + "3
computers"), "limits read 1 m ago", refresh.
