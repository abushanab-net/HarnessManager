//! A phone's actions on a session. In wave 1 a phone answers requests; Reply and Stop are refused (a later package,
//! WP-M8, designs them). One transaction claims an answer before it reaches the session: it checks the grant and the
//! enrollment, the deadline, the request, the state version the phone was shown and whether the run's events were
//! all recorded, consumes that version (so a second action on the same screen is refused) and reserves the receipt.
//! Just before dispatch the grant, the deadline and the claimed version are looked at again, and the answer's own
//! write checks them once more. The receipt is kept 24 h; the same `action_id` again returns it and does nothing. A
//! reservation that never got its outcome (the process ended on the way) reads as `unknown` and is never dispatched
//! again. An answer goes through `hooks::answer_guarded`.

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
pub(crate) const NOT_YET: &str = "Reply and Stop from a phone are not available yet";
const STATE_UNKNOWN: &str = "An event of this session could not be recorded, so this request may already be settled; answer in its terminal";

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

/// A run as the claim reads it: version, pid, start.
type RunNow = (i64, u32, i64);

/// In the claiming transaction: the outcome when the action cannot go ahead; None when it may, its run's version is
/// then consumed.
fn claim(c: &Connection, p: &Phone, a: &Action) -> Result<Option<(Status, String)>> {
    if a.kind != ActionKind::Answer { return Ok(un(NOT_YET)); }
    if !may_act(c, p)? { return Ok(un("Control is not granted to this phone")); }
    if a.expires_at <= now_ms() { return Ok(Some((Status::Expired, "the action expired before it arrived".into()))); }
    let run = &a.target.run;
    // the terminal answering first moves the version too: an answer looks at its request before the version
    let Some(ask) = a.target.ask_id.as_deref() else { return Ok(un("an answer names its request")) };
    if let Some(gone) = ask_gone(c, run, ask)? { return Ok(Some(gone)); }
    let row: Option<RunNow> = c.query_row("select version, pid, started from run_state where run = ?", [run], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let Some((version, _, _)) = row.filter(|r| crate::procs::start_ms(r.1).is_some_and(|s| r.2 == 0 || s == r.2)) else { return Ok(un("that session is no longer running here")) };
    if hooks::state_unknown(c, run) { return Ok(un(STATE_UNKNOWN)); }
    if version != a.state_version { return Ok(Some((Status::Expired, "the session changed since this was shown; look again".into()))); }
    c.execute("update run_state set version = version + 1 where run = ?1 and version = ?2", params![run, version])?;
    Ok(None)
}

/// Just before dispatch: the grant, the deadline and the state the claim left (a hook moving the run since stops it).
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
    let (run, ask) = (&a.target.run, a.target.ask_id.as_deref().unwrap_or(""));
    // where the answer is written, the claim's preconditions are checked again
    let guard = hooks::Guard { version: a.state_version + 1, deadline: a.expires_at, phone: Some((p.id.clone(), p.generation.clone())) };
    match hooks::answer_guarded(ctx, ask, &a.payload, Some(&guard)) {
        Ok(true) => (Status::Accepted, "answered".into()),
        Ok(false) => ctx.db.read(|c| Ok(ask_gone(c, run, ask)?.or(last_check(c, p, a)?))).ok().flatten()
            .unwrap_or((Status::Unavailable, "the session no longer waits for this answer".into())),
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
