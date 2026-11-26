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

/// `handle_asking` with the question window of a computer without a phone.
pub fn handle(c: &Connection, run: &str, v: &Value, wait: Duration) -> Result<Option<Value>> {
    handle_asking(c, run, v, wait, Duration::from_secs(crate::phones::QUESTION_WITHOUT.into()))
}

/// The event for run `run`: later events close what the terminal answered, the state is recorded, and a request
/// waits up to `wait` for the app's answer (a question up to `question`). Answers the decision for the tool, or None:
/// no decision, the terminal prompt stands. The run's `version` moves with every change of its state or of its
/// requests, so a phone's action names the state it was shown.
pub fn handle_asking(c: &Connection, run: &str, v: &Value, wait: Duration, question: Duration) -> Result<Option<Value>> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let input = v["tool_input"].to_string();
    // the terminal answered first, or the turn moved on: the waiting hook sees its request closed and steps aside.
    // A tool ran: the request for that very call. Other notifications can come while the prompt still shows.
    let closed = match v["hook_event_name"].as_str().unwrap_or("") {
        "Notification" if v["notification_type"] != "idle_prompt" => 0,
        "PostToolUse" => c.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open' and tool = ?2 and input = ?3", params![run, s("tool_name"), input])?,
        _ => c.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open'", [run])?,
    };
    if let Some((state, reason)) = state_of(v) {
        c.execute("update run_state set session_id = case when ?2 = '' then session_id else ?2 end, cwd = case when ?3 = '' then cwd else ?3 end,
            version = version + (state <> ?4 or ?7 > 0), state = ?4, reason = ?5, ts = ?6,
            stopping = case when state <> ?4 then null else stopping end, stop_token = case when state <> ?4 then null else stop_token end,
            needs_terminal = case when ?8 then 0 else needs_terminal end where run = ?1",
            // a new state ends a Stop's claim (a repeated one does not); a submitted prompt took what waited in the input
            params![run, s("session_id"), s("cwd"), state, reason, now_ms(), closed as i64, v["hook_event_name"] == "UserPromptSubmit"])?;
    } else if closed > 0 {
        c.execute("update run_state set version = version + 1 where run = ?", [run])?;
    }
    let Some(kind) = kind_of(v) else { return Ok(None) };
    let wait = if kind == "question" { wait.min(question) } else { wait };
    // only for a session hrm run started, and while the app offers answers
    if wait.is_zero() || !app_is_present(c)? || c.query_row("select 1 from run_state where run = ?", [run], |_| Ok(())).optional()?.is_none() { return Ok(None); }
    let pid = std::process::id();
    let Some(started) = crate::procs::start_ms(pid) else { return Ok(None) };
    let what = if kind == "question" { question_text(v) } else {
        v["tool_input"]["command"].as_str().or(v["tool_input"]["file_path"].as_str()).unwrap_or("").to_string()
    };
    let (id, created) = (crate::util::uuid4(), now_ms());
    let expires = created + wait.as_millis() as i64;
    let deadline = Instant::now() + wait;
    // the request and the version it moves come together: nobody sees the request beside the version before it
    let tx = c.unchecked_transaction()?;
    tx.execute("insert into run_ask(id, run, session_id, kind, tool, what, input, created, expires, hook_pid, hook_started) values(?,?,?,?,?,?,?,?,?,?,?)",
        params![id, run, s("session_id"), kind, s("tool_name"), what, input, created, expires, pid, started])?;
    tx.execute("update run_state set version = version + 1 where run = ?", [run])?;
    tx.commit()?;
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
            and (?6 is null or exists (select 1 from phones where phone_id = ?6 and generation = ?7 and revoked_at is null and json_extract(grants, '$.control') = 1))",
            params![id, answer, now, version, deadline, phone, generation])?;
        tx.commit()?;
        Ok(n == 1)
    })
}

/// What a phone's action carries to where its effect is committed (the hook's write, the terminal's relay): the run
/// version its claim left, its deadline (ms), and the enrollment it came from (`phone_id`, `generation`).
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
pub struct Guard {
    pub version: i64,
    pub deadline: i64,
    pub phone: Option<(String, String)>,
    /// A Stop's own claim (`claim_stop`): the relay takes no other.
    #[serde(default)]
    pub stop: Option<String>,
}

/// The guard still holds for `run`, just before input reaches it: the deadline is ahead, the run is at the claimed
/// version, the phone still has Control, and for a reply no request waits. Err: why not.
pub fn guard_holds(c: &Connection, run: &str, g: &Guard, reply: bool) -> std::result::Result<(), String> {
    let now = now_ms();
    if now >= g.deadline { return Err("the action's deadline passed".into()); }
    let row: Option<(i64, String)> = c.query_row("select version, state from run_state where run = ?", [run], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?;
    let Some((version, state)) = row else { return Err("the session is gone".into()) };
    if version != g.version { return Err("the session changed".into()); }
    if let Some(token) = &g.stop {
        let mine: bool = c.query_row("select stop_token is ?2 from run_state where run = ?1", params![run, token], |r| r.get(0)).map_err(|e| e.to_string())?;
        if !mine { return Err("this Stop's claim on the turn has ended".into()); }
    }
    if reply {
        let ask = c.query_row("select count(*) from run_ask where run = ?1 and state = 'open' and expires > ?2", params![run, now], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?;
        if state == "waiting" || ask > 0 { return Err("a request waits for an answer".into()); }
    }
    if let Some((phone, generation)) = &g.phone {
        let row: Option<(String, String, Option<i64>)> = c.query_row("select coalesce(grants, ''), generation, revoked_at from phones where phone_id = ?", [phone], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional().map_err(|e| e.to_string())?;
        let control = row.is_some_and(|(g, gen, revoked)| revoked.is_none() && &gen == generation && serde_json::from_str::<Value>(&g).is_ok_and(|g| g["control"] == true));
        if !control { return Err("Control is no longer granted to this phone".into()); }
    }
    Ok(())
}

/// The one live run `run` names: a run id, or a session id that exactly one live run reports. None when no row names
/// it at all (the terminal then says it has no owned terminal).
pub fn live_run(c: &Connection, run: &str) -> Result<Option<String>> {
    let rows: Vec<(String, u32, i64)> = c.prepare("select run, pid, started from run_state where run = ?1 or (?1 <> '' and session_id = ?1 and not exists (select 1 from run_state where run = ?1))")?
        .query_map([run], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    if rows.is_empty() { return Ok(None); }
    let live: Vec<String> = rows.into_iter().filter(|(_, pid, started)| crate::procs::start_ms(*pid).is_some_and(|s| *started == 0 || s == *started)).map(|(r, _, _)| r).collect();
    match live.as_slice() {
        [one] => Ok(Some(one.clone())),
        [] => bail!("run '{run}' is not running"),
        _ => bail!("session '{run}' has several live runs. Use the run ID"),
    }
}

/// How long a Stop's claim is good for, and its relay deadline: a claim older than this either ended its turn (the
/// relay finalises a Stop in the write that sends it) or surely never reached the terminal (the relay refuses it past
/// its deadline), so it may be claimed again. The same rule for the desktop and a phone; a lost acknowledgement
/// blocks Stop for at most this long, and never lets a second Ctrl+C reach a tool the first one already stopped.
pub const STOP_WINDOW_MS: i64 = 5_000;

/// A Stop's claim on one run's turn: the run, its version after the claim, the claim's own token and its deadline.
#[derive(Debug, Clone)]
pub struct StopClaim {
    pub run: String,
    pub version: i64,
    pub token: String,
    pub deadline: i64,
}

/// A Stop claimed less than `STOP_WINDOW_MS` ago is on its way.
pub fn stop_pending(stopping: Option<i64>, now: i64) -> bool { stopping.is_some_and(|t| t > now - STOP_WINDOW_MS) }

/// Stop claims the turn it interrupts, from the desktop and from a phone alike: one live run is resolved, and in one
/// write that finds it working or waiting (or not yet reported) and not already being stopped, the claim is recorded
/// with its own token and the version it leaves, which the write itself returns. The run's state is left alone: the
/// relay ends the turn in the write that sends the Ctrl+C (`stop_landed`); a Stop that surely did not reach the
/// terminal gives its claim back (`release_stop`). With `expected` (a phone's), the run must also still be at that
/// version. None when no row names `run` (the terminal then says it has no owned terminal).
pub fn claim_stop(c: &Connection, run: &str, expected: Option<i64>) -> Result<Option<StopClaim>> {
    let Some(run) = live_run(c, run)? else { return Ok(None) };
    let (now, token) = (now_ms(), crate::util::uuid4());
    let version: Option<i64> = c.query_row("update run_state set stopping = ?2, stop_token = ?5, version = version + 1 where run = ?1 and state in ('working', 'waiting', '')
        and (stopping is null or stopping <= ?3) and (?4 is null or version = ?4) returning version", params![run, now, now - STOP_WINDOW_MS, expected, token], |r| r.get(0)).optional()?;
    let Some(version) = version else {
        let (stopping, version): (Option<i64>, i64) = c.query_row("select stopping, version from run_state where run = ?", [&run], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if expected.is_some_and(|v| v != version) { bail!("the session changed since this was shown; look again"); }
        if stop_pending(stopping, now) { bail!("a Stop is already on its way to this session"); }
        bail!("Nothing to stop: the session is at its prompt");
    };
    Ok(Some(StopClaim { run, version, token, deadline: now + STOP_WINDOW_MS }))
}

/// A Stop that surely did not reach the terminal gives its claim back; only its own.
pub fn release_stop(c: &Connection, run: &str, token: &str) -> Result<()> {
    c.execute("update run_state set stopping = null, stop_token = null where run = ?1 and stop_token = ?2", [run, token])?;
    Ok(())
}

/// In the relay's held write, once the Ctrl+C went out: the turn this claim was for ends, with the requests it had
/// (not one opened since), and nothing changes when the claim is no longer the run's.
pub fn stop_landed(c: &Connection, run: &str, token: &str) -> Result<bool> {
    let Some(at) = c.query_row("select stopping from run_state where run = ?1 and stop_token = ?2", [run, token], |r| r.get::<_, i64>(0)).optional()? else { return Ok(false) };
    c.execute("update run_ask set state = 'closed' where run = ?1 and state = 'open' and created <= ?2", params![run, at])?;
    c.execute("update run_state set state = 'done', reason = '', stopping = null, stop_token = null, version = version + 1, ts = ?3 where run = ?1 and stop_token = ?2",
        params![run, token, now_ms()])?;
    Ok(true)
}

/// What a remote reply is told while text an earlier one left unsent waits in the input.
pub const UNSENT_TEXT: &str = "text from an earlier reply waits unsent in the session's input; finish or clear it in the terminal";

/// Text a reply left unsent waits in the run's input (read inside the relay's held write): no remote reply now.
pub fn unsent_waits(c: &Connection, run: &str) -> bool {
    c.query_row("select needs_terminal <> 0 from run_state where run = ?", [run], |r| r.get(0)).optional().ok().flatten().unwrap_or(true)
}

/// A reply's text was typed and its Enter withheld: remote replies stop until the terminal settles it.
pub fn withheld(c: &Connection, run: &str) -> Result<()> {
    c.execute("update run_state set needs_terminal = 1 where run = ?", [run])?;
    Ok(())
}

/// Someone typed at the terminal: what a withheld reply left there is theirs now.
pub fn terminal_typed(c: &Connection, run: &str) -> Result<()> {
    c.execute("update run_state set needs_terminal = 0 where run = ?1 and needs_terminal <> 0", [run])?;
    Ok(())
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
    /// A Stop is on its way (`claim_stop`): no other Stop, and no reply, until it lands or is released.
    pub stopping: bool,
    /// Text a reply left unsent waits in its input: no remote reply until the terminal settles it.
    pub needs_terminal: bool,
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
        let mut st = c.prepare("select run, pid, hwnd, profile, harness, session_id, cwd, state, reason, ts, started, terminal_port is not null, version, stopping, needs_terminal <> 0 from run_state order by ts")?;
        let mut v: Vec<(Run, i64)> = st.query_map([], |r| Ok((Run { run: r.get(0)?, pid: r.get(1)?, hwnd: r.get(2)?, profile: r.get(3)?, harness: r.get(4)?, session_id: r.get(5)?,
            cwd: r.get(6)?, state: r.get(7)?, reason: r.get(8)?, ts: r.get(9)?, version: r.get(12)?, stopping: stop_pending(r.get(13)?, now_ms()), needs_terminal: r.get(14)?, terminal: r.get(11)?, ask: None }, r.get(10)?)))?.collect::<rusqlite::Result<_>>()?;
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
/// PreToolUse of `AskUserQuestion`. Permission hooks can outlast their own wait. A question's entry allows the
/// longest question window (60 s, with a phone); the hook itself exits after the window its launch was given, since
/// Claude draws the terminal question only after the hook exits.
fn settings(ctx: &Ctx) -> Value {
    #[cfg(not(target_os = "linux"))]
    let command = command();
    #[cfg(target_os = "linux")]
    let command = crate::cli_path::installed_cli(ctx)
        .map(|exe| format!("{} hook", crate::launch::sh_quote(&exe.to_string_lossy())))
        .unwrap_or_else(command);
    let (entry, held) = (|timeout: u32| json!({"type": "command", "command": command, "timeout": timeout}), ctx.settings.request_seconds.min(MAX_REQUEST_SECONDS) + 30);
    let mut hooks: Map<String, Value> = EVENTS.iter().map(|e| (e.to_string(), json!([{"hooks": [entry(if *e == "PermissionRequest" { held } else { 10 })]}]))).collect();
    hooks.insert("PreToolUse".into(), json!([{"matcher": "AskUserQuestion", "hooks": [entry(crate::phones::QUESTION_WITH_PHONE + ENTRY_MARGIN)]}]));
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
        let g = |version: i64, deadline: i64| Guard { version, deadline, phone: None, stop: None };
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
        let g = |deadline: i64| Guard { version: v, deadline, phone: Some((phone.id.clone(), phone.generation.clone())), stop: None };
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

    /// (b): a Stop's claim survives a notification that repeats the state; it ends with a new state.
    #[test]
    fn a_stop_claim_survives_a_repeated_state_and_ends_with_a_new_one() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        handle(&c, &run, &json!({"hook_event_name": "UserPromptSubmit"}), Duration::ZERO).unwrap();
        let claim = claim_stop(&c, &run, None).unwrap().unwrap();
        handle(&c, &run, &json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {}}), Duration::ZERO).unwrap();
        let token: Option<String> = c.query_row("select stop_token from run_state where run = ?", [&run], |r| r.get(0)).unwrap();
        assert_eq!(token.as_deref(), Some(claim.token.as_str()), "working again is the same turn");
        handle(&c, &run, &json!({"hook_event_name": "Stop"}), Duration::ZERO).unwrap();
        let token: Option<String> = c.query_row("select stop_token from run_state where run = ?", [&run], |r| r.get(0)).unwrap();
        assert_eq!(token, None, "the turn ended by itself");
    }

    /// (b): a Stop's end and release touch only its own claim, and its end closes only the requests of its turn.
    #[test]
    fn a_stop_ends_and_releases_only_its_own_claim() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        handle(&c, &run, &json!({"hook_event_name": "UserPromptSubmit"}), Duration::ZERO).unwrap();
        let first = claim_stop(&c, &run, None).unwrap().unwrap();
        // the first claim's window ran out unanswered; a second Stop claims the turn, and a request opens after it
        c.execute("update run_state set stopping = stopping - ?2 where run = ?1", params![run, STOP_WINDOW_MS + 1]).unwrap();
        let second = claim_stop(&c, &run, None).unwrap().unwrap();
        c.execute("insert into run_ask(id, run, kind, tool, created, expires) values('later', ?1, 'permission', 'Bash', ?2, ?3)", params![run, now_ms() + 1, now_ms() + 60_000]).unwrap();
        release_stop(&c, &run, &first.token).unwrap();
        assert!(!stop_landed(&c, &run, &first.token).unwrap(), "the first claim is no longer the run's");
        let token: Option<String> = c.query_row("select stop_token from run_state where run = ?", [&run], |r| r.get(0)).unwrap();
        assert_eq!(token.as_deref(), Some(second.token.as_str()));
        assert!(stop_landed(&c, &run, &second.token).unwrap());
        let (state, ask): (String, String) = c.query_row("select (select state from run_state where run = ?1), (select state from run_ask where id = 'later')", [&run], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((state.as_str(), ask.as_str()), ("done", "open"), "a request opened after the Stop's claim is not its turn's");
    }

    /// (e): a Stop whose outcome is not known holds the turn for its window only, the same for every caller: once
    /// delivered it ended the turn (no second Ctrl+C); if it never arrived, Stop is offered again after the window.
    #[test]
    fn an_unknown_stop_holds_its_turn_for_its_window_only() {
        let (_d, ctx, run) = live_run();
        let c = conn(&ctx);
        handle(&c, &run, &json!({"hook_event_name": "UserPromptSubmit"}), Duration::ZERO).unwrap();
        claim_stop(&c, &run, None).unwrap().unwrap();
        assert_eq!(format!("{:#}", claim_stop(&c, &run, None).unwrap_err()), "a Stop is already on its way to this session");
        assert!(runs(&ctx).unwrap()[0].stopping, "a phone is told the same");
        // it never arrived: the relay refuses it past its deadline, so after the window the turn is free again
        c.execute("update run_state set stopping = stopping - ?2 where run = ?1", params![run, STOP_WINDOW_MS + 1]).unwrap();
        assert!(!runs(&ctx).unwrap()[0].stopping);
        let again = claim_stop(&c, &run, None).unwrap().unwrap();
        // it arrived: the relay ended the turn in the write that sent it, so nothing is left to stop
        assert!(stop_landed(&c, &run, &again.token).unwrap());
        c.execute("update run_state set stopping = null", []).unwrap();
        assert_eq!(format!("{:#}", claim_stop(&c, &run, None).unwrap_err()), "Nothing to stop: the session is at its prompt");
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

    #[test]
    fn a_hook_entry_outlasts_its_window_and_a_permission_waits_at_most_300_s() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
        ctx.settings.request_seconds = 900;
        let v = settings(&ctx);
        assert_eq!(v["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], json!(330));
        let question = v["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"].as_u64().unwrap();
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
        assert_eq!(v["hooks"]["PreToolUse"], json!([{"matcher": "AskUserQuestion", "hooks": [{"type": "command", "command": command(), "timeout": 70}]}]));
        let stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        settings_args(&ctx, &["fix it".into()]).unwrap().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), stamp, "the same content is not written again");
        for own in [vec!["--settings".to_string(), "mine.json".into()], vec!["--settings={}".into()]] { assert_eq!(settings_args(&ctx, &own).unwrap(), None, "{own:?}"); }
        ctx.settings.session_hooks = false;
        assert_eq!(settings_args(&ctx, &[]).unwrap(), None);
    }
}
