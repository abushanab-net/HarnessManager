//! Across computers: the shared folder, the direct connection and its pairing, the hub and the ring key; and the
//! phones: their pairing (a QR code, the three words, the grants), the phones paired, their grants and revoking one.

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

// phones: paired with a QR code (or its six digits) and approved here with the three words and the grants
pub fn phones_list(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::phones::list(ctx) }

pub fn phones_set_grants(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::phones::set_grants(ctx, &s(a, "id")?, &a["grants"]) }

pub fn phones_revoke(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::phones::revoke(ctx, &s(a, "id")?) }

/// The code for two minutes, with its QR code drawn as SVG (`qr_svg`) for the dialog.
pub fn phones_pair_start(state: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let mut v = state.peers()?.phone_pair_start(&state.ctx(), opt(a, "host").as_deref())?;
    state.refresh();
    if let Some(q) = v["qr"].as_str() { v["qr_svg"] = json!(qr_svg(q)?); }
    Ok(v)
}

pub fn phones_pair_status(state: &AppState, _: &Ctx, _: &Value) -> Result<Value> { Ok(state.peers()?.phone_pair_status().unwrap_or(Value::Null)) }

pub fn phones_pair_approve(state: &AppState, _: &Ctx, a: &Value) -> Result<Value> { state.peers()?.phone_pair_approve(&state.ctx(), &a["grants"]) }

pub fn phones_pair_refuse(state: &AppState, _: &Ctx, _: &Value) -> Result<Value> { state.peers()?.phone_pair_refuse() }

pub fn phones_pair_cancel(state: &AppState, _: &Ctx, _: &Value) -> Result<Value> { state.peers()?.phone_pair_cancel() }

/// `text` as a QR code in SVG, black on white with its quiet zone (a phone camera reads it in either theme).
fn qr_svg(text: &str) -> Result<String> {
    let svg = qrcode::QrCode::new(text)?.render::<qrcode::render::svg::Color>().min_dimensions(176, 176).build();
    // the XML declaration has no place inside the page
    Ok(svg.find("<svg").map_or(svg.clone(), |i| svg[i..].to_string()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn qr_svg_is_an_svg_element() {
        let s = super::qr_svg("hm-phone:1?h=10.0.0.2:47100&c=481920&f=ab12").unwrap();
        assert!(s.starts_with("<svg") && s.ends_with("</svg>") && s.contains("<path"), "{s}");
    }
}
