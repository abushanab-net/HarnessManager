# WP-9b report: the hub client, the ring key and the UI (phase 5)

Branch `feature/hub-client`, commits on top of `7ce97f5` (the plan), not pushed. Everything in the brief is built; the
Rust tests run against an in-process stand-in for the hub crate (see "Gated on the hub crate"), and the window test
ran against the real `hrm-hub.exe` built from WP-9a's source into this worktree's `target/debug`.

## What was built, by file

- `crates/core/src/sync/hub.rs` (new): **the ring key** (`ring_make`, `ring_show`, `ring_enter`, `ring_key`, `has_ring`,
  `ring_for_pairing`, `ring_adopt`; `encrypt` / `decrypt`: ChaCha20-Poly1305, a random 12-byte nonce prefixed); **the
  client** `Hub` (`reqwest::blocking`, bearer token, `get_json` / `get_bytes` / `put` / `post_json` / `delete`, a 401 or
  403 reads "the hub refused this computer; join it again"); `join` (`POST /v1/join` with the signed device record and a
  code or the admin token; the token into the credential store; the first computer makes the ring; the settings switch
  to `sync_mode = "hub"` with `hub_url`), `invite`, `leave` (`DELETE /v1/devices/{me}`, the token gone, mode off);
  **the round** (`push_chunks`: the chunks the hub's list lacks, sealed, with `X-From`, `X-To`, `X-Sha256` of the
  plaintext and `X-Sig`; `PUT /v1/devices/{me}`; presence up; `GET /v1/devices` → each record learned through
  `journal::learn_device`, its `peers` row `transport = 'hub'`; `pull_origin` → `journal::apply_manifest` with a reader
  that opens each chunk with the ring key; `GET /v1/presence` → each blob opened and stored where the folder's
  `presence.json` goes; rendezvous up and down with the direct connection on); `push_presence` (the minute tick);
  `carry` (phase 3's carry with the hub as destination: `PUT /v1/transcripts/{me}/{harness}/{profile}/{id}`, sealed,
  `X-Sha256` and `X-Bytes` of the compressed transcript; `DELETE` at the cap) and `fetch_transcript`; `status`; `watch`
  (a thread that keeps `GET /v1/events` open with `tokio-tungstenite`, runs `journal::round` on each message and tells the
  app; without the socket, a round every 30 s); `rendezvous_of` / `rendezvous_pairs` for the peer client.
- `crates/core/src/sync/hub_tests.rs` (new): seven tests, with a fake hub (`mod fake`: the contract's routes in memory,
  axum, loopback). `start_hub` is the one function to point at `hrm_hub::serve` after the merge.
- `crates/core/src/sync/mod.rs`: `pub mod hub;`.
- `crates/core/src/journal.rs`: `publish` takes a named lock (the refresh round, the 10 s tick and the events thread all
  publish); `backfill_if_needed` counts this computer's usage rows only (see "Decisions"); `round` carries to the hub
  before `publish` and runs `hub::round` after the folder; `presence_tick` pushes to the hub in hub mode; `status` gains
  `hub`; `configure` accepts `"hub"` (only once joined) and `hub_url`.
- `crates/core/src/sync/transcripts.rs`: `carry_to(ctx, cap, put, remove)` is the carry with any destination;
  `carry_with_cap` wraps it with the folder; `own_zst` gives one of this computer's transcripts as it is carried (for the
  peer route).
- `crates/core/src/transcripts.rs`: `fetch` tries a connected computer first, then the hub (in hub mode) or the folder;
  `local_path` is `pub(crate)` for the listener.
- `crates/core/src/sync/peer/listener.rs`: `GET /v1/transcript/{origin}/{harness}/{profile}/{id}` (own files through
  `own_zst`, fetched copies recompressed).
- `crates/core/src/sync/peer/client.rs`: `candidates` (rendezvous addresses, then mDNS, then `address`) and `reach`
  factored out of `pull_all` (`path = "rendezvous"` when reached that way); `transcript_from_peers`.
- `crates/core/src/sync/peer/pairing.rs`: the payload carries `ring` (the seed, or null); `store_peer` adopts it when
  this computer has none.
- `crates/core/src/sync/peer/tests.rs`: one appended test (the transcript route).
- `crates/core/Cargo.toml`, `Cargo.lock`: the crates below.
- `app/src-tauri/src/main.rs`: ops `hub_join`, `hub_leave`, `hub_invite`, `hub_status`, `ring_key_show`,
  `ring_key_enter` (all in `READS`); `hub::watch` started in `setup`, emitting `computers`.
- `crates/cli/src/main.rs`: `hrm hub join <url> --code <c> | --admin <t>`, `hrm hub invite`, `hrm hub status [--json]`,
  `hrm hub leave`, `hrm ring show`, `hrm ring enter [key]`; `hrm computers peers` says "through the hub".
- `app/ui/computers.js`: `transportLine` gains "direct · rendezvous · connected", "hub · connected" / "hub · heard …" /
  "not reachable · hub only"; the dialogs **Join a hub** (`hubJoinDialog`: address, the code formatted as typed, or the
  admin token; checks before any call; the op's error in `[data-testid=hub-error]`), **Invite a computer** (the code
  large, the address, ten minutes), **Your recovery key** (`ring_key_show`, "I saved it in my password manager"), **Enter
  the recovery key** (`ring_key_enter`, groups of four as typed); after a join the key dialog opens once on the computer
  that made the ring, or the Enter dialog on one without a ring; `ring-show` behind a confirm; `hub-leave` behind a
  confirm; the first-run dialog **Add this computer to your others** with both cards (`firstRunDialog`), shown once on
  the first start when no tour is in front of the page.
- `app/ui/views.js` (Settings › Computers): the hub card live (chosen when `mode = "hub"`, a `joined` / `live` pill, the
  address, "Join with a code…" or "Invite a computer…" and "Stop syncing", the line "joined · N computers · live|polling",
  a warning when the ring is missing); the **Recovery key** row (Show again, Enter recovery key…) once joined or with a
  ring; the CLI reference gains the hub and ring commands.
- `app/ui/guide.js`: the Computers tour step mentions the hub. `app/ui/app.css`: three rules appended.
- `e2e/app.e2e.mjs`: the preamble dismisses the first-run dialog like the tours; two assertions of the WP-4 off-state test
  flipped (the hub card is live, no "phase 5" pill); the `// ---- the hub (WP-9) ----` block appended (one ungated test,
  one under `HRM_E2E_HUB=1`).
- `README.md`: the "A hub you host" paragraph at the end of the Computers bullet.

## Gated on the hub crate

The hub crate landed in `D:\Projects\abushanab-net\HarnessManager-wt\hub-service` while this package was under way
(package `hrm-hub`, `hrm_hub::serve(Config { data_dir, admin_token, bind, max_blob_mb, retain_mb }) ->
(SocketAddr, axum_server::Handle)`); its REPORT.md shapes were read and the client matches them: `GET /v1/devices`
returns the record with `last_seen` and `last_seq` inside it (`split_device` strips them before `record_ok`), writes
answer 204, a wrong code is 401, presence blobs come back base64 in `blob`, the transcript key may carry slashes.

- The `[dev-dependencies] hrm-hub = { path = "../hub" }` line was **not added**: the crate is not in this worktree, and a
  path dependency that does not resolve breaks `cargo build --workspace` here. The Rust tests run against `mod fake` in
  `hub_tests.rs` instead. **At the merge:** add that line, and replace the body of `start_hub` with
  `let (addr, handle) = rt.block_on(hrm_hub::serve(hrm_hub::Config { data_dir, admin_token: admin.into(), bind:
  "127.0.0.1:0".parse().unwrap(), max_blob_mb: 64, retain_mb: 2048 })).unwrap();` (the two tests that reach into the
  fake's `blobs` and `transcripts` would read the files under `data_dir/blobs/<origin>/` instead). The fake can stay as
  a stand-in or go.
- The window test used the real service: `hrm-hub.exe` was built from the other worktree's source into this worktree's
  `target/debug` (`cargo build --locked --manifest-path ../hub-service/crates/hub/Cargo.toml --target-dir ./target -p
  hrm-hub`; nothing was written in that worktree). After the merge `cargo build --workspace` produces it.
- The CLI smoke below also ran against the real service.

## Decisions where the contract was silent

- **The recovery key is 20 random bytes** (32 base32 characters, RFC 4648 alphabet, 8 groups of 4); the 32-byte ring key
  is their SHA-256. A 160-bit secret is plenty, and the form the owner types stays the contract's 32 characters. Typing
  tolerates case, spaces and dashes, and reads `0`, `1`, `8` as `O`, `I`, `B` (not in the alphabet).
- **Credential names carry the device id** (`<prefix>:ring-key:<id>`, `<prefix>:hub-token:<id>`), as WP-5 did for the
  device key: two data folders on one computer must not share one key. A platform without a credential store keeps them
  in `<data>/ring.key` and `<data>/hub.token` (never in `settings.json`), marked as a shortcut like the device key.
- **The ring is made by the computer that joins with the admin token** and has none; a computer joining with a code
  gets it from a pairing or the recovery key. A second admin-token join on a computer without a ring would make a new
  ring that cannot read older blobs; the owner enters the old recovery key to fix that (`ring_enter` replaces it).
- **Without the ring, the hub round fails early** with "no ring key on this computer: enter the recovery key in Settings ›
  Computers" (nothing is pushed either, since pushing seals). The status carries it as `error`; the card shows the
  warning and the Recovery key row offers the dialog; the CLI prints it after `hrm hub join`.
- **The incarnation**: the hub lists only the current incarnation of an origin; the client asks with the cursor of the
  incarnation it knows and asks again with the one answered when they differ, as the peer client does.
- **`push_chunks` lists the hub's chunks every round** (`GET /v1/chunks/{me}?since=0`) and pushes the missing ones, so a
  hub that lost its data gets everything back without a local water line.
- **`transport = 'hub'`** on a peer's row is set each round for every device the hub lists; a paired computer stays
  `direct`-capable (its certificate) and its transport line prefers the direct connection when connected.
- **Rendezvous addresses** are every non-loopback IPv4 interface with the listen port plus `<hostname>:<port>`
  (`if-addrs`), published only with the direct connection on and a port chosen; they are kept in kv `hub:rendezvous`
  and tried before mDNS and `address` (`path = "rendezvous"`). Hole punching is not attempted, as decided.
- **The transcript route answers zstd bytes** recompressed at level 3 (own files from the tool's file, others' from
  the fetched copy), so the asker checks the journal entry's sha256 as it does with the folder; a transcript changed
  since it was carried does not match and the fetch falls through to the hub or the folder. `fetch` asks the origin
  first, then any other paired computer that holds the copy.
- **The events socket runs a whole `journal::round`** on each message (not only a pull): it is the same code path and
  what is new here goes up at once. Status `ws` is kv `hub:ws`. The CLI never opens the socket.
- **`hub_status` is also inside `computers_status`** (`hub: {...}`) so the Settings card is drawn from one read.
- **`hub_join` and `hub_leave` answer the `computers_status` shape** (after a round), like `computers_set`.
- **The first-run dialog** could not be a modal at app start without breaking the whole window suite (every test clicks
  through the page at once) and without colliding with the first-visit tour; it opens on the first `rendered` after the
  first start with no tour or dialog in front, once (`localStorage` `hm.firstrun`), and the e2e preamble dismisses it
  like the tours. Phase 1's mock shows it as a dialog, so it is one.
- **The backfill trigger counted merged rows**: `backfill_if_needed` re-ran whenever the outbox was empty beside any
  usage, which journaled merged tags as this computer's on every such round (tags carry no device). It now counts this
  computer's own usage rows. Found by the hub test's "nothing bounces back" assertion; the folder path had the same
  latent issue after its first merge.
- **`publish` holds a named lock** (`journal-<device id>`): the app's refresh round, the 10 s peer tick and now the
  events thread all publish, and two at once could cut overlapping chunks.
- A used or wrong code reads "the hub did not accept the code; check it and try again"; an address without a scheme
  "the hub address starts with https:// (or http:// on a private network)".
- Line endings: this worktree's working copies are LF throughout (the index is LF through autocrlf, as before); nothing
  was converted.

## Crates added

| Crate | Version | Licence | Why |
|---|---|---|---|
| `tokio-tungstenite` (`connect`, `rustls-tls-native-roots`) | 0.29.0 | MIT | the `/v1/events` WebSocket client (the version axum's `ws` uses, so one copy) |
| `tungstenite` | 0.29.0 | MIT OR Apache-2.0 | pulled by it |
| `rustls-native-certs` | 0.8.5 | Apache-2.0 OR ISC OR MIT | the OS roots for `wss://` (already in the lock) |
| `futures-util` (`sink`, `std`) | 0.3.34 | MIT OR Apache-2.0 | `next` / `send` on the socket (already in the lock) |
| `data-encoding` | 2.11.1 | MIT | base32 of the recovery key (already in the lock) |
| `if-addrs` | 0.15.0 | MIT OR BSD-3-Clause | the LAN addresses for rendezvous (already in the lock through mdns-sd) |
| `axum` feature `ws` | 0.8.9 | MIT | the fake hub's events socket in the tests |

`reqwest`'s `rustls` feature (already on) verifies a real hub's certificate with the platform verifier; no roots to
configure. `base64` 0.22 enters the lock as tungstenite's dependency beside the workspace's 0.23.

## Tests (verbatim)

```
cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s)

cargo test --workspace
     Running tests\e2e.rs (target\debug\deps\e2e-47abda2a06fc20af.exe)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.80s
     Running unittests src\lib.rs (target\debug\deps\hrm_core-d7327daa8f885d25.exe)
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.68s
     Running tests\data_safety.rs (target\debug\deps\data_safety-4460bdb334a3ee36.exe)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.70s
     Running tests\pi_sessions.rs (target\debug\deps\pi_sessions-cbd0030cacc2b297.exe)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.80s
     Running unittests src\lib.rs (target\debug\deps\hrm_router-dd8df418e208d19a.exe)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running testsouter_e2e.rs (target\debug\depsouter_e2e-aa9b812fa8f52e5d.exe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
(plus six empty `0 passed` targets)

cargo test -p hrm-core --lib hub
test sync::hub::tests::transcripts_go_to_the_hub_sealed_and_are_fetched_on_demand ... ok
test sync::hub::tests::rendezvous_addresses_flow_and_come_first_for_the_direct_connection ... ok
test sync::hub::tests::a_tampered_chunk_on_the_hub_is_skipped ... ok
test sync::hub::tests::presence_round_trips_through_the_hub ... ok
test sync::hub::tests::an_event_from_the_hub_runs_a_round_on_the_other_computer ... ok
test sync::hub::tests::the_ring_key_has_a_recovery_form_that_round_trips_and_seals_blobs ... ok
test sync::hub::tests::two_computers_join_a_pushes_and_b_pulls_and_applies_once ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 1.50s

cargo test -p hrm-core --lib peer::tests::a_transcript
test sync::peer::tests::a_transcript_comes_from_a_connected_peer_before_the_folder ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out; finished in 0.78s

cargo clippy --workspace: no warning in a new or edited line (the 13 pre-existing ones in service.rs, dash.rs,
codex.rs, secrets.rs, procs.rs, router/lib.rs and cli/main.rs:481 are on lines unchanged here)

cd e2e && HRM_E2E_SYNC=1 HRM_E2E_PEERS=1 HRM_E2E_HUB=1 node app.e2e.mjs   (after cargo build --workspace)
ok   hub: the card is live, the Join dialog takes a code or the admin token, an unreachable hub is the error text, the first-run choice offers both cards
ok   hub: joining from the card with the admin token, the recovery key once, Invite a computer, a second data folder joining with the code, its session after Sync now
48 passed, 0 failed; screenshots in D:\Projectsbushanab-net\HarnessManager-wt\hub-client\e2e\.work\shots
```

The seven hub tests cover: the recovery form (8 groups of 4 of A–Z and 2–7, shown once and `ring_shown`, typed back
on another computer with dashes, lower case and `0` for `O` giving the same key, a bad one refused, a blob sealed on A
opening on B and not on C, a flipped byte refused, the pairing payload carrying the ring and a computer without one
taking it); a wrong code and a wrong admin token refused, an address without a scheme refused, A joining with the
admin token (settings on `hub`, the ring made, `ring_shown` false), a one-time invite code B joins with (no ring), A's
rows pushed as ciphertext (the blob does not contain the request id), B's round failing with "enter the recovery
key" and nothing applied, then with A's key typed in B holding A's rows, cost, tag, key and `transport: "hub"`,
`applied_seq`/`last_seq`/`last_pull`, the chunk kept for relay, a second round adding nothing and nothing bouncing
back, B's row reaching A, `leave` dropping the token and the mode; presence both ways and the minute tick; a byte
flipped in the hub's blob not applied and no cursor, applied once put right; rendezvous addresses of A (every LAN
IPv4 plus the host name, with A's port) read by B and parsed into pairs; a transcript carried sealed under
`<origin>/claude/claude-main/s1` (not plain zstd on the hub), its entry merged by B, fetched and equal to the source;
the events socket open (`ws: true`) and a chunk put on A making B pull within seconds. The peer test: A's transcript
fetched by B over the direct connection with the folder's copy gone, and B serving the fetched copy to C.

The first full window run had the two hub tests failing: the "unreachable hub" test used port 9, where a TCP connect
hangs on this machine (UDP 9 is bound; the connect timeout did not fire for loopback), now a port taken and released
by the test; and the second computer's sessions, indexed before the join with the journal off, are journaled only when
listed with it on (phase 1's rule), so the test lists them once after joining, as the app does all the time. Both
passed alone and then in the full run above. Screenshots looked at: `51-hub-join-dialog`, `52-first-run-choice`,
`53-recovery-key`, `54-invite-code`, `55-hub-two-computers` (the card chosen with the "joined" pill, the address,
Invite a computer…, Stop syncing, "joined · 2 computers · polling", the Recovery key row), `55b-sessions-from-hub`.

## The op shapes

```
computers_status                      gains "hub": {url, joined, devices, last_push, last_pull, ws, error, ring, ring_shown}
                                      peers from the hub: "transport": "hub"; a peer reached through the hub's addresses: "path": "rendezvous"
computers_set {sync_mode: "hub"}      only once joined ("join a hub first: Join with a code… on the hub card"); {hub_url} normalizes the address
hub_join {url, code?, admin_token?}   -> the computers_status shape after a round
                                      errors: "the hub address starts with https:// (or http:// on a private network)", "the code is six digits",
                                      "give the six-digit code from a computer already on the hub, or the hub's admin token for the first computer",
                                      "the hub did not accept the code; check it and try again", "could not reach the hub at <url>: …"
hub_leave                             -> the computers_status shape (mode off)
hub_invite                            -> {"code": "620178", "expires_at": 1793896133000}
hub_status                            -> {"url": "http://127.0.0.1:18787", "joined": true, "devices": 2, "last_push": 1793895533468, "last_pull": 1793895533485,
                                          "ws": false, "error": null, "ring": true, "ring_shown": true}
ring_key_show                         -> {"key": "7C6W PLZY VY2G 4KYD JHHJ EYZO U2GZ M2E3"}   (marks it shown)
ring_key_enter {key}                  -> the computers_status shape (after a round in hub mode); error "the recovery key is 8 groups of 4 letters and digits (A to Z, 2 to 7)"
pairing payload                       + "ring": "<32 base32 characters>" | null
hrm hub join <url> --code <c> | --admin <t>   prints "joined <url> (N computers)", the recovery key once on the first computer, or the error of the first round
hrm hub invite                        prints "code     620178" and the join line to run
hrm hub status [--json]               the shape above
hrm hub leave
hrm ring show                         prints the recovery key
hrm ring enter [key]                  asks for it when not given
```

New kv keys: `hub:last_push`, `hub:last_pull`, `hub:ws`, `hub:rendezvous`, `ring:shown` (none needs a backup).
New credentials: `<prefix>:ring-key:<device id>`, `<prefix>:hub-token:<device id>`. New files in the data folder only
on a platform without a credential store: `ring.key`, `hub.token`.

## CLI smoke against the real `hrm-hub.exe` (two temp data folders, `HRM_CRED_PREFIX=HRMTEST-smoke-…`)

```
A$ hrm hub join http://127.0.0.1:18787/ --admin s3cret
joined http://127.0.0.1:18787 (1 computer)
recovery key   7C6W PLZY VY2G 4KYD JHHJ EYZO U2GZ M2E3
Keep it in your password manager: a computer that is not paired with this one needs it to read the hub.
A$ hrm hub invite
code     620178
B$ hrm hub join http://127.0.0.1:18787 --code 620178
joined http://127.0.0.1:18787 (1 computer)
  no ring key on this computer: enter the recovery key in Settings › Computers
B$ hrm hub status
http://127.0.0.1:18787  joined, 1 computer
  ring key   missing: hrm ring enter
  last push  -   last pull  -   events polling
  last run   no ring key on this computer: enter the recovery key in Settings › Computers
B$ hrm ring enter "7C6W PLZY VY2G 4KYD JHHJ EYZO U2GZ M2E3"
the ring key is set; the next round reads the hub
B$ hrm computers now
Lap-B  (59c151bd-…)  mode hub
  Desk-A                   windows  1.5.0    seen Thu 19:18  applied 0/0  0 running
A$ hrm computers peers
Lap-B                    59c151bd-8495-4dba-9a1f-d08bac25406a   through the hub          address -
B$ hrm hub join http://127.0.0.1:18787 --code 000000
error: the hub did not accept the code; check it and try again            exit 1
B$ hrm hub leave
left the hub; what was merged from the other computers stays
```

## Every line touched in shared files (against `develop`)

- `crates/core/src/journal.rs`: 59 (the lock in `publish`), 97–98 (the backfill trigger), 106–107, 308–309 (the hub
  carry in `round`), 317 (`hub::round`), 329–333 (`presence_tick`), 338, 345 (`"hub"` in `status`), 374, 377, 393, 396–397
  (`configure`: `hub_url`, `"hub"`), 411 (the joined check).
- `app/src-tauri/src/main.rs`: 75–76 (`READS`), 643–656 (the six ops before `other => bail!`), 826–828 (`hub::watch`).
- `crates/cli/src/main.rs`: 123–146 (`Cmd::Hub`, `Cmd::Ring`, `HubCmd`, `RingCmd`), 317 (the no-lock pattern), 622 (the
  peers line), 699–732 (the two match arms).
- `crates/core/src/transcripts.rs`: 57 (`local_path` visibility), 76–84 (`fetch`'s sources).
- `crates/core/src/sync/transcripts.rs`: 61–80 (`carry_with_cap`, `own_zst`, `carry_to`), 120 (`put`), 137 (`remove`).
- `crates/core/src/sync/peer/pairing.rs`: 55, 57 (`ring` in the payload), 74 (`ring_adopt`), 200 (`ring` in `join`'s
  payload is the same function). `listener.rs`: 82, 133–149. `client.rs`: 56, 88–126, 156–162. `tests.rs`: appended.
- `app/ui/views.js`: 7 (import unchanged but for a comment), 734, 743–744 (`hub`, `hubOn`), 764–768 (the hub card and the
  Recovery key row), 908–909 (the CLI reference). `app/ui/guide.js`: 71. `app/ui/app.css`: appended. `app/ui/computers.js`:
  the header comment, the import line, `transportLine`, and everything after the pairing block.
- `e2e/app.e2e.mjs`: 196 (the preamble), 1251, 1261, 1263 (the WP-4 test), 1587–1702 (the WP-9 block).
- `README.md`: 141–157.

## For the merge

- WP-8 (backup and archive) touches `main.rs` ops, `cli/main.rs` subcommands and `journal.rs`'s `round`: my ops sit
  right before `other => bail!`, my CLI variants at the end of `Cmd` and my arms at the end of the match, my `round`
  lines are the two `sync_mode == "hub"` branches (one before `publish`, one after the folder block). `configure` now
  accepts `"hub"`; a phase 4 field goes after `listen_port`.
- `journal::status` carries `hub`; `journal::publish` locks; `backfill_if_needed` counts own rows (behaviour change, see
  Decisions). `sync::transcripts::carry_with_cap` keeps its signature; `carry_to` is the general form.
- `transcripts::fetch` tries peers, then the hub or the folder; anything else that read the folder path directly is
  unchanged.
- `peers.transport` can be `"hub"`; the UI reads `computers_status.hub` and `path === "rendezvous"`.
- The hub tests: see "Gated on the hub crate" for the one-line switch to `hrm_hub::serve`; the window test expects
  `target/debug/hrm-hub.exe`, which `cargo build --workspace` gives once the crate is a member.
- The e2e preamble and two WP-4 assertions changed (listed above); the rest of the suite is untouched.
