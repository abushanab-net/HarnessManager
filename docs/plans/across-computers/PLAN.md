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


---

## Phase 3: resume here (contract, 5 Nov 2026; runs in parallel with phase 2)

Delivers: a session from another computer can be read here and resumed here as a fork, in this computer's
clone of the same repository, with the original untouched. Transcripts travel through the folder transport
in this phase; the direct-connection route for them (`GET /v1/transcript/...`) is a small follow-up after
phase 2 merges, so nothing here touches `crates/core/src/sync/peer/`.

| Package | Owns (only these paths) | Branch | Account |
|---|---|---|---|
| WP-7 resume here, core and UI | new `crates/core/src/transcripts.rs` and `crates/core/src/sync/transcripts.rs`; `crates/core/src/harness/{mod,claude,codex,opencode,pi,omp,agy}.rs` (a new trait method and `resume_args` only); `crates/core/src/launch.rs`; `crates/core/src/service.rs` (new functions only, plus the remote-transcript read); `crates/core/src/journal.rs` (one publish hook, one merge hook, both small and at the end of the existing functions); `app/src-tauri/src/main.rs` (new ops only); `crates/cli/src/main.rs` (new subcommands only); `app/ui/accounts.js` (`sessionRow` and the resume actions), the session-reading dialog wherever it lives, a new `app/ui/resume.js` for the dialog; `e2e/app.e2e.mjs` (one appended block); fixtures under `crates/core/tests/fixtures/` | `feature/resume-here` | sabushanab-resal |

Phase 2's packages own `journal.rs`'s signing and the peer modules, `db.rs`, the Computers section of
`views.js` and `computers.js`. Do not edit those regions; a new table or column needed here goes in
`transcripts.rs` through `ctx.db.with(...)` with its own `create table if not exists`.

### Transcripts in the folder

- On every publish round, with `sync_transcripts` on: for each of this computer's sessions whose source
  file changed since it was last carried, copy the source compressed with zstd (the workspace already reads
  `.zst`) to `devices/<id>/transcripts/<harness>/<profile>/<session id>.<ext>.zst` (Claude Code and Pi /
  Oh My Pi: the `.jsonl`; Codex: the rollout, plus its sidecar files if any; OpenCode: the JSON from
  `opencode session export <id>` or `opencode export <id>`, whichever the installed version has;
  Antigravity: nothing), and record a journal entry `op: "transcript", key: "<harness>/<profile>/<id>",
  data: {file, bytes, sha256, mtime, source_rel}` (`source_rel` = the path relative to the tool's home,
  so the receiver knows where the tool would look for it). Caps: a transcript over 20 MB is skipped
  (logged, `data.skipped: "too big"`), and the folder holds at most 200 MB per computer: the oldest
  transcripts by `mtime` are deleted first (their journal entries get `op: "tombstone"`).
- On merge, a remote transcript entry is stored (table `transcripts(device, key, file, bytes, sha256,
  mtime, source_rel, fetched_at)`), but the file is fetched only when needed: reading the session here, or
  resuming it. Fetched copies live under `<data>/transcripts/<origin>/<harness>/<profile>/` and are kept
  until the entry is tombstoned.
- `session` (the transcript-reading op) for a row whose `device` is not this computer reads the fetched copy
  (fetching first, with the status text "fetching from the folder" surfaced to the UI as a toast), through
  the same parser the tool's own sessions use.

### Materializing and resuming

A new `Harness` method, with a default that returns "resuming elsewhere is not available for <tool>":

```
fn materialize(&self, ctx, p: &Profile, local_root: &Path, t: &FetchedTranscript) -> Result<Materialized>
   // Materialized { launch_args: Vec<String>, cwd: PathBuf, new_session_id: Option<String>, note: String }
```

- Claude Code: write one copy to `<home>/projects/<cwd encoded the Claude way: [\\/:] -> "-">/<id>.jsonl`
  under this computer's `local_root`; never two copies (if one exists, reuse it); launch
  `claude --resume <absolute path> --fork-session` in `local_root`. Do not rewrite the file's contents.
- Codex: copy the rollout, with its original file name, under `<home>/sessions/YYYY/MM/DD/` taken from
  the name; launch `codex -C <local_root> resume <id>`.
- OpenCode: `opencode session import <file> --directory <local_root>` if the installed version has it,
  else `opencode import <file>`; launch `opencode --session <id>` in `local_root`.
- Pi / Oh My Pi: copy into `<home>/sessions/<cwd encoded the tool's way>/<original name>`; launch
  `pi --session <path>` / `omp --resume <id>` in `local_root`.
- Antigravity: the default (not available).

`session_resume_here {device, harness, profile, id}`: the project's `local_root` here is the root this
computer knows for the session's `project_key` (this computer's `device.json` roots, or `projects`); if
none, the error is "not cloned here: open <project> once on this computer"; if no profile of that name and
tool exists here, "no account <profile> for <tool> on this computer"; if the session is `open_elsewhere`,
"running on <name>: wait until it ends". Otherwise fetch the transcript, materialize, and launch through
the existing terminal launcher (what `resume` does today), with `Materialized.launch_args`. Record the
fork: `sessions` of this computer gets, once the tool writes its new file, nothing special; instead the
materialized copy's key is written to `kv` as `forked:<harness>/<profile>/<id>` = `{from: <device>, at}`,
and `sessions` rows whose id matches a forked copy carry `continued_from: <device name>` in the op answer.
A second "Resume here" on the same session reuses the copy and launches again.

### Ops and CLI

```
session            {profile, id, device?}      -> as today; with device = another computer it reads the fetched transcript
session_resume_here {device, harness, profile, id} -> {ok, cwd, args, note} | error text above
session_fetch      {device, harness, profile, id} -> {path, bytes}   (fetch without launching: the Read action)
transcripts_status  -> {carried: n, bytes, cap_bytes, skipped: [{key, why}], remote: n, fetched: n}
hrm resume-here <computer>/<harness>/<profile>/<id>     (computer = name or id)
hrm transcripts [--json]
```

`sessions` rows from another computer gain `resumable: true | "<reason>"` (the three error texts, or
`true`), computed cheaply from the roots and profiles known here.

### UI

- Sessions list: a row from another computer shows **Resume here** when `resumable === true`, else the
  reason in muted text ("not cloned here", "no account work here", "running on LAPTOP"). Reading such a
  session (the title click) fetches then opens the transcript as for a local one, with the "fetching from
  the folder" toast.
- The Resume-here dialog (per the mock in the analysis page): the session's title, project, account and
  tool, where it last ran and when, the local folder it opens in, "opens as a new session that continues
  from the <name> transcript; the <name> copy stays as it is", "only the conversation comes across: files
  changed since, attachments and running shells do not; pull the branch first", Cancel / Resume here.
- A session that was continued from another computer shows "continued from LAPTOP" under its title.
- Settings › Computers: the Carry transcripts row's info popover states the caps and the "carried N ·
  M MB" count from `transcripts_status` (a one-line addition inside the existing row; the Computers section
  itself belongs to phase 2).

### Tests

- Rust: transcripts are carried on publish (zstd, sha256, journal entry), skipped over the cap, pruned at
  200 MB with tombstones; a remote entry is stored on merge and fetched on demand once; `materialize` for
  Claude Code (encoded folder name on Windows and Linux paths, one copy only), Codex (date folder from the
  name), Pi and Oh My Pi (encoded folder), OpenCode (the import command chosen), Antigravity (the default
  error); `session_resume_here` errors for the three cases, and its launch args for each tool.
- Window (appended under `// ---- resume here (WP-7) ----`): seed a second computer's journal with a
  session entry and a transcript entry plus the compressed transcript file in the sync folder (the
  phase 1 test shows how to seed a device; copy its helper, do not change it), with the project key of the
  sandbox's repository; Sync now; the row shows Resume here; reading it shows the transcript text; Resume
  here launches the fake `claude` with `--resume <path ending in the id>.jsonl --fork-session` (the sandbox
  records launches in `runs.jsonl`, see how other tests read it) in the repository folder; a row whose
  project is not here shows "not cloned here".

### Definition of done

As phase 1: build, `cargo test --workspace`, clippy, the whole window suite green (`cargo build
--workspace` first), line endings, commits on the branch, no push, `REPORT.md` with verbatim test lines,
the op shapes, every line touched in shared files, and the merge notes.


---

## Phase 4: backup, archive and a fresh computer (contract, 5 Nov 2026)

Delivers: a backup that also holds the history, a restore that merges it, an archive that moves old
transcripts out of the tools' folders while sessions stay listed and readable, both the sync folder and a
folder of the owner's choice as places for archive and backups, and "Welcome back" on a fresh install.

| Package | Owns (only these paths) | Branch | Account |
|---|---|---|---|
| WP-8 backup, archive, restore, Welcome back; core and UI | `crates/core/src/backup.rs`, new `crates/core/src/archive.rs` and `crates/core/src/restore.rs`, `crates/core/src/ctx.rs` (settings fields only), `crates/core/src/service.rs` (archived sessions in the list: one function and one call), `crates/core/src/harness/mod.rs` (one trait method `archive_paths` with a default), harness modules (that method only), `crates/cli/src/main.rs` (new subcommands and flags only), `app/src-tauri/src/main.rs` (new ops only, appended), `app/ui/views.js` (the Backups section only, renamed "Backups and archive", and the Welcome back dialog), new `app/ui/restore.js`, `app/ui/app.css` (appended), `e2e/app.e2e.mjs` (one appended block), README | `feature/backup-archive` | samykabu-personal |

### Settings (per computer)

```
archive_days           u32   default 30; 0 = never
archive_in_sync_folder bool  default true   (archive copies go to <sync_folder>/archive/<device_id>/ when a folder is set)
archive_dir            string default ""    (a folder of the owner's choice; "" = none; both may be on)
backup_in_sync_folder  bool  default true   (every backup zip is also copied to <sync_folder>/backups/<device_id>/, keeping backup_keep of them)
backup_transcripts     bool  default false  (the zip also holds this computer's transcripts, compressed)
```

### The backup grows

The zip adds `history/usage.jsonl` (every usage row with `device`, `cost_usd`, `price_rev`),
`history/sessions.jsonl` (the session rows: cache, mirror and merged, each with its `device`),
`history/quota_hist.jsonl`, `history/tags.jsonl`, `history/peers.json` (names, ids, certificates and
addresses, never keys), `history/device.json` (this computer's id, name, incarnation, pubkey), and, with
`backup_transcripts`, `transcripts/<harness>/<profile>/<id>.<ext>.zst`. Keys and logins stay out, as today.
`hrm backup --with-transcripts`.

### Restore is a merge

`restore_preview {zip | folder}` reads a zip, or a sync folder (its `devices/*` journals and
`backups/*`), and answers counts: computers, requests, tokens, sessions, accounts with limit history,
archived transcripts, the newest date, and whether a device record carries this computer's name.
`restore_run {zip | folder, adopt: bool}` merges: usage by `request_id` (the existing rule), sessions by key
(newest `last_active_at`), quota points when they differ from the last, tags only where this computer has
none for that key, peers added when unknown. With `adopt`, this computer takes the restored device id and
name (new incarnation, so its sequence starts again and the others see a continuation); without it, it keeps
its own id and the restored computer stays a separate past computer. If the restored id is already a live
peer (another computer answers to it), `adopt` is refused with "another computer uses this identity".
Restoring twice adds nothing. `hrm restore <zip|folder> [--adopt]`.

### The archive

Every day (and `archive_now`): for each session of this computer older than `archive_days` by
`last_active_at`, not open, with a transcript the tool keeps as a file (Claude Code, Codex, Pi, Oh My Pi;
OpenCode through its export; Antigravity never): compress it to `<archive root>/<project name or
"no-folder">/<YYYY-MM>/<harness>-<profile>-<id>.<ext>.zst` plus `.json` with the session's meta, in every
enabled archive location, verify the sha256 of each copy, then delete the original from the tool's folder
(the `archive_paths` trait method says which files belong to a session; the default returns the source file
alone). The session row keeps its meta with `archived: true` and `source_path` pointing at the first archive
copy; `sessions()` lists archived sessions from the table (they no longer come from the scan), `transcript`
reads the archived copy, and `resume` on an archived session says "archived: it can be read, not resumed"
(materializing it back is not in this phase). Usage rows are untouched. `archive_status` answers counts and
bytes per location; `hrm archive status | now`.

### Welcome back

When a sync folder is chosen (the phase 1 chooser) or at first launch when `sync_folder` is already set and
`journal:backfilled` is absent, the app calls `restore_preview {folder}`; if the folder holds any history, the
dialog "Welcome back" shows the counts and "Restore and continue as <name>" (adopt, when a device record with
this computer's name exists) / "Restore as a new computer" / "Start fresh". The same preview and dialog run
from Settings › Backups and archive › Restore… for a zip (the list of this computer's backups and the sync
folder's) or a folder.

### Ops, CLI, UI

```
restore_preview {zip?, folder?}            restore_run {zip?, folder?, adopt}
archive_status                              archive_now
backups (existing) gains {in_sync_folder:[...]}; backup_create gains {transcripts: bool}
hrm backup [--with-transcripts] · hrm restore <zip|folder> [--adopt] · hrm archive status | now
```

Settings › Backups and archive: Automatic backups (existing) + "Also in the sync folder" switch + "Include
transcripts" switch; "Archive sessions older than" segment (Never · 30 days · 90 days · 1 year) with a small
"custom" input; "Archive location": the sync folder switch and the chosen folder with Choose…; "Archive now"
with the status line; Restore… opening the dialog above. Sessions: an "archived" pill on the row, readable,
no Resume. The Welcome back dialog.

### Tests

Rust: the zip holds the history files; a restore into an empty database reproduces the counts and a second
restore adds nothing; adopt takes the id and a new incarnation, and is refused for a live peer's id;
the archive moves only old, closed sessions, verifies before deleting, writes both locations, keeps the
session listed and readable, and respects `archive_days = 0`. Window (appended under
`// ---- backup, archive and welcome back (WP-8) ----`): the section's rows; `hrm archive now` with a tiny
age on a seeded old session shows the pill and the transcript still reads; Restore… from a backup zip made
in the test shows the preview and merges; choosing a sync folder seeded with another computer's journal (the
phase 1 helper) shows Welcome back and restoring lists its sessions.


---

## Phase 5: the hub (contract, 5 Nov 2026)

Delivers: "A hub you host" as the second choice in Settings: a small service on a VPS behind Caddy, in
Docker, that every computer pushes its journal to and pulls the others' from, with presence fan-out, a
rendezvous for direct connections across networks, encrypted transcripts, and a recovery key. The folder
and the direct connection keep working as before; a computer uses one of folder or hub, plus direct.

| Package | Owns (only these paths) | Branch | Account |
|---|---|---|---|
| WP-9a the hub service and its deployment | new crate `crates/hub` (binary `hrm-hub`), `deploy/hub/**` (compose, Caddyfile, env example, README), a workflow job that builds and pushes the image on tag (self-hosted runners), `Cargo.toml` workspace members | `feature/hub-service` | sabushanab-resal |
| WP-9b the client transport, the ring key, the UI | new `crates/core/src/sync/hub.rs`, `crates/core/src/sync/peer/**` (the transcript route follow-up only), `crates/core/src/journal.rs` (the hub branch of `round`), `crates/core/src/secrets.rs`, `ctx.rs` (settings), `app/src-tauri/src/main.rs` (ops only), `crates/cli/src/main.rs` (subcommands only), `app/ui/computers.js`, the Computers section of `app/ui/views.js`, `app/ui/app.css` (appended), `e2e/app.e2e.mjs` (one appended block), README | `feature/hub-client` | claude-main |

### The service (WP-9a)

One Rust binary, axum, plain HTTP on `0.0.0.0:8787` inside the container (Caddy terminates TLS with
Let's Encrypt on 443 and proxies to it), SQLite on the volume at `/data/hub.db` (one owner, a few
computers: SQLite is enough and has nothing to operate; the diagram's Postgres is not needed), blobs on
the volume under `/data/blobs/<origin>/`. Configuration by environment: `HUB_ADMIN_TOKEN` (bootstraps the
first computer), `HUB_DATA=/data`, `HUB_MAX_BLOB_MB=64`, `HUB_RETAIN_MB=2048` (oldest chunks beyond this are
dropped per origin, never the newest). Every request carries `Authorization: Bearer <token>`; tokens are
per computer, minted at join, stored hashed.

```
POST /v1/join                 {code | admin_token, device: <signed device.json>}  -> {token, devices:[...]}
POST /v1/invite               -> {code, expires_at}        a one-time six-digit code valid 10 minutes, made by a joined computer
GET  /v1/devices              -> [<device.json records, with their last_seen and last_seq>]
PUT  /v1/devices/{id}         <signed device.json>         own only (the token's id)
GET  /v1/chunks/{origin}?since=N  -> {origin, incarnation, chunks:[{file, from, to, sha256, sig, bytes, enc:true}]}
PUT  /v1/chunks/{origin}/{file}   body = the encrypted chunk; headers X-From, X-To, X-Sha256 (of the plaintext), X-Sig; own origin only
GET  /v1/chunk/{origin}/{file}    -> the bytes
PUT  /v1/presence             <the presence.json shape, encrypted>     GET /v1/presence -> {<id>: {at, blob}}
PUT  /v1/transcripts/{origin}/{key}   encrypted bytes + X-Sha256 + X-Bytes;   GET the same; DELETE own only
GET  /v1/rendezvous           -> {<id>: {addresses:[...], at}}   PUT /v1/rendezvous {addresses:[...]} own only
GET  /v1/events  (WebSocket)  -> one message per change: {kind: "presence"|"chunks"|"device", id}
DELETE /v1/devices/{id}       own, or admin token: forgets a computer and its token
```

The hub never sees a key: chunks, presence and transcripts arrive encrypted (see the ring key). It stores
sizes, times and the signed device records, which are plaintext by design (names, ids, pubkeys, roots).
`deploy/hub/`: `docker-compose.yml` (caddy + hrm-hub + two named volumes), `Caddyfile` (one site,
`reverse_proxy hrm-hub:8787`), `.env.example` (`HUB_DOMAIN`, `HUB_ADMIN_TOKEN`), `README.md` (a VPS in
ten lines: DNS, compose up, the admin token, the first join), a `Dockerfile` (multi-stage, distroless or
alpine). The workflow job builds the image on the self-hosted runners and pushes `ghcr.io/abushanab-net/
hrm-hub:<tag>` on a release tag. Tests: the service in-process (axum `oneshot` or a bound port): join with
the admin token then with an invite code, a wrong code refused, own-origin checks, retention, the WebSocket
event on a chunk put.

### The client, the ring key and the UI (WP-9b)

- **Ring key**: a 32-byte symmetric key, made on the first computer that turns the hub on, stored in
  Credential Manager as `hrm:ring-key`, carried to other computers inside the phase 2 pairing payload (so
  paired computers already share it) or typed as the **recovery key** (the key as 8 groups of 4 base32
  characters, shown once in a dialog with "I saved it in my password manager" to confirm; "Show again" is
  available in Settings behind a confirm). Chunks, presence and transcripts going to the hub are
  ChaCha20-Poly1305 with a random nonce prefixed; the folder stays plain, as decided.
- **Transport**: `sync_mode = "hub"`, `hub_url`, the token in Credential Manager (`hrm:hub-token`). The
  round: push unpublished chunks (encrypted) and the signed device record; pull every device's chunks since
  the cursor (decrypt, verify, apply with the phase 1 merge); push and pull presence; keep the WebSocket
  open while the app runs and run a pull on each event (fallback: poll every 30 s). Transcripts: carried to
  the hub like to the folder (phase 3's carry gets a second destination); fetched on demand.
- **Rendezvous**: with direct connection on and the hub on, the computer publishes its reachable addresses
  (LAN IPs with the listen port, plus `address` if the owner set one) and tries the others' before mDNS;
  `path` = "rendezvous" when reached that way. Hole punching is not attempted: a computer that cannot reach
  a peer directly pulls through the hub, which is the relay.
- **The transcript route over the direct connection** (phase 2's follow-up): `GET
  /v1/transcript/{origin}/{harness}/{profile}/{id}` on the peer listener, served from the fetched copies and
  own files; phase 3's fetch tries a connected peer before the folder or the hub.
- **Ops and CLI**: `computers_set {sync_mode: "hub", hub_url}`; `hub_join {url, code | admin_token}`;
  `hub_invite` -> {code, expires_at}; `hub_status` -> {url, joined, devices, last_push, last_pull, ws:
  bool, error}; `ring_key_show` (behind the UI's confirm) and `ring_key_enter {key}`; `hrm hub join <url>
  --code <c> | --admin <t>`, `hrm hub invite`, `hrm hub status`, `hrm ring show | enter`.
- **UI**: the "A hub you host" card becomes live: address field, "Join with a code…" (dialog: the code
  from another computer's "Invite a computer…" button on its card header, or the admin token for the first
  computer), status line (joined · N computers · live); the recovery key dialog at first hub use; "Enter
  recovery key" on a computer that has no ring key; the computer cards' transport line gains "hub ·
  connected" and "rendezvous"; Settings › Computers › Invite a computer…. The first-run "Add this computer
  to your others" dialog (phase 1 mock) offers both cards.

### Tests

WP-9b Rust: with the hub crate started in-process from its library (WP-9a exposes `hub::serve(config) ->
addr`; agree on this signature: `pub async fn serve(cfg: hub::Config) -> Result<(SocketAddr, Handle)>`),
two `Ctx` join, A pushes and B pulls and applies once, presence round-trips, a tampered chunk is skipped,
a computer without the ring key cannot read and gets "enter the recovery key", rendezvous addresses flow.
Window (appended under `// ---- the hub (WP-9) ----`): start `hrm-hub` as a child process on a free port
with an admin token, join from the Settings card, see "joined", seed a second computer by joining with
`hrm hub join` from a second data folder and pushing its journal (`hrm computers now`), Sync now shows its
sessions; the recovery key dialog appears once; Invite yields a code that the second folder can join with.
WP-9a's tests are its own; both packages build the whole workspace.
