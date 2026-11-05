# WP-1 Pi and Oh My Pi, WP-2 DeepSeek provider: report

Branch `feature/tools-pi-omp-deepseek`, three commits on top of `220133b`, not pushed:

```
e371f20 Pi and Oh My Pi as tools: one parser for the version-3 session log, usage, sessions, transcripts, apps and icons
28322fc DeepSeek as a router provider: presets for both endpoints, list prices, cache hits in the usage, README
ddf65ac Fixtures stay LF on every checkout (the Oh My Pi title slot is 256 bytes with its newline)
```

## Test results (verbatim)

`cargo build --workspace`: `Finished \`dev\` profile [unoptimized + debuginfo] target(s)`.

`cargo test --workspace` (after the build; the CLI's own tests need the binaries):

```
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.30s   (hrm, tests/e2e.rs)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s   (hrm-core, lib)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s   (hrm-core, tests/data_safety.rs)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s    (hrm-core, tests/pi_sessions.rs)
```

plus the router's unit tests (one new: `deepseek_cache_hits_are_reported_apart_from_fresh_input`), all green.
`cargo clippy --workspace`: no warning in the files this package added or changed (the pre-existing warnings
in other files are untouched).

`cd e2e && node app.e2e.mjs` (after `cargo build --workspace`):

```
40 passed, 0 failed; screenshots in D:\Projects\abushanab-net\HarnessManager-wt\tools\e2e\.work\shots
```

Screenshot `40-pi-omp.png` looked at: the Accounts page with the Pi and Oh My Pi chips and the `pi-main` /
`omp-main` series in the usage chart.

## What was built, by file

### WP-1 Pi and Oh My Pi

- `crates/core/src/harness/pi.rs` (new): `PiLike`, one `Harness` implementation used for both tools
  (`pub static PI`), and the shared parser for the version-3 session log.
  - `usage_rows(text, harness, profile)`: a row per assistant `message` entry and per `usage` (Pi) /
    `model_usage` (Oh My Pi) entry; `request_id = <harness>:<session id>:<entry id>`; `project` = header
    `cwd`; `cache_read`/`cache_write` from `cacheRead`/`cacheWrite`; `ts` = the message's own ms timestamp;
    `gen_ms` = entry timestamp − message timestamp (the entry is written when the reply is complete);
    `ttft_ms` = None (the log has no first-token time).
  - `parse_session(text, harness, profile, path, full)`: id and cwd from the header, title from the Oh My Pi
    `title` slot, else a `session_info` name, else the first prompt; prompts, tool calls (`toolCall` blocks),
    failed calls (`toolResult.isError`), messages, model (last assistant or `model_change`), tokens
    (input + output of assistant replies); turns with thinking, tool steps (edit/write carry a diff),
    results and durations.
  - Harness: home `~/.pi/agent`, profile homes `~/.pi-<name>` through `PI_CODING_AGENT_DIR`; `/login`
    happens inside the tool so `login` opens it; identity from the providers in `auth.json` plus the
    `auth_credentials` table of `agent.db` (Oh My Pi); import with a per-file cursor as Codex does;
    sessions cached by mtime/len; `resume_args` `--session <id>` (Pi) / `--resume <id>` (Oh My Pi);
    `running` = a `pi`/`omp` process whose cwd matches a session file written in the last two minutes;
    item paths `mcp.json` (Claude Code's `mcpServers` shape, format `claude-json`), `skills/`, `AGENTS.md`.
  - API-key and routed Pi accounts: launch writes an `hrm` provider into the profile's `models.json`
    (`baseUrl`, `api` = `openai-completions` for a key with `--base-url`, `anthropic-messages` for the
    router, `apiKey: "$HRM_PI_KEY"` so the key stays out of the file) and starts `pi --model hrm/<model>`;
    a model is required.
- `crates/core/src/harness/omp.rs` (new): `pub static OMP`, the same type with its names, `~/.omp/agent`,
  `--resume`, and caps without API-key/routed (its providers are `models.yml`, no YAML writer here).
- `crates/core/src/harness/mod.rs`: `pub mod omp; pub mod pi;` and both in `all()` (now a `static` array,
  since the entries are statics).
- `crates/core/src/apps.rs`: installers `PI` (npm `@earendil-works/pi-coding-agent`, then the deprecated
  `@mariozechner/pi-coding-agent` so an existing copy is still recognised and updated in place) and `OMP`
  (npm `@oh-my-pi/pi-coding-agent`); `newest()` reads the npm registry; `--version` detection comes for
  free through `run_version`. Two assertions added to the existing unit test.
- `crates/core/tests/fixtures/pi/--D--Projects-demo--/*.jsonl` (two: one with read/edit/bash and a failed
  command plus a `usage` cache-warm entry; one fork with `parentSession`, `branch_summary` and
  `session_info`), `crates/core/tests/fixtures/omp/-Projects-demo/*.jsonl` (title slot of exactly 256
  bytes, `model_usage` entry, DeepSeek models). `.gitattributes` keeps them LF.
- `crates/core/tests/pi_sessions.rs` (new, 5 tests): row counts and sums (input, output, cache read/write,
  request ids, timing), Oh My Pi rows, session facts and the transcript for both tools, idempotent
  re-import (twice, then with the cursors deleted: 10 rows, never 20), registration, homes, caps, identity.
- `crates/cli/tests/e2e.rs` (outside the package's listed paths, one assertion): `hrm harnesses` now
  returns 6; the "all installed" check skips `pi`/`omp`, which are whatever the computer has.
- `app/ui/icons/pi.svg`, `app/ui/icons/omp.svg`: simple π marks on a rounded tile, viewBox
  `-1.5 -1.5 27 27` like `agy.svg`. Icons are looked up by harness id (`lib.js::toolIcon`), so nothing
  else to wire.
- `hrm harnesses` lists both and `hrm init` adds `pi-main`/`omp-main` when `~/.pi/agent` / `~/.omp/agent`
  exist: both come from `all()` with no CLI change.

### WP-2 DeepSeek provider

- `crates/core/src/registry.rs`: `provider_presets()` with `deepseek` (`https://api.deepseek.com`, format
  `openai`) and `deepseek-anthropic` (`https://api.deepseek.com/anthropic`, format `anthropic`, for Claude
  Code routes), models `deepseek-flash`, `deepseek-v4-pro`; unit test.
- `crates/core/src/pricing.rs`: `builtin()` list prices (quoted with the page and date in the comment:
  flash 0.30 / 0.006 cache hit / 1.20 out; v4-pro 1.32 / 0.044 / 3.96; cache writes free; peak rates, the
  off-peak half price is not modelled) that win over the catalog; unit test.
- `crates/router/src/convert.rs`: `usage_of()` turns `prompt_cache_hit_tokens` (or OpenAI's
  `prompt_tokens_details.cached_tokens`) into `cache_read_input_tokens` and subtracts them from
  `input_tokens`, in replies and in streams (`StreamConv` carries `cache_read`); test added.
- `crates/router/src/lib.rs`: the non-streamed log line sums `input_tokens + cache_read_input_tokens`, so
  the `input` column keeps every input token as before.
- `crates/cli/src/main.rs` (outside the listed paths, 4 lines in `ProviderCmd::List`): presets not yet
  added are printed after the providers, each with the `hrm provider add ...` command that adds it.
- `README.md`: DeepSeek sentence under the API keys and router bullet; the Supported tools table with Pi
  and Oh My Pi columns and a line on the shared parser; the DeepSeek Harness paragraph (gated until it
  documents its session store, as Antigravity's usage is).
- DeepSeek Harness (`dsh`): no code.

## UI files touched (every line)

- `app/ui/accounts.js:208` (config-dir label map), `:260` (tool label map), `:409` (subscription
  sentence map), `:410` (folder prefix map): `pi`/`omp` entries added to each map, nothing else.
- `app/ui/home.js:8` (`TOOL`), `:9` (`VENDOR`): entries added.
- `app/ui/views.js:8` (`TOOL`), `:621` (`FILE`, prompt file name): entries added.
- `e2e/app.e2e.mjs:31` and `:63`: `pi` and `omp` copies of the fake harness and `HRM_BIN_PI`/`HRM_BIN_OMP`,
  so the sandbox never sees a real `pi` on PATH (this machine has one; without the pin the design test
  below got a "Pi" group it did not expect). `:659` and `:661`: the expected group and chip lists of
  "the window follows the Mission Control design" gain `'Pi', 'Oh My Pi'`. `:1241`–`:1291`: the appended
  test under `// ---- Pi and Oh My Pi (WP-1) ----`.

## Skipped, and why

- Oh My Pi API-key and routed accounts: its providers live in `models.yml` and there is no YAML dependency;
  caps say so, so the UI does not offer them.
- Pi `login` only opens the tool: `/login` is a slash command, there is no CLI login subcommand.
- `ttft_ms` for both tools: the log records when a reply began and when it was written, not its first token.
- Oh My Pi's XDG mode (`$XDG_DATA_HOME/omp/sessions`) and profile mode (`~/.omp/profiles/<name>/agent`):
  sessions are read from `<home>/sessions` only (shortcut comments in `omp.rs`).
- A cache column in `router_log`: the table has none, so cache hits are counted into `input` (shortcut
  comment in `lib.rs`); for the Anthropic-format pass-through (DeepSeek `/anthropic`) the router still reads
  only `input_tokens`/`output_tokens`.
- Off-peak DeepSeek prices (half of list) are not modelled; the price table has one price per model.
- Pi's own cost figures (`usage.cost`) are not stored; cost is computed from the price table like every
  other tool.
- No preset picker in the app's API keys page: `hrm provider list` shows the preset and its add command.

## Decisions where the docs were silent (with the source)

- Windows drive letters in the session folder: Pi `session-manager.ts::getDefaultSessionDirPath` does
  `` `--${resolvedCwd.replace(/^[/\\]/, "").replace(/[/\\:]/g, "-")}--` `` (badlogic/pi-mono,
  packages/coding-agent/src/core/session-manager.ts), so `D:\Projects\demo` is `--D--Projects-demo--`.
  Oh My Pi `session-paths.ts::getDefaultSessionDirName` encodes the same way, with `-<relative>` under
  home and `-tmp-<relative>` under the temp folder (can1357/oh-my-pi,
  packages/coding-agent/src/session/session-paths.ts). The parser never decodes the name: it reads `cwd`
  from the header. The fixtures and the e2e use these encodings.
- Pi's auth file is `auth.json` in the agent directory (docs/configuration.md, docs/providers.md). Oh My
  Pi keeps logins in `agent.db`, table `auth_credentials` with a `provider` column
  (packages/utils/src/dirs.ts `getAgentDbPath`, packages/ai/src/auth/sqlite-credential-store.ts).
- Oh My Pi honours `PI_CODING_AGENT_DIR` (packages/utils/src/dirs.ts, header comment) and has
  `<agent>/mcp.json` (`getMCPConfigPath`); its user skills folder is assumed to be `<agent>/skills` as
  Pi's (not confirmed in its source; `managed-skills` is a different, auto-learned folder).
- The Pi npm package `@mariozechner/pi-coding-agent` is deprecated on npm in favour of
  `@earendil-works/pi-coding-agent` (registry check, 5 Nov 2026); both are listed so an existing copy
  still shows and updates.
- Entry timestamps are ISO strings, the message's own `timestamp` is ms (docs/session-format.md); the
  assistant message's timestamp is taken as when the reply began (the agent loop stamps the message when it
  starts streaming), which gives `gen_ms`.
- Session title precedence: Oh My Pi title slot, then Pi `session_info.name`, then the first prompt.
- Both `usage` (Pi, e.g. `cache_warm`) and `model_usage` (Oh My Pi) entries are counted: both are model
  calls with a `usage` block and an entry id.

## For the merge

- Files outside the package's listed paths: `crates/cli/src/main.rs` (4 lines, preset display),
  `crates/cli/tests/e2e.rs` (harness count), `.gitattributes` (fixtures LF), `README.md` (one bullet, the
  table, one paragraph), `e2e/app.e2e.mjs` lines 31, 63, 659, 661 besides the appended test.
- `harness::all()` is now backed by a `static` array of six; WP-3 adds nothing there.
- The e2e appends its test before `exitCode = ...` (after the tray test, which shows the window again). It
  seeds the two session files inside the test, runs `hrm init` to add the main profiles, then checks
  `hrm harnesses`, `hrm list`, `hrm usage --json`, `hrm sessions`, the Sessions and Accounts pages. The
  `hrm sessions` read is retried for up to ten seconds: one full-suite run listed the two sessions late
  (the app rescans on its own schedule); the two later full runs were green on the first read.
- `parse_catalog` now starts from the built-in DeepSeek prices, so `HRM_PRICING_FILE` catalogs (tests)
  also price those two models.
- Claude Code routed through `deepseek-anthropic` sees `cache_read_input_tokens` as DeepSeek sends them;
  through `deepseek` (OpenAI format) the router now produces them from `prompt_cache_hit_tokens`.
