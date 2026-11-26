# Mobile companion: implementation plan

The design was approved with the owner on 26 Nov 2026 after three reviews (`docs/mobile-2026-11/review-1.html`,
`review-2.html`, `review-3.html`). The approved design is **Calibre** (`docs/mobile-2026-11/design-a/`). This file is
the contract that the work packages build against. Change it only by agreement; the sections marked **contract**
are what other packages rely on. The plan's review page is `docs/plans/mobile/plan.html`.

## Decisions already made

- **Flutter** for iOS and Android, built together. Home-screen widgets, lock-screen widgets, the Live Activity and the
  Android ongoing notification are **native** (SwiftUI WidgetKit / ActivityKit, Kotlin Glance / NotificationCompat).
- **Away from home through the owner's hub from day one**, on top of direct LAN / VPN.
- **Push through a small relay that we operate** for the store builds. Payloads carry no content ("A session needs
  you"); the phone fetches the details after unlock.
- **Grants per computer:** Summaries and Transcripts on by default, **Control** granted on the computer. In wave 1
  Control answers requests; a phone's Reply and Stop come later, with their own design (WP-M8).
- **Question window:** 60 s when a phone with Control is paired, 15 s otherwise. A permission request waits up to 300 s.
- **Game layer: balanced.** XP, harvest, streak, levels (Connected → Organized → Planner → Strategist), badges with
  their rule, weekly recap. Points only for saving room or time. It can be switched off.
- **Design:** Calibre. Every screen, state, moment, the three Android adaptations and the desktop S24 frames are
  kept as drawn.
- **Widgets (all designer A):**
  - iOS home screen: small A1 Use next dial; medium A1 Three dials + requests; large A1 Whole board.
  - iOS lock screen: circular A2 Waiting + timer; rectangular A1 Next reset + bar; inline A1 Pick and reset.
  - Live Activity A1 Followed session, private by default; Dynamic Island A1 set.
  - Android: resizable widget A1 Use next dial; ongoing notification A1 Chronometer + actions, with a public version.

### Decided with the plan (26 Nov 2026)

- **The app lives in a separate repository** (wave 2 creates it). This repository keeps the desktop, the hub, the push
  relay and `crates/companion-proto`; the app takes the protocol crate as a git dependency through
  **flutter_rust_bridge**, so both sides run the same pairing and sealing code.
- **The push relay runs in Docker next to the hub** on the owner's server.
- **No store accounts yet: internal builds first.** Android runs from the emulator and sideloaded builds. iOS needs the
  Mac (MINI) and a free Apple ID for simulator and own-device builds; APNs push and Live Activity push updates need a
  paid Apple Developer membership, so those spikes wait for it. Android push needs a Firebase project.
- **Minimum OS: iOS 17 and Android 8 (API 26).**
- **App name Harness Manager, identifier `net.abushanab.harnessmanager`** on both platforms.
- **Wave 1 starts now, in parallel.** The protocol crate is part of WP-M1, because the desktop's tests use it from the
  first day; WP-M2 builds against the `phones` contract below.

## What exists and what is new

| Exists today | Used for |
|---|---|
| `core/sync/peer/pairing.rs`: six-digit code, SPAKE2, sealed payloads, 2 min, 3 failures | Phone pairing reuses the exchange, with its own payload and route |
| `core/sync/peer/identity.rs`: Ed25519 key and self-signed certificate | The desktop's identity, pinned by the phone |
| `core/sync/peer/listener.rs`: mutual TLS with a `Paired` client-certificate verifier | The companion API is served by the same listener, with phone certificates in their own table |
| `core/sync/peer/mdns.rs` | The phone finds a computer showing a code on the LAN |
| `core/hooks.rs`: `run_ask` rows, `answer()` updates only an open, unexpired row of a live hook | Answering from the phone goes through `answer()` unchanged |
| `core/terminal.rs`: `reply()` and `stop()` for an owned Windows terminal | Reply and Stop from the phone, in WP-M8 (wave 2); unchanged in wave 1 |
| `hub` + `core/sync/hub.rs`: device records, encrypted chunks, presence, `/v1/events` WebSocket | The phone mailbox is added to the hub |
| `app/src-tauri/src/main.rs`: the session heartbeat runs in the background, not in the window | Requests wait for the phone while the app is in the tray |

**New:** phone enrollment and grants, the companion API, the action contract with receipts, a phone mailbox on the hub,
a push relay service, the desktop "Pair a phone" UI and Phones list, `hrm phones` in the CLI, a small Rust protocol
crate shared with the app, and the Flutter app with its native widgets.

**Must not happen:** a phone never receives the ring key and is never stored in `peers`. Today's computer pairing sends
the ring seed (`payload()` → `ring_for_pairing`) and admits paired certificates to journal and transcript routes. A
phone gets neither.

## Contract: phone enrollment

- Table `phones(phone_id text primary key, name text, platform text, cert_pem text, pubkey text, x25519 text,
  grants text, paired_at integer, last_seen integer, revoked_at integer, relay_handle text)`. `grants` is JSON
  `{"summaries": bool, "transcripts": bool, "control": bool}`.
- The desktop shows **Pair a phone**: a QR code holding `hm-phone:1?h=<host:port>&c=<code>&f=<desktop fingerprint>`
  and the same six digits in plain text. The pairing port lives two minutes and allows three failures, as for computers.
- Routes on the pairing port: `GET /pair/hello`, `POST /pair/phone/start` (SPAKE2, phone is A),
  `POST /pair/phone/finish` (the phone's sealed payload: id, name, platform, cert_pem, pubkey, x25519).
- After `finish` the desktop shows the phone's name, the **three verification words** (derived from the SPAKE2 key on
  both sides) and the grant switches. Nothing is sealed back until the owner approves.
- `GET /pair/phone/result?id=` returns `pending`, or the sealed desktop payload (id, name, cert_pem, pubkey,
  address hints, hub URL and a phone hub token when the hub is on, the granted scopes), or `refused`. The phone polls
  it; the S4 "Waiting for approval" screen shows the time left.
- Revoking a phone sets `revoked_at`, cuts its open connections, deletes its hub token and mailbox, and deregisters
  its relay handle. The Phones list shows which of these were acknowledged.

## Contract: the desktop's phone operations (WP-M1 implements, WP-M2 calls)

```rust
// crates/core/src/phones.rs
pub fn list(ctx: &Ctx) -> Result<Value>;
//   [{phone_id, name, platform, grants, paired_at, last_seen, revoked_at, route: "direct"|"hub"|null,
//     acks?: {connection, hub, push}}]   acks on a revoked phone, as they stand now
pub fn set_grants(ctx: &Ctx, phone_id: &str, grants: &Value) -> Result<Value>;   // the phone's row
pub fn revoke(ctx: &Ctx, phone_id: &str) -> Result<Value>;
//   {phone_id, revoked_at, acks: {connection: bool, hub: bool|null, push: bool|null}}
//   connection: true once the listener holding the phone's connections has cut them (it may run in another process;
//   revoke waits up to 3 s, and a later acknowledgement shows in list)

// crates/core/src/sync/peer/mod.rs, on Runtime
pub fn phone_pair_start(&self, ctx: &Arc<Ctx>, host_name: Option<&str>) -> Result<Value>;
//   {attempt, code, expires_at, host_hint, port, qr, state}   qr = "hm-phone:1?h=<host:port>&c=<code>&f=<fingerprint>"
pub fn phone_pair_status(&self) -> Option<Value>;
//   {attempt, state: "waiting"|"confirm"|"paired"|"refused"|"expired"|"failed", phone?: {name, platform}, words?: [3],
//    error?, code, expires_at, host_hint, port, qr}
pub fn phone_pair_approve(&self, ctx: &Arc<Ctx>, attempt: &str, grants: &Value) -> Result<Value>;  // the status after
pub fn phone_pair_refuse(&self, attempt: &str) -> Result<Value>;
pub fn phone_pair_cancel(&self, attempt: &str) -> Result<Value>;                // null
```

Every status change is also sent as the event `phones-pairing` with the status value, as `computers-pairing` is today.
`attempt` names one showing of a code: approve, refuse and cancel pass the `attempt` of the status the owner is looking
at, and are refused for any other, so a decision taken on one phone's words never lands on another's.

## Contract: companion API (`/c1`, on the peer listener, phone certificates only)

Every answer carries `computer` (id and name), `observed_at` (ms) and, where it applies, `actions` (what this phone may
do now, with the reason when it may not). A route outside the phone's grants answers 403. Peer certificates are
refused on `/c1`, and phone certificates on `/v1`.

| Route | Grant | Returns |
|---|---|---|
| `GET /c1/hello` | any | computer, app version, `api: 1`, grants, server time |
| `GET /c1/overview` | summaries | needs-you count and soonest expiry, running, next reset, today, `use_next` with its reason |
| `GET /c1/accounts`, `/c1/accounts/{profile}` | summaries | windows (left %, reset at), plan, reading age, login attention, 7-day tokens |
| `GET /c1/sessions?cursor=` | summaries | sessions with state, project, account, computer |
| `GET /c1/sessions/{harness}/{profile}/{id}/transcript?after=` | transcripts | transcript pages, bounded |
| `GET /c1/requests` | summaries | open requests: kind, tool, what (command or question), expires, state version |
| `GET /c1/events` | summaries | WebSocket: `needs`, `changed`, `run-state`, `revoked` |
| `POST /c1/actions` | control | a receipt (see below) |
| `GET /c1/actions/{action_id}` | control | the stored receipt, for "delivery unknown" |
| `PUT /c1/push` / `DELETE /c1/push` | any | registers or removes the phone's relay handle |

`use_next` is computed once, in core, from the rule the desktop uses today (`recommendedFor` in `app/ui/home.js` moves
to Rust), so the desktop, the phone and the widgets always agree.

## Contract: actions and receipts

```
POST /c1/actions
{ "action_id": uuid, "kind": "answer",
  "target": { "run": str, "ask_id": str }, "state_version": int, "expires_at": ms,
  "payload": { "behavior": "allow"|"deny" } | { "answers": {...} } }
→ { "action_id", "status": "accepted" | "already_handled" | "expired" | "unavailable" | "unknown", "detail", "at": ms }
```

- **Wave 1 actions are `answer` only.** `reply` and `stop` (kept in `companion-proto` for WP-M8) are `unavailable`
  with the detail "Reply and Stop from a phone are not available yet", and no setting turns them on. Each session's
  `actions` says the same. The desktop's own Reply and Stop and `hrm reply` / `hrm stop` are unchanged.
- The origin rechecks grant, run, request and state version immediately before it acts, serialises actions per run,
  and keeps each receipt 24 h in `phone_actions(action_id primary key, phone_id, kind, target, status, detail, at)`.
  One transaction claims the action: it consumes the state version (a second action on the same screen is `expired`)
  and reserves the receipt before anything reaches the session. `unknown`: the computer stopped between reserving and
  recording the outcome; the answer may have arrived and is never sent again. Where the answer is written (the same
  write that takes the open request) the claimed run version, the action's deadline and the phone's Control are
  checked once more, judged when that write gets the database.
- **A request belongs to the turn that opened it.** The hook writes the request in the same transaction that records
  its permission or question event, bound to the run's turn (a submitted prompt starts a new one); a phone's answer
  to a request from another turn is refused. Where the answer is written, the request must still be open and its hook
  live, the run at the claimed version and turn, both deadlines unpassed, and the enrollment current with Control. A
  hook that cannot record its event fails as it always did; the hook path does no file work beyond 1.13.2's (see the
  release criteria for what a lost event leaves open). The person at the computer answers throughout.
- A repeated `action_id` returns the stored receipt and does nothing again. An answer is never resent automatically.
  When the phone gets no answer, it asks `GET /c1/actions/{id}`, and shows **delivery unknown** if that fails too.
- `answer` goes through `hooks::answer()` with the phone's guard. An accepted answer's receipt reads "Sent to the
  session. If the terminal answered first, nothing changed."
- **Question window:** the wait variable carries 60 when a phone with Control is paired, else 15
  (`QUESTION_SECONDS` becomes a function of the phones table); the hook entry's timeout is then 70 s, 10 s beyond the
  window for the hook's start and for printing an answer taken at the window's end, and 15 s without one, as before.
  A permission request waits at most 300 s whatever `request_seconds` says.

## Contract: the hub, for phones

- `POST /v1/phones/invite` (a joined computer) mints a one-use code for a phone; the phone redeems it with
  `POST /v1/phones/join` and gets a **phone token** that can only reach its own mailbox. The code alone grants nothing:
  the computer still approves the phone (grants and verification words) through the mailbox.
- Mailbox: `PUT/GET /v1/mail/{phone_id}/up/{seq}` (phone → computer), `PUT/GET /v1/mail/{phone_id}/down/{seq}`
  (computer → phone), a 7-day expiry, and a `mail` event on `/v1/events` for the computer side.
- Everything in the mailbox is sealed to the recipient: X25519 + ChaCha20-Poly1305, the sender signing with its
  Ed25519 key. The computer posts snapshots (overview, accounts, requests) after each change, and the phone posts the
  same `/c1/actions` bodies; receipts come back the same way. The hub sees sizes and timing only.

## Contract: the push relay

- A new service, `crates/push-relay` (axum, Docker, next to the hub). It holds the APNs key and the FCM service
  account. It stores `relay_handle → (platform, device token)` and nothing else.
- `POST /v1/register {platform, token}` → `{relay_handle, secret}` (from the phone).
  `POST /v1/wake {relay_handle, kind: "needs"|"live", collapse_id}` signed with HMAC(secret), from the computer
  or the hub. `DELETE /v1/register/{handle}`.
- A wake becomes "A session needs you" (alert) or a Live Activity / data message with no content. The relay keeps no
  log of handles against senders, rate-limits each handle, and drops tokens the providers reject.

## Phases

| Phase | Delivers | Size (one agent) |
|---|---|---|
| 0 Spikes | Rust protocol crate via flutter_rust_bridge on both phones; mutual TLS from Dart with a pinned self-signed Ed25519 certificate; wake → visible → unlocked answer timing for a 60 s question; ActivityKit push update | 3–4 days |
| 1 Desktop | `phones` table, phone pairing with approval and words, `/c1` API, actions and receipts, the 60 s question window, `use_next` in core, desktop S24 UI, `hrm phones list/pair/revoke`, tests with a Rust fake phone | 2 weeks |
| 2 App over LAN / VPN | Flutter app with the Calibre theme: onboarding S1–S6, Home S7, Requests S8–S10, Sessions S11–S13, Accounts S14–S16, Computers S17, You S18–S21, states S22–S23, offline cache, app lock | 3–4 weeks |
| 3 Away and alerts | Hub phone invite and mailbox, the push relay, notification settings, private alerts, quiet hours | 1.5–2 weeks |
| 4 Widgets and game | The chosen widgets, the Live Activity and Dynamic Island, the Android widget and ongoing notification, Progress, badges, the recap image | 2–3 weeks |
| 5 Release | Signing, TestFlight and Play internal testing, store listings and privacy labels, the two end-to-end journeys on real phones | 1 week |

Phases 1 and 2 can overlap once the `/c1` contract is fixed: the app builds against a fake server answering from
`docs/mobile-2026-11/brief/data.json`. Phase 3's relay and phase 4's native widgets are independent of each other.

## Work packages (wave 1: phases 0 and 1)

| Package | Owns (only these paths) | Branch |
|---|---|---|
| WP-M0 spikes | `spikes/mobile-*/**` | `mobile/spikes` |
| WP-M1 desktop core and protocol crate | `crates/core/src/{phones.rs,companion/**,recommend.rs}`, `crates/core/src/sync/peer/{mod.rs,listener.rs,pairing.rs}`, `crates/core/src/{hooks.rs,db.rs}` (phones parts), `crates/companion-proto/**`, root `Cargo.toml` (members) | `mobile/core` |
| WP-M2 desktop UI and CLI | `app/ui/computers*.js` (Pair a phone, Phones list), `app/src-tauri/src/computers.rs`, `crates/cli/src/**` (`hrm phones`), `e2e/` (phone tests) | `mobile/desktop-ui` |

Wave 2: WP-M4 the Flutter app (its own repository), WP-M5 hub mailbox (`crates/hub/**`, `crates/core/src/sync/hub.rs`),
WP-M6 push relay (`crates/push-relay/**`, `deploy/push-relay/**`), WP-M7 native widgets (the widget targets in the app's repository),
WP-M8 phone Reply and Stop.

**WP-M8 phone Reply and Stop** gets its own design before any code; wave 1's five reviews tried to bolt it onto the
answer path and taught these inputs:

- **The check-to-write gap.** Bytes typed into a terminal cannot be checked against the tool's turn at the instant
  they are written: whatever is checked first, a prompt can open (or the turn end) before the bytes land, so a reply
  or a Ctrl+C can reach a different moment than the one the phone was shown.
- **Queue and cancel identity.** Every input needs an identity the relay checks: which Stop claimed which turn, which
  reply is queued, and which one a cancel or a later outcome refers to, so a stale claim never releases or ends a
  newer one and nothing is delivered twice.
- **Turn versioning.** The state version moves on every hook event; a reply or Stop needs the tool's own turn, which
  the hooks report only in part (no event follows an interrupt, a late hook from a stopped turn can arrive after it).
- **Local typing interleaves.** Text the person at the computer is typing, or text a reply left unsent before its
  Enter, mixes with remote input; a keystroke at the terminal settles nothing the desktop can see.
- **Recovery control on the desktop.** An input whose outcome is not known has to be settled by someone: the desktop
  needs a visible control to clear it, rather than a rule that waits for the next prompt.
- **Needs help from the tool.** A reliable remote reply or interrupt wants the harness itself to accept input with a
  turn id (or acknowledge one), rather than inferring the turn from terminal bytes and hook timing.

## Release criteria

- **Zero stale approvals** across: the terminal answering first, two phones at once, expiry during unlock, Wi-Fi to
  cellular, a receipt lost after an answer. (WP-M8 adds zero repeated terminal inputs: a receipt lost after a reply,
  two Stop taps.)
- **Known residual, accepted for wave 1:** if the terminal answers first and that hook event cannot be saved (SQLite
  busy past the hook's 2 s timeout), the request stays open in hm.db and a phone's answer to it may be accepted.
  Claude Code acts only on the first decision for a tool call, so the late one changes nothing (a spike is verifying
  this now); the desktop app behaves the same way in 1.13.2. The receipt says so: "Sent to the session. If the
  terminal answered first, nothing changed."
- A summaries-only phone gets 403 on transcripts, actions and every `/v1` route; two computers with the same account
  and session names stay apart.
- A revoked phone loses its connection, its hub token and its push handle within one minute while the computer is on.
- The two journeys pass on a real iPhone and a real Android phone: find a usable account from a stale Home; receive a
  permission request over cellular, unlock and approve it, and see the computer's receipt.
- Every screen meets the design's own checks: AA contrast in light and dark, 44 pt / 48 dp targets, AX5 text,
  Reduce Motion.
