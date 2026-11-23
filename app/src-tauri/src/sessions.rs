//! Sessions and Projects: the lists, one transcript, resuming, tags, project names and pins.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use hrm_core::service::{self, SessionFilter};
use hrm_core::Ctx;
use serde_json::{json, Value};

use crate::{opt, s, AppState};

pub fn sessions(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut f: SessionFilter = serde_json::from_value(a["filter"].clone()).unwrap_or_default();
    f.scope = f.scope.or_else(|| opt(a, "scope"));
    Ok(json!({"groups": service::grouped_sessions(ctx, &f, a["group"].as_str().unwrap_or("project"))?, "total": service::sessions_total(ctx, &f)?}))
}

// with `device` another computer's: its fetched transcript (phase 3)
pub fn transcript(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    Ok(match opt(a, "device").filter(|d| d != &ctx.db.device) {
        Some(d) => json!(hrm_core::transcripts::remote_transcript(ctx, &d, &s(a, "profile")?, &s(a, "id")?)?),
        None => json!(service::transcript(ctx, &s(a, "profile")?, &s(a, "id")?)?),
    })
}

// size and time only: an open session's page polls it and re-reads the transcript when it changes
pub fn file_stamp(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    let m = std::fs::metadata(s(a, "path")?)?;
    Ok(json!([m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64)]))
}

pub fn resume(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let (profile, id) = (s(a, "profile")?, s(a, "id")?);
    // the folder is the one the session's row names, not one typed: gone, it is left out, where `hrm resume --cwd` is refused
    let cwd = opt(a, "cwd").map(PathBuf::from).filter(|p| p.is_dir());
    service::resume(ctx, &profile, &id, cwd)?;
    Ok(json!(true))
}

pub fn export_markdown(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let t = service::transcript(ctx, &s(a, "profile")?, &s(a, "id")?)?;
    let mut out = format!("# {}\n\n{} · {} · {}\n\n", t.meta.title, t.meta.harness, t.meta.profile, t.meta.cwd);
    for turn in &t.turns {
        if !turn.prompt.is_empty() { out.push_str(&format!("## You\n\n{}\n\n", turn.prompt)); }
        for st in turn.steps.iter().filter(|x| x.kind == "tool") {
            out.push_str(&format!("- `{}` {} {}\n", st.tool, st.input, if st.status == "error" { "(failed)" } else { "" }));
        }
        for a in &turn.answer { out.push_str(&format!("\n{a}\n\n")); }
    }
    // whole or not at all: a reader (or a test) never sees it half written
    hrm_core::util::write_atomic(std::path::Path::new(&s(a, "path")?), out.as_bytes())?;
    Ok(json!(true))
}

pub fn show_window(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> {
    if !cfg!(windows) { bail!("showing a console window is not available on this system; switch to its terminal"); }
    Ok(json!(focus_console(a["hwnd"].as_i64().unwrap_or(0), a["pid"].as_u64().context("pid")? as u32)))
}

/// Best effort: the console window `hrm run` recorded (`hwnd`), or the window that owns it (Windows Terminal owns
/// the hidden one its tabs have; the window comes forward, not the tab); else the window of process `pid`.
pub fn focus_console(hwnd: i64, pid: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow, GA_ROOTOWNER, SW_RESTORE};
        let h = hwnd as windows_sys::Win32::Foundation::HWND;
        if !h.is_null() && IsWindow(h) != 0 {
            let top = GetAncestor(h, GA_ROOTOWNER);
            let top = if top.is_null() { h } else { top };
            if IsWindowVisible(top) != 0 {
                if IsIconic(top) != 0 { ShowWindow(top, SW_RESTORE); }
                if SetForegroundWindow(top) != 0 { return true; }
            }
        }
    }
    let _ = hwnd;
    std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("(New-Object -ComObject WScript.Shell).AppActivate({pid})")])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "True").unwrap_or(false)
}

/// The sessions hrm run started, as their hooks last reported (see `hrm_core::hooks`).
pub fn needs(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(json!(hrm_core::hooks::runs(ctx)?)) }

/// The answer to a request a session's hook holds (`{id, behavior}` or `{id, answers}`); false: it was no longer
/// open (answered in the terminal, or expired), and the answer was dropped.
pub fn answer(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::hooks::answer(ctx, &s(a, "id")?, a)?)) }

pub fn session_stats(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { hrm_core::dash::session_stats(ctx, &s(a, "profile")?, &s(a, "id")?) }

// resume here (phase 3): a session from another computer as a fork in this computer's clone
pub fn session_resume_here(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    hrm_core::transcripts::resume_here(ctx, &s(a, "device")?, &s(a, "harness")?, &s(a, "profile")?, &s(a, "id")?, opt(a, "cwd").map(PathBuf::from).as_deref())
}

pub fn transcripts_status(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::transcripts::status(ctx) }

// tags: free text on sessions ("session", key profile/id) and projects ("project", the project key)
pub fn tag_set(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let tags: Vec<String> = a["tags"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
    service::tag_set(ctx, &s(a, "kind")?, &s(a, "key")?, &tags)?;
    Ok(json!(true))
}

pub fn tag_list(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    Ok(json!(ctx.db.tag_list()?.into_iter().map(|(t, s, p)| json!({"tag": t, "sessions": s, "projects": p})).collect::<Vec<_>>()))
}

pub fn projects(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::projects(ctx, a["days"].as_i64(), a["scope"].as_str())?)) }

pub fn project_alias(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::alias_project(ctx, &s(a, "key")?, opt(a, "name"), opt(a, "merge_into"))?; Ok(json!(true)) }

// pin or unpin a project in the sidebar
pub fn project_pin(state: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let key = s(a, "key")?;
    let mut fresh = Ctx::open_at(&ctx.data, &ctx.home)?;
    fresh.settings.pinned_projects.retain(|k| k != &key);
    if a["on"] == true { fresh.settings.pinned_projects.push(key); }
    fresh.save_settings()?;
    *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(fresh);
    *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(json!(true))
}
