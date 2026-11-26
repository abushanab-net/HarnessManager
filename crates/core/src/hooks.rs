//! "Needs you": whether a session `hrm run` started is working, waiting for the user or done. The tool's hooks run
//! `hrm hook`, which reads the event and records it in hm.db (`run_state`) for that `hrm run`. Claude Code gets the
//! hook entries per launch, as `--settings <file>` with a file in Harness Manager's data folder: no account's files
//! change, and a session started outside Harness Manager is not touched.
//!
//! A permission request or a question (`AskUserQuestion`) is also held open for an answer from the app: `hrm hook`
//! stores it in `run_ask` and polls the row; the app writes the answer there (Phase 3 will write remote answers the
//! same way). A permission prompt stays answerable in the terminal, while a question appears there after the hook
//! exits. A later event or the hook's expiry closes the request and the hook exits 0 printing nothing.

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::registry::Profile;
use crate::util::{now_ms, write_atomic};
use crate::Ctx;

/// Set by `hrm run` in the tool's environment: `<pid>:<start ms>` of that `hrm run`. Without it `hrm hook` does nothing.
pub const RUN_VAR: &str = "HRM_RUN";
/// Set by `hrm run` beside `RUN_VAR`: how many seconds a request waits for the app (`request_seconds`).
pub const WAIT_VAR: &str = "HRM_REQUEST_SECONDS";
/// Set by `hrm run` beside `RUN_VAR`: how many seconds a question waits (`phones::question_seconds`, 60 while a phone
/// with Control is paired, else 15). Claude draws the question in the terminal only once the hook exits.
pub const QUESTION_VAR: &str = "HRM_QUESTION_SECONDS";
/// The longest a permission request waits (the plan's ceiling), whatever `request_seconds` says.
pub const MAX_REQUEST_SECONDS: u32 = 300;
/// What a hook entry's timeout allows beyond the window its request waits: the hook's start, its last poll and
/// printing the decision, so an answer taken at the window's end still reaches the tool.
const ENTRY_MARGIN: u32 = 10;
pub(crate) const APP_HEARTBEAT: &str = "hooks:app-heartbeat";

/// The desktop's session watcher calls this every second while it can offer answers.
pub fn heartbeat(ctx: &Ctx) -> Result<()> {
    ctx.db.set_app_heartbeat(&now_ms().to_string())
}

fn app_is_present(c: &Connection) -> rusqlite::Result<bool> {
    let beat: Option<String> = c.query_row("select v from kv where k = ?", [APP_HEARTBEAT], |r| r.get(0)).optional()?;
    Ok(beat.and_then(|v| v.parse::<i64>().ok()).is_some_and(|beat| (0..=3000).contains(&now_ms().saturating_sub(beat))))
}

/// The events Claude Code reports. PostToolUse: a tool ran (after an approval too), so the session works again.
/// Besides these, PreToolUse for `AskUserQuestion` only (see `settings`).
pub const EVENTS: &[&str] = &["UserPromptSubmit", "PermissionRequest", "Notification", "PostToolUse", "Stop", "SessionEnd"];

/// One row per `hrm run` in hm.db; `hwnd` is its console window (0 when unknown), to bring it to the front. `run_ask`:
/// the requests the hook holds for an answer; `state` is open, answered (then `answer` holds it), closed, or expired
/// (the hook let it go at its deadline).
pub const TABLE: &str = "create table if not exists run_state(run text primary key, pid integer not null, started integer not null,
  hwnd integer not null default 0, profile text not null, harness text not null, session_id text not null default '',
  cwd text not null default '', state text not null default '', reason text not null default '', ts integer not null default 0);
create table if not exists run_ask(id text primary key, run text not null, session_id text not null default '', kind text not null,
  tool text not null, what text not null default '', input text not null default '{}', created integer not null, expires integer not null,
  state text not null default 'open', answer text not null default '');";

/// What one hook event says about its session: the state (`waiting`, `working` or `done`) and why. Only a permission
/// or a question waits; the prompt for the next message after a turn is `done`. Claude Code and Codex name their
/// events alike. None: the event says nothing about it, and the state stays as it was.
pub fn state_of(v: &Value) -> Option<(&'static str, String)> {
    let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    match v["hook_event_name"].as_str()? {
        "PermissionRequest" => {
            let tool = s("tool_name");
            let what = v["tool_input"]["command"].as_str().or(v["tool_input"]["file_path"].as_str()).unwrap_or("").trim();
            Some(("waiting", if what.is_empty() { tool } else { crate::util::truncate(&format!("{tool}: {what}"), 120) }))
        }
        "Notification" => match v["notification_type"].as_str()? {
            "permission_prompt" | "agent_needs_input" | "elicitation_dialog" => Some(("waiting", s("message"))),
            "idle_prompt" => Some(("done", String::new())),
            _ => None,
        },
        "PreToolUse" if v["tool_name"] == "AskUserQuestion" => Some(("waiting", crate::util::truncate(&format!("Question: {}", question_text(v)), 120))),
        "UserPromptSubmit" | "PostToolUse" => Some(("working", String::new())),
        "Stop" | "SessionEnd" => Some(("done", String::new())),
        _ => None,
    }
}

/// An `AskUserQuestion`'s questions, one line.
fn question_text(v: &Value) -> String {
    v["tool_input"]["questions"].as_array().into_iter().flatten().filter_map(|q| q["question"].as_str()).collect::<Vec<_>>().join(" / ")
}

/// The request an event asks the user to decide, held for the app: "permission" or "question". None for the rest.
fn kind_of(v: &Value) -> Option<&'static str> {
    match v["hook_event_name"].as_str()? {
        "PermissionRequest" => Some("permission"),
        "PreToolUse" if v["tool_name"] == "AskUserQuestion" && v["tool_input"]["questions"].as_array().is_some_and(|q| !q.is_empty()) => Some("question"),
        _ => None,
    }
}

/// `hrm hook`: the event on standard input, recorded for the `hrm run` named in the environment, and for a request
/// the decision to print, once the app gave one. Only a row that `hrm run` made is changed, so a stale or foreign
/// variable changes nothing.
pub fn hook(input: &str) -> Result<Option<String>> {
    let Ok(run) = std::env::var(RUN_VAR) else { return Ok(None) };
    let v: Value = serde_json::from_str(input).context("the hook's input is not JSON")?;
    // not Ctx::open: no migrations, no settings, on every tool call; hm.db is there, hrm run opened it
    let c = hook_connection(&crate::ctx::data_dir().join("hm.db"))?;
    let secs = std::env::var(WAIT_VAR).ok().and_then(|s| s.parse().ok()).unwrap_or(0u32);
    let question = std::env::var(QUESTION_VAR).ok().and_then(|s| s.parse().ok()).unwrap_or(crate::phones::QUESTION_WITHOUT).min(crate::phones::QUESTION_WITH_PHONE);
    Ok(handle_asking(&c, &run, &v, Duration::from_secs(secs.into()), Duration::from_secs(question.into()))?.map(|d| d.to_string()))
}

/// The database as `hrm hook` writes it. An `hrm run` started before an update may still run, and its hooks run the
/// updated `hrm` before anything else migrated: the columns this version writes are made first.
pub fn hook_connection(path: &std::path::Path) -> Result<Connection> {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    c.busy_timeout(Duration::from_secs(2))?;
    crate::db::hook_columns(&c)?;
    Ok(c)
}

/// A request an event opens, written in the same transaction as the event.
struct Opening<'a> {
    id: String,
    kind: &'a str,
    what: String,
    created: i64,
    expires: i64,
    pid: u32,
    started: i64,
}

/// What an event says about its session, written in one transaction with the request it opens (bound to the run's
/// current turn), so no later event can come between the two.
fn record(c: &Connection, run: &str, v: &Value, opening: Option<&Opening>) -> rusqlite::Result<()> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let input = v["tool_input"].to_string();
    let event = v["hook_event_name"].as_str().unwrap_or("");
    let tx = c.unchecked_transaction()?;
    // a submitted prompt starts a new turn
    if event == "UserPromptSubmit" { tx.execute("update run_state set turn = turn + 1 where run = ?", [run])?; }
    // the terminal answered first, or the turn moved on: the waiting hook sees its request closed and steps aside.
    // A tool ran: the request for that very call. Other notifications can come while the prompt still shows.
    let closed = match event {
        "Notification" if v["notification_type"] != "idle_prompt" => 0,
        "PostToolUse" => tx.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open' and tool = ?2 and input = ?3", params![run, s("tool_name"), input])?,
        _ => tx.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open'", [run])?,
    };
    if let Some((state, reason)) = state_of(v) {
        tx.execute("update run_state set session_id = case when ?2 = '' then session_id else ?2 end, cwd = case when ?3 = '' then cwd else ?3 end,
            version = version + (state <> ?4 or ?7 > 0), state = ?4, reason = ?5, ts = ?6 where run = ?1", params![run, s("session_id"), s("cwd"), state, reason, now_ms(), closed as i64])?;
    } else if closed > 0 {
        tx.execute("update run_state set version = version + 1 where run = ?", [run])?;
    }
    if let Some(o) = opening {
        // the request and the version it moves come together: nobody sees the request beside the version before it
        tx.execute("insert into run_ask(id, run, session_id, kind, tool, what, input, created, expires, hook_pid, hook_started, turn)
            values(?,?,?,?,?,?,?,?,?,?,?, (select turn from run_state where run = ?2))",
            params![o.id, run, s("session_id"), o.kind, s("tool_name"), o.what, input, o.created, o.expires, o.pid, o.started])?;
        tx.execute("update run_state set version = version + 1 where run = ?", [run])?;
    }
    tx.commit()
}

/// `handle_asking` with the question window of a computer without a phone.
pub fn handle(c: &Connection, run: &str, v: &Value, wait: Duration) -> Result<Option<Value>> {
    handle_asking(c, run, v, wait, Duration::from_secs(crate::phones::QUESTION_WITHOUT.into()))
}

/// The event for run `run`: later events close what the terminal answered, the state is recorded, and a request
/// waits up to `wait` for the app's answer (a question up to `question`). Answers the decision for the tool, or None:
/// no decision, the terminal prompt stands. The run's `version` moves with every change of its state or of its
/// requests, so a phone's action names the state it was shown. An event that cannot be written fails as it always
/// did (see the release criteria in docs/plans/mobile/PLAN.md for what that leaves open).
pub fn handle_asking(c: &Connection, run: &str, v: &Value, wait: Duration, question: Duration) -> Result<Option<Value>> {
    let kind = kind_of(v);
    let wait = if kind == Some("question") { wait.min(question) } else { wait };
    // whether the event holds a request is decided first, so the request is written with the event: only for a
    // session hrm run started, and while the app offers answers
    let opening = match kind {
        Some(kind) if !wait.is_zero() && app_is_present(c)? && c.query_row("select 1 from run_state where run = ?", [run], |_| Ok(())).optional()?.is_some() => {
            let pid = std::process::id();
            crate::procs::start_ms(pid).map(|started| {
                let what = if kind == "question" { question_text(v) } else {
                    v["tool_input"]["command"].as_str().or(v["tool_input"]["file_path"].as_str()).unwrap_or("").to_string()
                };
                let created = now_ms();
                Opening { id: crate::util::uuid4(), kind, what, created, expires: created + wait.as_millis() as i64, pid, started }
            })
        }
        _ => None,
    };
    record(c, run, v, opening.as_ref())?;
    let Some(Opening { id, kind, expires, .. }) = opening else { return Ok(None) };
    let deadline = Instant::now() + wait;
    loop {
        // a busy database is waited out: leaving would strand an answer the app then reports as given
        let row: rusqlite::Result<Option<(String, String)>> = c.query_row("select state, answer from run_ask where id = ?", [&id], |r| Ok((r.get(0)?, r.get(1)?))).optional();
        match row.as_ref().map(|r| r.as_ref().map(|(s, a)| (s.as_str(), a.as_str()))) {
            Ok(Some(("answered", a))) => {
                let _ = c.execute("update run_state set version = version + 1, state = 'working', reason = '', ts = ?2 where run = ?1", params![run, now_ms()]);
                return Ok(Some(decision(kind, &v["tool_input"], a)));
            }
            Ok(Some(("open", _))) | Err(_) => {}
            Ok(_) => return Ok(None), // closed, or gone with its run
        }
        // The close and an answer both update only an open row; a close at the deadline says so, for a phone's receipt
        let timed_out = Instant::now() >= deadline || now_ms() >= expires;
        if (timed_out || matches!(app_is_present(c), Ok(false)))
            && c.execute("update run_ask set state = ?2 where id = ?1 and state = 'open'", params![id, if timed_out { "expired" } else { "closed" }])? == 1 {
            let _ = c.execute("update run_state set version = version + 1 where run = ?", [run]);
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// What the hook prints for an answer, as Claude Code reads it: a permission allowed or denied (once; nothing
/// wider), or the question's answers in `updatedInput` beside its own questions.
pub fn decision(kind: &str, tool_input: &Value, answer: &str) -> Value {
    if kind == "question" {
        let mut input = tool_input.clone();
        input["answers"] = serde_json::from_str(answer).unwrap_or_default();
        return json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow", "updatedInput": input}});
    }
    let d = if answer == "allow" { json!({"behavior": "allow"}) } else { json!({"behavior": "deny", "message": "Denied in Harness Manager."}) };
    json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": d}})
}

fn hook_is_live(pid: u32, started: i64) -> bool {
    if started == 0 { return false; }
    #[cfg(windows)]
    if !crate::procs::alive(pid) { return false; }
    crate::procs::start_ms(pid) == Some(started)
}

/// The app's answer to request `id`: `{"behavior": "allow" | "deny"}` for a permission, `{"answers": {question:
/// [label, ...]}}` for a question, with one of each question's own options (several where it allows several).
/// False when the request is no longer open (the terminal answered, or it expired): the answer is dropped.
pub fn answer(ctx: &Ctx, id: &str, a: &Value) -> Result<bool> { answer_guarded(ctx, id, a, None) }

/// `answer`, for a phone: the run must still be at the version its action claimed, and the action's deadline not
/// passed, when the answer is written.
pub fn answer_guarded(ctx: &Ctx, id: &str, a: &Value, guard: Option<&Guard>) -> Result<bool> {
    let row: Option<(String, String, u32, i64)> = ctx.db.read(|c| Ok(c.query_row(
        "select kind, input, hook_pid, hook_started from run_ask where id = ?1 and state = 'open' and expires > ?2",
        params![id, now_ms()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?))?;
    let Some((kind, input, pid, started)) = row else { return Ok(false) };
    if !hook_is_live(pid, started) { return Ok(false); }
    let answer = if kind == "question" {
        let input: Value = serde_json::from_str(&input)?;
        let mut out = Map::new();
        for q in input["questions"].as_array().into_iter().flatten() {
            let text = q["question"].as_str().unwrap_or("");
            let picked: Vec<&str> = a["answers"][text].as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect();
            let options: Vec<&str> = q["options"].as_array().into_iter().flatten().filter_map(|o| o["label"].as_str()).collect();
            if picked.is_empty() { bail!("answer every question"); }
            if picked.len() > 1 && q["multiSelect"] != true { bail!("that question takes one answer"); }
            if let Some(l) = picked.iter().find(|l| !options.contains(l)) { bail!("'{l}' is not one of the question's options"); }
            out.insert(text.into(), json!(picked.join(", ")));
        }
        Value::Object(out).to_string()
    } else {
        match a["behavior"].as_str() { Some(b @ ("allow" | "deny")) => b.to_string(), _ => bail!("the answer is allow or deny") }
    };
    ctx.db.with(|c| {
        // the write lock first, then the clock: a write that waited for another writer is judged at its own time
        let tx = c.transaction()?;
        if !hook_is_live(pid, started) { return Ok(false); }
        let now = now_ms();
        // a phone's answer also names the run version it claimed, its own deadline and its enrollment, which must
        // still have Control: all hold in the same write that takes the open request
        let (version, deadline) = guard.map(|g| (Some(g.version), g.deadline)).unwrap_or((None, i64::MAX));
        let (phone, generation) = guard.and_then(|g| g.phone.clone()).map(|(p, g)| (Some(p), Some(g))).unwrap_or((None, None));
        let n = tx.execute("update run_ask set state = 'answered', answer = ?2 where id = ?1 and state = 'open' and expires > ?3 and ?3 < ?5
            and (?4 is null or ?4 = (select version from run_state where run_state.run = run_ask.run))
            and (?6 is null or exists (select 1 from phones where phone_id = ?6 and generation = ?7 and revoked_at is null and json_extract(grants, '$.control') = 1))
            and (?6 is null or turn = (select turn from run_state where run_state.run = run_ask.run))",
            params![id, answer, now, version, deadline, phone, generation])?;
        tx.commit()?;
        Ok(n == 1)
    })
}

/// What a phone's answer carries to the write that takes its request: the run version its claim left, its deadline
/// (ms), and the enrollment it came from (`phone_id`, `generation`).
#[derive(Debug, Clone, PartialEq)]
pub struct Guard {
    pub version: i64,
    pub deadline: i64,
    pub phone: Option<(String, String)>,
}

/// `hrm run`, before the tool starts: its row, and the variables that let the tool's hooks find it. Answers the run
/// id for `end_run`; None when the row could not be made.
pub fn start_run(ctx: &Ctx, p: &Profile, env: &mut Vec<(String, String)>) -> Option<String> {
    let pid = std::process::id();
    let started = crate::procs::start_ms(pid).unwrap_or(0);
    let run = format!("{pid}:{started}");
    let r = ctx.db.with(|c| Ok(c.execute("insert or replace into run_state(run, pid, started, hwnd, profile, harness, ts) values(?,?,?,?,?,?,?)",
        params![run, pid, started, console_window(), p.name, p.harness, now_ms()])?));
    if let Err(e) = r { crate::log::info(&format!("run state for {}: {e:#}", p.name)); return None; }
    env.push((RUN_VAR.into(), run.clone()));
    env.push((WAIT_VAR.into(), ctx.settings.request_seconds.min(MAX_REQUEST_SECONDS).to_string()));
    env.push((QUESTION_VAR.into(), crate::phones::question_seconds(ctx).to_string()));
    Some(run)
}

/// `hrm run`, after the tool exited: its rows go.
pub fn end_run(ctx: &Ctx, run: &str) {
    let _ = ctx.db.with(|c| { c.execute("delete from run_state where run = ?", [run])?; c.execute("delete from run_ask where run = ?", [run])?; Ok(()) });
}

#[cfg(windows)]
fn console_window() -> i64 { unsafe { windows_sys::Win32::System::Console::GetConsoleWindow() as i64 } }
#[cfg(not(windows))]
fn console_window() -> i64 { 0 }

/// One session `hrm run` started, as its hooks last reported it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Run {
    pub run: String,
    pub pid: u32,
    pub hwnd: i64,
    pub profile: String,
    pub harness: String,
    pub session_id: String,
    pub cwd: String,
    pub state: String,
    pub reason: String,
    pub ts: i64,
    /// Moves with every change of its state or its requests: what a phone's action must name (`companion::actions`).
    pub version: i64,
    /// It owns its terminal (`own_terminal`), so `terminal::reply` and `terminal::stop` reach it.
    pub terminal: bool,
    /// The request its hook holds open for an answer, if any.
    pub ask: Option<Ask>,
}

/// A request a session's hook holds open (see `handle`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ask {
    pub id: String,
    /// "permission" or "question".
    pub kind: String,
    pub tool: String,
    /// The command or file, or the questions in one line.
    pub what: String,
    /// The tool's input as the tool gave it: a question's `questions` with their options.
    pub input: Value,
    pub created: i64,
    pub expires: i64,
}

/// The sessions that reported a state, oldest report first, each with its open request. The rows of an `hrm run`
/// that is gone (its window was closed, so it could not remove its row) are deleted here.
pub fn runs(ctx: &Ctx) -> Result<Vec<Run>> {
    let rows: Vec<(Run, i64)> = ctx.db.read(|c| {
        let mut st = c.prepare("select run, pid, hwnd, profile, harness, session_id, cwd, state, reason, ts, started, terminal_port is not null, version from run_state order by ts")?;
        let mut v: Vec<(Run, i64)> = st.query_map([], |r| Ok((Run { run: r.get(0)?, pid: r.get(1)?, hwnd: r.get(2)?, profile: r.get(3)?, harness: r.get(4)?, session_id: r.get(5)?,
            cwd: r.get(6)?, state: r.get(7)?, reason: r.get(8)?, ts: r.get(9)?, version: r.get(12)?, terminal: r.get(11)?, ask: None }, r.get(10)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare("select run, id, kind, tool, what, input, created, expires, hook_pid, hook_started from run_ask where state = 'open' and expires > ? order by created")?;
        let asks = st.query_map([now_ms()], |r| Ok((r.get::<_, String>(0)?, Ask { id: r.get(1)?, kind: r.get(2)?, tool: r.get(3)?, what: r.get(4)?,
            input: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(), created: r.get(6)?, expires: r.get(7)? }, r.get::<_, u32>(8)?, r.get::<_, i64>(9)?)))?;
        for ask in asks {
            let (run, ask, pid, started) = ask?;
            if !hook_is_live(pid, started) { continue; }
            if let Some((r, _)) = v.iter_mut().find(|(r, _)| r.run == run) { r.ask = Some(ask); }
        }
        Ok(v)
    })?;
    // the start time tells the hrm run that made the row from a later process given the same pid
    let (live, gone): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(r, started)| match crate::procs::start_ms(r.pid) {
        Some(s) => *started == 0 || s == *started,
        None => false,
    });
    if !gone.is_empty() {
        ctx.db.with(|c| { for (r, _) in &gone { c.execute("delete from run_state where run = ?", [&r.run])?; c.execute("delete from run_ask where run = ?", [&r.run])?; } Ok(()) })?;
    }
    Ok(live.into_iter().map(|(r, _)| r).filter(|r| !r.state.is_empty()).collect())
}

// ---------- the hook entries Claude Code gets with each launch ----------

/// The command the entries run: `hrm hook` by its full path, with forward slashes and without spaces where Windows
/// keeps a short name, so it reads the same in bash, cmd and PowerShell.
pub fn command() -> String {
    let p = crate::launch::short_path(&crate::launch::hrm_exe()).display().to_string();
    let p = if cfg!(windows) { p.replace('\\', "/") } else { p };
    if p.contains(' ') { format!("\"{p}\" hook") } else { format!("{p} hook") }
}

/// The settings Claude Code is started with: only our hook entries, one group under each of `EVENTS`, and one for
/// PreToolUse of `AskUserQuestion`. Permission hooks can outlast their own wait. A question's entry is 15 s, as
/// before, or 70 s while a phone with Control is paired (its 60 s window and the hook's start and print); Claude
/// draws the terminal question only after the hook exits.
fn settings(ctx: &Ctx) -> Value {
    #[cfg(not(target_os = "linux"))]
    let command = command();
    #[cfg(target_os = "linux")]
    let command = crate::cli_path::installed_cli(ctx)
        .map(|exe| format!("{} hook", crate::launch::sh_quote(&exe.to_string_lossy())))
        .unwrap_or_else(command);
    let (entry, held) = (|timeout: u32| json!({"type": "command", "command": command, "timeout": timeout}), ctx.settings.request_seconds.min(MAX_REQUEST_SECONDS) + 30);
    let mut hooks: Map<String, Value> = EVENTS.iter().map(|e| (e.to_string(), json!([{"hooks": [entry(if *e == "PermissionRequest" { held } else { 10 })]}]))).collect();
    let question = crate::phones::question_seconds(ctx);
    let question = if question > crate::phones::QUESTION_WITHOUT { question + ENTRY_MARGIN } else { question };
    hooks.insert("PreToolUse".into(), json!([{"matcher": "AskUserQuestion", "hooks": [entry(question)]}]));
    json!({"hooks": hooks})
}

/// `--settings <file>` for a Claude Code launch while `session_hooks` is on: the file in the data folder, written
/// when its content is not already that. None when off, or when the user's own arguments carry `--settings`.
pub fn settings_args(ctx: &Ctx, user_args: &[String]) -> Result<Option<[String; 2]>> {
    // Claude Code takes one --settings: the user's wins, and that session is not tracked
    if !ctx.settings.session_hooks || user_args.iter().any(|a| a == "--settings" || a.starts_with("--settings=")) { return Ok(None); }
    let path = ctx.data.join("claude-hooks.json");
    let want = serde_json::to_string_pretty(&settings(ctx))?;
    if crate::util::read_or_empty(&path)? != want { write_atomic(&path, want.as_bytes())?; }
    Ok(Some(["--settings".into(), path.display().to_string()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_map_to_waiting_working_and_done_for_claude_code_and_codex() {
        let ev = |v: Value| state_of(&v);
        assert_eq!(ev(json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": {"command": "npm test"}})), Some(("waiting", "Bash: npm test".into())));
        assert_eq!(ev(json!({"hook_event_name": "PermissionRequest", "tool_name": "Edit", "tool_input": {"file_path": "src/a.rs"}})), Some(("waiting", "Edit: src/a.rs".into())));
        assert_eq!(ev(json!({"hook_event_name": "PermissionRequest", "tool_name": "WebFetch"})), Some(("waiting", "WebFetch".into())));
        for t in ["permission_prompt", "agent_needs_input", "elicitation_dialog"] {
            assert_eq!(ev(json!({"hook_event_name": "Notification", "notification_type": t, "message": "Claude needs you"})), Some(("waiting", "Claude needs you".into())), "{t}");
        }
        assert_eq!(ev(json!({"hook_event_name": "Notification", "notification_type": "idle_prompt", "message": "Claude is waiting for your input"})), Some(("done", String::new())), "the next message, not a decision");
        assert_eq!(ev(json!({"hook_event_name": "Notification", "notification_type": "auth_success"})), None, "not about the session's state");
        assert_eq!(ev(json!({"hook_event_name": "Notification"})), None);
        for e in ["UserPromptSubmit", "PostToolUse"] { assert_eq!(ev(json!({"hook_event_name": e})), Some(("working", String::new())), "{e}"); }
        for e in ["Stop", "SessionEnd"] { assert_eq!(ev(json!({"hook_event_name": e})), Some(("done", String::new())), "{e}"); }
        // Codex: the same names and fields; SessionStart and PreToolUse say nothing about waiting
        assert_eq!(ev(json!({"hook_event_name": "PermissionRequest", "tool_name": "shell", "tool_input": {"command": "cargo build"}, "session_id": "s", "cwd": "/w"})), Some(("waiting", "shell: cargo build".into())));
        for e in ["SessionStart", "PreToolUse", "SubagentStop", "PreCompact"] { assert_eq!(ev(json!({"hook_event_name": e})), None, "{e}"); }
        assert_eq!(ev(json!({"no": "event"})), None);
    }

    #[test]
    fn a_hook_without_hrm_run_in_its_environment_records_nothing() {
        if std::env::var_os(RUN_VAR).is_some() { return; } // a test run started from inside hrm run
        assert_eq!(hook(&json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash"}).to_string()).unwrap(), None);
        assert_eq!(hook("not json").unwrap(), None);
    }

    // ---------- the request lifecycle: open → answered / closed by a later event / expired; a late answer is dropped ----------

    /// A data folder with one live run (this process), and a connection to its hm.db as `hrm hook` opens it.
    fn live_run() -> (tempfile::TempDir, Ctx, String) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
        let pid = std::process::id();
        let run = format!("{pid}:1");
        ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness) values(?, ?, ?, 'work', 'claude')",
            params![run, pid, crate::procs::start_ms(pid).unwrap_or(0)])?)).unwrap();
        (dir, ctx, run)
    }
    fn conn(ctx: &Ctx) -> Connection { let c = Connection::open(ctx.data.join("hm.db")).unwrap(); c.busy_timeout(std::time::Duration::from_secs(5)).unwrap(); c }
    const BASH: &str = r#"{"hook_event_name": "PermissionRequest", "session_id": "s-1", "tool_name": "Bash", "tool_input": {"command": "npm test", "description": "Run the tests"}}"#;

    /// The event handled on another thread, as the waiting hook; the open request once the app can see it.
    fn hold(ctx: &Ctx, run: &str, ev: &str, wait_ms: u64) -> (std::thread::JoinHandle<Option<Value>>, Ask) {
        let open = || runs(ctx).unwrap().into_iter().find(|r| r.run == run).and_then(|r| r.ask);
        let before = open().map(|a| a.id);
        heartbeat(ctx).unwrap();
        let (c, run2, v): (Connection, String, Value) = (conn(ctx), run.to_string(), serde_json::from_str(ev).unwrap());
        let h = std::thread::spawn(move || handle(&c, &run2, &v, std::time::Duration::from_millis(wait_ms)).unwrap());
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(a) = open().filter(|a| Some(&a.id) != before.as_ref()) { return (h, a); }
            assert!(std::time::Instant::now() < end, "the request never showed");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn a_permission_request_waits_for_the_apps_answer_and_prints_it_once() {
        let (_d, ctx, run) = live_run();
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        assert_eq!((a.kind.as_str(), a.tool.as_str(), a.what.as_str()), ("permission", "Bash", "npm test"));
        assert_eq!(a.input["description"], "Run the tests");
        assert_eq!(runs(&ctx).unwrap()[0].state, "waiting");
        assert!(answer(&ctx, &a.id, &json!({"behavior": "always"})).is_err(), "nothing wider than once");
        assert!(answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap());
        assert_eq!(h.join().unwrap(), Some(json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})));
        let r = &runs(&ctx).unwrap()[0];
        assert_eq!((r.state.as_str(), r.ask.is_none()), ("working", true), "answered: the card goes, the session works");
        assert!(!r.terminal, "no owned terminal: the app's reply box says so");
        ctx.db.with(|c| Ok(c.execute("update run_state set terminal_port = 1 where run = ?", [&run])?)).unwrap();
        assert!(runs(&ctx).unwrap()[0].terminal);
        assert!(!answer(&ctx, &a.id, &json!({"behavior": "deny"})).unwrap(), "a second answer is dropped");

        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        assert!(answer(&ctx, &a.id, &json!({"behavior": "deny"})).unwrap());
        assert_eq!(h.join().unwrap().unwrap()["hookSpecificOutput"]["decision"], json!({"behavior": "deny", "message": "Denied in Harness Manager."}));
    }

    #[test]
    fn a_later_event_closes_the_request_the_terminal_answered_and_a_late_answer_is_dropped() {
        let (_d, ctx, run) = live_run();
        // the terminal approved: the tool ran. A different call's PostToolUse and a notification leave it open
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        for other in [json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": "ls"}}), json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"})] {
            assert_eq!(handle(&conn(&ctx), &run, &other, std::time::Duration::from_secs(5)).unwrap(), None);
            assert!(runs(&ctx).unwrap()[0].ask.is_some(), "{other}");
        }
        let mut ran: Value = serde_json::from_str(BASH).unwrap();
        ran["hook_event_name"] = json!("PostToolUse");
        handle(&conn(&ctx), &run, &ran, std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(h.join().unwrap(), None, "no decision: the terminal's stands");
        assert!(runs(&ctx).unwrap()[0].ask.is_none());
        assert!(!answer(&ctx, &a.id, &json!({"behavior": "deny"})).unwrap(), "dropped by request id");
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        let idle = json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"});
        assert_eq!(handle(&conn(&ctx), &run, &idle, Duration::ZERO).unwrap(), None);
        assert_eq!(h.join().unwrap(), None);
        let r = &runs(&ctx).unwrap()[0];
        assert_eq!((r.state.as_str(), r.ask.is_none()), ("done", true));
        assert!(!answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap());
        // the terminal denied, then the next prompt; a new request; the turn's end
        for next in ["UserPromptSubmit", "PermissionRequest", "Stop", "SessionEnd"] {
            let (h, _) = hold(&ctx, &run, BASH, 20_000);
            let ev = if next == "PermissionRequest" { serde_json::from_str(BASH).unwrap() } else { json!({"hook_event_name": next}) };
            if next == "PermissionRequest" {
                let (h2, a2) = hold(&ctx, &run, &ev.to_string(), 20_000);
                assert_eq!(h.join().unwrap(), None, "the earlier one closed");
                assert!(answer(&ctx, &a2.id, &json!({"behavior": "allow"})).unwrap());
                assert!(h2.join().unwrap().is_some());
            } else {
                handle(&conn(&ctx), &run, &ev, std::time::Duration::from_secs(5)).unwrap();
                assert_eq!(h.join().unwrap(), None, "{next}");
            }
        }
    }

    #[test]
    fn a_stale_heartbeat_skips_both_holds_and_releases_an_open_hold() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        let version = ctx.db.data_version().unwrap();
        heartbeat(&ctx).unwrap();
        assert_eq!(ctx.db.data_version(), Some(version), "heartbeats leave the page cache valid");
        assert!(app_is_present(&c).unwrap(), "the hook's connection sees the heartbeat");
        ctx.db.kv_set("test:page-data", "changed").unwrap();
        heartbeat(&ctx).unwrap();
        assert_ne!(ctx.db.data_version(), Some(version), "a heartbeat does not hide a regular commit");
        let stale = || ctx.db.kv_set(APP_HEARTBEAT, &(now_ms() - 4000).to_string()).unwrap();
        stale();
        let bash: Value = serde_json::from_str(BASH).unwrap();
        let ask = json!({"hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Which colour?"}]}});
        for event in [bash, ask] {
            assert_eq!(handle(&c, &run, &event, Duration::from_millis(500)).unwrap(), None);
            assert!(!c.prepare("select 1 from run_ask").unwrap().exists([]).unwrap());
        }
        let (h, a) = hold(&ctx, &run, BASH, 5000);
        stale();
        assert_eq!(h.join().unwrap(), None);
        assert!(now_ms() < a.expires, "the app disappeared before expiry");
        assert!(!answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap());
    }

    #[test]
    fn missing_or_reused_hook_processes_cannot_receive_answers() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        let pid = std::process::id();
        let started = crate::procs::start_ms(pid).unwrap();
        for (id, pid, started) in [("gone", i32::MAX as u32, started), ("reused", pid, started - 1000), ("legacy", 0, 0)] {
            c.execute("insert into run_ask(id, run, kind, tool, created, expires, hook_pid, hook_started) values(?, ?, 'permission', 'Bash', ?, ?, ?, ?)",
                params![id, run, now_ms(), now_ms() + 30_000, pid, started]).unwrap();
            assert!(!answer(&ctx, id, &json!({"behavior": "allow"})).unwrap());
        }
        c.execute("update run_state set state = 'waiting' where run = ?", [&run]).unwrap();
        assert!(runs(&ctx).unwrap()[0].ask.is_none());
    }

    #[test]
    fn past_its_expiry_the_hook_steps_aside_and_the_answer_is_dropped() {
        let (_d, ctx, run) = live_run();
        let (h, a) = hold(&ctx, &run, BASH, 600);
        assert_eq!(h.join().unwrap(), None);
        assert!(runs(&ctx).unwrap()[0].ask.is_none());
        assert_eq!(runs(&ctx).unwrap()[0].state, "waiting", "the terminal prompt still waits");
        assert!(!answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap());
        // the app does not offer answers, or the run is not one hrm run made: no wait at all
        let v: Value = serde_json::from_str(BASH).unwrap();
        assert_eq!(handle(&conn(&ctx), &run, &v, std::time::Duration::ZERO).unwrap(), None);
        assert_eq!(handle(&conn(&ctx), "1:2", &v, std::time::Duration::from_secs(30)).unwrap(), None);
        // the hrm run ended: its requests go with it
        let (h, _) = hold(&ctx, &run, BASH, 20_000);
        end_run(&ctx, &run);
        assert_eq!(h.join().unwrap(), None);
    }

    #[test]
    fn a_question_is_answered_with_its_own_options_in_updated_input() {
        let (_d, ctx, run) = live_run();
        let questions = json!([
            {"question": "Which colour?", "header": "Colour", "multiSelect": false, "options": [{"label": "Blue", "description": "calm"}, {"label": "Red", "description": "loud"}]},
            {"question": "Which sizes?", "header": "Sizes", "multiSelect": true, "options": [{"label": "S", "description": ""}, {"label": "M", "description": ""}, {"label": "L", "description": ""}]}]);
        let ev = json!({"hook_event_name": "PreToolUse", "session_id": "s-1", "tool_name": "AskUserQuestion", "tool_input": {"questions": questions}}).to_string();
        let (h, a) = hold(&ctx, &run, &ev, 20_000);
        assert_eq!((a.kind.as_str(), a.what.as_str()), ("question", "Which colour? / Which sizes?"));
        assert_eq!(a.expires - a.created, 15_000);
        assert_eq!(runs(&ctx).unwrap()[0].reason, "Question: Which colour? / Which sizes?");
        for bad in [json!({"answers": {"Which colour?": ["Green"], "Which sizes?": ["S"]}}), json!({"answers": {"Which colour?": ["Blue", "Red"], "Which sizes?": ["S"]}}), json!({"answers": {"Which colour?": ["Blue"]}})] {
            assert!(answer(&ctx, &a.id, &bad).is_err(), "{bad}");
        }
        assert!(answer(&ctx, &a.id, &json!({"answers": {"Which colour?": ["Blue"], "Which sizes?": ["S", "L"]}})).unwrap());
        assert_eq!(h.join().unwrap(), Some(json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow",
            "updatedInput": {"questions": questions, "answers": {"Which colour?": "Blue", "Which sizes?": "S, L"}}}})));
        // another tool's PreToolUse is no request
        assert_eq!(handle(&conn(&ctx), &run, &json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {}}), std::time::Duration::from_secs(5)).unwrap(), None);
    }

    #[test]
    fn a_question_waits_the_window_its_launch_was_given_and_the_version_moves_with_the_state() {
        let (_d, ctx, run) = live_run();
        let version = || runs(&ctx).unwrap()[0].version;
        let ev: Value = json!({"hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Go?", "options": [{"label": "Yes"}]}]}});
        heartbeat(&ctx).unwrap();
        let (c, run2, ev2) = (conn(&ctx), run.clone(), ev.clone());
        let h = std::thread::spawn(move || handle_asking(&c, &run2, &ev2, Duration::from_secs(300), Duration::from_secs(crate::phones::QUESTION_WITH_PHONE.into())).unwrap());
        let end = std::time::Instant::now() + Duration::from_secs(5);
        let a = loop {
            if let Some(a) = runs(&ctx).unwrap().into_iter().find_map(|r| r.ask) { break a; }
            assert!(std::time::Instant::now() < end, "the request never showed");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(a.expires - a.created, 60_000, "a phone with Control gets a minute");
        let asked = version();
        assert!(asked >= 2, "waiting, then the request: {asked}");
        // a notification that changes nothing leaves it; the answer moves it
        handle(&conn(&ctx), &run, &json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"}), Duration::ZERO).unwrap();
        assert_eq!(version(), asked);
        assert!(answer(&ctx, &a.id, &json!({"answers": {"Go?": ["Yes"]}})).unwrap());
        assert!(h.join().unwrap().is_some());
        assert!(version() > asked);
        let mut env = vec![];
        let p = Profile { name: "work".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 };
        start_run(&ctx, &p, &mut env);
        assert!(env.contains(&(QUESTION_VAR.to_string(), crate::phones::QUESTION_WITHOUT.to_string())), "{env:?}");
    }

    /// #3: a phone's answer is written only while the run is at the version its action claimed and before the
    /// action's deadline, in the same write that takes the open request.
    #[test]
    fn a_guarded_answer_holds_only_at_the_claimed_version_and_before_the_deadline() {
        let (_d, ctx, run) = live_run();
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        let v: i64 = ctx.db.read(|c| Ok(c.query_row("select version from run_state where run = ?", [&run], |r| r.get(0))?)).unwrap();
        let allow = json!({"behavior": "allow"});
        let g = |version: i64, deadline: i64| Guard { version, deadline, phone: None };
        // the hook lets a request go once the app's heartbeat is 3 s old: a loaded test run must not get there first
        heartbeat(&ctx).unwrap();
        assert!(!answer_guarded(&ctx, &a.id, &allow, Some(&g(v - 1, now_ms() + 60_000))).unwrap(), "another version");
        assert!(!answer_guarded(&ctx, &a.id, &allow, Some(&g(v, now_ms() - 1))).unwrap(), "past the deadline");
        assert!(answer_guarded(&ctx, &a.id, &allow, Some(&g(v, now_ms() + 60_000))).unwrap());
        assert!(h.join().unwrap().is_some());
    }

    /// (a): a phone's answer is judged at the moment its write gets the database: an action whose deadline passed while
    /// another writer held it is refused, and so is one from a phone whose Control was withdrawn after its last check.
    #[test]
    fn an_answer_is_judged_when_its_write_gets_the_database() {
        let (_d, ctx, run) = live_run();
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        let v: i64 = ctx.db.read(|c| Ok(c.query_row("select version from run_state where run = ?", [&run], |r| r.get(0))?)).unwrap();
        let allow = json!({"behavior": "allow"});
        // a phone with Control, then without it
        let me = companion_proto::PhoneIdentity::generate().unwrap();
        crate::phones::add(&ctx, &companion_proto::pairing::PhonePayload::of(&me, "P", "ios").unwrap(), companion_proto::api::Grants { summaries: true, transcripts: true, control: true }).unwrap();
        let phone = crate::phones::get(&ctx, &me.id).unwrap().unwrap();
        let g = |deadline: i64| Guard { version: v, deadline, phone: Some((phone.id.clone(), phone.generation.clone())) };
        crate::phones::set_grants(&ctx, &me.id, &json!({"control": false})).unwrap();
        heartbeat(&ctx).unwrap();
        assert!(!answer_guarded(&ctx, &a.id, &allow, Some(&g(now_ms() + 60_000))).unwrap(), "Control was withdrawn");
        crate::phones::set_grants(&ctx, &me.id, &json!({"control": true})).unwrap();
        // another writer holds the database past the action's deadline
        let other = conn(&ctx);
        other.execute_batch("begin immediate").unwrap();
        let release = std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(400)); other.execute_batch("commit").unwrap(); });
        assert!(!answer_guarded(&ctx, &a.id, &allow, Some(&g(now_ms() + 150))).unwrap(), "the deadline passed while the write waited");
        release.join().unwrap();
        heartbeat(&ctx).unwrap();
        assert!(answer_guarded(&ctx, &a.id, &allow, Some(&g(now_ms() + 60_000))).unwrap());
        assert!(h.join().unwrap().is_some());
    }

    /// (f): the hook of an `hrm run` started before an update writes a database the update has not migrated yet.
    #[test]
    fn a_hook_writes_a_database_from_before_the_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hm.db");
        let pid = std::process::id();
        {
            // the hook tables as the release before them made them, and a run going on
            let c = Connection::open(&path).unwrap();
            c.execute_batch("create table kv(k text primary key, v text not null);
                create table run_state(run text primary key, pid integer not null, started integer not null, hwnd integer not null default 0, profile text not null,
                  harness text not null, session_id text not null default '', cwd text not null default '', state text not null default '', reason text not null default '',
                  ts integer not null default 0, terminal_port integer, terminal_token blob);
                create table run_ask(id text primary key, run text not null, session_id text not null default '', kind text not null, tool text not null,
                  what text not null default '', input text not null default '{}', created integer not null, expires integer not null, state text not null default 'open',
                  answer text not null default '', hook_pid integer not null default 0, hook_started integer not null default 0);").unwrap();
            c.execute("insert into run_state(run, pid, started, profile, harness) values('r', ?1, ?2, 'work', 'claude')", params![pid, crate::procs::start_ms(pid).unwrap()]).unwrap();
        }
        let c = hook_connection(&path).unwrap();
        handle(&c, "r", &json!({"hook_event_name": "UserPromptSubmit", "session_id": "s"}), Duration::ZERO).unwrap();
        let state: String = c.query_row("select state from run_state where run = 'r'", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "working");
    }

    /// A phone with Control: its id and enrollment.
    fn a_phone(ctx: &Ctx) -> (String, String) {
        let me = companion_proto::PhoneIdentity::generate().unwrap();
        crate::phones::add(ctx, &companion_proto::pairing::PhonePayload::of(&me, "P", "ios").unwrap(), companion_proto::api::Grants { summaries: true, transcripts: true, control: true }).unwrap();
        let p = crate::phones::get(ctx, &me.id).unwrap().unwrap();
        (p.id, p.generation)
    }
    fn version_of(ctx: &Ctx, run: &str) -> i64 { ctx.db.read(|c| Ok(c.query_row("select version from run_state where run = ?", [run], |r| r.get(0))?)).unwrap() }

    /// A request is made in the write that records its event and belongs to that turn. If the turn moved on without
    /// the request being closed (the event that closed it was lost), a phone's answer is refused; the desktop's is not.
    #[test]
    fn a_request_belongs_to_the_turn_that_opened_it() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        handle(&c, &run, &json!({"hook_event_name": "UserPromptSubmit"}), Duration::ZERO).unwrap();
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        let (asked, now): (i64, i64) = c.query_row("select (select turn from run_ask where id = ?1), (select turn from run_state where run = ?2)", params![a.id, run], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((asked, now), (1, 1));
        c.execute("update run_state set turn = turn + 1 where run = ?", [&run]).unwrap();
        let phone = a_phone(&ctx);
        heartbeat(&ctx).unwrap();
        let g = Guard { version: version_of(&ctx, &run), deadline: now_ms() + 60_000, phone: Some(phone) };
        assert!(!answer_guarded(&ctx, &a.id, &json!({"behavior": "allow"}), Some(&g)).unwrap(), "another turn's request");
        assert!(answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap(), "the person at the computer still can");
        assert!(h.join().unwrap().is_some());
    }

    /// The files under the data folder, other than SQLite's own.
    fn files(dir: &std::path::Path) -> Vec<String> {
        let mut out = vec![];
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("hm.db") { continue; }
            if e.path().is_dir() { out.extend(files(&e.path()).into_iter().map(|f| format!("{name}/{f}"))); } else { out.push(name); }
        }
        out.sort();
        out
    }

    /// A hook that cannot record its event (the database held past its busy wait) fails as it always did, at once,
    /// without a retry, and the desktop still answers the request it left open.
    #[test]
    fn a_hook_that_cannot_record_fails_at_once_and_the_desktop_still_answers() {
        let (_d, ctx, run) = live_run();
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        let c = Connection::open(ctx.data.join("hm.db")).unwrap();
        c.busy_timeout(Duration::from_millis(50)).unwrap();
        let other = conn(&ctx);
        other.execute_batch("begin immediate").unwrap();
        let mut ran: Value = serde_json::from_str(BASH).unwrap();
        ran["hook_event_name"] = json!("PostToolUse");
        let t = Instant::now();
        assert!(handle(&c, &run, &ran, Duration::ZERO).is_err());
        assert!(t.elapsed() < Duration::from_secs(2), "no retry: {:?}", t.elapsed());
        other.execute_batch("commit").unwrap();
        assert!(answer(&ctx, &a.id, &json!({"behavior": "allow"})).unwrap());
        assert!(h.join().unwrap().is_some());
    }

    /// The hook path of a session with no phone does no filesystem work beyond hm.db, as in 1.13.2: every event, a
    /// request answered, one that times out, and an event that cannot be written leave the data folder as it was.
    #[test]
    fn the_hook_path_without_a_phone_touches_no_file() {
        let (_d, ctx, run) = live_run();
        let before = files(&ctx.data);
        let c = hook_connection(&ctx.data.join("hm.db")).unwrap();
        for e in ["UserPromptSubmit", "Notification", "PostToolUse", "Stop", "SessionEnd"] {
            handle(&c, &run, &json!({"hook_event_name": e, "notification_type": "permission_prompt", "tool_name": "Bash", "tool_input": {}}), Duration::ZERO).unwrap();
        }
        let (h, a) = hold(&ctx, &run, BASH, 20_000);
        assert!(answer(&ctx, &a.id, &json!({"behavior": "deny"})).unwrap());
        assert!(h.join().unwrap().is_some());
        let (h, _) = hold(&ctx, &run, BASH, 300);
        assert_eq!(h.join().unwrap(), None);
        let busy = Connection::open(ctx.data.join("hm.db")).unwrap();
        busy.busy_timeout(Duration::from_millis(50)).unwrap();
        let other = conn(&ctx);
        other.execute_batch("begin immediate").unwrap();
        assert!(handle(&busy, &run, &json!({"hook_event_name": "Stop"}), Duration::ZERO).is_err());
        other.execute_batch("commit").unwrap();
        end_run(&ctx, &run);
        assert_eq!(files(&ctx.data), before, "no marker or other file");
        assert!(!ctx.data.join("run-unknown").exists());
    }

    #[test]
    fn a_hook_entry_outlasts_its_window_and_a_permission_waits_at_most_300_s() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
        ctx.settings.request_seconds = 900;
        let v = settings(&ctx);
        assert_eq!(v["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], json!(330));
        let question = v["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"].as_u64().unwrap();
        assert_eq!(question, 15, "no phone: the question's entry is 1.13.2's");
        a_phone(&ctx);
        let question = settings(&ctx)["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"].as_u64().unwrap();
        assert!(question >= u64::from(crate::phones::QUESTION_WITH_PHONE + ENTRY_MARGIN), "the tool would end the hook before an answer taken at the window's end prints: {question}");
        let p = Profile { name: "work".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 };
        let mut env = vec![];
        start_run(&ctx, &p, &mut env);
        assert!(env.contains(&(WAIT_VAR.to_string(), "300".to_string())), "{env:?}");
    }

    #[test]
    fn the_settings_file_holds_only_our_entries_and_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
        let [flag, path] = settings_args(&ctx, &[]).unwrap().unwrap();
        assert_eq!(flag, "--settings");
        assert!(std::path::Path::new(&path).starts_with(&ctx.data), "in the data folder: {path}");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v.as_object().unwrap().keys().collect::<Vec<_>>(), ["hooks"]);
        for e in EVENTS { assert_eq!(v["hooks"][e][0]["hooks"][0]["command"], json!(command()), "{e}"); }
        // PermissionRequest outlasts its wait. PreToolUse gets its own shorter cap.
        assert_eq!(v["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], json!(ctx.settings.request_seconds.min(MAX_REQUEST_SECONDS) + 30));
        assert_eq!(v["hooks"]["PreToolUse"], json!([{"matcher": "AskUserQuestion", "hooks": [{"type": "command", "command": command(), "timeout": 15}]}]));
        let stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        settings_args(&ctx, &["fix it".into()]).unwrap().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), stamp, "the same content is not written again");
        for own in [vec!["--settings".to_string(), "mine.json".into()], vec!["--settings={}".into()]] { assert_eq!(settings_args(&ctx, &own).unwrap(), None, "{own:?}"); }
        ctx.settings.session_hooks = false;
        assert_eq!(settings_args(&ctx, &[]).unwrap(), None);
    }
}
