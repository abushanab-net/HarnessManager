//! A phone's actions on a session: answer a request, reply one line, Stop. Each is checked again just before it
//! acts (the grant, the run, the request, the state version the phone was shown) and gets a receipt kept 24 h; the
//! same `action_id` again returns that receipt and does nothing. An answer goes through `hooks::answer`, a reply and
//! Stop through `terminal::reply` and `terminal::stop` with the desktop's own rules.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use companion_proto::api::{Action, ActionKind, Receipt, Status};
use rusqlite::{params, OptionalExtension};
use serde_json::json;

use crate::phones::{self, Phone};
use crate::util::now_ms;
use crate::{hooks, Ctx};

const KEPT_MS: i64 = 24 * 3_600_000;

/// shortcut: one action at a time on this computer, which serialises each run's actions too; per-run locks if
/// phones ever act on many runs at once and one slow terminal holds the others up
static ONE: Mutex<()> = Mutex::new(());

fn stored(ctx: &Ctx, action_id: &str) -> Result<Option<(String, Receipt)>> {
    ctx.db.read(|c| Ok(c.query_row("select phone_id, status, detail, at from phone_actions where action_id = ?", [action_id], |r| {
        let status: String = r.get(1)?;
        Ok((r.get(0)?, Receipt { action_id: action_id.into(), status: Status::parse(&status).unwrap_or(Status::Unavailable), detail: r.get(2)?, at: r.get(3)? }))
    }).optional()?))
}

/// Act on `a` for phone `p`, once: the receipt.
pub fn act(ctx: &Ctx, p: &Phone, a: &Action) -> Result<Receipt> {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    ctx.db.with(|c| { c.execute("delete from phone_actions where at < ?", [now_ms() - KEPT_MS])?; Ok(()) })?;
    if let Some((phone, r)) = stored(ctx, &a.action_id)? {
        if phone == p.id { return Ok(r); }
        return Ok(Receipt { action_id: a.action_id.clone(), status: Status::Unavailable, detail: "that action id was used by another phone".into(), at: now_ms() });
    }
    let (status, detail) = decide(ctx, p, a);
    let r = Receipt { action_id: a.action_id.clone(), status, detail, at: now_ms() };
    let kind = serde_json::to_value(a.kind)?.as_str().unwrap_or("").to_string();
    ctx.db.with(|c| {
        c.execute("insert into phone_actions(action_id, phone_id, kind, target, status, detail, at) values(?,?,?,?,?,?,?)",
            params![r.action_id, p.id, kind, serde_json::to_string(&a.target)?, r.status.as_str(), r.detail, r.at])?;
        Ok(())
    })?;
    crate::log::info(&format!("phones: {} {kind} on {}: {}", p.name, a.target.run, r.status.as_str()));
    Ok(r)
}

/// Why an answer's request is no longer open: the terminal or another phone answered it, or it expired.
fn ask_gone(ctx: &Ctx, run: &str, ask_id: &str) -> Result<Option<(Status, String)>> {
    let row: Option<(String, i64)> = ctx.db.read(|c| Ok(c.query_row("select state, expires from run_ask where id = ?1 and run = ?2", [ask_id, run], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))?;
    Ok(match row {
        None => Some((Status::Unavailable, "that request is gone with its session".into())),
        Some((state, _)) if state != "open" => Some((Status::AlreadyHandled, "it was answered already, in the terminal or elsewhere".into())),
        Some((_, expires)) if expires <= now_ms() => Some((Status::Expired, "the request expired".into())),
        _ => None,
    })
}

fn decide(ctx: &Ctx, p: &Phone, a: &Action) -> (Status, String) {
    match try_decide(ctx, p, a) { Ok(x) => x, Err(e) => (Status::Unavailable, format!("{e:#}")) }
}

fn try_decide(ctx: &Ctx, p: &Phone, a: &Action) -> Result<(Status, String)> {
    let un = |why: &str| Ok((Status::Unavailable, why.to_string()));
    // the grant as it is now, not as the request began
    let Some(now) = phones::get(ctx, &p.id)? else { return un("this phone is not paired") };
    if now.revoked_at.is_some() || !now.grants.control { return un("Control is not granted to this phone"); }
    if a.expires_at <= now_ms() { return Ok((Status::Expired, "the action expired before it arrived".into())); }
    let run = &a.target.run;
    // the terminal answering first moves the version too: an answer looks at its request before the version
    if a.kind == ActionKind::Answer {
        let Some(ask) = a.target.ask_id.as_deref() else { return un("an answer names its request") };
        if let Some(gone) = ask_gone(ctx, run, ask)? { return Ok(gone); }
    }
    let Some(r) = hooks::runs(ctx)?.into_iter().find(|r| &r.run == run) else { return un("that session is no longer running here") };
    if r.version != a.state_version { return Ok((Status::Expired, "the session changed since this was shown; look again".into())); }
    match a.kind {
        ActionKind::Answer => {
            let ask = a.target.ask_id.as_deref().unwrap_or("");
            match hooks::answer(ctx, ask, &a.payload) {
                Ok(true) => Ok((Status::Accepted, "answered".into())),
                Ok(false) => Ok(ask_gone(ctx, run, ask)?.unwrap_or((Status::Unavailable, "the session no longer waits for this answer".into()))),
                Err(e) => un(&format!("{e:#}")),
            }
        }
        ActionKind::Reply => {
            let rules = super::run_actions(&now, &r);
            if rules["reply"]["ok"] != true { return un(rules["reply"]["reason"].as_str().unwrap_or("")); }
            let Some(text) = a.payload["text"].as_str() else { return un("a reply needs some text") };
            match crate::terminal::reply(&ctx.data, run, text) { Ok(()) => Ok((Status::Accepted, "sent".into())), Err(e) => un(&format!("{e:#}")) }
        }
        ActionKind::Stop => {
            let rules = super::run_actions(&now, &r);
            if rules["stop"]["ok"] != true { return un(rules["stop"]["reason"].as_str().unwrap_or("")); }
            match crate::terminal::stop(&ctx.data, run) { Ok(()) => Ok((Status::Accepted, "stopped".into())), Err(e) => un(&format!("{e:#}")) }
        }
    }
}

pub(super) async fn post(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Json(a): Json<Action>) -> Response {
    if a.action_id.is_empty() || a.action_id.len() > 100 { return (StatusCode::BAD_REQUEST, Json(json!({"error": "an action needs its action_id"}))).into_response(); }
    let c = ctx.clone();
    match super::blocking(move || act(&c, &p, &a)).await {
        Ok(r) => Json(super::stamped(&ctx, json!(r))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("{e:#}")}))).into_response(),
    }
}

/// The stored receipt, for a phone that got no answer to its post.
pub(super) async fn get(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Path(action_id): Path<String>) -> Response {
    let c = ctx.clone();
    match super::blocking(move || stored(&c, &action_id)).await {
        Ok(Some((phone, r))) if phone == p.id => Json(super::stamped(&ctx, json!(r))).into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error": "no such action"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("{e:#}")}))).into_response(),
    }
}
