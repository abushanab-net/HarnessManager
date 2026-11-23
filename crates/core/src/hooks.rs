//! "Needs you": whether a session `hrm run` started is working, waiting for the user or done. The tool's hooks run
//! `hrm hook`, which reads the event and records it in hm.db (`run_state`) for that `hrm run`. Claude Code gets the
//! hook entries per launch, as `--settings <file>` with a file in Harness Manager's data folder: no account's files
//! change, and a session started outside Harness Manager is not touched.
//!
//! A permission request or a question (`AskUserQuestion`) is also held open for an answer from the app: `hrm hook`
//! stores it in `run_ask` and polls the row; the app writes the answer there (Phase 3 will write remote answers the
//! same way). The terminal prompt stays answerable meanwhile and the first answer wins: a later event of the session
//! closes the request, so does the hook's own expiry, and then the hook exits 0 printing nothing. Every other event
//! prints nothing and exits 0, so the tool behaves as without it.

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

/// The events Claude Code reports. PostToolUse: a tool ran (after an approval too), so the session works again.
/// Besides these, PreToolUse for `AskUserQuestion` only (see `settings`).
pub const EVENTS: &[&str] = &["UserPromptSubmit", "PermissionRequest", "Notification", "PostToolUse", "Stop", "SessionEnd"];

/// One row per `hrm run` in hm.db; `hwnd` is its console window (0 when unknown), to bring it to the front. `run_ask`:
/// the requests the hook holds for an answer; `state` is open, answered (then `answer` holds it) or closed.
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
    let c = Connection::open_with_flags(crate::ctx::data_dir().join("hm.db"), OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    c.busy_timeout(std::time::Duration::from_secs(2))?;
    let secs = std::env::var(WAIT_VAR).ok().and_then(|s| s.parse().ok()).unwrap_or(0u64);
    Ok(handle(&c, &run, &v, std::time::Duration::from_secs(secs))?.map(|d| d.to_string()))
}

/// The event for run `run`: later events close what the terminal answered, the state is recorded, and a request
/// waits up to `wait` for the app's answer. Answers the decision for the tool, or None: no decision, the terminal
/// prompt stands.
pub fn handle(c: &Connection, run: &str, v: &Value, wait: std::time::Duration) -> Result<Option<Value>> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let input = v["tool_input"].to_string();
    // the terminal answered first, or the turn moved on: the waiting hook sees its request closed and steps aside.
    // A tool ran: the request for that very call. A notification comes while the prompt still shows: none
    match v["hook_event_name"].as_str().unwrap_or("") {
        "Notification" => {}
        "PostToolUse" => { c.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open' and tool = ?2 and input = ?3", params![run, s("tool_name"), input])?; }
        _ => { c.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open'", [run])?; }
    }
    if let Some((state, reason)) = state_of(v) {
        c.execute("update run_state set session_id = case when ?2 = '' then session_id else ?2 end, cwd = case when ?3 = '' then cwd else ?3 end,
            state = ?4, reason = ?5, ts = ?6 where run = ?1", params![run, s("session_id"), s("cwd"), state, reason, now_ms()])?;
    }
    let Some(kind) = kind_of(v) else { return Ok(None) };
    // only for a session hrm run started, and while the app offers answers
    if wait.is_zero() || c.query_row("select 1 from run_state where run = ?", [run], |_| Ok(())).optional()?.is_none() { return Ok(None); }
    let what = if kind == "question" { question_text(v) } else {
        v["tool_input"]["command"].as_str().or(v["tool_input"]["file_path"].as_str()).unwrap_or("").to_string()
    };
    let (id, created) = (crate::util::uuid4(), now_ms());
    let expires = created + wait.as_millis() as i64;
    c.execute("insert into run_ask(id, run, session_id, kind, tool, what, input, created, expires) values(?,?,?,?,?,?,?,?,?)",
        params![id, run, s("session_id"), kind, s("tool_name"), what, input, created, expires])?;
    loop {
        // a busy database is waited out: leaving would strand an answer the app then reports as given
        let row: rusqlite::Result<Option<(String, String)>> = c.query_row("select state, answer from run_ask where id = ?", [&id], |r| Ok((r.get(0)?, r.get(1)?))).optional();
        match row.as_ref().map(|r| r.as_ref().map(|(s, a)| (s.as_str(), a.as_str()))) {
            Ok(Some(("answered", a))) => {
                let _ = c.execute("update run_state set state = 'working', reason = '', ts = ?2 where run = ?1", params![run, now_ms()]);
                return Ok(Some(decision(kind, &v["tool_input"], a)));
            }
            Ok(Some(("open", _))) | Err(_) => {}
            Ok(_) => return Ok(None), // closed, or gone with its run
        }
        // the expiry and an answer are one update each on the open row: whichever lands first wins
        if now_ms() >= expires && c.execute("update run_ask set state = 'closed' where id = ? and state = 'open'", [&id]).unwrap_or(0) == 1 { return Ok(None); }
        std::thread::sleep(std::time::Duration::from_millis(250));
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

/// The app's answer to request `id`: `{"behavior": "allow" | "deny"}` for a permission, `{"answers": {question:
/// [label, ...]}}` for a question, with one of each question's own options (several where it allows several).
/// False when the request is no longer open (the terminal answered, or it expired): the answer is dropped.
pub fn answer(ctx: &Ctx, id: &str, a: &Value) -> Result<bool> {
    let row: Option<(String, String)> = ctx.db.read(|c| Ok(c.query_row("select kind, input from run_ask where id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))?;
    let Some((kind, input)) = row else { return Ok(false) };
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
    Ok(ctx.db.with(|c| Ok(c.execute("update run_ask set state = 'answered', answer = ?2 where id = ?1 and state = 'open' and expires > ?3", params![id, answer, now_ms()])?))? == 1)
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
    env.push((WAIT_VAR.into(), ctx.settings.request_seconds.to_string()));
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
        let mut st = c.prepare("select run, pid, hwnd, profile, harness, session_id, cwd, state, reason, ts, started, terminal_port is not null from run_state order by ts")?;
        let mut v: Vec<(Run, i64)> = st.query_map([], |r| Ok((Run { run: r.get(0)?, pid: r.get(1)?, hwnd: r.get(2)?, profile: r.get(3)?, harness: r.get(4)?, session_id: r.get(5)?,
            cwd: r.get(6)?, state: r.get(7)?, reason: r.get(8)?, ts: r.get(9)?, terminal: r.get(11)?, ask: None }, r.get(10)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare("select run, id, kind, tool, what, input, created, expires from run_ask where state = 'open' and expires > ? order by created")?;
        let a: Vec<(String, Ask)> = st.query_map([now_ms()], |r| Ok((r.get(0)?, Ask { id: r.get(1)?, kind: r.get(2)?, tool: r.get(3)?, what: r.get(4)?,
            input: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(), created: r.get(6)?, expires: r.get(7)? })))?.collect::<rusqlite::Result<_>>()?;
        for (run, a) in a { if let Some((r, _)) = v.iter_mut().find(|(r, _)| r.run == run) { r.ask = Some(a); } }
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
/// PreToolUse of `AskUserQuestion`. The two that wait for the app get a timeout past `wait` seconds, so the hook
/// ends its own wait first.
fn settings(wait: u32) -> Value {
    let (entry, held) = (|timeout: u32| json!({"type": "command", "command": command(), "timeout": timeout}), wait.saturating_add(30));
    let mut hooks: Map<String, Value> = EVENTS.iter().map(|e| (e.to_string(), json!([{"hooks": [entry(if *e == "PermissionRequest" { held } else { 10 })]}]))).collect();
    hooks.insert("PreToolUse".into(), json!([{"matcher": "AskUserQuestion", "hooks": [entry(held)]}]));
    json!({"hooks": hooks})
}

/// `--settings <file>` for a Claude Code launch while `session_hooks` is on: the file in the data folder, written
/// when its content is not already that. None when off, or when the user's own arguments carry `--settings`.
pub fn settings_args(ctx: &Ctx, user_args: &[String]) -> Result<Option<[String; 2]>> {
    // Claude Code takes one --settings: the user's wins, and that session is not tracked
    if !ctx.settings.session_hooks || user_args.iter().any(|a| a == "--settings" || a.starts_with("--settings=")) { return Ok(None); }
    let path = ctx.data.join("claude-hooks.json");
    let want = serde_json::to_string_pretty(&settings(ctx.settings.request_seconds))?;
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
    fn the_settings_file_holds_only_our_entries_and_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
        let [flag, path] = settings_args(&ctx, &[]).unwrap().unwrap();
        assert_eq!(flag, "--settings");
        assert!(std::path::Path::new(&path).starts_with(&ctx.data), "in the data folder: {path}");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v.as_object().unwrap().keys().collect::<Vec<_>>(), ["hooks"]);
        for e in EVENTS { assert_eq!(v["hooks"][e][0]["hooks"][0]["command"], json!(command()), "{e}"); }
        // the two that wait for the app outlast the hook's own wait; questions only for PreToolUse
        assert_eq!(v["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], json!(ctx.settings.request_seconds + 30));
        assert_eq!(v["hooks"]["PreToolUse"], json!([{"matcher": "AskUserQuestion", "hooks": [{"type": "command", "command": command(), "timeout": ctx.settings.request_seconds + 30}]}]));
        let stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        settings_args(&ctx, &["fix it".into()]).unwrap().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), stamp, "the same content is not written again");
        for own in [vec!["--settings".to_string(), "mine.json".into()], vec!["--settings={}".into()]] { assert_eq!(settings_args(&ctx, &own).unwrap(), None, "{own:?}"); }
        ctx.settings.session_hooks = false;
        assert_eq!(settings_args(&ctx, &[]).unwrap(), None);
    }
}
