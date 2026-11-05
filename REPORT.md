# WP-5 report: the direct connection, core

Branch `feature/peers-core`, five commits on top of `26ec971` (release 1.4.0 plus the phase 2 contract). Nothing
pushed. Nothing outside the package's paths was edited: `e2e/app.e2e.mjs` and `app/ui/**` are untouched, the
window e2e was not run (WP-6 owns it).

## What was built, by file

- `crates/core/Cargo.toml`, `Cargo.lock`: the crates below.
- `crates/core/src/sync/peer/identity.rs` (new): the Ed25519 device key, its seed hex in the credential store
  under `<prefix>:device-key:<device_id>` (`HarnessManager:device-key:…` for the app, the `HRM_CRED_PREFIX`
  namespace in tests); the self-signed certificate at `<data>/device.crt` (CN = device id, serial 1, valid
  2026-01-01 to 2036-01-01: with Ed25519's deterministic signatures the certificate is the same bytes whenever it
  is made from the key, so a lost `device.crt` does not break pairings); `pubkey` kept in kv `device:pubkey`;
  `sign`, `verify`, `chunk_message` (`"<incarnation>-<from>-<to>:<sha256>"`), `sign_record` / `record_ok` for
  `device.json` (`sig` over the record's compact JSON without `sig`, keys in the order written: serde_json keeps
  the order through a parse, so a reader re-serializes exactly what the signer signed).
- `crates/core/src/sync/peer/store.rs` (new): the local journal store `<data>/journal-cache/<origin>/`
  (`manifest.json`, `journal/<file>`, `device.json`) for this computer's own chunks and every received one;
  other origins pruned to 50 MB, oldest first, never below the newest chunk; a new incarnation starts the list over.
- `crates/core/src/sync/peer/listener.rs` (new): axum-server over rustls (aws-lc-rs provider, the one reqwest
  already uses in this workspace) with a `ClientCertVerifier` that reads `peers.cert_pem` at each handshake and
  accepts only those certificates; the four routes `GET /v1/hello` (plus `origins`: the ids it holds chunks of),
  `GET /v1/chunks/{origin}?incarnation&since` (plus `last_seq`; another incarnation answers with no chunks and
  its own name), `GET /v1/chunk/{origin}/{file}`, `GET /v1/presence`. Handlers run on blocking threads.
- `crates/core/src/sync/peer/client.rs` (new): `reqwest::blocking` with a preconfigured rustls `ClientConfig`:
  our certificate as the client identity, the peer's certificate pinned by byte equality as the only one
  accepted (so a name or IP that changes does not matter); `pull_all`: find each paired peer (mDNS answer, else
  `address`), `hello`, its chunks since the cursor, its presence; then every origin known (the `peers` table and
  the peers' `origins`) that is not itself connected, through any connected peer that holds it (`path =
  "relay"`, `relayed_by`).
- `crates/core/src/sync/peer/pairing.rs` (new): `pair_start` (a six-digit code from the OS CSPRNG, a plain-HTTP
  route on a free port for two minutes, kv `pairing:current` for the status), SPAKE2 (`Ed25519Group`, the code
  as the password, identities the two device ids, the joiner as A: `GET /pair/hello` gives the joiner the host's
  id first), ChaCha20-Poly1305 payloads `{id, name, os, incarnation, pubkey, cert_pem, address_hint,
  app_version}`, three failures or two minutes end it, `store_peer` on both sides, the `computers-pairing`
  callback with `{ok, peer?, error?}` on the host; `join` on the other computer.
- `crates/core/src/sync/peer/mdns.rs` (new): `mdns-sd` advertising `_hrm._tcp.local.` with TXT `id`, `name`,
  `port`, `pairing` (1 while a code is shown) and `pairing_port`; a browse thread keeps what it finds by id.
- `crates/core/src/sync/peer/mod.rs` (new): `Runtime` (its own tokio runtime, the listener, the mDNS daemon, a
  pairing in progress, the event callback): `ensure` (listener and mDNS follow the setting; a port of 0 is
  taken once and saved), `tick` (every 10 s: publish what is new here locally, `pull_all`), `pair_start`,
  `pair_cancel`, `pair_join`, `pair_result`, `stop`; `forget`, `set_peer`.
- `crates/core/src/sync/peer/tests.rs` (new): the eight tests of the contract.
- `crates/core/src/journal.rs`: `publish` writes signed chunks into the local store (its manifest is the water
  line; kv `journal:published_seq` follows it for the status); `mirror` copies missing chunks and the manifest to
  the sync folder; `apply_manifest(ctx, id, manifest, read)` takes a reader (the folder or a peer), verifies
  sha256 and, when the origin's key is known, the signature, keeps the chunk in the store for relay, and applies;
  `learn_device` takes a self-signed `device.json` (folder or peer), learns the key once and never replaces it;
  `on(ctx)` (folder or direct); `round` publishes, mirrors and merges through the folder when in folder mode and
  pulls the peers when the direct connection is on; `status` gains the new fields; `configure` takes
  `direct_connection` and `listen_port`; `Peer` carries the new columns.
- `crates/core/src/sync/folder.rs`: `device_record(ctx)` (signed, with `pubkey` and `sig`), used by
  `write_device` and by `/v1/chunks` for this computer.
- `crates/core/src/db.rs`: `peers` gains `address, port, cert_pem, pubkey, paired_at, last_pull, path,
  connected, relayed_by` (added when missing).
- `crates/core/src/ctx.rs`: `listen_port`; the journal records when `sync_mode != "off" || direct_connection`.
- `crates/cli/src/main.rs`: `hrm computers pair [--host [name]] | join <code> [--host name:port] | forget <id or
  name> | peers [--json] | serve --seconds <n>`, `set --direct on|off --port <n>`.
- `app/src-tauri/src/main.rs`: the runtime in the app state (bound to every interface), a thread ticking it every
  10 s and emitting `computers` while on; `computers_set` starts or stops the listener at once;
  `computers_pair_start`, `computers_pair_cancel`, `computers_pair_join`, `computers_forget`,
  `computers_set_peer` (all in `READS`); the refresh loop runs a round whenever sync is on in any form; the
  window gets `computers-pairing`.
- `README.md`: the "Computers" paragraph gains the direct connection, pairing, mDNS, the VPN address and the CLI.

## Decisions where the contract was silent

- **The key's credential name carries the device id** (`<prefix>:device-key:<device_id>`, not a bare
  `hrm:device-key`): two data folders on one computer (the tests, the e2e's second folder, `HRM_HOME`) would
  otherwise share one key.
- **A platform without a credential store** (the non-Windows stub in `secrets.rs`) keeps the seed in
  `<data>/device.key` with a warning in the log; without this, folder sync would stop there, since publishing
  now signs. Marked as a shortcut in the code.
- **One local journal store for everything.** This computer's chunks are cut once, into
  `<data>/journal-cache/<own id>/`, signed, and the folder is a mirror of that store (chunks the folder lacks
  are copied, the manifest written when it differs). "Own chunks from the outbox store" in the contract is this
  store; it is never pruned. Every chunk received, from the folder or a peer, is kept there too, so the folder's
  peers can be relayed as well.
- **Signature rule in the merge:** when the origin's key is known (paired, or learned from a signed
  `device.json`), a chunk without a valid signature stops that origin's merge for this round, exactly like a
  sha256 mismatch (order matters; it is retried next round and never skipped over). A folder peer whose key is
  not known yet (a computer still on 1.4.0) is merged as in phase 1: the folder is the owner's.
- **A relayed origin is learned from its signed record**, as the contract allows, and gets a `peers` row with
  `transport = "relay"`, its `pubkey` and no certificate, so it is never connected to; the key is learned once and
  a record under another key is ignored and logged (tested).
- **`connected`** is stored as the time of the last `hello` that answered; the status reports
  `now - connected < 30 s`. `path` keeps the last way it was reached.
- **`hello` lists `origins`**: without it a computer could not know which origins a peer can relay (the contract's
  `/v1/chunks` takes an origin). The relay pulls every known origin that is not itself connected through each
  connected peer that holds it.
- **`peers.port`** (extra column): the peer's listener port from pairing, used when `address` has no port.
- **Addresses at pairing:** the joiner with `--host name:port` stores `name:<the host's listener port>`; the host
  stores the joiner's hint `<hostname>:<its listener port>` (the joiner's `address` field), empty for mDNS on
  both sides. The host hint and the address hint use the computer's **host name** (not `device_name`, which may be
  anything): it resolves on a LAN and most VPNs; `--host <name>` / the op's `host` replace it with a VPN name or an
  IP.
- **`pair`, `join` and `serve` turn `direct_connection` on** (saved) when it is off, as the pairing ops do in the
  app: pairing without the listener makes no sense, and `serve` needs the journal recording.
- **The 10 s tick also publishes** (locally) so live data is a tick away; the folder mirror stays on the refresh
  tick. **Presence over the direct connection is pulled** (`/v1/presence`, computed now), nothing is pushed.
- **`forget`** deletes the peer's row and its store folder; cursors stay (its merged rows stay, and a later
  pairing must not apply them twice). The folder transport may recreate a folder-only row for it.
- **The app binds `0.0.0.0`** (contract: all interfaces only while the direct connection is on; the listener
  exists only then); tests bind loopback through `Runtime::new(bind, …)`.
- `computers_status` gains a top-level `direct_connection: bool` beside the contract's fields.

## Skipped

- No mDNS test (no network in `cargo test`); the two-process CLI smoke below did find the host by mDNS.
- No IPv6 test; IPv6 literals in brackets are accepted in addresses.
- Clippy: every remaining warning is on a line that exists unchanged in `develop` (`service.rs`, `dash.rs`,
  `harness/codex.rs`, `secrets.rs`, `procs.rs`, `cli/main.rs:453`, `router/lib.rs`); none in a new or edited line.
- The window e2e suite was not run and `e2e/app.e2e.mjs` was not changed.

## Crates added (all build together with the workspace's axum 0.8 / tokio / reqwest-rustls, which use aws-lc-rs)

| Crate | Version | Licence | Why |
|---|---|---|---|
| `axum` | 0.8.9 | MIT | the listener's routes (already in the lock through the router) |
| `axum-server` (`tls-rustls`) | 0.8.0 | MIT | serving axum over rustls with a `Handle` to stop it |
| `rustls` (`aws_lc_rs`, `std`, `tls12`, `logging`) | 0.23.46 | Apache-2.0 OR ISC OR MIT | the server and client configs with custom verifiers (already in the lock through reqwest) |
| `rcgen` (`pem`, `aws_lc_rs`) | 0.14.10 | MIT OR Apache-2.0 | the self-signed certificate from the Ed25519 key |
| `ed25519-dalek` | 3.0.0 | BSD-3-Clause | chunk and record signatures |
| `spake2` (`getrandom`) | 0.4.0 | MIT OR Apache-2.0 | the code exchange |
| `chacha20poly1305` | 0.11.0 | Apache-2.0 OR MIT | the pairing payloads |
| `mdns-sd` (no default features) | 0.21.7 | Apache-2.0 OR MIT | advertise and browse `_hrm._tcp.local.` |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | `pubkey`, `sig`, the payloads (already in the lock) |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 | the key seed, the code, the nonces (already in the lock) |
| `tokio` (`rt-multi-thread`, `net`, `time`, `sync`, `macros`) | 1.54.1 | MIT | the runtime the listener lives on (already in the lock) |

Not used from the suggestions: `aes-gcm` (one AEAD is enough), `hkdf` (SPAKE2's output is already a 32-byte
key), `reqwest` `identity`/`rustls-tls` features (the preconfigured rustls config does both pinning and the
client certificate), `tokio-rustls` directly (axum-server brings it).

## Test results (verbatim, final run on the committed code)

```
cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.90s

cargo test --workspace
     Running unittests src\main.rs (target\debug\deps\fakeharness-0442db9950a5c0c3.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\main.rs (target\debug\deps\hrm-3f54445874d2f448.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\e2e.rs (target\debug\deps\e2e-5b31c1dcb8e52c6a.exe)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.54s
     Running unittests src\main.rs (target\debug\deps\HarnessManager-2385fbfee951cded.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\lib.rs (target\debug\deps\hrm_core-446829be55d564c6.exe)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.14s
     Running tests\data_safety.rs (target\debug\deps\data_safety-15bf25350e7d1f1a.exe)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.05s
     Running tests\pi_sessions.rs (target\debug\deps\pi_sessions-9b7a23802bc5638b.exe)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.12s
     Running unittests src\lib.rs (target\debug\deps\hrm_router-bf4e9bcd35c42307.exe)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\router_e2e.rs (target\debug\deps\router_e2e-653afad3ff736423.exe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.83s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cargo test -p hrm-core --lib peer
test sync::peer::tests::pairing_with_the_right_code_stores_both_peers ... ok
test sync::peer::tests::forget_makes_the_next_hello_fail ... ok
test sync::peer::tests::bs_presence_reaches_a_within_a_poll ... ok
test sync::peer::tests::after_pairing_as_chunks_reach_b_over_tls_and_apply_once ... ok
test sync::peer::tests::a_tampered_chunk_is_skipped ... ok
test sync::peer::tests::a_connection_from_an_unpaired_certificate_is_refused ... ok
test sync::peer::tests::c_unreachable_by_a_is_pulled_through_b_with_its_signature_verified ... ok
test sync::peer::tests::a_wrong_code_fails_and_stores_nothing ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out; finished in 5.09s

cargo clippy --workspace: no warning in a new or edited line (see Skipped)
```

The eight tests (two or three `Ctx` in a temp folder, listeners on 127.0.0.1 at free ports, credentials under
`HRMTEST-core-<pid>` and deleted on drop): the right code stores both peers with certificate, key and
`paired_at`, the joiner shows the host `direct` and `connected` by `address`, the host's `computers-pairing`
event says `ok` with the peer; a wrong code fails with "the code did not match", stores nothing on either side,
one failure keeps the code alive, three end it with an `ok: false` event and the route gone; A's usage, cost and
tag reach B over TLS from a signed chunk, `applied_seq`/`last_seq`/`last_pull` are set, B keeps the chunk for
relay, a second pull adds nothing and nothing bounces back; B's presence (`at`, `accounts`, `tps_5m`) is in A's
`peers` after one tick; a client with an unpaired certificate is refused at the handshake while a paired one
reads `hello` with `origins`; `forget` empties the list, deletes the store folder and the next `hello` fails; C's
rows reach A through B with C `path: "relay"`, `relayed_by: B`, `direct: false`, its key learned from the signed
record B relays, and a record B re-signs under its own key is ignored while C's next chunk still verifies; a
chunk whose bytes changed is not applied (sha256), nor when the manifest is made to match the bytes (signature),
and applies once when put right.

The phase 1 journal tests now set the test credential prefix (publishing signs) and delete their keys.

## The JSON shapes

`computers_status` (also what `computers_now`, `computers_set`, `computers_pair_cancel`, `computers_pair_join`,
`computers_forget` and `computers_set_peer` return), as taken from the CLI smoke on computer B right after
joining, one peer paired and connected:

```json
{"mode": "off", "folder": "", "direct_connection": true,
 "this": {"id": "1b3938dc-6f0c-4e7d-a061-d3429d51278b", "name": "Lap-B", "incarnation": "2e00ff57-371e-49a4-b811-28233d6f0d48",
          "last_seq": 0, "published_seq": 0, "port": 52519, "pubkey": "NhlPLmROPQXqvV3UO5G72yOtUkdR4+OagVWnQ193KO0=", "pairing": null},
 "last_run": null, "error": null,
 "peers": [{"id": "d907e07e-d4b7-42ff-9f73-b0463328c576", "name": "Desk-A", "os": "windows", "app_version": "1.4.0",
            "last_seen": 1793893022726, "last_seq": 0, "applied_seq": 0, "transport": "direct", "presence_at": 1793893022726, "running": [],
            "address": "127.0.0.1:52470", "path": "mdns", "connected": true, "last_pull": 1793893022724, "paired_at": 1793893022665,
            "relayed_by": "", "direct": true}]}
```

While a code is shown, `this.pairing` is `{"code": "098329", "expires_at": 1793893140000, "host_hint": "127.0.0.1:52471", "port": 52471}`,
`null` otherwise. `path` is `"mdns"`, `"address"`, `"relay"` or `""`; a relayed computer has `"relayed_by": "<peer id>"`,
`"direct": false` and `"connected": false`; a folder-only computer has `"direct": false`, `"address": ""`.

`computers_pair_start {host?}` → `{"code": "098329", "expires_at": 1793893140000, "host_hint": "DEVELOPMENTPC:52471", "port": 52471}`
(`host` replaces the host name in the hint). The app then emits `computers-pairing` with
`{"ok": true, "peer": {"id": "…", "name": "Lap-B", "address": "DEVELOPMENTPC:52519"}}` or
`{"ok": false, "error": "three wrong codes; show a new one" | "the code expired"}`.

`computers_pair_join {code, host?}` → the status shape, with the new peer `direct: true` and (once it answered)
`connected: true`; errors as the dispatcher's string: "the code is six digits", "the code did not match; check it
and try again", "no computer on this network is showing a code; give its name:port", "could not reach name:port",
"the other computer did not accept the pairing: …".

`computers_set` args gain `direct_connection?: bool` and `listen_port?: 0..65535` ("the port is a number up to
65535, or 0 to pick one"). `computers_forget {id}` ("no computer is called …"); `computers_set_peer {id, address?, name?}`
(`id` may be the name; "give the computer a name of up to 60 characters").

## CLI output of a pairing (two processes, two temp data folders, `HRM_CRED_PREFIX=HRMTEST-smoke-…`)

```
A$ hrm computers set --name Desk-A --direct on
Desk-A  (d907e07e-d4b7-42ff-9f73-b0463328c576)  mode off
  journal    0 entries, 0 published
A$ hrm computers pair --host 127.0.0.1
code     098329
host     127.0.0.1:52471
waiting two minutes for the other computer to join
paired with Lap-B
exit 0

B$ hrm computers set --name Lap-B --direct on
B$ hrm computers join 098329 --host 127.0.0.1:52471
paired with Desk-A
exit 0
B$ hrm computers peers
Desk-A                   d907e07e-d4b7-42ff-9f73-b0463328c576   connected by mdns        address 127.0.0.1:52470

A$ hrm computers serve --seconds 12
listening on port 52470 for 12 s
exit 0
```

B reached A by mDNS for the pull right after joining (the two processes advertised and browsed on this machine);
A stored B as `DEVELOPMENTPC:52519`. The smoke credentials were deleted afterwards.

## Windows firewall

The CLI smoke bound `0.0.0.0` twice (the listener and the pairing port) from an unsigned `hrm.exe` in
`target\debug`. This session has no screen, so the prompt itself could not be seen. After the smoke, `Get-NetFirewallRule`
and `Get-NetFirewallApplicationFilter` show **no rule** naming `hrm`, `HarnessManager` or that program path: Windows
creates one only when the prompt is answered, so either no prompt was raised or it was left unanswered. In both
cases the pairing, the TLS pull and the mDNS discovery over loopback worked unaffected: the firewall does not
filter loopback. The tests bind loopback and raise nothing. Expect the prompt the first time the installed app has
the direct connection on (and once more for `hrm.exe` from a terminal); until it is allowed on the Private
profile, another computer on the LAN cannot reach the listener, and mDNS answers from this computer may be
dropped.

## For the merge into develop

- `journal::apply_manifest(ctx, id, manifest, read)` takes a reader closure now; `journal::publish` is `pub`;
  `journal::on(ctx)` says whether sync is on in any form; `journal::Peer` has more fields; `folder::write_device`
  goes through the new `folder::device_record`. `Db::open`'s journal flag is `sync_mode != "off" || direct_connection`.
- New kv keys: `device:pubkey`, `pairing:current` (neither needs a backup; `computers:*`/`journal:*` are already
  excluded, these two could be added to that list by phase 4 if it reorganises them).
- New files in the data folder: `device.crt`, `journal-cache/` (this computer's chunks and the received ones; a
  deleted store is made again from the outbox, and the folder still has the published chunks), on platforms without
  a credential store `device.key`.
- The manifest in the sync folder now carries `sig` per chunk and `device.json` carries `pubkey` and `sig`; 1.4.0
  readers ignore both. A 1.4.0 computer's chunks (no `sig`) still merge here as long as its key is unknown.
- The UI (WP-6) reads `direct_connection`, `this.port`, `this.pairing`, and on each peer `direct`, `connected`,
  `path`, `address`, `relayed_by`, `last_pull`, `paired_at`; `transport` is `"direct"` for a paired computer that the
  folder has not touched, `"folder"` once the folder merge saw it (as in phase 1), `"relay"` for one learned by
  relay. The `computers` event is emitted every 10 s while the direct connection is on, as well as after each
  refresh round; `computers-pairing` on the host when a pairing ends.
- The e2e's second peer: `hrm computers pair --host 127.0.0.1` in a second `HRM_HOME` prints `code` and `host`
  lines and exits after the pairing; for the app to pull its sessions afterwards, run `hrm computers serve
  --seconds <n>` in that folder (the CLI's listener dies with the process), or keep `serve` running from the start
  and pair from the app side with `computers_pair_start` instead.
- The app's first tick after a settings change reads the fresh `Ctx` by `settings.json` mtime as before;
  `computers_set` applies the listener change at once.
