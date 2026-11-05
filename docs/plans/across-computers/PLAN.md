# Across computers: implementation plan

Decided with the owner on 5 Nov 2026 after two rounds of feedback (the analysis page is
`docs/across-computers-2026-11.html`). This file is the contract that parallel work packages build
against. Change it only by agreement; the sections marked **contract** are what other packages rely on.

## Decisions

- Both ways to sync ship: **a shared folder** (OneDrive, Google Drive, SMB, NAS) in phase 1 and **a hub you
  host** (a VPS running Docker, H3) in phase 5. Each computer picks **one** in Settings; the **direct
  connection** (peers) is an add-on on either, for live routes within a second (phase 2).
- One **append-only journal per computer**, merged into every other computer's database. Usage rows keep
  their `request_id`, so merging never double counts. Cost is **frozen per row** with the price revision it
  was valued at.
- Peers find each other by **mDNS, then VPN names** (the owner runs Pangolin VPN; Tailscale-style names
  work the same); hub rendezvous and relay come with the hub. Pairing is a **six-digit code** (PAKE).
- Transcripts travel (**on** by default, size cap), **not encrypted in the folder** ("the folder is mine"),
  **encrypted through the hub** with a device key unwrapped by a **recovery key kept in a password
  manager**, shown once at setup.
- Resume elsewhere is a **fork**; the original is never written.
- Archive age is a **setting, 30 days by default**; archive and backups live in the sync folder **and** a
  folder of the owner's choice.
- Overview default **All computers**; Activity grid **added up with a per-computer switch**; the day of a
  usage row is taken in **this computer's time zone on every computer**.
- New tools as their own track, alongside phase 1: **Pi** and **Oh My Pi** sharing one parser; **DeepSeek**
  as a router provider now, **DeepSeek Harness** behind a feasibility gate (like Antigravity).

## Phases

| Phase | Delivers | Work packages |
|---|---|---|
| 1 | Journal and merge, folder transport, Computers settings, "All computers" views, Activity switch | WP-3 core, WP-4 UI |
| T | Pi, Oh My Pi, DeepSeek provider | WP-1 tools, WP-2 DeepSeek |
| 2 | Direct connection: identity, pairing, mDNS and VPN names, mutual TLS, pull with cursors and relay, presence every 10 s, live routes with computer dots | later |
| 3 | Resume here: transcripts in the journal, project map, Claude Code by path as a fork, Codex with `-C`, OpenCode export/import; Antigravity read-only | later |
| 4 | Backup and archive: bigger backup, archive age setting, both locations, restore as a merge, "Welcome back" | later |
| 5 | The hub (H3, VPS + Docker; H2 Worker optional) as the second choice in Settings; rendezvous and relay; device keys and recovery key | later |

## Wave 1 (now, in parallel)

| Package | Owns (only these paths) | Branch | Account |
|---|---|---|---|
| WP-3 core journal + folder transport + ops + CLI | `crates/core/src/{journal.rs,sync/**,db.rs,ctx.rs,dash.rs,service.rs,model.rs,backup.rs}`, `crates/cli/src/**`, `app/src-tauri/src/main.rs`, README "Computers" paragraph | `feature/sync-core` | claude-main |
| WP-4 UI for computers and scope | `app/ui/**` (not `app/ui/icons/pi.svg`, `omp.svg`), `e2e/app.e2e.mjs` (sync tests only), `README.md` (screens) | `feature/sync-ui` | samykabu-personal |
| WP-1 Pi + Oh My Pi, then WP-2 DeepSeek provider | `crates/core/src/harness/{pi.rs,omp.rs,mod.rs}`, `crates/core/src/{apps.rs,pricing.rs,registry.rs (presets only)}`, `app/ui/icons/{pi,omp,deepseek}.svg`, `app/ui/*.js` only where a tool or provider list is hard-coded, `e2e/app.e2e.mjs` (tools tests only), fixtures under `crates/core/tests/fixtures/` | `feature/tools-pi-omp-deepseek` | sabushanab-resal |

Collisions to avoid: `main.rs` and `db.rs` belong to WP-3; WP-1 adds nothing there (a new harness registers in
`harness/mod.rs::all()` only). `app.e2e.mjs` is edited by WP-4 and WP-1 in separate, clearly named tests
appended at the end of the file; the orchestrator merges. Shared JS (`core.js`, `lib.js`) is WP-4's; WP-1
touches UI only if a list of tools is hard-coded somewhere (grep first; the README says the UI reads `caps`).

## Contract: settings (per computer, `settings.json`)

```
device_id        string  uuid v4, made once; empty means "make one now"
device_name      string  default: the computer name (hostname)
incarnation      string  uuid v4, made once per fresh database (a restore keeps device_id, new incarnation)
sync_mode        "off" | "folder" | "hub"     default "off"
sync_folder      string  absolute path; used when sync_mode = "folder"
hub_url          string  used when sync_mode = "hub" (phase 5); the token lives in Credential Manager
sync_transcripts bool    default true   (phase 3 uses it; phase 1 only stores it)
sync_presence    bool    default true
direct_connection bool   default false  (phase 2)
archive_days     u32     default 30     (phase 4)
archive_dir      string  default ""     (phase 4; "" = <sync_folder>/archive)
```

## Contract: the folder layout (transport "folder")

```
<sync_folder>/
  devices/<device_id>/device.json        {id, name, os, app_version, incarnation, roots:{<project_key>:<local path>}, updated_at}
  devices/<device_id>/manifest.json      {incarnation, last_seq, chunks:[{file, from, to, sha256, bytes}]}   written atomically (tmp + rename)
  devices/<device_id>/journal/<incarnation>-<from>-<to>.jsonl   immutable once listed in the manifest; <= 1 MB each
  devices/<device_id>/presence.json      {at, app_version, running:[{profile, harness, project, project_name, session_id, cwd, started_at}],
                                          accounts:{<profile>:{tokens_5m, requests_5m}}, tps_5m}   overwritten every 60 s while the app runs
  devices/<device_id>/transcripts/...    phase 3
  archive/...                            phase 4
```

A **journal entry** is one JSON object per line:

```
{"seq": 1205, "ts": 1793886695359, "op": "usage",     "key": "<request_id>", "data": {UsageRow fields + "cost_usd", "price_rev"}}
{"seq": 1206, "ts": ..., "op": "session",  "key": "<harness>/<profile>/<id>", "data": {SessionMeta fields, "cwd", "project_key", "project_name", "source_path"}}
{"seq": 1207, "ts": ..., "op": "quota",    "key": "<profile>/<tier>",        "data": {"used", "resets_at", "at"}}
{"seq": 1208, "ts": ..., "op": "tag",      "key": "<kind>/<key>",            "data": {"tags": [..]}}
{"seq": 1209, "ts": ..., "op": "tombstone","key": "<op>/<key>",              "data": {}}
```

Rules: `seq` is contiguous per incarnation; entries are applied in `seq` order per origin; readers keep a
cursor `(device_id, incarnation, seq)`; a chunk whose sha256 does not match is skipped and retried next
round (OneDrive placeholders: open with read, on failure note "waiting for the sync service" and retry).
`usage` merges with today's upsert rule plus `device`; `session` upserts by key keeping the newest
`last_active_at`; `quota` appends to `quota_hist` when it differs from the last reading; `tag` replaces;
`tombstone` deletes the named session/tag record (never a usage row).

## Contract: the database (WP-3 migrates; WP-4 and WP-1 read through ops only)

```
usage     + device text not null default ''   ('' is migrated to this device_id once), + cost_usd real, + price_rev text
sessions  + device text not null default ''
outbox    (seq integer primary key autoincrement, ts integer, op text, key text, data text)   this computer's unpublished entries
peers     (device_id text primary key, name text, os text, app_version text, incarnation text, last_seen integer,
           last_seq integer, applied_seq integer, roots text, transport text, presence text)
cursors2  (device_id text, incarnation text, seq integer, primary key(device_id, incarnation))
```

"This computer" means **executed here** (`device = this device_id`). "All computers" is every row (unique by
`request_id`). Cache hit is `sum(cache_read) / sum(input + cache_read + cache_write)`, never an average of
percentages. The `day` column is computed in **this computer's local zone from `ts`** on merge, so every
computer buckets the same instant into the same day as this one would.

## Contract: ops (Tauri `call(op, args)`) and CLI

```
computers_status  -> {mode, folder, this:{id, name, incarnation, last_seq, published_seq}, last_run, error,
                      peers:[{id, name, os, app_version, last_seen, last_seq, applied_seq, transport, running:[...]}]}
computers_now     -> runs one publish + merge round now; returns computers_status
computers_set     {device_name?, sync_mode?, sync_folder?, sync_transcripts?, sync_presence?} -> computers_status
home              {hours?, harness?, scope?: "all"|"here"}   rows and tiles carry per-device split: tiles[*].by_device:[{id,name,value}]
                                                            activity: {days:[{day, tokens, by_device:{id:tokens}}]}
sessions          {... scope?}  rows gain device, device_name, open_elsewhere:bool
projects          {... scope?}  rows gain devices:[{id,name,sessions,tokens}], here:bool
live              (restored) adds peers' presence: accounts/providers sums include peers; running rows carry device
hrm computers status | now | set --name <n> --mode off|folder --folder <path> [--transcripts on|off] [--presence on|off]
```

WP-4 may build and test against these shapes with a seeded sync folder: write a second device's
`device.json`, `manifest.json`, one journal chunk and a `presence.json` by hand in the e2e (the e2e already
seeds `.claude.json` and session files), then call `computers_now` and assert the views.

## Contract: a new harness (WP-1)

A module implements `Harness` (see `crates/core/src/harness/mod.rs`) and is added to `all()`. Required:
`id`, `label`, `caps`, `binary`, `default_home`, `dir_prefix`, `launch` (set the tool's config-dir env to the
profile home: `PI_CODING_AGENT_DIR` for both Pi and Oh My Pi), `login`, `identity` (from the tool's
settings or credential file; `needs_login` when none), `import_usage` (JSONL: every `message` entry whose
`message.role == "assistant"` with `message.usage`; for Oh My Pi also `model_usage` entries; `request_id` =
`<harness>:<session id>:<entry id>`; `project` = the session header's `cwd`; cache fields from `cacheRead` /
`cacheWrite`), `sessions` (header `cwd`, first user message as title, counts), `transcript`, `resume_args`
(Pi: `["--session", <path>]`; Oh My Pi: `["--resume", <id>]`), `running` (a process named `pi`/`omp` plus a
session file written in the last two minutes, as Codex does), `item_paths` (skills: `<home>/skills`; MCP:
none unless documented). Fixtures: real-shaped session files under `crates/core/tests/fixtures/{pi,omp}/`.
Icons: `app/ui/icons/pi.svg`, `omp.svg` at the same optical size as `agy.svg` (viewBox with 1.5 padding).
Apps page: npm install/update entries (`@mariozechner/pi-coding-agent`, `@oh-my-pi/pi-coding-agent`).

DeepSeek (WP-2): a provider preset in the router/providers (`https://api.deepseek.com`, format "openai";
and the Anthropic-format endpoint `https://api.deepseek.com/anthropic` for Claude Code routes), models
`deepseek-flash` and `deepseek-v4-pro`, prices in the pricing table (list prices from api-docs.deepseek.com
at the time of writing, with a comment), cache hit read from `prompt_cache_hit_tokens` /
`prompt_cache_miss_tokens` where the router logs usage. DeepSeek Harness: no code; a note in `apps.rs` or
the README that it is gated until it documents its session store.

## Definition of done, every package

- `cargo build --workspace` and `cargo test --workspace` green; `cargo clippy` no new warnings.
- UI changes: `cargo build --workspace` **before** `cd e2e && node app.e2e.mjs` (the exe embeds the UI), and the
  whole suite green, screenshots looked at for the new screens.
- Line endings: UI and e2e files CRLF, Rust/README/JSON LF (write with `newline` set).
- Commits on the package's branch with clear messages; **no push**; a `REPORT.md` at the worktree root:
  what was built, what was skipped, test results verbatim (counts), and anything the merge must know.
- Nothing outside the package's paths is edited without saying so in `REPORT.md`.


---

## Phase 2: the direct connection (contract, 5 Nov 2026)

Delivers: each computer has an identity (a key pair and a self-signed certificate), two computers pair once
with a six-digit code, they find each other by mDNS on the same network or by an address (the owner's VPN
is Pangolin, so a stable host name or IP per computer), talk over mutual TLS, pull each other's journal
chunks with cursors, relay chunks of computers they both know, and share presence every ten seconds. The
folder transport keeps working underneath; a computer reachable neither directly nor through a relay is
still merged through the folder, just not live.

| Package | Owns (only these paths) | Branch | Account |
|---|---|---|---|
| WP-5 peer core: identity, pairing, discovery, TLS listener and client, pull and relay, presence, ops, CLI, Rust tests | `crates/core/src/sync/**` (new `peer/` modules), `crates/core/src/{journal.rs,db.rs,ctx.rs,secrets.rs,dash.rs,service.rs}`, `crates/core/Cargo.toml`, `crates/cli/**`, `app/src-tauri/src/main.rs`, `app/src-tauri/Cargo.toml`, `Cargo.lock`, README "Computers" paragraph | `feature/peers-core` | claude-main |
| WP-6 UI: Direct connection switch, Pair dialog, computer cards with transport and address, Forget, chip; e2e with `hrm computers pair` as the second peer | `app/ui/**`, `e2e/app.e2e.mjs` (appended tests, one named block), README screens | `feature/peers-ui` | samykabu-personal |

Suggested crates (the core package verifies they build together with axum 0.8 / tokio / reqwest-rustls
already in the workspace): `rcgen` (self-signed certificate from an Ed25519 key), `rustls` +
`axum-server` with `tls-rustls` (the listener with a client-certificate verifier that accepts only paired
certificates), `reqwest` with `rustls-tls` and `identity` for the client side (pin the peer's certificate
as the only root), `ed25519-dalek` (chunk signatures), `spake2` (the code exchange), `chacha20poly1305` or
`aes-gcm` (the pairing payload), `mdns-sd` (advertise and browse `_hrm._tcp.local.`). Fewer is better;
replace any that does not fit, and say so in the report.

### Identity and storage

- `device_id` and `incarnation` stay as in phase 1. New at first use: an Ed25519 key pair; the private key
  in Windows Credential Manager under `hrm:device-key` (see `secrets.rs`; never in `settings.json`, which
  backups carry), the certificate (self-signed, CN = device_id, 10 years) at `<data>/device.crt`, and
  `pubkey` (base64) in `device.json` and in every `hello`.
- `peers` gains: `address text` (host or host:port the owner or pairing gave; may be empty), `cert_pem text`,
  `pubkey text`, `paired_at integer`, `last_pull integer`, `path text` ("mdns" | "address" | "relay" | ""),
  `connected integer`, `relayed_by text`. A peer with an empty `cert_pem` is folder-only (phase 1) and is
  never connected to.
- Chunks are signed: the manifest's `chunks[]` (folder) and the `/v1/chunks` answer (direct) carry
  `sig` = base64 Ed25519 signature by the origin over the UTF-8 bytes of `"<incarnation>-<from>-<to>:<sha256>"`.
  A chunk whose `sig` does not verify against the origin's `pubkey` is skipped (logged). Received chunks
  are kept under `<data>/journal-cache/<origin>/<file>` so they can be relayed; the cache is pruned to the
  last 50 MB per origin, oldest first, never below the newest chunk.
- Settings: `direct_connection bool` (phase 1 field, now honoured), `listen_port u16` (0 = choose a free
  port once and save it).

### Listener (mutual TLS, only paired certificates), on `listen_port`

```
GET /v1/hello                                   -> {id, name, incarnation, last_seq, app_version, pubkey}
GET /v1/chunks/{origin}?incarnation=X&since=N  -> {origin, incarnation, chunks:[{file, from, to, sha256, sig, bytes}], device: <origin's device.json or null>}
GET /v1/chunk/{origin}/{file}                   -> the chunk bytes (own chunks from the outbox store, others' from journal-cache)
GET /v1/presence                                -> the presence.json shape of the folder contract, computed now
```

`{origin}` may be the server itself or any peer whose chunks it holds (relay). A requester trusts a
relayed origin only if it already has that origin's `pubkey` (paired directly, or carried in the `device`
record the relayer returns and signed by that origin: `device.json` gains `sig` over its own canonical
JSON without the `sig` field).

### Pulling, presence, discovery

- Every refresh tick and every 10 s while `direct_connection` is on: for each paired peer, find it
  (mDNS answer for its id, else `address`), `hello`, then for every origin known (the peer itself and every
  origin in `peers`) pull chunks since the cursor, verify and apply with the phase 1 merge, then fetch
  `/v1/presence` and store it where the folder's `presence.json` goes. `connected` = a `hello` succeeded
  in the last 30 s; `path` = how it was reached.
- mDNS: advertise `_hrm._tcp.local.` with TXT `id`, `name`, `port`, `pairing` (1 while a code is shown).
  Browse continuously while direct connection is on; resolve by `id`.
- Relay: an unreachable peer is pulled from any connected peer that lists it in `/v1/chunks`; `path` =
  "relay" and `relayed_by` = that peer's id.
- The hub (phase 5) will add rendezvous and relay over the internet; nothing here should assume a hub.

### Pairing

- `computers_pair_start` picks a six-digit code (CSPRNG), starts a plain-HTTP pairing route on a second
  ephemeral port for two minutes, advertises `pairing=1` with that port, and returns
  `{code, expires_at, host_hint: "<device_name>:<pairing port>", port}`. `computers_pair_cancel` stops it.
- `computers_pair_join {code, host?}` on the other computer: find the pairing computer by mDNS
  (`pairing=1`), or use `host` (`name:port`); run SPAKE2 with the code as the password (identities: the
  two device ids, the joiner as A), derive a key, exchange encrypted payloads `{id, name, incarnation,
  pubkey, cert_pem, address_hint, app_version}` both ways, verify each decrypts, store the peer on both
  sides with `paired_at` and `address` = the host used (empty for mDNS). A wrong code fails at decryption:
  three failures or two minutes end the pairing. The result is the `computers_status` shape; the app emits
  `computers-pairing` with `{ok, peer?, error?}` on both sides.
- `computers_forget {id}` deletes the peer's row and certificate (its merged rows stay); the other side
  learns of it when its next `hello` is refused.
- `computers_set_peer {id, address?, name?}` edits a peer's address (a VPN name) or display name.

### Ops and CLI (added to the phase 1 ones)

```
computers_status: this gains {port, pubkey, pairing: {code, expires_at, host_hint} | null};
                  peers gain {address, path, connected, last_pull, paired_at, relayed_by, direct: bool}
computers_set: + direct_connection, listen_port
computers_pair_start | computers_pair_cancel | computers_pair_join {code, host?} | computers_forget {id} | computers_set_peer {id, address?, name?}
hrm computers pair [--host]            prints the code and the host hint, waits up to two minutes, prints the peer's name on success
hrm computers join <code> [--host <name:port>]
hrm computers forget <id or name>
hrm computers peers [--json]
hrm computers serve --seconds <n>      runs the listener (and mDNS) for n seconds without the app: for tests and headless use
```

### Tests

- Rust, in `crates/core` (two or three `Ctx` in temp folders, loopback, random ports): pairing with the
  right code stores both peers; a wrong code fails and stores nothing; after pairing A's chunks reach B
  over TLS and apply once; B's presence reaches A within a poll; a connection from an unpaired certificate
  is refused; `forget` makes the next `hello` fail; C unreachable by A is pulled through B (relay) with its
  signature verified; a tampered chunk is skipped.
- Window (WP-6, appended under `// ---- direct connection (WP-6) ----`): start `hrm computers pair` in a
  second data folder as a child process (read the code and host from its stdout), turn Direct connection on
  in Settings, open Pair a computer, enter the code and host, assert the card shows "direct" and connected,
  that a session seeded in the second folder's profile appears in Sessions after Sync now, and that Forget
  removes the card. The second data folder reuses the e2e's env with its own `HRM_HOME`.

### Definition of done

As phase 1 (build, `cargo test --workspace`, clippy, line endings, commits on the branch, no push,
`REPORT.md` with verbatim test lines, the exact op shapes, and the merge notes). The core package must
also report the chosen crates and their licences, and the Windows firewall behaviour it observed.
