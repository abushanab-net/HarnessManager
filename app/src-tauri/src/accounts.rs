//! The accounts: the list the window opens with, one account, adding and changing, signing in, limits.

use std::path::PathBuf;

use anyhow::Result;
use hrm_core::service::{self, AddRequest, SessionFilter};
use hrm_core::{harness, Ctx};
use serde_json::{json, Value};

use crate::{opt, s, AppState};

pub fn state(state: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    let reg = ctx.registry()?;
    Ok(json!({
        "profiles": service::list_views(ctx)?,
        "harnesses": service::harnesses(),
        "settings": ctx.settings,
        "live_quota": ctx.db.kv_get("live_quota")?.as_deref() == Some("1"),
        "autostart": hrm_core::launch::autostart::get().is_some(),
        // windows, macos or linux: the window leaves out what only Windows has
        "os": std::env::consts::OS,
        // where keys are kept here, as the texts name it: "Windows Credential Manager", "the Keychain", …
        "secret_store": hrm_core::secrets::store().name,
        // tools with a newer version, from the last check (at launch or on the Apps page)
        "app_updates": ctx.db.kv_get("apps:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).and_then(|v| v["updates"].as_u64()).unwrap_or(0),
        "router": {"running": service::router_running(ctx), "port": ctx.settings.router_port},
        "routes": reg.routes,
        "registry_path": ctx.registry_path(),
        "computer": hrm_core::util::hostname(),
        "version": env!("CARGO_PKG_VERSION"),
        "first_run": state.first_run.lock().unwrap_or_else(|e| e.into_inner()).take(),
        "now": hrm_core::util::now_ms(),
    }))
}

pub fn detail(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let name = s(a, "name")?;
    let reg = ctx.registry()?;
    let p = reg.get(&name)?;
    let f = SessionFilter { profile: Some(name.clone()), days: Some(30), per: Some(40), ..Default::default() };
    let sessions = service::sessions(ctx, &f)?;
    Ok(json!({
        "view": service::profile_view(ctx, p),
        "projects": service::profile_projects(ctx, &name, 7)?,
        "sessions": sessions,
        "usage": service::usage_summary(ctx, 7, Some(&name))?,
        "item_paths": {
            "mcp": harness::of(p).item_paths(ctx, p).mcp.map(|m| m.0),
            "skills": harness::of(p).item_paths(ctx, p).skills,
            "prompt": harness::of(p).item_paths(ctx, p).prompt,
        },
        "cli": format!("hrm run {name}"),
    }))
}

pub fn add(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let req: AddRequest = serde_json::from_value(a.clone())?;
    Ok(json!(service::add_profile(ctx, req)?))
}

pub fn remove(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::remove_profile(ctx, &s(a, "name")?, a["delete_home"] == true)?; Ok(json!(true)) }

pub fn update(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    service::update_profile(ctx, &s(a, "name")?, opt(a, "email"), a["base_url"].as_str().map(String::from), a["model"].as_str().map(String::from), a.get("intro").map(|v| v.as_str().map(String::from)))?;
    Ok(json!(true))
}

// the refresh button of one account: its limits read now
pub fn quota_live(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::quota_live(ctx, &s(a, "name")?)?)) }

pub fn set_cap(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::set_daily_cap(ctx, &s(a, "name")?, a["cap"].as_f64())?; Ok(json!(true)) }

pub fn set_remote_control(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::set_remote_control(ctx, &s(a, "name")?, a["on"] == true)?; Ok(json!(true)) }

pub fn set_label(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::set_label(ctx, &s(a, "name")?, a["label"].as_str().unwrap_or(""))?; Ok(json!(true)) }

pub fn set_key(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::set_api_key(ctx, &s(a, "name")?, &s(a, "key")?)?; Ok(json!(true)) }

pub fn start(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let extra: Vec<String> = a["args"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    service::start(ctx, &s(a, "name")?, opt(a, "cwd").map(PathBuf::from), &extra)?;
    Ok(json!(true))
}

pub fn login_start(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::login_start(ctx, &s(a, "name")?)?)) }

pub fn login_submit(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { service::login_submit(&s(a, "id")?, &s(a, "code")?)?; Ok(json!(true)) }

pub fn login_poll(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::login_poll(ctx, &s(a, "id")?)?)) }

pub fn login_cancel(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { service::login_cancel(&s(a, "id")?); Ok(json!(true)) }

pub fn login_terminal(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let name = s(a, "name")?;
    hrm_core::launch::open_terminal(&ctx.settings.terminal, &format!("Sign in · {name}"), &["login".into(), name], None)?;
    Ok(json!(true))
}

pub fn refresh(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let n = service::import_usage(ctx)?;
    let errors = service::refresh_quotas(ctx, a["live"] == true);
    Ok(json!({"new_rows": n, "quota_errors": errors}))
}

pub fn account_stats(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::dash::account_stats(ctx, &s(a, "name")?) }

// what Accounts adds to each row: limit forecasts and daily caps
pub fn limits_extra(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    Ok(json!({"forecast": hrm_core::dash::forecasts(ctx, hrm_core::util::now_ms())?, "caps": hrm_core::dash::caps(ctx, &ctx.registry()?)}))
}
