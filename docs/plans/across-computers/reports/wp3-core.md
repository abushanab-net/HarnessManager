# WP-3 report: core journal, folder transport, ops and CLI

Branch `feature/sync-core`, three commits on top of `220133b` (the plan). Nothing pushed. Nothing outside the
package's paths was edited (`e2e/app.e2e.mjs` untouched, no UI files touched).

## What was built, by file

- `crates/core/src/ctx.rs`: the settings contract (`device_id`, `device_name`, `incarnation`, `sync_mode`,
  `sync_folder`, `hub_url`, `sync_transcripts`, `sync_presence`, `direct_connection`, `archive_days`,
  `archive_dir`) with the plan's defaults. `device_id` and `incarnation` are made once at open and saved;
  `device_name` defaults to the hostname. `Db::open` now takes the device id and whether sync is on.
- `crates/core/src/util.rs`: `uuid4()` (v4-shaped id from the OS-seeded hasher; no new dependency),
  `hostname()`, `sha256_hex()`.
- `crates/core/Cargo.toml`: `sha2 = "0.10.9"` (already in `Cargo.lock` through reqwest; no new download).
- `crates/core/src/db.rs`: the database contract: `usage + device, cost_usd, price_rev`, `sessions + device`,
  tables `outbox` (with an index on `(op, key)`), `peers`, `cursors2`, and a partial index on unpriced rows.
  Migration: columns added when missing; existing `usage` and `sessions` rows stamped `device = this` once
  (kv `migrated:device`, logged). `insert_usage(rows, prices)` freezes `cost_usd` and `price_rev` at insert
  and records each stored row to the outbox when sync is on. `price_unpriced(prices)` prices rows that have
  no cost yet (one SQL update per model, logged). Journal hooks in `quota_point`, `tag_set`, `tag_rename`,
  `forget_profile` (tombstones), and `journal_session`. Shared helpers for the merge: `upsert_usage`,
  `upsert_session`, `quota_point_in`, `local_day`, `session_key`. `forget_profile` now deletes only this
  computer's usage and session rows of that account.
- `crates/core/src/pricing.rs`: `rev(prices)`: first 8 hex of the SHA-256 of the sorted table.
- `crates/core/src/harness/{claude,codex,opencode}.rs`: load prices once per import and pass them to
  `insert_usage`.
- `crates/core/src/journal.rs` (new): `record`, `backfill`, publish, merge (`apply_manifest`, `apply`),
  `round`, `presence_tick`, `peers`, `device_names`, `open_elsewhere`, `status`, `configure`; five tests.
- `crates/core/src/sync/{mod,folder}.rs` (new): the folder layout of the contract: `root` (checks the folder
  is reachable), `mine`, `others`, chunk and manifest writes (`write_atomic`: temp + rename), `device.json`
  with `roots` (project key -> local root for every distinct project folder in this computer's usage),
  `presence.json` (running consoles with their project, per-account tokens and requests in the last 5 min,
  `tps_5m`).
- `crates/core/src/model.rs`: `SessionMeta + device, device_name, open_elsewhere` (serde default).
- `crates/core/src/service.rs`: `SessionFilter.scope`; `sessions()` adds the merged sessions of the other
  computers (same filters), stamps local ones with this device and journals them once their project is
  resolved; `projects(ctx, days, scope)` adds `devices` and `here`; `overview(..., scope)`,
  `request_log_since(..., scope, ...)` (rows gain `device`), `breakdown_of`, all summing `cost_usd`;
  new `activity(ctx, harness, scope)`; `import_usage` runs `price_unpriced` after the imports.
- `crates/core/src/dash.rs`: `dashboard(..., scope)` reads `cost_usd` and `device`, adds `tiles`;
  `live` is `pub` again and adds the peers' presence (accounts summed, `running` rows with device);
  `account_stats` and `session_stats` read `cost_usd`.
- `app/src-tauri/src/main.rs`: `live` restored; `home`, `requests`, `sessions`, `projects` take `scope`;
  `computers_status`, `computers_now`, `computers_set`; `live`, `computers_status`, `computers_now` are in
  `READS` (no registry lock); the background loop runs a round every refresh tick when `sync_mode == "folder"`
  and emits `computers` with the status; a thread writes `presence.json` every 60 s.
- `crates/cli/src/main.rs`: `hrm computers status | now | set --name <n> --mode off|folder --folder <path>
  [--transcripts on|off] [--presence on|off]`, each with `--json`; runs without the registry lock.
- `crates/core/src/backup.rs`: kv keys `journal:*` and `computers:*` stay out of backups (test updated);
  the zip already carries `settings.json` (so the new fields) and never the `outbox`.
- `README.md`: the "Computers" paragraph under Features.

## Decisions where the plan was silent

- **Recording only while sync is on.** `Db.journal` follows `sync_mode != "off"`. With sync off the outbox does
  not grow (it would double the database). Turning sync on queues a backfill (kv `journal:backfilled` reset),
  and the backfill is an anti-join (`not exists` in the outbox), so it never writes an entry twice and also
  covers "outbox empty, usage not". Published entries stay in the outbox; `journal:published_seq` marks the
  water line, so `seq` never restarts within an incarnation.
- **Sessions are journaled from `service::sessions`,** not from `put_session`: only there is the project key
  resolved (the receiving computer cannot resolve a path it does not have). A local session gets a "mirror"
  row in `sessions` keyed `harness/profile/id` (the per-file cache rows keep their `harness|profile|path`
  keys), and is recorded when new or when `last_active_at` moved. OpenCode and Antigravity sessions are
  covered by this too, since it runs on every listed session.
- **Tag changes are journaled as whole lists** (`tag` with `tags: []` clears); `tag_rename` records every
  key that carried the old tag. Tombstones are written for sessions when an account is forgotten.
- **Chunk order is the manifest's order,** and a chunk is skipped when `to <= cursor`; there is no
  `from == cursor + 1` check, because SQLite's autoincrement leaves gaps after rolled-back transactions.
  A chunk that cannot be read or whose sha256 differs stops that computer's merge for this round (order
  matters) and is retried next round; the file name is checked for path separators before it is opened.
- **Cost:** `cost_usd` is frozen at insert; rows without a cost (older rows, or a model the catalog lacked)
  are priced by `price_unpriced` after every import (the migration pass is this, run on the first refresh after
  the upgrade; logged "N usage rows priced at list prices (rev)"). "Revalue at today's prices" was not a
  one-liner, so it is gone: every view sums `cost_usd`. `price_rev` of such late-priced rows is the revision
  at that time; their already-published journal entries keep `cost_usd: null`.
- **`quota_hist` has no device column** (as in the contract), so merged readings and local ones share it.
  The backfill therefore also writes merged readings into this computer's outbox once; the other side
  applies them as "same as the last reading" and nothing changes.
- **`live.running`** holds the peers' running rows only (each with `device`, `device_name`); this computer's
  running consoles are already in `state.profiles[*].running`, and scanning processes on every `live` poll
  was not worth it.
- **`computers_set` with `sync_mode: "hub"`** fails with "the hub comes with a later version"; the CLI
  documents `off|folder`. Setting `folder` mode requires an existing absolute folder, and runs a round at once.
- `account_stats`, `session_stats`, `breakdown` and the `usage` op count every computer (no scope): the
  contract lists scope for `home`, `sessions`, `projects`, `live` only. `hrm overview` and `hrm projects
  list` are "all".

## Skipped

- No `scope` flag on the CLI's `overview`, `requests`, `sessions`, `projects` (not in the contract).
- `device.json` `roots` comes from the distinct project folders in this computer's usage rows (sessions whose
  folder never produced a usage row are not in it); phase 3 can widen it.
- The window e2e suite was not run (WP-4 owns it) and `e2e/app.e2e.mjs` was not changed.
- Clippy: the remaining warnings are all on lines that exist unchanged in `develop` (a newer clippy's
  `unnecessary_sort_by`, `type_complexity`, `manual_contains`, `chunks_exact_to_as_chunks`); none is in
  code this package added. The two that were in new code were fixed.

## Test results (verbatim, final run on the committed code)

```
cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.33s

cargo test --workspace
     Running unittests src\main.rs (target\debug\deps\fakeharness-42fae65481b4374b.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\main.rs (target\debug\deps\hrm-858c0da823c47cdd.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\e2e.rs (target\debug\deps\e2e-78d378d220e080b2.exe)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.71s
     Running unittests src\main.rs (target\debug\deps\HarnessManager-e61357056ba13488.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\lib.rs (target\debug\deps\hrm_core-2110fb40716c0496.exe)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests\data_safety.rs (target\debug\deps\data_safety-faecc4d71b879312.exe)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.75s
     Running unittests src\lib.rs (target\debug\deps\hrm_router-d225f47e475eeef0.exe)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests\router_e2e.rs (target\debug\deps\router_e2e-c7eba528272334fe.exe)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.50s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cargo test -p hrm-core --lib journal
test journal::tests::a_database_from_before_gets_its_columns_and_this_computers_stamp ... ok
test journal::tests::backfill_writes_what_was_there_before_sync_once ... ok
test journal::tests::what_a_records_b_merges_once_with_a_as_its_device ... ok
test journal::tests::a_chunk_that_does_not_match_its_manifest_waits_and_is_applied_once_fixed ... ok
test journal::tests::a_merged_session_and_its_tombstone ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.33s

cargo clippy --workspace: no warning in new code (see Skipped for the pre-existing ones)

CLI smoke (HRM_HOME and HRM_PROFILES_HOME set to a temp folder):
$ target/debug/hrm.exe computers status
DEVELOPMENTPC  (474bd873-aed7-4ec2-a8d1-8a6721239655)  mode off
  journal    0 entries, 0 published
exit 0
$ target/debug/hrm.exe computers set --mode folder --folder <tmp>/shared --name Desk
Desk  (41a4a375-...)  mode folder  folder <tmp>/shared
  journal    0 entries, 0 published
  last run   Thu 17:21
exit 0        -> <tmp>/shared/devices/<id>/{device.json, manifest.json, presence.json} written
```

The five journal tests cover: A records usage, tags and a limit reading, publishes; B merges and sees the rows
with `device = A`, the frozen cost and revision, the tag and the reading; merging twice adds nothing; a chunk
with a wrong sha256 is skipped (no error in the status) and applied once fixed, and not again; backfill with
sync off then on writes two usage rows, one tag list and two readings once and nothing the second time; a
database with the pre-1.4 schema gets its columns, the device stamp, stable ids on reopen, and late pricing;
a merged session shows in `sessions` (scope all) with `device_name`, is absent with scope "here", shows in
`projects` with `here: false` and `devices[0].name = "A"`, and its tombstone removes it.

## The JSON shapes the ops return

`computers_status`, `computers_now` (same shape; `now` runs a round first), `computers_set` (same shape;
runs a round when the mode is "folder"):

```json
{"mode": "off|folder", "folder": "D:\\OneDrive\\hrm-sync",
 "this": {"id": "uuid", "name": "Desk", "incarnation": "uuid", "last_seq": 1205, "published_seq": 1205},
 "last_run": 1793886695359 | null, "error": "text" | null,
 "peers": [{"id": "uuid", "name": "Laptop", "os": "windows", "app_version": "1.3.2", "last_seen": 1793886695359 | null,
            "last_seq": 880, "applied_seq": 880, "transport": "folder", "presence_at": 1793886695359 | null,
            "running": [{"profile": "work", "harness": "claude", "project": "github.com/o/r", "project_name": "r",
                         "session_id": "abc" | null, "cwd": "C:\\p" | null, "started_at": 1793886000000 | null}]}]}
```

`computers_set` args: `{device_name?, sync_mode?: "off"|"folder", sync_folder?, sync_transcripts?: bool, sync_presence?: bool}`.
Errors (as the dispatcher's string): "give this computer a name of up to 60 characters", "the mode is off or
folder", "the hub comes with a later version; choose a folder for now", "choose an existing folder (its full
path) to sync through".

`home {days?, hours?, harness?, scope?: "all"|"here", limit?, offset?}` returns the previous shape plus:

```json
{"overview": {...as before; "cost" and "heatmap[*].cost" are sums of cost_usd...},
 "requests": {"total": n, "rows": [{...as before, "cost": 0.0123 | null, "device": "uuid"}]},
 "dash": {...as before,
          "tiles": {"tokens":   {"by_device": [{"id": "uuid", "name": "Desk", "value": 123456}]},
                    "requests": {"by_device": [{"id", "name", "value": 42}]},
                    "cost":     {"by_device": [{"id", "name", "value": 1.23 | null}]},
                    "hit":      {"by_device": [{"id", "name", "value": 0.81}]}},
          "live": {"window_ms": 300000, "accounts": {"work": {"tokens": n, "requests": n}}, "providers": {...}, "breakers": {...},
                   "running": [{...presence running row..., "device": "uuid", "device_name": "Laptop"}]}},
 "activity": {"days": [{"day": "2026-11-05", "tokens": 1234, "by_device": {"uuid": 1000, "uuid2": 234}}]}}
```

`tiles[*].by_device` is sorted by tokens, biggest first, and lists only computers with rows in the window;
`activity.days` is the same 53 weeks as `overview.heatmap`, oldest first. `dash.live.accounts` sums this
computer's last five minutes with the peers' `presence.accounts` whose `at` is within five minutes.

`live` (restored): `{"now": ms, "live": {...the dash.live shape above...}}`.

`requests {days?, hours?, harness?, scope?, limit?, offset?}`: `{"total", "rows": [..., "device"]}`.

`sessions {filter: {..., scope?}, scope?, group?}`: as before; every `SessionMeta` gains
`"device": "uuid", "device_name": "Desk", "open_elsewhere": false`. Remote sessions have `open: false`,
their origin's `project_key`/`project_name`/`source_path`, and this computer's tags for `profile/id`.
`filter.open: true` lists this computer's open sessions only.

`projects {days?, scope?}`: each row gains `"devices": [{"id": "uuid", "name": "Desk", "sessions": 3, "tokens": 12345}]`
(in order of first appearance) and `"here": true|false`; `cost` is the sum of `cost_usd`.

`state.settings` carries the new settings fields (see the contract) with the ids filled in.

The app emits `computers` (payload: the `computers_status` shape) after each background round.

## For the merge into develop

- `Db::open(path, device, journal)`, `Db::insert_usage(rows, prices)`, `service::overview(ctx, days, harness, scope)`,
  `service::request_log_since(ctx, days, since_ms, harness, scope, limit, offset)`, `service::projects(ctx, days, scope)`,
  `dash::dashboard(ctx, hours, harness, scope)` changed signature; `dash::live` is `pub`. WP-1's harness
  modules call `insert_usage` with prices (`let prices = crate::pricing::load(ctx);` once per import, as
  `claude.rs` does); `cargo build` will point at any caller.
- `SessionMeta` and `SessionFilter` gained fields with serde defaults: older JSON still parses.
- The `sessions` table now holds three kinds of rows: per-file cache rows (`harness|profile|path`, device = this),
  this computer's journal mirror rows (`harness/profile/id`, device = this) and merged rows (`harness/profile/id`,
  device = the peer). Nothing reads mirror rows except the backfill and the dedupe.
- New kv keys: `migrated:device`, `journal:backfilled`, `journal:published_seq`, `computers:last` (the last
  two and `computers:*` are excluded from backups).
- `settings.json` is written at open when the ids are missing; the app's `AppState::ctx()` reopens once on
  that mtime change (no loop).
- A backup imported on another computer carries this computer's `device_id`: two computers would then share
  an id. Phase 4 ("restore keeps device_id, new incarnation") should make the import path mint a new id when
  the backup's id is already in `peers`.
- The first `hrm computers set --mode folder` on a computer with a large history runs the backfill and
  publishes everything in that call; a 200k-row history is a few MB of chunks.
