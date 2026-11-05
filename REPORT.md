# WP-9a: the hub service and its deployment

Branch `feature/hub-service`, five commits plus this report, not pushed. Everything in the brief is built and tested; the
image builds on this computer.

## What was built, by file

| File | What |
|---|---|
| `Cargo.toml` | `crates/hub` added to the workspace members |
| `crates/hub/Cargo.toml` | package `hrm-hub` (library + binary) |
| `crates/hub/src/lib.rs` | the whole service: `Config`, `serve`, the schema, the routes, retention, the WebSocket fan-out |
| `crates/hub/src/main.rs` | the binary: reads `HUB_ADMIN_TOKEN`, `HUB_DATA`, `HUB_BIND`, `HUB_MAX_BLOB_MB`, `HUB_RETAIN_MB`, calls `serve`, waits forever |
| `crates/hub/src/tests.rs` | seven in-process tests on a loopback port |
| `deploy/hub/Dockerfile` | multi-stage: `rust:1-bookworm` builds `-p hrm-hub`, `gcr.io/distroless/cc-debian12:nonroot` runs it; `/data` owned by `nonroot` |
| `deploy/hub/Dockerfile.dockerignore` | BuildKit's per-Dockerfile ignore: no `target/`, `node_modules`, `.git`, `docs` in the context |
| `deploy/hub/docker-compose.yml` | `caddy` (80/443) + `hrm-hub`, volumes `caddy-data` and `hub-data` |
| `deploy/hub/Caddyfile` | `{$HUB_DOMAIN} { reverse_proxy hrm-hub:8787 }` |
| `deploy/hub/.env.example` | `HUB_DOMAIN`, `HUB_ADMIN_TOKEN`, optional `HUB_TAG`, `HUB_MAX_BLOB_MB`, `HUB_RETAIN_MB` |
| `deploy/hub/README.md` | a VPS in ten lines, what runs, building the image yourself |
| `.github/workflows/release.yml` | job `hub` on `homek8-general`: builds and pushes `ghcr.io/<owner>/hrm-hub:<X.Y.Z>` and `:latest` on a release tag; `permissions` gains `packages: write` |
| `README.md` | "The hub." paragraph at the end of the Computers bullet, pointing at `deploy/hub/README.md` |

## The library

```rust
pub struct Config {
    pub data_dir: PathBuf,     // hub.db and blobs/ live here; created if missing
    pub admin_token: String,   // must not be empty
    pub bind: SocketAddr,      // 127.0.0.1:0 in tests; 0.0.0.0:8787 in the container
    pub max_blob_mb: u64,      // DefaultBodyLimit: a bigger body is refused (413)
    pub retain_mb: u64,        // per origin; the oldest chunks beyond it are dropped, never the newest
}

pub async fn serve(cfg: Config) -> anyhow::Result<(std::net::SocketAddr, axum_server::Handle<std::net::SocketAddr>)>
```

`serve` binds, opens the database, spawns the server on the current tokio runtime and returns at once.
`handle.shutdown()` stops it (the same `axum_server::Handle` the peer listener uses). WP-9b starts it with
`hrm_hub::serve(hrm_hub::Config { .. }).await?` from a `#[tokio::test]` and talks to `http://{addr}`.

## Request and answer shapes

Every route but `/health` and `POST /v1/join` needs `Authorization: Bearer <token>`. A missing or unknown
token is 401; a put under another computer's origin is 403 `{"error": "own only"}`. The admin token is a
caller with no id: it may join, list, read and `DELETE /v1/devices/{id}`, but never put. Writes answer
`204 No Content`; errors answer `{"error": "<text>"}` with 400, 401, 403, 404 or 413.

```
GET  /health                              -> 200 "ok"   (no token; for Caddy or a probe)

POST /v1/join            {"admin_token": "…", "device": <signed device.json>}
                       | {"code": "123456",  "device": <signed device.json>}
  -> 200 {"token": "<64 hex>", "devices": [<device.json + last_seen + last_seq>, …]}
  400 unsigned record / no code and no admin token; 401 wrong admin token, wrong, used or expired code
  A second join by the same id replaces its token (a computer that lost it).
  example: {"admin_token": "s3cret", "device": {"id": "aaaa", "name": "pc-aaaa", "os": "windows", "app_version": "1.5.0",
            "incarnation": "inc1", "roots": {}, "updated_at": 1, "pubkey": "<b64>", "sig": "<b64>"}}
        -> {"token": "9f…", "devices": [{"id": "aaaa", …, "last_seen": 1793886695359, "last_seq": 0}]}

POST /v1/invite          (a joined computer or the admin)  -> 200 {"code": "482913", "expires_at": <ms>}
  six digits, ten minutes, one use

GET  /v1/devices         -> 200 [<device.json + last_seen + last_seq>, …]   (last_seq = max X-To of its chunks)
PUT  /v1/devices/{id}    <signed device.json>  own only; 400 if the record's id is not the path's  -> 204
DELETE /v1/devices/{id}  own or admin: token, record, chunks, blobs, presence, transcripts, addresses gone -> 204

GET  /v1/chunks/{origin}?since=N
  -> 200 {"origin": "aaaa", "incarnation": "inc1", "chunks": [{"file": "inc1-1-5.jsonl", "from": 1, "to": 5,
          "sha256": "…", "sig": "…", "bytes": 13, "enc": true}, …]}   (to > N, of the record's incarnation)
  404 unknown origin
PUT  /v1/chunks/{origin}/{file}   body = the encrypted bytes; X-From, X-To, X-Sha256, X-Sig all needed (400 otherwise)
  own only; file name = letters, digits, `.`, `-`, `_`, not starting with a dot -> 204; then retention runs
GET  /v1/chunk/{origin}/{file}    -> 200 the bytes; 404

PUT  /v1/presence        body = the encrypted presence bytes (a computer's token)  -> 204
GET  /v1/presence        -> 200 {"aaaa": {"at": <ms>, "blob": "<base64 of the bytes put>"}, …}

PUT  /v1/transcripts/{origin}/{key}   body = encrypted bytes; X-Sha256, X-Bytes needed; own only -> 204
     {key} may contain slashes (`claude/p/s1`)
GET  /v1/transcripts/{origin}/{key}   -> 200 the bytes, with X-Sha256 and X-Bytes echoed; 404
DELETE /v1/transcripts/{origin}/{key} own only -> 204

PUT  /v1/rendezvous      {"addresses": ["10.0.0.5:4711", "pc-a.vpn:4711"]}  (a computer's token; the id is the token's) -> 204
GET  /v1/rendezvous      -> 200 {"aaaa": {"addresses": [...], "at": <ms>}, …}

GET  /v1/events  (WebSocket, same bearer header)  -> one text message per change:
     {"kind": "chunks", "id": "aaaa"} | {"kind": "presence", "id": "aaaa"} | {"kind": "device", "id": "aaaa"}
     (device: join, PUT /v1/devices, DELETE /v1/devices)
```

## Decisions where the contract was silent

- **Device records are verified** at join and at `PUT /v1/devices/{id}`: the same check as the core's
  `record_ok` (Ed25519 over the canonical JSON without `sig`, `serde_json` with `preserve_order`). An
  unsigned or tampered record is 400. Chunk `sig`s are stored, not verified: the hub cannot check `X-Sha256`
  (it is of the plaintext) and the clients verify anyway.
- **`last_seen`** is bumped on every authenticated request; **`last_seq`** is computed from the chunk index.
- **Incarnation** of a chunk is parsed from its file name (`<incarnation>-<from>-<to>.<ext>`), else taken
  from the origin's current record. `GET /v1/chunks` lists only the record's current incarnation; an older
  one's chunks stay on disk until retention drops them.
- **Presence blobs** are returned base64-encoded inside the JSON (`blob`), since the bytes are opaque.
- **Transcript blobs** are stored at `blobs/<origin>/transcripts/<sha256(key)>` so a key with slashes or
  odd characters never becomes a path.
- **Events** are only the three kinds of the contract; a transcript or rendezvous put emits nothing.
- **Retention** runs after each chunk put, on that origin only, ordered by time of put: `count <= 1 || total
  <= cap` stops it. Deleting a device removes its whole `blobs/<id>/` folder.
- **One `Mutex<rusqlite::Connection>`** guards the database and the blob writes (a few computers, never a
  crowd); handlers do their I/O inline. `shortcut:` fine for one owner; move to `spawn_blocking` if a hub
  ever serves dozens of computers.
- `/health` was added (the router has one; Caddy or a probe can use it).
- The image runs as distroless `nonroot`; `/data` is copied from the build stage with that owner so a
  fresh named volume is writable.
- The release job uses the plain `docker` CLI (login, build, push) rather than the docker/* actions, so the
  only thing the runner needs is Docker itself.

## Crates added

All in `crates/hub/Cargo.toml`; everything else is already in the workspace lockfile.

| Crate | Why | Licence |
|---|---|---|
| `axum` feature `ws` | the `/v1/events` WebSocket; pulls `tokio-tungstenite` 0.29 and `tungstenite` | axum MIT; tokio-tungstenite MIT; tungstenite MIT/Apache-2.0 |
| `tokio-tungstenite` 0.29 (dev) | the WebSocket client in the tests (already pulled by axum's `ws`) | MIT |
| `futures-util` (dev) | `StreamExt::next` on the test WebSocket (already in the workspace) | MIT/Apache-2.0 |
| `reqwest` (dev, `json` only, no TLS) | the HTTP client in the tests (already in the workspace) | MIT/Apache-2.0 |

Runtime dependencies are workspace ones: `axum`, `axum-server`, `tokio`, `rusqlite` (bundled), `serde`,
`serde_json` (`preserve_order`), `sha2`, `hex`, `base64`, `ed25519-dalek`, `getrandom`, `anyhow`.

## Tests

`cargo test -p hrm-hub` (also inside `cargo test --workspace`, all green):

```
     Running unittests src\lib.rs (target\debug\deps\hrm_hub-065ad3931efde31c.exe)
test tests::chunks_round_trip_with_their_headers ... ok
test tests::join_with_the_admin_token_then_an_invite_code ... ok
test tests::a_token_is_tied_to_its_computer ... ok
test tests::the_websocket_tells_of_a_chunk_put ... ok
test tests::presence_and_rendezvous_round_trip ... ok
test tests::delete_a_device_by_its_owner_and_by_the_admin ... ok
test tests::retention_drops_the_oldest_chunk_but_never_the_newest ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
```

What each covers: join with the admin token, a wrong admin token (401), no credential (400), an unsigned or
tampered record (400), an invite code (six digits, `expires_at`), a wrong code and a used code (401), the
device list with `last_seen` and `last_seq`, no token and a made-up token (401); a put under another origin
(403), `PUT /v1/devices/{other}` (403), a transcript under another origin (403), the admin token putting
(403); chunk put with the four headers, missing headers (400), a name decoding to a path (400), list since
0 / 5 / 9, unknown origin (404), chunk bytes back, a missing chunk (404), `last_seq` after puts, a transcript
put / get with its headers / delete by owner only / gone; presence and rendezvous round trips; retention
with `retain_mb = 1`: the oldest 700 KB chunk dropped, a 1.1 MB chunk kept alone, a 5 MB body refused
(`max_blob_mb = 4`); the WebSocket getting `{"kind":"chunks","id":"aaaa"}` then `{"kind":"presence",…}`,
and the upgrade refused without a token; delete by another computer (403), by its owner (its token dies,
its chunks are gone), then by the admin.

Workspace, on this computer (Windows 11, stable toolchain):

```
cargo build --workspace            Finished `dev` profile ... in 1m 34s          exit 0
cargo clippy --workspace           13 warnings, all pre-existing (hrm-core 11, hrm-router 1, hrm 1); hrm-hub 0   exit 0
cargo test --workspace --no-fail-fast
  e2e.rs 24 passed · hrm_core 38 passed · data_safety 25 passed · pi_sessions 5 passed
  hrm_hub 7 passed · hrm_router 8 passed · router_e2e 3 passed · 0 failed                exit 0
```

The window e2e (`node e2e/app.e2e.mjs`) was not run and `e2e/app.e2e.mjs` was not edited, as the brief says.

## The image

Docker 29.9.2 is on this computer. `docker build -f deploy/hub/Dockerfile -t ghcr.io/abushanab-net/hrm-hub:dev .`
built in one go (exit 0); the image is 46.2 MB. A smoke run (`docker run -e HUB_ADMIN_TOKEN=smoke -p
18787:8787 …`) logged `hrm-hub listening on 0.0.0.0:8787`, answered `ok` on `/health`, 401 on
`/v1/devices` without a token and `[]` with the admin token, so `/data` is writable by `nonroot` and the
container was removed afterwards. `docker compose` with Caddy was not brought up (it needs a public name
for Let's Encrypt).

## What the merge must know

- `Cargo.toml` members gained `"crates/hub"` (one line); `Cargo.lock` gained the hub crate and the
  tungstenite crates. A conflict there resolves by keeping both sides' members.
- `.github/workflows/release.yml`: top-level `permissions` is now `{ contents: write, packages: write }`
  (ghcr needs it) and a `hub` job sits before `publish`. The job runs on `homek8-general` as the brief
  says; whether those runners have Docker is unverified from here (see the comment next to `runs-on`). If
  they do not, the fix is Docker on the runner, not a GitHub-hosted one.
- The release job pushes `ghcr.io/<repository owner, lower-cased>/hrm-hub`; the package is private by
  default on first push and must be made public once in the GitHub package settings for a VPS to pull it
  without a token.
- `README.md` gained one paragraph at the end of the Computers bullet (after "Resuming elsewhere"); WP-9b
  also edits the README, so expect a trivial merge there.
- The hub never verifies chunk signatures or the ring key: WP-9b's client must verify `sig` against the
  origin's `pubkey` from the device record, as the direct connection does.
- Nothing in `crates/core`, `app/`, `e2e/` or the CLI was touched.
