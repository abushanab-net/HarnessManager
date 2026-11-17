//! Settings and the system around the app: backups, restore, the archive, imports, the log, folders and links.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use hrm_core::{items, service, Ctx};
use serde_json::{json, Value};

use crate::{open_path, opt, s, AppState};

pub fn settings_set(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let fresh = service::settings_set(ctx, &s(a, "key")?, a["value"].clone())?;
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(fresh);
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(json!(true))
}

pub fn live_quota_set(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { ctx.db.kv_set("live_quota", if a["on"] == true { "1" } else { "0" })?; Ok(json!(true)) }

pub fn autostart_set(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let exe = std::env::current_exe()?;
    hrm_core::launch::autostart::set(a["on"] == true, &exe)?;
    Ok(json!(hrm_core::launch::autostart::get().is_some()))
}

pub fn import_ps(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::import_powershell(ctx, opt(a, "path").map(PathBuf::from))?)) }

pub fn detect(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(json!(service::detect(ctx)?)) }

pub fn backups(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    Ok(json!({"list": hrm_core::backup::list(ctx), "dir": hrm_core::backup::dir(ctx), "data": ctx.data, "log": hrm_core::log::file(&ctx.data), "in_sync_folder": hrm_core::backup::in_sync_folder(ctx)}))
}

pub fn backup_create(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    Ok(json!(hrm_core::backup::create_with(ctx, a["label"].as_str().unwrap_or("Manual"), a["transcripts"].as_bool().unwrap_or(ctx.settings.backup_transcripts))?))
}

pub fn backup_delete(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::backup::delete(ctx, &s(a, "name")?)?; Ok(json!(true)) }

pub fn backup_rename(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::backup::rename(ctx, &s(a, "name")?, &s(a, "label")?)?)) }

pub fn backup_export(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::backup::export(ctx, std::path::Path::new(&s(a, "path")?))?; Ok(json!(true)) }

// a backup file from elsewhere is kept with the others, then put back
fn backup_restore_or_backup_import(state: &AppState, ctx: &Ctx, a: &Value, op: &str) -> Result<Value> {
    let r = if op == "backup_import" {
        let from = PathBuf::from(s(a, "path")?);
        let b = hrm_core::backup::create(ctx, "Imported")?;
        std::fs::copy(&from, hrm_core::backup::dir(ctx).join(&b.name)).with_context(|| format!("read {}", from.display()))?;
        hrm_core::backup::restore(ctx, &b.name)?
    } else {
        hrm_core::backup::restore(ctx, &s(a, "name")?)?
    };
    // its settings take effect now, and the tools follow the restored list
    let fresh = Arc::new(Ctx::open_at(&ctx.data, &ctx.home)?);
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh.clone();
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let synced = serde_json::to_value(items::sync(&fresh, &fresh.registry()?)?)?;
    Ok(json!({"restored": r, "sync": synced}))
}
pub fn backup_restore(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { backup_restore_or_backup_import(state, ctx, a, "backup_restore") }
pub fn backup_import(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { backup_restore_or_backup_import(state, ctx, a, "backup_import") }

pub fn log_open(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    let f = hrm_core::log::file(&ctx.data);
    if !f.exists() { std::fs::create_dir_all(f.parent().unwrap())?; }
    open_path(&f)?;
    Ok(json!(true))
}

// phase 4: restore as a merge (a backup zip or a sync folder), Welcome back, and the archive
pub fn restore_preview(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::restore::preview(ctx, &hrm_core::restore::Source::parse(a)?) }

pub fn restore_run(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let r = hrm_core::restore::run(ctx, &hrm_core::restore::Source::parse(a)?, a["adopt"] == true)?;
    hrm_core::restore::welcome_done(ctx)?;
    // with adopt the settings changed: the identity follows now
    let fresh = Arc::new(Ctx::open_at(&ctx.data, &ctx.home)?);
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh;
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(r)
}

pub fn restore_welcome(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    Ok(if a["done"] == true { hrm_core::restore::welcome_done(ctx)?; json!(true) } else { hrm_core::restore::welcome(ctx)? })
}

pub fn archive_status(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::archive::status(ctx) }

pub fn archive_now(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::archive::archive_now(ctx) }

// a record written by the version before an update offers the version now running: not an offer
pub fn self_latest(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    Ok(ctx.db.kv_get("self:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).filter(|v| v["current"] == env!("CARGO_PKG_VERSION")).unwrap_or(Value::Null))
}

pub fn open_folder(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let p = s(a, "path")?;
    if !std::path::Path::new(&p).is_dir() { bail!("{p} is not a folder"); }
    open_path(std::path::Path::new(&p))?;
    Ok(json!(true))
}

pub fn open_url(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let url = s(a, "url")?;
    if !url.starts_with("https://") { bail!("only https links open"); }
    // explorer opens a folder window for a link it does not take as one (the long sign-in link); the URL handler hands any to the browser
    #[cfg(windows)]
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn()?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&url).spawn()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    std::process::Command::new("xdg-open").arg(&url).spawn()?;
    Ok(json!(true))
}
