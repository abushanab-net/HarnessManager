# WP-7 report: resume here (phase 3), core and UI

Branch `feature/resume-here`, commits on top of `57685d9` (the plan). Nothing pushed. The line endings follow the
repository's autocrlf (every file is stored LF; the UI, e2e and CSS working copies are CRLF; the new Rust files LF).

## What was built, by file

- `crates/core/src/sync/transcripts.rs` (new): `carry(ctx, root)` runs on every publish round with Carry
  transcripts on. For each of this computer's journaled sessions whose source changed since it was carried
  (file mtime; OpenCode: `last_active_at`) it writes `devices/<id>/transcripts/<harness>/<profile>/<id>.<ext>.zst`
  (Claude Code, Pi, Oh My Pi: the `.jsonl`; Codex: the rollout, decompressed first when the origin kept it `.zst`;
  OpenCode: the JSON of `opencode session export <id>`, else `opencode export <id>`; Antigravity: nothing) and
  records `op: "transcript", key: "<harness>/<profile>/<id>", data: {file, bytes, sha256, mtime, source_rel}` in
  the outbox (`file` relative to the device folder, `source_rel` relative to the tool's home, forward slashes,
  `bytes`/`sha256` of the compressed file). Over 20 MB (or an export that fails) the entry carries
  `data.skipped: "<why>"` and the reason is kept in `transcripts_out`. Past 200 MB per computer the oldest by
  mtime are deleted from the folder with an `op: "tombstone", key: "transcript/<key>"` entry, never the newest one.
  A pruned or skipped session is not carried again until it changes (its `transcripts_out` row keeps the stamp:
  without that, a pruned transcript came back on the next round and was pruned again). `apply` stores a remote
  entry in `transcripts(device, key, file, bytes, sha256, mtime, source_rel, fetched_at)` (a skipped one removes
  what was known); `tombstone` deletes the row. Both tables are created on first use (`transcripts::tables`),
  as the contract asks, not in `db.rs`.
- `crates/core/src/transcripts.rs` (new): `FetchedTranscript`, `Materialized`; `fetch` (reads the folder's file
  once, checks its sha256, decompresses it to `<data>/transcripts/<origin>/<harness>/<profile>/<id>.<ext>`, marks
  `fetched_at`; later reads come from the copy, so the folder need not be reachable); `roots_here` (project key ->
  this computer's root, from the distinct folders of this computer's usage rows and sessions, through the
  Resolver, existing folders only); `resumable` (the contract's three reasons in its order, plus "resuming
  elsewhere is not available for Antigravity"); `forks` (the kv `forked:*` records); `resume_here` (the three
  errors, fetch, materialize, `service::start` with `Materialized.launch_args` in `Materialized.cwd`, the kv
  record `forked:<harness>/<profile>/<id>` = `{from, at}`); `status`; `remote_transcript` (the fetched copy through
  `claude::parse_session`, `codex::parse_rollout`, `pi::parse_session`; an OpenCode export through a small
  `{messages:[{info, parts}]}` reader); four tests.
- `crates/core/src/harness/mod.rs`: `materialize(ctx, p, local_root, t) -> Result<Materialized>` with the default
  error `resuming elsewhere is not available for <tool>`, and `resumes_elsewhere() -> bool` (false by default) so
  `resumable` can name Antigravity without fetching anything.
- `crates/core/src/harness/claude.rs`: `project_dir_name(cwd)` and `materialize`: one copy at
  `<home>/projects/<encoded local_root>/<id>.jsonl` (reused when there, never rewritten), launch args
  `["--resume", <absolute path of the copy>, "--fork-session"]`, cwd `local_root`.
- `crates/core/src/harness/codex.rs`: `materialize`: the rollout under `<home>/sessions/YYYY/MM/DD/<original name
  without .zst>` (the date from the name; a name without one is an error), args `["-C", <local_root>, "resume", <id>]`.
- `crates/core/src/harness/opencode.rs`: `import_args(v2, file, root)` and `materialize`: runs
  `opencode session import <file> --directory <local_root>` through the profile's launch spec (so `--standalone`
  and the XDG folders apply), on failure `opencode import <file>`, then args `["--session", <id>]`.
- `crates/core/src/harness/pi.rs`: `session_dir_name(cwd, home)` (`--<cwd with / \ : as ->--`; with a home given,
  a folder under it is `-<relative>`, Oh My Pi's form) and `materialize` for both tools: the log under
  `<home>/sessions/<encoded>/<original name>`; Pi args `["--session", <path of the copy>]`, Oh My Pi `["--resume", <id>]`.
- `crates/core/src/model.rs`: `SessionMeta` gains `resumable: Option<Value>` and `continued_from: Option<String>`
  (serde default, skipped when absent, so journaled rows and older JSON are unchanged).
- `crates/core/src/service.rs`: `sessions()` sets `resumable` on every remote row (roots computed once per call,
  only when there are remote rows) and `continued_from` on local rows whose key has a `forked:` record;
  `transcript()` prefers this computer's row when a local copy shares its id with the row from elsewhere.
- `crates/core/src/journal.rs`: `round()` calls `carry` between the backfill and `publish`, so a transcript goes
  out in the same chunk as its session; `apply()` gains the `transcript` arm and the `transcript/<key>` tombstone arm.
- `app/src-tauri/src/main.rs`: ops `session_resume_here`, `session_fetch`, `transcripts_status` (all in `READS`),
  and `transcript` takes `device`.
- `crates/cli/src/main.rs`: `hrm resume-here <computer>/<tool>/<account>/<id>` (computer by name, case-insensitive,
  or id) and `hrm transcripts [--json]`.
- `app/ui/resume.js` (new): `resumeHereDialog(s)` (the mock's dialog: title, project · account · tool, "Last ran
  on <name> · <when>", "Opens in <this computer's root of the project>" from the `projects` op, the two sentences
  about the fork and what does not come across, Cancel / Resume here -> `session_resume_here`, then the note as a
  toast), the `resume-here` action, `resumeData(s)` (the row as JSON on the button) and `reasonShort(r)`.
- `app/ui/accounts.js` (`sessionRow`): a remote row shows **Resume here** when `resumable === true`
  (`data-testid=resume-here-<id>`), else the reason's short form in muted text with the full text as its title
  (`reason-<id>`); the title click of a remote row opens the session with its `device`; "continued from <name>"
  before the folder line under the title (`continued-<id>`); `open-session` passes `device` through.
- `app/ui/views.js`: the session view calls `transcript` with `device`, toasts "Fetching from the folder…" the
  first time a remote session opens, shows "ran on <name>" / "continued from <name>" in the meta line, and for a
  remote session replaces Export/Resume with **Resume here** (or the reason); the Carry transcripts popover states
  both caps and "Carried N · M MB [· K skipped]" from `transcripts_status` (fetched only with sync on).
- `app/ui/app.css`: one rule for the "continued from" marker.
- `e2e/app.e2e.mjs`: one test appended under `// ---- resume here (WP-7) ----` (the WP-4 seeding shape copied, not
  changed): seeds LAPTOP with two session entries, a transcript entry and the compressed transcript, chooses the
  folder, asserts Resume here on the cloned project's row and "not cloned here" on the other, reads the remote
  session (the answer text, "ran on LAPTOP", the fetched copy on disk), opens the dialog (the local clone named),
  resumes, and asserts the terminal record `run claude-main -- --resume <...laptop-s7.jsonl> --fork-session` with
  cwd = the sandbox repository and the copy under the sandbox's `.claude/projects`, then "continued from LAPTOP" on
  the copy's row and the Carry transcripts popover text.
- `README.md`: the "Resuming elsewhere" paragraph under the Computers bullet.

## Decisions where the contract was silent, and deviations

- **Claude Code's folder name** replaces every character that is not a letter or digit with `-`, not only `\ / :`:
  that is what Claude Code does (`~/.claude/projects/C--Users-sabus--claude-dev-mods` for `.claude-dev-mods` on
  this machine). The copy is resumed by its absolute path, so the name only decides where the copy sits.
- **Codex sidecar files**: none carried (a rollout has no sidecar I could find; `.zst` rollouts are carried plain).
- **The transcript op** is `transcript {profile, id, device?}` (the app's existing name for the contract's
  `session`); the CLI `hrm session` was not given a `--device` (not in the contract).
- **The "fetching" toast** is shown by the window when a remote session is opened the first time in a visit,
  before the call; the core logs the fetch. The transcript answer has no field for it.
- **The dialog's local folder** comes from the `projects` op (the group's root), not from a new field on the row.
- **`resumable`** also says "resuming elsewhere is not available for Antigravity" for that tool (a fourth text),
  so the row does not offer a button that can only fail.
- **A fetched copy stays on disk after a tombstone** (the merge has no data folder at hand; the row goes). It is
  a cache under `<data>/transcripts`, never read once the row is gone. Marked as a shortcut in the code.
- **Export as Markdown** is not offered on a remote session (its export reads the tool's own file by path).
- **`hrm resume-here`** resolves the computer by name case-insensitively, then by id.
- **The carry runs inside `round()` before `publish`**, so the entry and the file land in the same round; an error
  in the carry is logged and never stops the round.
- The second computer's `device.json` roots are not used for `resumable` (the contract's `local_root` is this
  computer's, from its usage rows and sessions; `device.json` roots of *this* computer are the same data).

## Skipped

- The direct-connection route for transcripts (`GET /v1/transcript/...`): phase 2's follow-up, as the contract says.
- Deleting the fetched copy on tombstone (above).
- A Rust test that runs the real `opencode` import or export (the arg builder and the fallback order are tested;
  the fake harness answers nothing, so in the window suite OpenCode's export is recorded as skipped with
  "opencode export gave no session JSON").

## Tests (verbatim)

```
cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.86s

cargo test --workspace
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.34s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.92s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.89s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

cargo test -p hrm-core --lib transcripts
test transcripts::tests::materialize_claude_codex_pi_omp_opencode_and_the_antigravity_default ... ok
test transcripts::tests::transcripts_are_carried_on_publish_and_fetched_once_on_demand ... ok
test transcripts::tests::resume_here_names_its_three_errors_then_launches_each_tool_in_the_local_clone ... ok
test transcripts::tests::a_transcript_over_the_cap_is_skipped_and_the_folder_is_pruned_at_the_cap_with_tombstones ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out; finished in 0.65s

cargo clippy --workspace: no warning in the new code (the pre-existing ones: sort_by_key x8, type_complexity,
manual_contains, chunks_exact_to_as_chunks, result_large_err, an empty format string; all on lines unchanged here).

cd e2e && node app.e2e.mjs   (after cargo build --workspace)
43 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\resume\e2e\.work\shots
(the first full run had 42 passed, 1 failed: my test's terminal-log lookup tripped on records without `args`;
one guard fixed it, the single test then passed, and the whole suite was run again for the line above)
Screenshots looked at: 41-sessions-resume-here.png (the row with Resume here, "not cloned here" on the other),
42-session-remote.png (the remote transcript, "ran on LAPTOP", the fetching toast), 43-resume-here-dialog.png.

CLI smoke (HRM_HOME and HRM_PROFILES_HOME in a temp folder):
$ hrm transcripts
carried 0 (0.0 MB of 200 MB); from the other computers 0 known, 0 fetched            exit 0
$ hrm transcripts --json
{"carried": 0, "bytes": 0, "cap_bytes": 209715200, "skipped": [], "remote": 0, "fetched": 0}   exit 0
$ hrm resume-here LAPTOP/claude/work/abc
error: no computer named LAPTOP                                                       exit 1
$ hrm resume-here bad
error: give <computer>/<tool>/<account>/<session id>                                  exit 1
```

The four core tests cover: the transcript carried on A's publish (zstd, sha256 and `source_rel` in the entry, in the
same round as the session, not again when unchanged), stored on B's merge without a file, fetched once on demand
(the second read works with the folder gone) and read through Claude Code's parser with B's device name; a 20 MB+1
transcript skipped with `data.skipped: "too big"` and listed in `transcripts_status.skipped`, a 1-byte cap pruning
to the newest with a tombstone and the file gone from the folder, B keeping one row after the merge; `materialize`
for Claude Code (Windows and Linux folder names, the copy reused on a second call), Codex (the date folder from the
name, `.zst` stripped), Pi (`--sessions--` folder, `--session <path>`), Oh My Pi (`-<relative>` folder,
`--resume <id>`), OpenCode (the import command chosen) and Antigravity (the default error); `resume_here` with the
three errors in order (each also what `sessions()` puts in `resumable`), then the launch recorded as
`["run","claude-main","--","--resume","<copy>","--fork-session"]` with cwd = B's clone, the `forked:` record, the
copy listed as B's session with `continued_from: "A"`, and a second Resume here reusing the one copy.

## The launch args per tool, as the tests saw them

| Tool | `hrm run <profile> -- …` | cwd |
|---|---|---|
| Claude Code | `--resume <home>\.claude\projects\<encoded clone>\<id>.jsonl --fork-session` | the clone |
| Codex | `-C <clone> resume <id>` | the clone |
| OpenCode | `--session <id>` (after `session import <file> --directory <clone>`, else `import <file>`) | the clone |
| Pi | `--session <home>\.pi\agent\sessions\--<encoded clone>--\<original name>` | the clone |
| Oh My Pi | `--resume <id>` (the copy under `.omp\agent\sessions\-<clone relative to home>\`) | the clone |
| Antigravity | error `resuming elsewhere is not available for Antigravity` | — |

## The exact op shapes

```
transcript          {profile, id, device?}            -> Transcript (as today; with device = another computer: its
                                                         fetched copy, meta = the sessions row with device, device_name, resumable)
session_resume_here {device, harness, profile, id}    -> {"ok": true, "cwd": "D:\\p\\app", "args": ["--resume", "...", "--fork-session"], "note": "..."}
                     errors (the dispatcher's string): "not cloned here: open <project> once on this computer",
                     "no account <profile> for <tool> on this computer", "running on <name>: wait until it ends",
                     "no transcript from <name> for this session yet: it is carried on that computer's next sync round",
                     "resuming elsewhere is not available for Antigravity", "session not found"
session_fetch       {device, harness, profile, id}    -> {"path": "<data>\\transcripts\\<origin>\\claude\\work\\abc.jsonl", "bytes": 12345}
transcripts_status                                    -> {"carried": 3, "bytes": 48213, "cap_bytes": 209715200,
                                                          "skipped": [{"key": "claude/work/abc", "why": "too big" | "at the cap" | "opencode export gave no session JSON"}],
                                                          "remote": 2, "fetched": 1}
sessions rows from another computer                   + "resumable": true | "<reason text above>"
sessions rows of this computer continued from another + "continued_from": "<computer name>"
journal entries     {"op": "transcript", "key": "<harness>/<profile>/<id>", "data": {"file": "transcripts/claude/work/abc.jsonl.zst", "bytes", "sha256", "mtime", "source_rel": "projects/C--p/abc.jsonl"}}
                    {"op": "transcript", "key": ..., "data": {"skipped": "too big"}}
                    {"op": "tombstone",  "key": "transcript/<harness>/<profile>/<id>", "data": {}}
hrm resume-here <computer>/<tool>/<account>/<id>      prints "Resumed <id> from <computer> in <cwd>: <note>"
hrm transcripts [--json]
```

New kv keys: `forked:<harness>/<profile>/<id>` = `{"from": "<device id>", "at": ms}` (kept in backups, as tags are).
New tables (created on first use by `transcripts::tables`): `transcripts`, `transcripts_out`.

## Every line touched in shared files

- `crates/core/src/journal.rs`: `round()` lines 235–236 (the carry call before `publish`); `apply()` line 153 (the
  `("transcript", k)` tombstone arm, before the catch-all) and lines 156–157 (the `"transcript"` arm, before the catch-all).
- `crates/core/src/service.rs` (`sessions()`): line 440 (`let forks = …`), lines 456–458 (`let remote …`, `let roots …`,
  the loop header), line 465 (`s.resumable = …`), line 495 (`s.continued_from = …`); `transcript()` lines 636–638
  (prefer this computer's row).
- `crates/core/src/model.rs`: lines 130–135 (the two `SessionMeta` fields). Not in the package's path list; needed
  for `resumable` and `continued_from`, serde-defaulted so nothing else changes.
- `app/src-tauri/src/main.rs`: `READS` line 67 (three names appended), the `transcript` op lines 221–225, the three
  new ops lines 619–625 (before `other => bail!`).
- `crates/cli/src/main.rs`: `Cmd` lines 119–122 (two variants appended), line 273 (`| Cmd::ResumeHere { .. }` appended
  to the no-lock pattern), lines 585–599 (two match arms appended).
- `app/ui/views.js`: line 8 (import), the session view (the toast line, the `device` in the `transcript` call, the
  meta-line span, the actions block), `drawComputers` (the `ts` line after the status, the Carry transcripts row's
  popover text). The Computers section itself is otherwise untouched.
- `app/ui/accounts.js`: line 8 (import), `sessionRow` (the remote branch of the action cell, `data-device` on the
  title, `cfrom`), the `open-session` action.
- `app/ui/app.css`: two lines appended at the end.
- `e2e/app.e2e.mjs`: one block appended before `exitCode = …`.

## For the merge

- Phase 2 touches `journal.rs` (signing) and `main.rs` (peer ops): my hooks are the two arms in `apply()` (keep
  them before the catch-alls), the carry call in `round()` (keep it before `publish`), and three ops before
  `other => bail!`. `READS` gained three names at its end.
- `SessionMeta` has two new optional fields; `service::sessions()` and `service::transcript()` changed as listed.
- The `Harness` trait has two new defaulted methods; a new harness needs nothing.
- `remote_transcript` reads the fetched copy with the tools' public parsers; when phase 2 adds
  `GET /v1/transcript/...`, `transcripts::fetch` is the one place to add the direct route (it reads the folder
  under `if fetched_at.is_none() || !path.exists()`).
- A Codex session resumed here without a fork (`codex resume` appends to the copied rollout) makes the copy newer
  than the origin's row: the journal's "newest `last_active_at` wins" rule then lists the session as this
  computer's, with `continued_from`. Claude Code's `--fork-session` never writes the copy, so its rows stay as they are.
- `e2e/node_modules` was installed in this worktree (`npm install` in `e2e/`) to run the suite; it is ignored by git.
