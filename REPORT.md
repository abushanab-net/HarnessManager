# WP-8 report: backup, archive, restore and Welcome back (phase 4)

Branch `feature/backup-archive`, commits on top of `7ce97f5` (the plan). Nothing pushed. Line endings follow the
repository's autocrlf (stored LF; the UI, e2e, CSS and README working copies CRLF; the Rust files LF).

## What was built, by file

- `crates/core/src/ctx.rs`: the three settings `archive_in_sync_folder` (true), `backup_in_sync_folder` (true),
  `backup_transcripts` (false), appended after `archive_dir`; `archive_days` and `archive_dir` were already there
  (their comments now say 0 = never and "" = none, as the phase 4 contract has it, not `<sync_folder>/archive`).
- `crates/core/src/backup.rs`: the zip adds `history/usage.jsonl` (every row with `device`, `cost_usd`,
  `price_rev`), `history/sessions.jsonl` (every row of the `sessions` table: cache, mirror and merged, each with
  its `device`), `history/quota_hist.jsonl`, `history/tags.jsonl`, `history/archived.jsonl` (the archive table),
  `history/peers.json` (ids, names, os, version, incarnation, address, port, certificate, public key, roots; never
  keys) and `history/device.json` (id, name, incarnation, pubkey). With `backup_transcripts`, or
  `create_with(ctx, label, true)` / `backup_create {transcripts: true}` / `hrm backup --with-transcripts`:
  `transcripts/<harness>/<profile>/<id>.jsonl.zst` for every session of Claude Code, Codex, Pi and Oh My Pi
  whose file is still in the tool's folder (stored, not deflated again). `create` copies the zip to
  `<sync_folder>/backups/<device_id>/` when `backup_in_sync_folder` is on and a folder is set, keeping
  `backup_keep` there (a folder out of reach is logged, never a failed backup). `in_sync_folder(ctx)` lists every
  computer's backups in the folder with the computer's name, for the `backups` op and the Restore… picker.
  `export` follows the transcripts setting. The classic restore (`restore_file`) is unchanged.
- `crates/core/src/archive.rs` (new): table `archived(key, harness, profile, id, device, json, path, bytes, at)`
  made on first use (`tables`); `roots` (the sync folder's `archive/<device_id>` when on and set, then
  `archive_dir`); `run(ctx, days)`: for every account of Claude Code, Codex, Pi and Oh My Pi, each session older
  than `days` by `last_active_at`, not open here (a console names it, or it is among the newest for consoles that
  name none, as the session list reckons) nor elsewhere (presence), has its transcript compressed (zstd 3, Codex's
  `.zst` rollouts decompressed first so every copy is zstd of the log) to
  `<root>/<project name or no-folder>/<YYYY-MM>/<harness>-<profile>-<id>.jsonl.zst` in every root, each copy read
  back and compared by sha256, plus `<harness>-<profile>-<id>.json` beside it (the session's meta with
  `archived: true`, sha256, bytes, `archived_at`, device and name, the original path); only then are the files of
  `archive_paths` deleted; the row goes into `archived` with `source_path` = the first copy, the scan's cache row
  is removed. `archive_now` (the setting's age), `daily` (once a day while the app runs, from kv `archive:last`;
  0 never runs), `sessions` (the archived rows as session rows: `archived`, `device`, `device_name`, and from
  another computer `resumable` = the refusal text), `is_archived`, `transcript` (the copy through
  `claude::parse_session`, `codex::parse_rollout`, `pi::parse_session`), `status` (`days`, `archived`, `bytes`,
  `last`, `locations[{path, kind, on, sessions, bytes}]`, each location counted from its own files), the refusal
  text `archived: it can be read, not resumed`. One test.
- `crates/core/src/restore.rs` (new): `Source::{Zip, Folder}` (`parse` from `{zip?, folder?}`, `of_path`); a
  `History` gathered from a zip (`history/*`, `profiles.json`) or a folder (`devices/*`: `device.json`, the
  manifest's chunks in order with `usage`, `session`, `quota`, `tag` and session tombstones; `backups/*`: the
  newest zip of each computer; `archive/**/*.json`: the metas, their `.jsonl.zst` beside them); the scan's cache
  rows of a zip (`<harness>|<profile>|<path>`) come back under their journal key `<harness>/<profile>/<id>` as
  sessions of that computer. `preview` answers the counts. `run(ctx, src, adopt)` merges in one transaction:
  usage by `request_id` through `upsert_usage` (the existing rule), sessions by key through `upsert_session`
  (newest `last_active_at`), limit readings in time order when they differ from the last and are not already
  there as the same (profile, tier, ts, used) row, tags only for keys this computer has none for, peers added
  when unknown (and a bare row for every computer the source names), archived rows added when unknown, the
  zip's accounts added by name when missing here (no keys or logins), and `cursors2` moved past what a folder's
  journals held, so the sync merge does not apply it again. `adopt`: refused without a record of this
  computer's name ("no record of this computer's name in it: restore as a new computer instead") or when a peer
  with that id was heard from or connected within five minutes ("another computer uses this identity"); else the
  rows under the old id are re-stamped, the peer row of the adopted id removed, `device:pubkey` and
  `journal:published_seq` cleared, `journal:backfilled` set to 0, and `settings.json` written with the id, the
  name and a new incarnation. `welcome` (the folder's preview when `sync_mode` is folder, `journal:backfilled`
  is absent and the question was not answered before; null otherwise), `welcome_done`. Three tests.
- `crates/core/src/harness/mod.rs`: `archive_paths(ctx, p, s) -> Vec<PathBuf>`, default the transcript alone.
  No harness overrides it (no tool keeps sidecar files WP-7 could find).
- `crates/core/src/model.rs`: `SessionMeta.archived: bool` (serde default, skipped when false).
- `crates/core/src/service.rs`: `sessions()` appends the archived rows (this computer's only with scope "here";
  never with `open: true`), with their tags, before the sort; `transcript()` reads an archived row from the
  archive; `resume()` refuses an archived session with the text.
- `app/src-tauri/src/main.rs`: ops `restore_preview`, `restore_run` (marks Welcome back answered and reopens
  the context, so an adopted identity holds at once), `restore_welcome {done?}`, `archive_status`,
  `archive_now`; `backups` gains `in_sync_folder`; `backup_create` takes `transcripts`; the refresh loop runs
  `archive::daily` after the automatic backup.
- `crates/cli/src/main.rs`: `hrm backup [--with-transcripts] [--json]`, `hrm restore <zip|folder> [--adopt]
  [--preview] [--json]`, `hrm archive status | now [--json]`.
- `app/ui/views.js`: the section is "Backups and archive" (`data-testid=sec-backups-and-archive`): the existing
  rows, "Also in the sync folder" (`st-bk-folder`), "Include transcripts" (`st-bk-transcripts`), the sync
  folder's backups under this computer's list (each with Restore…), "Archive sessions older than" (segment
  Never · 30 days · 90 days · 1 year, `st-archive-days`, plus a days input with a Set button,
  `st-archive-custom`), "Archive location" (the sync folder switch `st-archive-sync`, the chosen folder
  `st-archive-dir` with Choose…/Change… and None), "Archive now" with the status line (counts, each location,
  the last run), "Restore…" (`restore-open`). The session view marks an archived session with the pill and
  shows the refusal text instead of Resume. The folder chooser calls `welcomeBack(p)` after `computers_set`.
- `app/ui/restore.js` (new): `restoreDialog(src, {welcome, preview})`: the preview's counts, the computers, the
  path, and Start fresh / Cancel, "Restore as a new computer" (`welcome-new`), "Restore and continue as <name>"
  (`welcome-adopt`, only with a record of this computer's name); `welcomeBack(folder)`; the first-launch check
  (`restore_welcome` on load); the Restore… picker (`restore-pick`: the sync folder, this computer's backups,
  the folder's backups, a folder…, a zip file…) and `restore-src`.
- `app/ui/accounts.js` (`sessionRow`, two lines): the "archived" pill after the title (`archived-<id>`), no
  Resume on an archived row, and the title click of an archived row from another computer reads it locally
  (no `data-device`), since the copy is in the archive, not the folder's transcripts.
- `app/ui/app.css`: ten rules appended (the pill, the folder in a row, the Restore picker, the counts list).
- `e2e/app.e2e.mjs`: one test appended under `// ---- backup, archive and welcome back (WP-8) ----`, and the
  WP-4 assertion of the section order now expects "Backups and archive" (one string).
- `README.md`: the Backups bullet grew the paragraph on the history in backups, the sync-folder copy, Restore as
  a merge, adopt, the archive, Welcome back and the three commands.

## Decisions where the contract was silent, and deviations

- **Archived sessions live in their own table** (`archived`), not in `sessions`: the journal, the backfill, the
  carry and `forget_profile` all read `sessions` by device and key shape, and an archived row must reach none
  of them. The backup carries it as `history/archived.jsonl`.
- **OpenCode is not archived.** Its transcript is rows in its own SQLite database, so "delete the original from
  the tool's folder" would mean deleting from OpenCode's database, and the export needs the real binary. Pi and
  Oh My Pi are archived like Claude Code and Codex. Antigravity never.
- **A restored zip also brings back its account list**, by name, only for accounts missing here (the Welcome
  back mock says "each account asks you to log in again"); settings, skills and the ledger are not touched by
  `restore_run` (the classic Restore button on a backup row still does that).
- **A record with this computer's own id is this computer**, never a past self to adopt: the folder gets one the
  moment it is chosen, and a backup of this computer names it. Only another id with this computer's name is
  offered as "Restore and continue as <name>"; this computer's own backup or folder shows a plain "Restore".
- **A live peer** is one heard from or connected within five minutes (the `peers` row's `last_seen` or
  `connected`).
- **Welcome back is asked once**: kv `welcome:done` is set when any button is pressed, so a folder with history
  but no answer does not ask at every launch; the contract's `journal:backfilled` condition still gates it.
- **Welcome back runs after `computers_set`** in the chooser, so Start fresh still leaves the folder chosen; the
  sync round that ran first makes the restore mostly a no-op (nothing twice), except the folder's backups and
  archive metas, which sync never reads.
- **After adopt, this computer's past sessions whose files are not here** are in the table but not in the
  session list (the list reads this computer's sessions from the tools' folders; archived ones are listed).
- **The "custom" age** is a number input beside the segment, saved with its button.
- `hrm restore --preview` is extra, for scripts.

## Skipped

- OpenCode in the archive (above). No harness overrides `archive_paths`.
- The identity after adopt: the other computers that learned the old installation's public key will refuse
  records and chunks signed by this one's new key (`learn_device` keeps the first key). Nothing in this phase
  can carry the private key across; they need to forget and pair again, or the hub (phase 5) to re-key. Noted
  for the merge.
- Materializing an archived session back for resume (the contract defers it).

## Tests (verbatim)

```
cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s)

cargo test --workspace
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.14s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.34s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.36s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.68s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
(two of four full runs had the CLI's routed_profile_starts_the_router_and_gets_its_address_and_token fail on
"the router did not start on port <n>"; it passed alone each time and in the run above: a port race on a machine
running other sessions' tests at the same time, nothing of this package)

cargo test -p hrm-core --lib -- archive restore backup
test backup::keep_tests::the_router_token_and_caches_stay_out_of_backups ... ok
test backup::keep_tests::the_zip_holds_the_history_files_and_with_the_switch_the_transcripts ... ok
test archive::tests::the_archive_moves_only_old_closed_sessions_into_both_locations_keeps_them_listed_and_readable_and_respects_never ... ok
test restore::tests::a_restore_into_an_empty_database_reproduces_the_counts_and_a_second_restore_adds_nothing ... ok
test restore::tests::a_sync_folder_restores_every_computers_journal_and_its_backups_and_archive ... ok
test restore::tests::adopt_takes_the_id_and_a_new_incarnation_and_is_refused_for_a_live_peers_id ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.42s

cargo clippy --workspace: no warning in the new code (the pre-existing ones: sort_by_key in codex.rs, type_complexity,
manual_contains, chunks_exact_to_as_chunks, result_large_err in the router, the empty format string at cli main.rs:483).

cd e2e && HRM_E2E_SYNC=1 HRM_E2E_PEERS=1 node app.e2e.mjs   (after cargo build --workspace)
47 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\backup\e2e\.work\shots
(the first full run had 46 passed, 1 failed: my test's assertion on the CLI's plural ("1 sessions moved") and a
backspace byte that a patch script had put in the regex; both fixed, the single test passed, and the whole suite
was run again for the line above. The worktree's e2e had no node_modules: `npm ci` first.)
Screenshots looked at: 44-settings-backups-archive.png (the section's rows), 45-sessions-archived.png (the pill, no
Resume), 46-session-archived.png (read from the archive, "archived: it can be read, not resumed"),
47-restore-dialog.png (the counts of this computer's backup, a plain Restore), 48-welcome-back.png (both
computers listed, Start fresh / Restore).

CLI smoke (HRM_HOME and HRM_PROFILES_HOME in a temp folder):
$ hrm backup
Backed up: 2026-11-05 20.02.54 Manual.zip (0.0 MB)                                   exit 0
$ hrm backup --with-transcripts --json                                                {"name": ..., "label": "Manual", ...}
$ hrm archive status
0 archived (0.0 MB); sessions older than 30 days                                      exit 0
$ hrm archive now
0 sessions moved to the archive (0.0 MB); no archive location: turn on the sync folder or choose a folder
$ hrm restore <zip> --preview
0 requests, 0 tokens, 0 sessions, 0 accounts with limit history, 0 archived transcripts, newest -
  DEVELOPMENTPC            0 requests, 0 sessions
  DEVELOPMENTPC is this computer's name: --adopt continues as it                      exit 0
$ hrm restore <zip> --adopt
Restored: 0 requests, 0 sessions, 0 limit readings, 0 tags, 0 computers, 0 archived transcripts, 0 accounts added; this computer continues as DEVELOPMENTPC
$ hrm restore <missing>
error: ...
ope is not a folder or a zip                                                exit 1
$ hrm restore <a folder that is not a sync folder>
error: ...\arch is not a sync folder (no devices, backups or archive in it)             exit 1
```

The core tests cover: the zip's ten entries with the usage row's `device` and the device record, and the transcript
entry with the switch; the archive leaving a fresh session and moving a 2026-01 one into both locations (equal
copies, the meta beside, zstd of the log), listed with `archived` and its numbers, read through Claude Code's
parser, refused by `resume`, counted per location, nothing moved with 0 days or on a second run; a restore of A's
backup into empty B reproducing 2 requests (A's frozen cost), 1 session listed as A's, 2 readings, the tag, A as a
peer, the archived row and the account, then a second run adding nothing (every count 0); a sync folder restored
from A's journal, the backup copied beside it and the archive table, with `cursors2` moved past it; adopt refused
while a peer answers to A's id, then taking A's id and name with a new incarnation, nothing left under the old id,
no peer row, the journal starting again, and refused on a computer with another name.

## The op shapes

```
backups           -> as before + "in_sync_folder": [{"name", "label", "at", "size", "device", "device_name", "path"}]
backup_create     {label?, transcripts?: bool}        -> Backup (as before)
restore_preview   {zip?, folder?}                     -> {"source": "zip"|"folder", "path", "computers": [{"id", "name", "requests", "sessions"}],
                                                          "requests", "tokens", "sessions", "accounts", "archived", "newest": ms|null,
                                                          "this": {"id", "name"}|null, "empty": bool}
restore_run       {zip?, folder?, adopt: bool}        -> {"usage", "sessions", "quota", "tags", "peers", "archived", "profiles": n added, "adopted": bool, "device": {"id", "name"}|null}
                   errors: "another computer uses this identity", "no record of this computer's name in it: restore as a new computer instead",
                           "<path> is not a sync folder (no devices, backups or archive in it)", "<zip> holds no history (a backup from before 1.6 ...)"
restore_welcome   {}            -> the preview above, or null;   {done: true} -> true
archive_status                  -> {"days", "archived", "bytes", "last": {"at", "archived", "bytes"}|null, "locations": [{"path", "kind": "sync_folder"|"folder", "on", "sessions", "bytes"}]}
archive_now                     -> {"archived", "bytes", "why"?}
sessions rows                   + "archived": true (readable; a row from another computer carries "resumable": "archived: it can be read, not resumed")
resume on an archived session   error "archived: it can be read, not resumed"
hrm backup [--with-transcripts] · hrm restore <zip|folder> [--adopt] [--preview] · hrm archive status | now
```

New kv keys: `archive:last` = `{"at", "archived", "bytes"}`, `welcome:done` = "1". New table: `archived`.
The zip: `history/{usage,sessions,quota_hist,tags,archived}.jsonl`, `history/{peers,device}.json`,
`transcripts/<harness>/<profile>/<id>.jsonl.zst`. The folder: `backups/<device_id>/<zip name>`,
`archive/<device_id>/<project>/<YYYY-MM>/<harness>-<profile>-<id>.{jsonl.zst,json}`.

## Every line touched in shared files

- `crates/core/src/service.rs`: `resume()` line 331 (the refusal); `sessions()` lines 504–511 (the archived rows,
  before the sort); `transcript()` line 648 (the archived branch).
- `app/src-tauri/src/main.rs`: `READS` lines 75–76 (four names appended); lines 578–579 (`backups` gains
  `in_sync_folder`, `backup_create` takes `transcripts`); lines 643–656 (the five ops, before `other => bail!`);
  line 789 (`archive::daily` after the automatic backup in the refresh loop).
- `crates/cli/src/main.rs`: `Cmd` lines 123–148 (three variants and `ArchiveCmd`, appended); lines 701–732 (the
  three match arms, appended).
- `crates/core/src/ctx.rs`: lines 66–75 (the fields, after `archive_dir`) and 85–86 (the defaults).
- `crates/core/src/model.rs`: lines 136–138 (`archived`). Not in the package's paths; serde-defaulted.
- `crates/core/src/harness/mod.rs`: lines 75–77 (`archive_paths`). `crates/core/src/lib.rs`: lines 25–26.
- `app/ui/accounts.js` (`sessionRow`): lines 271 and 277. Not in the package's paths; the contract's pill is on
  the row, which lives there.
- `app/ui/views.js`: line 9 (import), 381 and 386 (the session view), 714 (the section's name), 782 (the
  chooser), 815–849 (`drawBackups` and the four archive actions).
- `e2e/app.e2e.mjs`: line 1268 (the WP-4 section-order string) and the appended block from line 1585.

## For the merge

- `Settings` gains three fields with defaults; older `settings.json` files load unchanged.
- `archived` is made on first use by `archive::tables`; `backup::history` and `restore::run` call it too.
- Phase 5 (the hub): `restore::read_folder` reads the folder transport only; a hub restore would feed the same
  `History`. `in_sync_folder` and `copy_to_folder` key on `sync_mode == "folder"`.
- The adopted-identity key problem above.
