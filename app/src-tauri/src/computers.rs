//! Across computers: the shared folder, the direct connection and its pairing, the hub and the ring key.

use std::sync::Arc;

use anyhow::Result;
use hrm_core::Ctx;
use serde_json::{json, Value};

use crate::{opt, s, AppState};

// across computers: the shared folder, this computer and the others
pub fn computers_status(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::journal::status(ctx) }

pub fn computers_now(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::journal::round(ctx) }

pub fn computers_set(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut fresh = Arc::new(hrm_core::service::computers_set(ctx, a)?);
    // the listener follows the setting at once (and may pick and save its port)
    if let Some(p) = state.peers.get() { fresh = p.ensure(&fresh)?; }
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh.clone();
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    hrm_core::service::computers_round(&fresh)
}

// the direct connection: pairing with a six-digit code, the computers paired
pub fn computers_pair_start(state: &AppState, _: &Ctx, a: &Value) -> Result<Value> { let v = state.peers()?.pair_start(&state.ctx(), opt(a, "host").as_deref())?; state.refresh(); Ok(v) }

pub fn computers_pair_cancel(state: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { state.peers()?.pair_cancel(ctx) }

pub fn computers_pair_join(state: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let v = state.peers()?.pair_join(&state.ctx(), &s(a, "code")?, opt(a, "host").as_deref())?;
    state.refresh();
    Ok(v)
}

pub fn computers_forget(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::sync::peer::forget(ctx, &s(a, "id")?) }

pub fn computers_set_peer(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::sync::peer::set_peer(ctx, &s(a, "id")?, a["address"].as_str(), a["name"].as_str()) }

// the hub (phase 5): joining with a code or the admin token, inviting, leaving, the recovery key
fn hub_join_or_hub_leave(state: &AppState, ctx: &Ctx, a: &Value, op: &str) -> Result<Value> {
    let fresh = Arc::new(if op == "hub_join" { hrm_core::sync::hub::join(ctx, &s(a, "url")?, opt(a, "code").as_deref(), opt(a, "admin_token").as_deref())? } else { hrm_core::sync::hub::leave(ctx)? });
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh.clone();
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(if hrm_core::journal::on(&fresh) { hrm_core::journal::round(&fresh)? } else { hrm_core::journal::status(&fresh)? })
}
pub fn hub_join(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hub_join_or_hub_leave(state, ctx, a, "hub_join") }
pub fn hub_leave(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hub_join_or_hub_leave(state, ctx, a, "hub_leave") }

pub fn hub_invite(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::sync::hub::invite(ctx) }

pub fn ring_key_show(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(json!({"key": hrm_core::sync::hub::ring_show(ctx)?})) }

pub fn ring_key_enter(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    hrm_core::sync::hub::ring_enter(ctx, &s(a, "key")?)?;
    Ok(if ctx.settings.sync_mode == "hub" { hrm_core::journal::round(ctx)? } else { hrm_core::journal::status(ctx)? })
}
