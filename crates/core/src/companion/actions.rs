//! A phone's actions on a session: answer a request, reply one line, Stop. One transaction claims an action before
//! anything reaches the session: it checks the grant and the enrollment, the deadline, the request, the state version
//! the phone was shown and the desktop's own rules, consumes that version (so a second action on the same screen is
//! refused) and reserves the receipt. Just before dispatch the grant, the deadline and the claimed version are
//! looked at again. The receipt is kept 24 h; the same `action_id` again returns it and does nothing. A reservation
//! that never got its outcome (the process ended on the way) reads as `unknown` and is never dispatched again.
//! An answer goes through `hooks::answer`, a reply and Stop through `terminal::reply` and `terminal::stop`.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use companion_proto::api::{Action, ActionKind, Receipt, Status};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::phones::Phone;
use crate::util::now_ms;
use crate::{hooks, Ctx};

const KEPT_MS: i64 = 24 * 3_600_000;
/// A reply's text as the terminal takes it (control.rs): one line, no control characters, at most 16 KiB.
const MAX_TEXT: usize = 16 * 1024;

/// shortcut: one action at a time on this computer, which serialises each run's actions too; per-run locks if
/// phones ever act on many runs at once and one slow terminal holds the others up
static ONE: Mutex<()> = Mutex::new(());

#[cfg(test)]
thread_local! {
    /// Test seams: run just before dispatch; fail between dispatch and the outcome's write; count dispatches.
    pub(crate) static BEFORE_DISPATCH: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
    pub(crate) static CRASH_AFTER_DISPATCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static DISPATCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The receipt kept for `action_id` (younger than 24 h), with the enrollment that made it.
fn stored(ctx: &Ctx, action_id: &str) -> Result<Option<(String, String, Receipt)>> {
    ctx.db.read(|c| Ok(c.query_row("select phone_id, generation, status, detail, at from phone_actions where action_id = ?1 and at >= ?2", params![action_id, now_ms() - KEPT_MS], |r| {
        let status: String = r.get(2)?;
        let (status, detail) = match Status::parse(&status) {
            Some(s) => (s, r.get(3)?),
            None => (Status::Unknown, "it was being delivered when the computer stopped; look at the session before acting again".to_string()),
        };
        Ok((r.get(0)?, r.get(1)?, Receipt { action_id: action_id.into(), status, detail, at: r.get(4)? }))
    }).optional()?))
}

/// Act on `a` for phone `p`, once: the receipt.
pub fn act(ctx: &Ctx, p: &Phone, a: &Action) -> Result<Receipt> {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    ctx.db.with(|c| { c.execute("delete from phone_actions where at < ?", [now_ms() - KEPT_MS])?; Ok(()) })?;
    if let Some((phone, generation, r)) = stored(ctx, &a.action_id)? {
        if phone == p.id && generation == p.generation { return Ok(r); }
        return Ok(Receipt { action_id: a.action_id.clone(), status: Status::Unavailable, detail: "that action id was used by another phone".into(), at: now_ms() });
    }
    let kind = serde_json::to_value(a.kind)?.as_str().unwrap_or("").to_string();
    let mut at = now_ms();
    let decided = ctx.db.with(|c| {
        let tx = c.transaction()?;
        let d = claim(&tx, p, a)?;
        tx.execute("insert into phone_actions(action_id, phone_id, generation, kind, target, status, detail, at) values(?,?,?,?,?,?,?,?)",
            params![a.action_id, p.id, p.generation, kind, serde_json::to_string(&a.target)?, d.as_ref().map(|(s, _)| s.as_str()).unwrap_or("pending"), d.as_ref().map(|(_, t)| t.as_str()).unwrap_or(""), at])?;
        tx.commit()?;
        Ok(d)
    })?;
    let (status, detail) = match decided {
        Some(d) => d,
        None => {
            #[cfg(test)]
            if let Some(f) = BEFORE_DISPATCH.with(|b| b.borrow_mut().take()) { f(); }
            let out = match ctx.db.read(|c| last_check(c, p, a))? { Some(no) => no, None => dispatch(ctx, p, a) };
            #[cfg(test)]
            if CRASH_AFTER_DISPATCH.with(|c| c.replace(false)) { anyhow::bail!("the process ended before the outcome was written"); }
            at = now_ms();
            ctx.db.with(|c| { c.execute("update phone_actions set status = ?2, detail = ?3, at = ?4 where action_id = ?1 and status = 'pending'", params![a.action_id, out.0.as_str(), out.1, at])?; Ok(()) })?;
            out
        }
    };
    crate::log::info(&format!("phones: {} {kind} on {}: {}", p.name, a.target.run, status.as_str()));
    Ok(Receipt { action_id: a.action_id.clone(), status, detail, at })
}

fn un(why: &str) -> Option<(Status, String)> { Some((Status::Unavailable, why.to_string())) }

/// The phone's enrollment as it stands, with Control.
fn may_act(c: &Connection, p: &Phone) -> Result<bool> {
    let row: Option<(String, String, Option<i64>)> = c.query_row("select coalesce(grants, ''), generation, revoked_at from phones where phone_id = ?", [&p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    Ok(row.is_some_and(|(g, generation, revoked)| revoked.is_none() && generation == p.generation && serde_json::from_str::<companion_proto::api::Grants>(&g).is_ok_and(|g| g.control)))
}

/// A reply the terminal will take: one line of text, no control characters, at most 16 KiB.
fn reply_text(a: &Action) -> Option<&str> {
    a.payload["text"].as_str().filter(|t| !t.trim().is_empty() && t.len() <= MAX_TEXT && !t.chars().any(char::is_control))
}

/// In the claiming transaction: the outcome when the action cannot go ahead; None when it may, its run's version is
/// then consumed.
fn claim(c: &Connection, p: &Phone, a: &Action) -> Result<Option<(Status, String)>> {
    if !may_act(c, p)? { return Ok(un("Control is not granted to this phone")); }
    if a.expires_at <= now_ms() { return Ok(Some((Status::Expired, "the action expired before it arrived".into()))); }
    let run = &a.target.run;
    // the terminal answering first moves the version too: an answer looks at its request before the version
    if a.kind == ActionKind::Answer {
        let Some(ask) = a.target.ask_id.as_deref() else { return Ok(un("an answer names its request")) };
        if let Some(gone) = ask_gone(c, run, ask)? { return Ok(Some(gone)); }
    }
    let row: Option<(String, i64, bool, u32, i64, bool)> = c.query_row("select state, version, terminal_port is not null, pid, started, stopping is not null from run_state where run = ?", [run],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).optional()?;
    let Some((state, version, terminal, _, _, stopping)) = row.filter(|r| crate::procs::start_ms(r.3).is_some_and(|s| r.4 == 0 || s == r.4)) else { return Ok(un("that session is no longer running here")) };
    if version != a.state_version { return Ok(Some((Status::Expired, "the session changed since this was shown; look again".into()))); }
    // an open request blocks a reply whatever the state says: the reply's Enter would answer its prompt
    let asked = c.query_row("select count(*) from run_ask where run = ?1 and state = 'open' and expires > ?2", params![run, now_ms()], |r| r.get::<_, i64>(0))? > 0;
    match a.kind {
        ActionKind::Answer => {}
        ActionKind::Reply | ActionKind::Stop if !terminal => return Ok(un("Needs Harness Manager's own terminal")),
        ActionKind::Reply | ActionKind::Stop if stopping => return Ok(un("a Stop is already on its way to this session")),
        ActionKind::Reply if state == "waiting" || asked => return Ok(un("Answer the request first")),
        ActionKind::Reply if reply_text(a).is_none() => return Ok(un("a reply is one line of text, at most 16 KiB")),
        ActionKind::Stop if state != "working" && state != "waiting" => return Ok(un("Nothing to stop: the session is at its prompt")),
        _ => {}
    }
    c.execute("update run_state set version = version + 1 where run = ?1 and version = ?2", params![run, version])?;
    Ok(None)
}

/// Just before dispatch: the grant, the deadline and the state the claim left (a hook moving the run since, say a
/// prompt opening under a reply, stops it).
fn last_check(c: &Connection, p: &Phone, a: &Action) -> Result<Option<(Status, String)>> {
    if !may_act(c, p)? { return Ok(un("Control is not granted to this phone")); }
    if a.expires_at <= now_ms() { return Ok(Some((Status::Expired, "the action expired before it could be delivered".into()))); }
    let v: Option<i64> = c.query_row("select version from run_state where run = ?", [&a.target.run], |r| r.get(0)).optional()?;
    if v != Some(a.state_version + 1) { return Ok(Some((Status::Expired, "the session changed while this was on its way; look again".into()))); }
    Ok(None)
}

fn dispatch(ctx: &Ctx, p: &Phone, a: &Action) -> (Status, String) {
    #[cfg(test)]
    DISPATCHED.with(|d| d.set(d.get() + 1));
    let run = &a.target.run;
    // where the effect is committed (the hook's write, the terminal's relay), the claim's preconditions are checked again
    let guard = hooks::Guard { version: a.state_version + 1, deadline: a.expires_at, phone: Some((p.id.clone(), p.generation.clone())) };
    let r = match a.kind {
        ActionKind::Answer => {
            let ask = a.target.ask_id.as_deref().unwrap_or("");
            match hooks::answer_guarded(ctx, ask, &a.payload, Some(&guard)) {
                Ok(true) => Ok("answered"),
                Ok(false) => return ctx.db.read(|c| Ok(ask_gone(c, run, ask)?.or(last_check(c, p, a)?))).ok().flatten()
                    .unwrap_or((Status::Unavailable, "the session no longer waits for this answer".into())),
                Err(e) => Err(e),
            }
        }
        ActionKind::Reply => crate::terminal::reply_guarded(&ctx.data, run, reply_text(a).unwrap_or_default(), Some(&guard)).map(|_| "sent"),
        ActionKind::Stop => crate::terminal::stop_guarded(&ctx.data, run, Some(&guard)).map(|_| "stopped"),
    };
    match r {
        Ok(d) => (Status::Accepted, d.into()),
        // sent, and not answered: it may have arrived, so it must not read as failed (a resend would repeat it)
        Err(e) if e.downcast_ref::<crate::terminal::MaybeDelivered>().is_some() => (Status::Unknown, format!("{e:#}")),
        Err(e) => (Status::Unavailable, format!("{e:#}")),
    }
}

/// Why an answer's request is no longer open: the terminal or another phone answered it, or it expired.
fn ask_gone(c: &Connection, run: &str, ask_id: &str) -> Result<Option<(Status, String)>> {
    let row: Option<(String, i64)> = c.query_row("select state, expires from run_ask where id = ?1 and run = ?2", [ask_id, run], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    Ok(match row {
        None => un("that request is gone with its session"),
        Some((state, _)) if state == "expired" => Some((Status::Expired, "the request expired".into())),
        Some((state, _)) if state != "open" => Some((Status::AlreadyHandled, "it was answered already, in the terminal or elsewhere".into())),
        Some((_, expires)) if expires <= now_ms() => Some((Status::Expired, "the request expired".into())),
        _ => None,
    })
}

pub(super) async fn post(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Json(a): Json<Action>) -> Response {
    if a.action_id.is_empty() || a.action_id.len() > 100 { return (StatusCode::BAD_REQUEST, Json(json!({"error": "an action needs its action_id"}))).into_response(); }
    let c = ctx.clone();
    match super::blocking(move || act(&c, &p, &a)).await {
        Ok(r) => Json(super::stamped(&ctx, json!(r))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("{e:#}")}))).into_response(),
    }
}

/// The stored receipt, for a phone that got no answer to its post: only this enrollment's, only for 24 h.
pub(super) async fn get(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Path(action_id): Path<String>) -> Response {
    let c = ctx.clone();
    match super::blocking(move || stored(&c, &action_id)).await {
        Ok(Some((phone, generation, r))) if phone == p.id && generation == p.generation => Json(super::stamped(&ctx, json!(r))).into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error": "no such action"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("{e:#}")}))).into_response(),
    }
}
