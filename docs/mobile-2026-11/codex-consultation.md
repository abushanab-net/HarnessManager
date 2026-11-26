# Harness Manager mobile companion

**Ship the phone around three jobs: see which account has room, find the session that needs attention, and answer it while the request is still open.** A useful first release can do all three on a LAN or VPN. Away from home, an optional hub should deliver the same experience through an encrypted relay. The session’s computer must remain the authority for every action.

This plan assumes one owner, native iOS and Android interfaces, and an existing Harness Manager desktop installation. At revision `7cd0a8a`, the desktop and CLI already share Rust operations for accounts, usage and sessions. The direct peer listener serves reads. The hub stores encrypted sync data and announces changes. Neither exposes the desktop’s answer, reply or Stop operations to a phone. [Architecture](docs/architecture.md) · [Desktop session operations](app/src-tauri/src/sessions.rs) · [Peer routes](crates/core/src/sync/peer/listener.rs) · [Hub routes](crates/hub/src/lib.rs)

## 1. Feature list

**NEW backend work needed for every connected feature:** a versioned companion API on the desktop with distinct grants for summary reads, transcript reads and session control. It should return the origin computer, observation time and available actions with each result. Expose selected fields from the existing Rust operations. The desktop’s general Tauri dispatcher also includes filesystem, settings and launch operations that exceed a phone’s grant. [Desktop operations](app/src-tauri/src/ops.rs) · [Account state](app/src-tauri/src/accounts.rs) · [Session operations](app/src-tauri/src/sessions.rs)

Paths abbreviated below use `core/` for `crates/core/src/` and `ui/` for `app/ui/`.

| Area | Feature and user benefit | Priority | Existing basis or additional backend work |
|---|---|---|---|
| Onboarding & pairing | Scan a desktop invitation or enter its code and address. See the computer name and requested access before activation. | MVP | `core/sync/peer/{pairing,identity,mdns}.rs`, `ui/computers.js`. **NEW:** companion invitation, desktop confirmation and scoped grant. |
| Onboarding & pairing | Enable away-from-home access through a self-hosted hub. | MVP, optional | `core/sync/hub.rs`, `crates/hub/src/lib.rs`, `deploy/hub/`. **NEW:** phone enrollment, recipient-encrypted reads and an online command relay. |
| Overview | Put **Needs you** first, followed by running sessions, the next reset and today’s tokens and estimated cost. Tap a card to open its source. | MVP | `core/{hooks,dash,service,journal}.rs`, `ui/home.js`. Pending requests come from each live origin. |
| Overview | Recommend an account for the selected tool and show the reason, such as weekly headroom or an expiring 5-hour window. | MVP | `ui/home.js` `recommendedFor`, `ui/limits.js` `metricsOf`. **NEW:** return the same ranking and reason to both clients from one implementation. |
| Accounts & limits | List accounts by tool and computer. Show plan or payment kind, measured windows, reset time, reading age and login attention. | MVP | `core/harness/`, `core/service.rs`, `ui/{accounts,limits}.js`. Claude and Codex report plan windows. Antigravity reads the currently signed-in account. OpenCode, Pi and Oh My Pi have no plan-window readings here. |
| Accounts & limits | Show reported credits, reset offers, configured daily caps and forecasts where available. Label measurement, estimate and missing data separately. | v1.1 | `ui/{accounts,limits}.js`, `core/{dash,db}.rs`, `app/src-tauri/src/accounts.rs`. Provider login and credit management stay on their existing surfaces. |
| Sessions & remote control | Browse and search sessions by computer, project, account, tool or tag. Distinguish running, archived and last seen. | MVP | `core/{service,index,journal}.rs`, `ui/sessions.js`. Use origin-qualified session IDs. |
| Sessions & remote control | Read turns, answers, failed steps and changed files. Follow an open session while connected. | MVP | `core/transcripts.rs`, `core/sync/transcripts.rs`, `ui/sessions.js`. **NEW:** bounded, authorized transcript pages and change cursors. |
| Sessions & remote control | Approve once, deny, or select the offered answers for a live hooked request. | MVP | `core/hooks.rs`, `core/harness/claude.rs`, `ui/sessions.js` `askCard`. **NEW:** route to the origin and return its decision result. |
| Sessions & remote control | Send one line to an eligible owned terminal or interrupt its active turn with Stop. | MVP | `core/terminal.rs`, `core/terminal/{control,windows}.rs`, `core/launch.rs`. **NEW:** enforce run-state checks at the origin and handle duplicate or uncertain deliveries. |
| Notifications | Receive a private “A session needs you” alert that opens the current request. Keep the in-app inbox usable when push is disabled. | MVP | `core/hooks.rs`, `ui/home.js`. **NEW:** expiring attention events, push registration, deduplication and relay. |
| Notifications | Choose account reset reminders, low-room alerts and quiet hours. | v1.1 | `core/db.rs`, `ui/limits.js`. Phone can schedule a known reset locally. **NEW:** desktop threshold events for alerts while the app is closed. |
| Computers | See authorized computers, connection path, last contact, last successful read and control availability. Retry or pair another computer. | MVP | `core/journal.rs`, `core/sync/peer/`, `ui/{computers,computers-page}.js`. **NEW:** distinguish a reachable command endpoint from recent sync presence. |
| Projects / usage & cost | Open a project to see its sessions, running work, tokens and estimated API list-price cost. | MVP | `core/{projects,service,pricing,journal}.rs`, `ui/projects.js`, `app/src-tauri/src/usage.rs`. Preserve request-ID deduplication and recorded cost. |
| Projects / usage & cost | Compare 7-, 14- and 30-day usage by account or model and inspect contributing requests. | v1.1 | `ui/accounts.js`, `core/{dash,db,service}.rs`, `app/src-tauri/src/usage.rs`. The mobile response must apply the selected computer scope consistently. |
| Widgets | Show attention, account headroom and reset cards inside Overview. Add home-screen and lock-screen glances, then a followed-session activity. | MVP in-app · v1.1 system widgets | `ui/{home,limits}.js`, `core/{hooks,journal}.rs`. Native rendering and redacted cache on the phone. **NEW:** followed-session update delivery. |
| Settings / security | Set app lock, preview privacy, cache retention and alerts. Inspect grants, disconnect or revoke a lost phone. | MVP | `core/secrets.rs`, `core/sync/peer/mod.rs`, `crates/hub/src/lib.rs`. **NEW:** phone grant store, revocation propagation and action receipts. |
| Settings / security | Inspect router health and the names of configured MCP servers, skills and prompts on a selected desktop. | Later | `core/{registry,items,service}.rs`, `crates/router/src/`, `ui/{routing,mcp,skills,prompts}.js`. **NEW:** metadata-only projection excluding keys, MCP values and prompt bodies. |
| Gamification | Opt into private consistency, reset-planning and organization milestones. | Later | `core/{db,journal,service}.rs`, `ui/{home,limits}.js`. Compute locally from authorized history and phone-local milestones. |

## 2. Registration and activation

1. **First install.** Offer **Pair a computer** and **Connect through my hub**. Generate a fresh installation ID and phone key. Keep key material in iOS Keychain or protected by Android Keystore, exclude it from transferable backups, and defer camera, local-network and notification prompts until their features are used. Existing desktop identity uses an Ed25519 key and a self-signed certificate. Verify phone key support before promising hardware-backed Ed25519 on both platforms. [Desktop identity](crates/core/src/sync/peer/identity.rs) · [Desktop secret storage](crates/core/src/secrets.rs)

2. **Direct invitation.** Add **Pair a phone** to desktop Computers. Its QR carries the protocol version, invitation type, short-lived pairing details, desktop identity fingerprint and address hint. Manual entry uses the six digits and LAN discovery, with `host:port` as a fallback. Reuse the current SPAKE2 code exchange, two-minute lifetime and three-failure cutoff. The QR must contain no recovery key, administrative token or lasting credential. [Pairing implementation](crates/core/src/sync/peer/pairing.rs) · [mDNS discovery](crates/core/src/sync/peer/mdns.rs) · [Existing pairing UI](app/ui/computers.js)

3. **Approve trust on the desktop.** After proof of the code, show the phone name, matching verification value and three grants: summaries, transcripts and control. Default to summaries. Bind the selected grants to the phone key and pin the desktop identity on the phone. **Use a separate companion enrollment payload and authorization path.** Current computer pairing can transmit the ring recovery seed, stores the entrant as a full peer and admits paired certificates to journal and transcript routes. A phone enrolled for summaries must be denied those routes even if it knows their URLs. [Pairing payload and `store_peer`](crates/core/src/sync/peer/pairing.rs) · [Peer listener](crates/core/src/sync/peer/listener.rs) · [Ring adoption](crates/core/src/sync/hub.rs)

4. **Activate through a hub.** Today the first desktop joins over HTTPS with the hub admin token and creates the ring. A joined computer can issue a one-use six-digit invite valid for ten minutes. A joining computer submits a signed device record, receives its own bearer token, and obtains the ring secret separately through trusted pairing or recovery-key entry. For phones, add a distinct invite class and phone-scoped token. Have an already trusted desktop approve the phone key and establish recipient encryption through an authenticated exchange bound to the invitation. The hub can carry that exchange. A copied code alone must confer neither data access nor control. Keep the admin token and ring recovery key on the desktop. [Hub join and invite](crates/hub/src/lib.rs) · [Client and recovery key](crates/core/src/sync/hub.rs) · [Deployment](deploy/hub/README.md)

5. **Add computers deliberately.** The first desktop may already know about other computers through folder, direct or hub sync. Show their names as discovered. Grant access separately at each origin, and scope any aggregated response to authorized origins. Use one phone identity with distinct grants per computer. A run on computer B always routes to B using its computer ID, incarnation and run ID. Two profiles named `work` must remain distinguishable. [Across-computers plan](docs/plans/across-computers/PLAN.md) · [Journal and presence](crates/core/src/journal.rs) · [Computer scope UI](app/ui/computers.js)

6. **Choose the available route.** Prefer pinned direct TLS on LAN or a reachable VPN address. With the proposed hub extension, the desktop opens an outbound connection and the phone can retrieve recipient-encrypted snapshots or contact the live origin through it. Today’s hub WebSocket announces changes and rendezvous supplies addresses. Neither delivers commands through NAT. If the desktop sleeps, show the last snapshot and its age. If the phone is offline, show its protected cache. In either case discard pending actions when their live target cannot be confirmed. [Direct client](crates/core/src/sync/peer/client.rs) · [Hub routes](crates/hub/src/lib.rs) · [Hub deployment behind Caddy](deploy/hub/docker-compose.yml)

7. **Register push after activation.** Ask whether the owner wants attention alerts. Register with APNs or FCM and bind the current provider token to the authenticated phone installation. Refresh the binding when the token changes, prune rejected tokens, and delete it on revocation. For public store builds, place provider credentials in an **optional publisher-operated push relay**. The self-hosted hub, or a directly paired desktop with outbound internet, sends it an authenticated opaque wake-up addressed to a relay handle. The phone fetches details after unlock. A direct-only user away from the LAN may receive an alert but still needs VPN or hub access to act. Self-built distributions can run the relay with their own app credentials. [Apple APNs registration](https://developer.apple.com/documentation/usernotifications/registering-your-app-with-apns) · [FCM token management](https://firebase.google.com/docs/cloud-messaging/manage-tokens) · [Android notification permission](https://developer.android.com/develop/ui/compose/notifications/notification-permission)

8. **Revoke or replace a phone.** From a trusted desktop, revoke the phone grant, terminate live connections, invalidate its hub credential and push binding, and stop publishing snapshots for it. Show which origins acknowledged revocation. For a disconnected origin, bound exposure with expiring grants and require a fresh authorization check before control resumes. A replacement phone creates a new key, pairs again and receives fresh grants. Existing peer **Forget** removes a local pin while retaining merged history. Existing hub deletion accepts the device itself or the admin token. Neither can recall data already copied to a lost phone. [Direct forget](crates/core/src/sync/peer/mod.rs) · [Hub deletion](crates/hub/src/lib.rs)

### What the phone may safely offer on a session

| Action | Availability and result |
|---|---|
| **Approve once / Deny** | Offer only for a live, authorized `PermissionRequest`. Show the computer, account, tool, full supplied command or file and description. For edits where only the filename is available, say that the change can be inspected in the desktop terminal. The normal permission wait defaults to **300 seconds** and requires the desktop watcher’s heartbeat. A terminal decision or expiry closes the request. [Hook lifecycle](crates/core/src/hooks.rs) · [Request card](app/ui/sessions.js) · [Defaults](crates/core/src/ctx.rs) |
| **Answer question** | Render each supplied question and its exact options, including multi-select when offered. The hook caps the mobile opportunity at **15 seconds**, or less if configured. After release, the question appears in the terminal. Automatic hook injection exists for managed Claude Code launches. Codex’s event names are recognized, but its adapter does not install those hooks. [Hooks](crates/core/src/hooks.rs) · [Claude launch](crates/core/src/harness/claude.rs) · [Architecture](docs/architecture.md) |
| **Reply** | Require an owned Windows terminal and an origin that can establish the current state. `own_terminal` defaults off and applies at launch. Accept one nonblank line, at most 16 KiB, without control characters. Block reply while a prompt waits, since Enter can select a dialog choice. The local terminal token stays on the desktop. [Launch gate](crates/core/src/launch.rs) · [Input validation](crates/core/src/terminal/control.rs) · [Reply bar](app/ui/sessions.js) |
| **Stop** | Confirm the target and send Ctrl+C only for a working or waiting owned terminal. A second Ctrl+C at an idle prompt can exit the tool. Report “interrupt delivered” on the origin’s acknowledgment and refresh the run. A lost acknowledgment produces “delivery unknown” and must never trigger automatic resend. [Stop handling](crates/core/src/terminal.rs) · [Desktop eligibility](app/ui/sessions.js) |

**New command contract.** Bind each action to the phone grant, origin, run, request or observed state version, expiry and a unique action ID. The origin must recheck these immediately before execution, serialize conflicting actions and retain a short-lived receipt. Distinguish *accepted*, *already handled*, *expired*, *origin unavailable* and *delivery unknown*. The hook already conditionally updates an open request. That protection does not by itself make terminal input safe to retry. [Hook answer transaction](crates/core/src/hooks.rs) · [Terminal delivery outcomes](crates/core/src/terminal/control.rs)

## 3. Screen and sheet inventory

Use four primary destinations: **Overview, Sessions, Accounts and Computers**. Projects opens from Overview or a session. The attention inbox is reachable from every primary destination.

| Screen or sheet | Purpose and key elements |
|---|---|
| Welcome / connection method | Explain what requires a desktop. Offer scan, manual code or hub URL. |
| QR scanner / manual pair / hub invite | Camera permission on scan, code field, discovered computer or address, expiry, retry and cancel. Clearly label phone invitations. |
| Verify and activate / paired | Matching identity, requested scopes, waiting-for-desktop state, approved scopes and first-sync result. Desktop counterpart shows **Pair a phone**, approval and grant review. |
| Overview / scope and range sheet | Needs you, running now, recommended account with reason, next reset, usage and freshness. Select authorized computers, tool and reporting period. |
| Attention inbox / request detail | Open and recently resolved requests. Permission detail shows tool input and expiry. Question detail shows options and selection rules. Expired links explain where the user can still respond. |
| Sessions / filter and group sheet | Search, running first, project, account, tool, tag, computer, archived state and paging. |
| Session detail / reply composer / Stop confirmation | Origin and capability, conversation, changed files, failures, follow-latest and inline request. Show why a control is unavailable and distinguish terminal delivery from a subsequent model response. |
| Accounts / account detail | Tool and origin, kind, sign-in state, 5-hour or weekly windows, reset clock, stale or unavailable reading, cap, usage and related sessions. |
| Projects / project detail / usage breakdown | Project identity, sessions across authorized computers, running work, period totals and model/account breakdown. Drill into contributing requests. |
| Computers / computer detail / reconnect | Authorized and discovered devices, direct or hub route, last contact, capabilities, grants and connection diagnostics. Offer pair, retry or remove. |
| Settings / privacy and storage / access review | Biometric or device lock, preview detail, transcript retention, clear cache, active grants and disconnect. Desktop counterpart includes lost-phone revocation and acknowledgment status. |
| Notification settings / alert rule sheet | Permission state, attention alerts, per-account thresholds, quiet hours, test alert and last delivery status where known. |
| Widget configuration / follow-session sheet | Choose account, computer, size-dependent detail, preview privacy and which session to follow. |
| Progress / milestone detail | Opt-in, rule behind each milestone, evidence period, history completeness and hide/reset options. |
| Setup inventory, later | Selected desktop’s router status and names of configured MCP servers, skills and prompts. |
| Shared connection states | Inline first sync, stale cache, desktop offline, hub unreachable, unsupported desktop version, revoked access and re-pair. A revoked deep link opens access status. |

## 4. Widget concepts

| Surface | What it shows and opens |
|---|---|
| **In-app cards · MVP** | Needs you opens the exact request. Running now opens the origin session. Use next shows the deciding window. Next reset opens the measured account reading. |
| **Small home widget · v1.1** | One pinned account, limiting window, percentage remaining and next reset. Use a dash for unknown and show the age of the reading. |
| **Medium · v1.1** | Compare 5-hour and weekly room for two pinned accounts, with the next reset and a single attention count. Tap an account to inspect it. |
| **Large · v1.1** | Up to four account rows, running-session count per computer and the oldest pending request’s age. The default preview omits titles and commands. |
| **Lock screen · v1.1** | A redacted waiting count or one reset countdown. Use an iOS accessory widget and a private notification or supported lock-screen widget on Android. Tap to unlock and fetch. |
| **iOS Live Activity / Android ongoing notification · v1.1** | Follow one explicitly chosen run. Show working, waiting, interrupted, ended or last seen, plus elapsed time. Open the session on tap. The origin’s end event dismisses it, with a stale timeout if contact is lost. |

Use native WidgetKit timelines and Android resizable app widgets over a redacted snapshot. A countdown can advance locally, but reaching zero only means a **refresh is due**. A fresh reading must confirm recovered quota. Use ActivityKit updates for a followed run and an Android notification updated from origin events. Keep approval, reply and Stop inside the unlocked app. [Desktop quota presentation](app/ui/limits.js) · [WidgetKit refresh behavior](https://developer.apple.com/documentation/widgetkit/keeping-a-widget-up-to-date) · [ActivityKit](https://developer.apple.com/documentation/activitykit) · [Android app widgets](https://developer.android.com/develop/ui/views/appwidgets/overview)

## 5. Professional gamification

Make progress private, optional and computable from evidence the owner can inspect.

| Idea | Rule and evidence |
|---|---|
| **Consistency streak** | Count consecutive user-selected workdays with at least one recorded request in the chosen time zone. Deduplicate across computers. Mark incomplete sync days as unknown and recalculate after catch-up. `core/{journal,db,service}.rs` |
| **Reset planning (“harvesting”)** | Highlight an eligible account with at least half of its open 5-hour window left and the earliest reset, matching the desktop rule. Count one planned session when the user selects that suggestion and a subsequent request is recorded on that account before reset. This needs a new phone-local selection event. `ui/{home,limits}.js`, `core/db.rs` |
| **Organization badges** | “First project organized” for a project with a tag. “Second computer connected” after two distinct authorized origins complete a verified read. Both have evidence the user can reopen. `app/src-tauri/src/sessions.rs`, `core/journal.rs`, proposed phone grants |
| **Levels** | Use a short progression such as **Connected → Organized → Planned**, earned from pairing, organizing a project and completing one reset-planning milestone. Store progress locally. Exclude token volume, spend, approval speed and permission decisions from scoring. |

## 6. Open questions and release risks

| Decision or risk | Recommendation and acceptance check |
|---|---|
| **Can a user finish a question from a push in 15 seconds?** | Measure origin event → visible notification → unlocked detail → accepted answer on physical iOS and Android devices. Report questions separately from permissions. If most questions expire, position them as foreground actions and make terminal handoff clear. |
| **How much can a summary-only phone infer?** | Test a grant against full-peer routes, transcript reads, request inputs and aggregates containing an unauthorized computer. Include two computers with identical account and session labels. Keep hub-visible metadata and push payloads minimal. |
| **How quickly does revocation take effect?** | Set an explicit maximum offline grant lifetime. Test a lost phone, a sleeping desktop that reconnects, an existing connection and restored app storage. Document that previously viewed plaintext cannot be recalled. |
| **What happens on an uncertain command result?** | Test terminal-first answers, simultaneous phones, expiry during unlock, Wi-Fi-to-cellular handoff, an acknowledgment lost after a reply, and two Stop taps. The release criterion is zero stale approvals and zero repeated terminal inputs in those cases. |
| **Who operates push for the store builds?** | Decide credential ownership, relay funding and retention before enabling public push. Preserve direct foreground use and self-hosted data access when the publisher relay is disabled. |
| **Which numbers deserve confidence?** | Label list-price estimates, unavailable prices, stale plan readings and gaps in history. Honor transcript-sharing settings and existing size or retention limits. A missing transcript or quota reading should still leave the session discoverable. [Pricing](crates/core/src/pricing.rs) · [Transcript limits](crates/core/src/transcripts.rs) · [Account presentation](app/ui/accounts.js) |
| **What makes the first release useful?** | Verify two end-to-end journeys on both phone platforms: find a usable account from a stale dashboard, and receive then resolve a real desktop permission request over cellular. Record origin acknowledgment, time to resolution and whether the UI accurately explained each unavailable action. |

**Verification.** `git diff --check` and syntax checks for 26 JavaScript files passed. Workspace build, Rust tests and Clippy were attempted and stopped at the Cargo build lock because the checkout is read-only. Desktop E2E and physical mobile delivery were unverified. No files changed.

<<<DELEGATION_RESULT>>>
status: successful
summary: I traced the desktop, peer, hub and session-control implementations and produced a prioritized native mobile plan with activation, screens, widgets, professional milestones and release checks. I identified the new trust, command-delivery and push capabilities required and documented the verification limits.
files_changed: none
<<<END_DELEGATION_RESULT>>>