//! "Needs you": whether a session `hrm run` started is working, waiting for the user or done. The tool's hooks run
//! `hrm hook`, which reads the event and records it in hm.db (`run_state`) for that `hrm run`. Claude Code gets the
//! hook entries per launch, as `--settings <file>` with a file in Harness Manager's data folder: no account's files
//! change, and a session started outside Harness Manager is not touched.
//!
//! Phase 1 only watches: `hrm hook` prints nothing and always exits 0, so the tool behaves as without it.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::registry::Profile;
use crate::util::{now_ms, write_atomic};
use crate::Ctx;

/// Set by `hrm run` in the tool's environment: `<pid>:<start ms>` of that `hrm run`. Without it `hrm hook` does nothing.
pub const RUN_VAR: &str = "HRM_RUN";

/// The events Claude Code reports. PostToolUse: a tool ran (after an approval too), so the session works again.
pub const EVENTS: &[&str] = &["UserPromptSubmit", "PermissionRequest", "Notification", "PostToolUse", "Stop", "SessionEnd"];

/// One row per `hrm run` in hm.db; `hwnd` is its console window (0 when unknown), to bring it to the front.
pub const TABLE: &str = "create table if not exists run_state(run text primary key, pid integer not null, started integer not null,
  hwnd integer not null default 0, profile text not null, harness text not null, session_id text not null default '',
  cwd text not null default '', state text not null default '', reason text not null default '', ts integer not null default 0);";

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
        "UserPromptSubmit" | "PostToolUse" => Some(("working", String::new())),
        "Stop" | "SessionEnd" => Some(("done", String::new())),
        _ => None,
    }
}

/// `hrm hook`: the event on standard input, recorded for the `hrm run` named in the environment. Only a row that
/// `hrm run` made is changed, so a stale or foreign variable changes nothing.
pub fn record(input: &str) -> Result<()> {
    let Ok(run) = std::env::var(RUN_VAR) else { return Ok(()) };
    let v: Value = serde_json::from_str(input).context("the hook's input is not JSON")?;
    let Some((state, reason)) = state_of(&v) else { return Ok(()) };
    // not Ctx::open: no migrations, no settings, on every tool call; hm.db is there, hrm run opened it
    let c = Connection::open_with_flags(crate::ctx::data_dir().join("hm.db"), OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    c.busy_timeout(std::time::Duration::from_secs(2))?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    c.execute("update run_state set session_id = case when ?2 = '' then session_id else ?2 end, cwd = case when ?3 = '' then cwd else ?3 end,
        state = ?4, reason = ?5, ts = ?6 where run = ?1", params![run, s("session_id"), s("cwd"), state, reason, now_ms()])?;
    Ok(())
}

/// `hrm run`, before the tool starts: its row, and the variable that lets the tool's hooks find it. Answers the run
/// id for `end_run`; None when the row could not be made.
pub fn start_run(ctx: &Ctx, p: &Profile, env: &mut Vec<(String, String)>) -> Option<String> {
    let pid = std::process::id();
    let started = crate::procs::start_ms(pid).unwrap_or(0);
    let run = format!("{pid}:{started}");
    let r = ctx.db.with(|c| Ok(c.execute("insert or replace into run_state(run, pid, started, hwnd, profile, harness, ts) values(?,?,?,?,?,?,?)",
        params![run, pid, started, console_window(), p.name, p.harness, now_ms()])?));
    if let Err(e) = r { crate::log::info(&format!("run state for {}: {e:#}", p.name)); return None; }
    env.push((RUN_VAR.into(), run.clone()));
    Some(run)
}

/// `hrm run`, after the tool exited: its row goes.
pub fn end_run(ctx: &Ctx, run: &str) { let _ = ctx.db.with(|c| Ok(c.execute("delete from run_state where run = ?", [run])?)); }

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
}

/// The sessions that reported a state, oldest report first. The rows of an `hrm run` that is gone (its window
/// was closed, so it could not remove its row) are deleted here.
pub fn runs(ctx: &Ctx) -> Result<Vec<Run>> {
    let rows: Vec<(Run, i64)> = ctx.db.read(|c| {
        let mut st = c.prepare("select run, pid, hwnd, profile, harness, session_id, cwd, state, reason, ts, started from run_state order by ts")?;
        let v = st.query_map([], |r| Ok((Run { run: r.get(0)?, pid: r.get(1)?, hwnd: r.get(2)?, profile: r.get(3)?, harness: r.get(4)?, session_id: r.get(5)?,
            cwd: r.get(6)?, state: r.get(7)?, reason: r.get(8)?, ts: r.get(9)? }, r.get(10)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    // the start time tells the hrm run that made the row from a later process given the same pid
    let (live, gone): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(r, started)| match crate::procs::start_ms(r.pid) {
        Some(s) => *started == 0 || s == *started,
        None => false,
    });
    if !gone.is_empty() {
        ctx.db.with(|c| { for (r, _) in &gone { c.execute("delete from run_state where run = ?", [&r.run])?; } Ok(()) })?;
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

/// The settings Claude Code is started with: only our hook entries, one group under each of `EVENTS`.
fn settings() -> Value {
    let group = json!([{"hooks": [{"type": "command", "command": command(), "timeout": 10}]}]);
    json!({"hooks": EVENTS.iter().map(|e| (e.to_string(), group.clone())).collect::<Map<String, Value>>()})
}

/// `--settings <file>` for a Claude Code launch while `session_hooks` is on: the file in the data folder, written
/// when its content is not already that. None when off, or when the user's own arguments carry `--settings`.
pub fn settings_args(ctx: &Ctx, user_args: &[String]) -> Result<Option<[String; 2]>> {
    // Claude Code takes one --settings: the user's wins, and that session is not tracked
    if !ctx.settings.session_hooks || user_args.iter().any(|a| a == "--settings" || a.starts_with("--settings=")) { return Ok(None); }
    let path = ctx.data.join("claude-hooks.json");
    let want = serde_json::to_string_pretty(&settings())?;
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
        record(&json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash"}).to_string()).unwrap();
        record("not json").unwrap();
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
        let stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        settings_args(&ctx, &["fix it".into()]).unwrap().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), stamp, "the same content is not written again");
        for own in [vec!["--settings".to_string(), "mine.json".into()], vec!["--settings={}".into()]] { assert_eq!(settings_args(&ctx, &own).unwrap(), None, "{own:?}"); }
        ctx.settings.session_hooks = false;
        assert_eq!(settings_args(&ctx, &[]).unwrap(), None);
    }
}
