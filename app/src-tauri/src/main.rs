//! Harness Manager desktop app: the window is a view over hrm-core. Every action goes through one
//! command, `call(op, args)`, which runs the same core functions the `hrm` CLI uses.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use hrm_core::registry::{Kind, McpServer, Provider, Route};
use hrm_core::service::{self, AddRequest, SessionFilter};
use hrm_core::{harness, items, Ctx};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

struct AppState {
    ctx: Mutex<Arc<Ctx>>,
    /// The pages' data, kept: see `cached`.
    pages: Mutex<HashMap<String, Page>>,
    /// Moves on with every change made through the window: an answer computed across one is never kept.
    epoch: AtomicU64,
    first_run: Mutex<Option<Value>>,
    settings_mtime: Mutex<Option<std::time::SystemTime>>,
    /// The direct connection's listener, mDNS and pairing; set once the window exists to tell.
    peers: std::sync::OnceLock<Arc<hrm_core::sync::peer::Runtime>>,
}

impl AppState {
    fn peers(&self) -> Result<&Arc<hrm_core::sync::peer::Runtime>> { self.peers.get().context("the direct connection is not up yet") }

    /// Settings were saved by a core function: take them on the next call.
    fn refresh(&self) { *self.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None; }

    /// The context, reopened when settings.json changed on disk (for example through `hrm settings`).
    fn ctx(&self) -> Arc<Ctx> {
        let mut cur = self.ctx.lock().unwrap_or_else(|e| e.into_inner());
        let m = std::fs::metadata(cur.data.join("settings.json")).and_then(|m| m.modified()).ok();
        let mut seen = self.settings_mtime.lock().unwrap_or_else(|e| e.into_inner());
        if m != *seen {
            if let Ok(fresh) = Ctx::open_at(&cur.data, &cur.home) { *cur = Arc::new(fresh); }
            *seen = m;
        }
        cur.clone()
    }
}

/// One page's data as last computed, and whether a fresher one is being computed now.
struct Page { value: Value, at: Instant, stamp: Stamp, busy: bool }

/// What the pages are computed from, as of now: the database (any commit, by anyone) and the account list and
/// settings files. None when it cannot be told: nothing is kept then.
type Stamp = Option<(i64, Option<std::time::SystemTime>, Option<std::time::SystemTime>)>;
fn stamp(st: &AppState) -> Stamp {
    let ctx = st.ctx();
    let mtime = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    Some((ctx.db.data_version()?, mtime(ctx.registry_path()), mtime(ctx.data.join("settings.json"))))
}

/// The calls a page opens with: answered from what was kept, at once, while no new data has arrived since; one
/// older than `PAGE_FRESH` is also redone in the background for the next call (the clock-driven parts: open
/// consoles, limits). After new data the next call computes it, so the redraw that new data brings shows it.
const PAGE_CALLS: &[&str] = &["state", "home", "today", "projects"];
const PAGE_FRESH: Duration = Duration::from_secs(10);
/// Calls that change nothing the pages show. Any other call (a change, a login, a refresh) drops what was kept,
/// so the page drawn after it is computed fresh.
const NO_CHANGE: &[&str] = &["state", "home", "today", "projects", "dash", "live", "requests", "usage", "sessions", "detail", "items",
    "self_latest", "sync_last", "mcp_last_import", "computers_status", "transcripts_status", "archive_status", "hub_status", "account_stats",
    "session_stats", "limits_extra", "routing", "backups", "tag_list", "transcript", "file_stamp", "skill_check", "discover", "prompt_live",
    "open_folder", "open_url", "show_window", "export_markdown"];

fn page_key(op: &str, args: &Value) -> String { format!("{op} {args}") }

/// A page call through the kept answers (see `PAGE_CALLS`).
fn cached(st: &Arc<AppState>, op: &str, args: &Value) -> Result<Value> {
    // the first-run report is handed over once: never kept, never computed in the background
    if op == "state" && st.first_run.lock().unwrap_or_else(|e| e.into_inner()).is_some() { return dispatch(st, op, args); }
    let key = page_key(op, args);
    let now = stamp(st);
    if now.is_none() { return dispatch(st, op, args); }
    let kept = {
        let mut pages = st.pages.lock().unwrap_or_else(|e| e.into_inner());
        match pages.get_mut(&key) {
            // anything changed since it was kept: computed now, so the redraw that follows new data shows it
            Some(p) if p.stamp != now => None,
            Some(p) => {
                let redo = p.at.elapsed() > PAGE_FRESH && !p.busy;
                if redo { p.busy = true; }
                Some((p.value.clone(), redo))
            }
            None => None,
        }
    };
    match kept {
        Some((v, redo)) => {
            if redo { let (st, op, args) = (st.clone(), op.to_string(), args.clone()); std::thread::spawn(move || { let _ = compute_page(&st, &op, &args); }); }
            Ok(v)
        }
        None => compute_page(st, op, args),
    }
}

/// Compute a page call and keep it, with the data generation it started from.
fn compute_page(st: &Arc<AppState>, op: &str, args: &Value) -> Result<Value> {
    let (stamp, epoch) = (stamp(st), st.epoch.load(Ordering::SeqCst));
    let r = dispatch(st, op, args);
    let mut pages = st.pages.lock().unwrap_or_else(|e| e.into_inner());
    match &r {
        Ok(_) if st.epoch.load(Ordering::SeqCst) != epoch => {}
        Ok(v) => {
            let first = !pages.contains_key(&page_key(op, args));
            pages.insert(page_key(op, args), Page { value: v.clone(), at: Instant::now(), stamp, busy: false });
            drop(pages);
            // a write: it must not hold the answer behind whatever is writing now
            if first { let (st, op, args) = (st.clone(), op.to_string(), args.clone()); std::thread::spawn(move || remember_page_call(&st, &op, &args)); }
        }
        Err(_) => { if let Some(p) = pages.get_mut(&page_key(op, args)) { p.busy = false; } }
    }
    r
}

/// The page calls this window makes, kept between launches (the last twelve), so they are ready at the next start.
fn remember_page_call(st: &AppState, op: &str, args: &Value) {
    // one at a time: two pages opened together must not each write back a list without the other
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = st.ctx();
    let mut list: Vec<Value> = ctx.db.kv_get("pages:calls").ok().flatten().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    let item = json!([op, args]);
    // shortcut: a listed call keeps its place, so the oldest listed goes first, not the least used; a commit here would mark every kept page stale at each launch
    if list.contains(&item) { return; }
    list.insert(0, item);
    list.truncate(12);
    let _ = ctx.db.kv_set("pages:calls", &Value::Array(list).to_string());
}

/// At launch: the page calls of the last session computed ahead, so the first page opens with its data ready.
fn prefetch_pages(st: &Arc<AppState>) {
    let list: Vec<Value> = st.ctx().db.kv_get("pages:calls").ok().flatten().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    for c in list {
        if let (Some(op), Some(args)) = (c[0].as_str(), c.get(1)) {
            if PAGE_CALLS.contains(&op) { let _ = cached(st, op, args); }
        }
    }
}


fn s(args: &Value, k: &str) -> Result<String> {
    args[k].as_str().map(String::from).with_context(|| format!("missing {k}"))
}
fn opt(args: &Value, k: &str) -> Option<String> { args[k].as_str().map(String::from).filter(|s| !s.is_empty()) }

fn sync_items(ctx: &Ctx) -> Result<Value> {
    let reg = ctx.registry()?;
    Ok(serde_json::to_value(items::sync(ctx, &reg)?)?)
}

/// The MCP servers every tool already has, into the list and written back in step; the report of which files
/// were read is kept for the MCP page.
fn import_mcp_now(ctx: &Ctx) -> Result<Vec<Value>> {
    let mut reg = ctx.registry()?;
    let report = items::import_mcp_all(ctx, &mut reg);
    ctx.save_registry(&reg)?;
    sync_items(ctx)?;
    ctx.db.kv_set("mcp:last_import", &json!({"at": hrm_core::util::now_ms(), "report": report}).to_string())?;
    Ok(report)
}

/// The sync report, or `{"error": ...}`: after a change is saved, a link that fails must not undo the answer.
fn sync_soft(ctx: &Ctx) -> Value {
    sync_items(ctx).unwrap_or_else(|e| json!({"error": format!("{e:#}")}))
}

/// Operations that never change the registry; they can be slow (network, terminals), so they run without its lock.
const READS: &[&str] = &["state", "detail", "start", "open_folder", "open_url", "export_markdown", "show_window", "login_start", "login_submit",
    "login_poll", "login_cancel", "login_terminal", "refresh", "home", "dash", "live", "requests", "today", "usage", "sessions", "projects", "tag_set", "tag_list", "tag_rename", "transcript", "file_stamp", "resume",
    "items", "skill_check", "discover", "apps", "apps_latest", "app_update", "app_install", "app_remove_copy", "autostart_set", "backups", "backup_export", "log_open", "prompt_live", "routing", "router_start", "router_stop", "live_quota_set",
    "limits_extra", "account_stats", "session_stats", "sync_last", "self_latest", "mcp_last_import", "usage_csv", "computers_status", "computers_now",
    "computers_pair_start", "computers_pair_cancel", "computers_pair_join", "computers_forget", "computers_set_peer",
    "session_resume_here", "session_fetch", "transcripts_status",
    "hub_join", "hub_leave", "hub_invite", "hub_status", "ring_key_show", "ring_key_enter",
    "restore_preview", "restore_welcome", "archive_status", "archive_now"];

fn dispatch(state: &AppState, op: &str, a: &Value) -> Result<Value> {
    let ctx = state.ctx();
    let ctx = ctx.as_ref();
    // every other operation loads, changes and saves the registry: one at a time across threads and processes
    let _reg_lock = (!READS.contains(&op)).then(|| ctx.lock_registry());
    Ok(match op {
        "state" => {
            let reg = ctx.registry()?;
            json!({
                "profiles": service::list_views(ctx)?,
                "harnesses": service::harnesses(),
                "settings": ctx.settings,
                "live_quota": ctx.db.kv_get("live_quota")?.as_deref() == Some("1"),
                "autostart": hrm_core::launch::autostart::get().is_some(),
                // windows, macos or linux: the window leaves out what only Windows has
                "os": std::env::consts::OS,
                // tools with a newer version, from the last check (at launch or on the Apps page)
                "app_updates": ctx.db.kv_get("apps:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).and_then(|v| v["updates"].as_u64()).unwrap_or(0),
                "router": {"running": service::router_running(ctx), "port": ctx.settings.router_port},
                "routes": reg.routes,
                "registry_path": ctx.registry_path(),
                "computer": std::env::var("COMPUTERNAME").unwrap_or_default(),
                "version": env!("CARGO_PKG_VERSION"),
                "first_run": state.first_run.lock().unwrap_or_else(|e| e.into_inner()).take(),
                "now": hrm_core::util::now_ms(),
            })
        }
        "detail" => {
            let name = s(a, "name")?;
            let reg = ctx.registry()?;
            let p = reg.get(&name)?;
            let f = SessionFilter { profile: Some(name.clone()), days: Some(30), per: Some(40), ..Default::default() };
            let sessions = service::sessions(ctx, &f)?;
            json!({
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
            })
        }
        "add" => {
            let req: AddRequest = serde_json::from_value(a.clone())?;
            json!(service::add_profile(ctx, req)?)
        }
        "remove" => { service::remove_profile(ctx, &s(a, "name")?, a["delete_home"] == true)?; json!(true) }
        "update" => { service::update_profile(ctx, &s(a, "name")?, opt(a, "email"), a["base_url"].as_str().map(String::from), a["model"].as_str().map(String::from))?; json!(true) }
        // the refresh button of one account: its limits from Anthropic now, with the login Claude Code saved
        "quota_live" => {
            let reg = ctx.registry()?;
            let p = reg.get(&s(a, "name")?)?;
            if p.harness != "claude" || p.kind != hrm_core::registry::Kind::Subscription { bail!("only Claude subscriptions have limits to read live"); }
            json!(harness::claude::Claude.live_quota(ctx, p)?)
        }
        "set_cap" => { service::set_daily_cap(ctx, &s(a, "name")?, a["cap"].as_f64())?; json!(true) }
        "set_label" => { service::set_label(ctx, &s(a, "name")?, a["label"].as_str().unwrap_or(""))?; json!(true) }
        "set_key" => { service::set_api_key(ctx, &s(a, "name")?, &s(a, "key")?)?; json!(true) }
        "start" => {
            let extra: Vec<String> = a["args"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            service::start(ctx, &s(a, "name")?, opt(a, "cwd").map(PathBuf::from), &extra)?;
            json!(true)
        }
        "open_folder" => {
            let p = s(a, "path")?;
            if !std::path::Path::new(&p).is_dir() { bail!("{p} is not a folder"); }
            std::process::Command::new("explorer").arg(&p).spawn()?;
            json!(true)
        }
        "open_url" => {
            let url = s(a, "url")?;
            if !url.starts_with("https://") { bail!("only https links open"); }
            std::process::Command::new("explorer").arg(&url).spawn()?;
            json!(true)
        }
        "export_markdown" => {
            let t = service::transcript(ctx, &s(a, "profile")?, &s(a, "id")?)?;
            let mut out = format!("# {}\n\n{} · {} · {}\n\n", t.meta.title, t.meta.harness, t.meta.profile, t.meta.cwd);
            for turn in &t.turns {
                if !turn.prompt.is_empty() { out.push_str(&format!("## You\n\n{}\n\n", turn.prompt)); }
                for st in turn.steps.iter().filter(|x| x.kind == "tool") {
                    out.push_str(&format!("- `{}` {} {}\n", st.tool, st.input, if st.status == "error" { "(failed)" } else { "" }));
                }
                for a in &turn.answer { out.push_str(&format!("\n{a}\n\n")); }
            }
            std::fs::write(s(a, "path")?, out)?;
            json!(true)
        }
        "show_window" => {
            // best effort: activate the console window that runs this process
            let pid = a["pid"].as_u64().context("pid")?;
            let ok = std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("(New-Object -ComObject WScript.Shell).AppActivate({pid})")])
                .output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "True").unwrap_or(false);
            json!(ok)
        }
        "login_start" => json!(service::login_start(ctx, &s(a, "name")?)?),
        "login_submit" => { service::login_submit(&s(a, "id")?, &s(a, "code")?)?; json!(true) }
        "login_poll" => json!(service::login_poll(ctx, &s(a, "id")?)?),
        "login_cancel" => { service::login_cancel(&s(a, "id")?); json!(true) }
        "login_terminal" => {
            let name = s(a, "name")?;
            hrm_core::launch::open_terminal(&ctx.settings.terminal, &format!("Sign in · {name}"), &["login".into(), name], None)?;
            json!(true)
        }
        "refresh" => {
            let n = service::import_usage(ctx)?;
            let errors = service::refresh_quotas(ctx, a["live"] == true);
            json!({"new_rows": n, "quota_errors": errors})
        }
        // scope "all" (every computer, the default) or "here" (this one)
        "home" => {
            let days = a["days"].as_i64();
            let h = a["harness"].as_str().filter(|s| !s.is_empty());
            let scope = a["scope"].as_str();
            let since = a["hours"].as_i64().map(hrm_core::util::since_hours);
            let (total, rows) = service::request_log_since(ctx, days, since, h, scope, a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
            json!({"overview": service::overview(ctx, days, h, scope)?, "requests": {"total": total, "rows": rows},
                   "dash": hrm_core::dash::dashboard(ctx, a["hours"].as_i64().unwrap_or(24), h, scope)?, "activity": service::activity(ctx, h, scope)?})
        }
        // the sidebar's readouts: tokens, cost and the cache share for the range, every tool
        "dash" => hrm_core::dash::dashboard(ctx, a["hours"].as_i64().unwrap_or(0), None, a["scope"].as_str())?,
        // the last five minutes of traffic, here and on the other computers
        "live" => { let now = hrm_core::util::now_ms(); json!({"now": now, "live": hrm_core::dash::live(ctx, now)?}) }
        "requests" => {
            let since = a["hours"].as_i64().map(hrm_core::util::since_hours);
            let (total, rows) = service::request_log_since(ctx, a["days"].as_i64(), since, a["harness"].as_str().filter(|s| !s.is_empty()), a["scope"].as_str(), a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
            json!({"total": total, "rows": rows})
        }
        "today" => json!(service::overview(ctx, Some(1), None, None)?.today_cost),
        "usage" => {
            let days = a["days"].as_i64().unwrap_or(7);
            json!({"summary": service::usage_summary(ctx, days, a["profile"].as_str())?, "route_log": ctx.db.route_log(30)?,
                   "accounts": service::breakdown(ctx, "profile", Some(days), None)?, "models": service::breakdown_of(ctx, "model", Some(days), None, a["profile"].as_str())?})
        }
        // Usage > Export CSV: one line per account, to the file the user picked
        "usage_csv" => {
            let days = a["days"].as_i64().unwrap_or(7);
            let mut out = String::from("account,tool,tokens,input,output,cache_read,requests,list_price_usd\n");
            for b in service::breakdown(ctx, "profile", Some(days), None)? {
                out.push_str(&format!("{},{},{},{},{},{},{},{}\n", b.key, b.harness, b.tokens, b.input, b.output, b.cache_read, b.requests, b.cost.map(|c| format!("{c:.4}")).unwrap_or_default()));
            }
            std::fs::write(s(a, "path")?, out)?;
            json!(true)
        }
        "sessions" => {
            let mut f: SessionFilter = serde_json::from_value(a["filter"].clone()).unwrap_or_default();
            f.scope = f.scope.or_else(|| opt(a, "scope"));
            json!({"groups": service::grouped_sessions(ctx, &f, a["group"].as_str().unwrap_or("project"))?, "total": service::sessions_total(ctx, &f)?})
        }
        // with `device` another computer's: its fetched transcript (phase 3)
        "transcript" => match opt(a, "device").filter(|d| d != &ctx.db.device) {
            Some(d) => json!(hrm_core::transcripts::remote_transcript(ctx, &d, &s(a, "profile")?, &s(a, "id")?)?),
            None => json!(service::transcript(ctx, &s(a, "profile")?, &s(a, "id")?)?),
        },
        // size and time only: an open session's page polls it and re-reads the transcript when it changes
        "file_stamp" => {
            let m = std::fs::metadata(s(a, "path")?)?;
            json!([m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64)])
        }
        "resume" => {
            let (profile, id) = (s(a, "profile")?, s(a, "id")?);
            let cwd = opt(a, "cwd").map(PathBuf::from).filter(|p| p.is_dir());
            service::resume(ctx, &profile, &id, cwd)?;
            json!(true)
        }
        "project_alias" => { service::alias_project(ctx, &s(a, "key")?, opt(a, "name"), opt(a, "merge_into"))?; json!(true) }
        "projects" => json!(service::projects(ctx, a["days"].as_i64(), a["scope"].as_str())?),
        // across computers: the shared folder, this computer and the others
        "computers_status" => hrm_core::journal::status(ctx)?,
        "computers_now" => hrm_core::journal::round(ctx)?,
        "computers_set" => {
            let mut fresh = Arc::new(hrm_core::journal::configure(&ctx.data, &ctx.home, a)?);
            // the listener follows the setting at once (and may pick and save its port)
            if let Some(p) = state.peers.get() { fresh = p.ensure(&fresh)?; }
            *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh.clone();
            *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if hrm_core::journal::on(&fresh) { hrm_core::journal::round(&fresh)? } else { hrm_core::journal::status(&fresh)? }
        }
        // the direct connection: pairing with a six-digit code, the computers paired
        "computers_pair_start" => { let v = state.peers()?.pair_start(&state.ctx(), opt(a, "host").as_deref())?; state.refresh(); v }
        "computers_pair_cancel" => state.peers()?.pair_cancel(ctx)?,
        "computers_pair_join" => { let v = state.peers()?.pair_join(&state.ctx(), &s(a, "code")?, opt(a, "host").as_deref())?; state.refresh(); v }
        "computers_forget" => hrm_core::sync::peer::forget(ctx, &s(a, "id")?)?,
        "computers_set_peer" => hrm_core::sync::peer::set_peer(ctx, &s(a, "id")?, a["address"].as_str(), a["name"].as_str())?,
        // tags: free text on sessions ("session", key profile/id) and projects ("project", the project key)
        "tag_set" => {
            let tags: Vec<String> = a["tags"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            ctx.db.tag_set(&s(a, "kind")?, &s(a, "key")?, &tags)?;
            json!(true)
        }
        "tag_list" => json!(ctx.db.tag_list()?.into_iter().map(|(t, s, p)| json!({"tag": t, "sessions": s, "projects": p})).collect::<Vec<_>>()),
        "tag_rename" => { ctx.db.tag_rename(&s(a, "from")?, a["to"].as_str().unwrap_or("").trim())?; json!(true) }
        // pin or unpin a project in the sidebar
        "project_pin" => {
            let key = s(a, "key")?;
            let mut fresh = Ctx::open_at(&ctx.data, &ctx.home)?;
            fresh.settings.pinned_projects.retain(|k| k != &key);
            if a["on"] == true { fresh.settings.pinned_projects.push(key); }
            fresh.save_settings()?;
            *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(fresh);
            *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
            json!(true)
        }

        "items" => {
            let reg = ctx.registry()?;
            // each installed skill's description, from its SKILL.md in the store
            let store = items::skill_store(ctx);
            let info: serde_json::Map<String, Value> = reg.skills.iter().map(|s| {
                let (_, d) = items::skill_front(&std::fs::read_to_string(store.join(&s.name).join("SKILL.md")).unwrap_or_default());
                (s.name.clone(), json!({"description": d}))
            }).collect();
            let unmanaged: Vec<Value> = items::unmanaged_skills(ctx, &reg).into_iter().map(|(n, linked)| {
                let (_, d) = items::skill_front(&std::fs::read_to_string(store.join(&n).join("SKILL.md")).unwrap_or_default());
                json!({"name": n, "description": d, "linked": linked})
            }).collect();
            json!({"mcp": reg.mcp, "skills": reg.skills, "skill_info": info, "unmanaged": unmanaged, "skill_repos": items::skill_repos(&reg), "prompts": reg.prompts, "targets": reg.targets, "skill_store": store,
                   "profiles": reg.profiles.iter().map(|p| json!({"name": p.name, "harness": p.harness})).collect::<Vec<_>>()})
        }
        "mcp_add" => {
            let mut reg = ctx.registry()?;
            let server: McpServer = serde_json::from_value(a["server"].clone())?;
            items::add_mcp(&mut reg, server)?;
            ctx.save_registry(&reg)?;
            sync_items(ctx)?
        }
        "item_remove" => {
            let mut reg = ctx.registry()?;
            let (kind, name) = (s(a, "kind")?, s(a, "name")?);
            items::remove_item(&mut reg, &kind, &name)?;
            ctx.save_registry(&reg)?;
            let rep = sync_items(ctx)?;
            if kind == "skill" { items::delete_skill_files(ctx, &name)?; }
            rep
        }
        "target" => {
            let mut reg = ctx.registry()?;
            items::set_target(&mut reg, &s(a, "kind")?, &s(a, "item")?, &s(a, "harness")?, opt(a, "profile").as_deref(), a["on"] == true)?;
            ctx.save_registry(&reg)?;
            sync_items(ctx)?
        }
        // a column's header tick: every listed item on or off for one tool, written once
        "target_bulk" => {
            let mut reg = ctx.registry()?;
            let (kind, h) = (s(a, "kind")?, s(a, "harness")?);
            for it in a["items"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                items::set_target(&mut reg, &kind, it, &h, None, a["on"] == true)?;
            }
            ctx.save_registry(&reg)?;
            sync_items(ctx)?
        }
        // add (refused when the name exists) or edit a server, and set the tools it is written to
        "mcp_save" => {
            let mut reg = ctx.registry()?;
            let server: McpServer = serde_json::from_value(a["server"].clone())?;
            if a["replace"] != true && reg.mcp.iter().any(|m| m.name == server.name) { bail!("a server named '{}' already exists", server.name); }
            let name = server.name.clone();
            items::add_mcp(&mut reg, server)?;
            if let Some(on) = a["targets"].as_array() {
                let on: Vec<&str> = on.iter().filter_map(|v| v.as_str()).collect();
                for h in service::harnesses().iter().filter(|h| h.caps.mcp) {
                    items::set_target(&mut reg, "mcp", &name, &h.id, None, on.contains(&h.id.as_str()))?;
                }
            }
            ctx.save_registry(&reg)?;
            sync_items(ctx)?
        }
        "mcp_import_all" => json!(import_mcp_now(ctx)?),
        // what Accounts adds to each row: limit forecasts and daily caps
        "session_stats" => hrm_core::dash::session_stats(ctx, &s(a, "profile")?, &s(a, "id")?)?,
        "account_stats" => hrm_core::dash::account_stats(ctx, &s(a, "name")?)?,
        "limits_extra" => json!({"forecast": hrm_core::dash::forecasts(ctx, hrm_core::util::now_ms())?, "caps": hrm_core::dash::caps(ctx, &ctx.registry()?)}),
        // a record written by the version before an update offers the version now running: not an offer
        "self_latest" => ctx.db.kv_get("self:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).filter(|v| v["current"] == env!("CARGO_PKG_VERSION")).unwrap_or(Value::Null),
        "sync_last" => ctx.db.kv_get("sync:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null),
        "mcp_last_import" => ctx.db.kv_get("mcp:last_import")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null),
        "mcp_import" => {
            let mut reg = ctx.registry()?;
            let (added, kept) = items::import_mcp(ctx, &mut reg, &s(a, "harness")?)?;
            ctx.save_registry(&reg)?;
            json!({"added": added, "kept": kept})
        }
        "skill_install" => {
            let mut reg = ctx.registry()?;
            let sk = items::install_skill(ctx, &mut reg, &s(a, "source")?, opt(a, "name").as_deref())?;
            for h in a["enable"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                items::set_target(&mut reg, "skill", &sk.name, h, None, true)?;
            }
            ctx.save_registry(&reg)?;
            let mut v = serde_json::to_value(&sk)?;
            v["sync"] = sync_soft(ctx);
            v
        }
        // the skills in each Discover repository; a day's cache unless asked to refresh
        "discover" => {
            let reg = ctx.registry()?;
            let mut repos = items::Repos::new(ctx.data.join("discover"));
            let installed: std::collections::HashSet<&str> = reg.skills.iter().map(|s| s.name.as_str()).collect();
            let mut out = vec![];
            for r in items::skill_repos(&reg) {
                let key = format!("discover:{}/{}@{}", r.owner, r.name, r.branch);
                let cached: Option<Value> = ctx.db.kv_get(&key)?.and_then(|v| serde_json::from_str(&v).ok())
                    .filter(|v: &Value| a["refresh"] != true && hrm_core::util::now_ms() - v["at"].as_i64().unwrap_or(0) < 86_400_000);
                let got = match cached {
                    Some(v) => v,
                    None => {
                        let v = match items::repo_skills(&mut repos, &r) {
                            Ok(list) => json!({"at": hrm_core::util::now_ms(), "skills": list}),
                            Err(e) => json!({"at": 0, "error": format!("{e:#}")}),
                        };
                        if v.get("error").is_none() { ctx.db.kv_set(&key, &v.to_string())?; }
                        v
                    }
                };
                let skills: Vec<Value> = got["skills"].as_array().cloned().unwrap_or_default().into_iter()
                    .map(|mut s| { let i = installed.contains(s["folder"].as_str().unwrap_or("")); s["installed"] = json!(i); s }).collect();
                out.push(json!({"repo": format!("{}/{}", r.owner, r.name), "owner": r.owner, "name": r.name, "branch": r.branch, "skills": skills, "error": got.get("error")}));
            }
            json!(out)
        }
        "skill_repo_add" => {
            let mut reg = ctx.registry()?;
            let raw = s(a, "repo")?;
            let rest = raw.trim().trim_start_matches("https://github.com/").trim_end_matches('/').trim_end_matches(".git").to_string();
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() { bail!("enter owner/repository or a GitHub link"); }
            let branch = if parts.len() >= 4 && parts[2] == "tree" { parts[3].to_string() } else { "HEAD".into() };
            let r = hrm_core::registry::SkillRepo { owner: parts[0].into(), name: parts[1].into(), branch };
            // the defaults become the user's list the first time it is changed
            reg.skill_repos = items::skill_repos(&reg);
            if !reg.skill_repos.iter().any(|x| x.owner.eq_ignore_ascii_case(&r.owner) && x.name.eq_ignore_ascii_case(&r.name)) { reg.skill_repos.push(r); }
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "skill_repo_remove" => {
            let mut reg = ctx.registry()?;
            reg.skill_repos = items::skill_repos(&reg);
            let (o, n) = (s(a, "owner")?, s(a, "name")?);
            reg.skill_repos.retain(|x| !(x.owner == o && x.name == n));
            if reg.skill_repos.is_empty() { bail!("keep at least one repository"); }
            ctx.save_registry(&reg)?;
            json!(true)
        }
        // install again from where it came from; its ticks stay
        "skill_update" => {
            let mut reg = ctx.registry()?;
            let name = s(a, "name")?;
            let src = reg.skills.iter().find(|x| x.name == name).map(|x| x.source.clone()).context("no such skill")?;
            if !src.starts_with("https://github.com/") && !std::path::Path::new(&src).is_dir() { bail!("{name} has no source to update from ({src})"); }
            let sk = items::install_skill(ctx, &mut reg, &src, Some(&name))?;
            ctx.save_registry(&reg)?;
            json!(sk)
        }
        // the tools themselves: installed where, which version, the newest one (last checked), install or update
        "apps" => {
            let apps = hrm_core::apps::list();
            let mut latest = ctx.db.kv_get("apps:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok());
            let updates = hrm_core::apps::recount(ctx, &apps, &mut latest)?;
            json!({"apps": apps, "latest": latest, "updates": updates})
        }
        "apps_latest" => hrm_core::apps::check(ctx)?,
        "autostart_set" => {
            let exe = std::env::current_exe()?;
            hrm_core::launch::autostart::set(a["on"] == true, &exe)?;
            json!(hrm_core::launch::autostart::get().is_some())
        }
        "app_update" => json!(hrm_core::apps::update(&s(a, "id")?)?),
        "app_install" => json!(hrm_core::apps::install(&s(a, "id")?, &s(a, "method")?)?),
        "app_remove_copy" => json!(hrm_core::apps::remove_copy(&s(a, "id")?, &s(a, "path")?)?),
        // skill folders in the store that the list does not have: added, with the ticks their links already show
        "skill_adopt" => {
            let mut reg = ctx.registry()?;
            let names: Vec<String> = a["names"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            let done = items::adopt_skills(ctx, &mut reg, &names)?;
            ctx.save_registry(&reg)?;
            json!({"adopted": done, "sync": sync_soft(ctx)})
        }
        // the tools' own copies of ticked skills go to .hrm-backup, then the sync links the store's
        "skill_takeover" => {
            let reg = ctx.registry()?;
            let names: Vec<String> = a["names"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            let moved = items::takeover_skills(ctx, &reg, &names)?;
            json!({"moved": moved, "sync": sync_soft(ctx)})
        }
        "skill_import" => {
            let mut reg = ctx.registry()?;
            let (added, kept) = items::import_skills(ctx, &mut reg, &s(a, "harness")?)?;
            ctx.save_registry(&reg)?;
            json!({"added": added, "kept": kept})
        }
        "skill_check" => {
            let reg = ctx.registry()?;
            json!(items::check_skill_updates(ctx, &reg).into_iter().map(|(n, u)| json!({"name": n, "update": u})).collect::<Vec<_>>())
        }
        "prompt_add" => {
            let mut reg = ctx.registry()?;
            let id = items::add_prompt(&mut reg, &s(a, "harness")?, &s(a, "title")?, a["body"].as_str().unwrap_or(""))?;
            ctx.save_registry(&reg)?;
            json!(id)
        }
        "prompt_save" => {
            let mut reg = ctx.registry()?;
            let id = s(a, "id")?;
            let p = reg.prompts.iter_mut().find(|p| p.id == id).context("no such prompt")?;
            if let Some(t) = a["title"].as_str() { p.title = t.into(); }
            if let Some(b) = a["body"].as_str() { p.body = b.into(); }
            let enabled = p.enabled;
            ctx.save_registry(&reg)?;
            if enabled { items::enable_prompt(ctx, &mut reg, &id)?; ctx.save_registry(&reg)?; }
            json!(true)
        }
        "prompt_enable" => {
            let mut reg = ctx.registry()?;
            items::enable_prompt(ctx, &mut reg, &s(a, "id")?)?;
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "prompt_delete" => {
            let mut reg = ctx.registry()?;
            let id = s(a, "id")?;
            reg.prompts.retain(|p| p.id != id);
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "prompt_live" => {
            // what the tool's prompt file holds right now
            let reg = ctx.registry()?;
            let h = s(a, "harness")?;
            let p = reg.profiles.iter().find(|p| p.harness == h && p.main).or_else(|| reg.profiles.iter().find(|p| p.harness == h)).context("no profile for this tool")?;
            let f = harness::of(p).item_paths(ctx, p).prompt.context("this tool has no prompt file")?;
            json!({"path": f, "body": std::fs::read_to_string(&f).unwrap_or_default()})
        }
        "sync" => {
            let mut relinked = vec![];
            if a["relink"] == true {
                let reg = ctx.registry()?;
                let c = harness::claude::Claude;
                for p in reg.profiles.iter().filter(|p| p.harness == "claude" && !p.main) {
                    let done = hrm_core::shared::relink_copies(&harness::Harness::default_home(&c, ctx), &harness::Harness::home(&c, ctx, p))?;
                    if !done.is_empty() { relinked.push(format!("{}: {}", p.name, done.join(", "))); }
                }
            }
            json!({"items": sync_items(ctx)?, "relinked": relinked})
        }

        "routing" => {
            let reg = ctx.registry()?;
            let breakers: Value = reg.providers.iter().map(|p| (p.id.clone(), ctx.db.kv_get(&format!("breaker:{}", p.id)).ok().flatten()
                .and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!({"state": "closed", "failures": 0})))).collect::<serde_json::Map<_, _>>().into();
            let keys: Value = reg.providers.iter().map(|p| (p.id.clone(), json!(hrm_core::secrets::get_key(&hrm_core::secrets::provider_target(&p.id)).is_some()))).collect::<serde_json::Map<_, _>>().into();
            json!({"providers": reg.providers, "routes": reg.routes, "breakers": breakers, "keys": keys,
                   "running": service::router_running(ctx), "port": ctx.settings.router_port, "log": ctx.db.route_log(40)?,
                   "stats": hrm_core::dash::router_stats(ctx, hrm_core::util::now_ms())?, "now": hrm_core::util::now_ms(),
                   "routed_profiles": reg.profiles.iter().filter(|p| p.kind == Kind::Routed).map(|p| json!({"name": p.name, "route": p.route})).collect::<Vec<_>>()})
        }
        "provider_add" => {
            let id = s(a, "id")?;
            if !hrm_core::util::valid_name(&id) { bail!("'{id}' is not a valid id: use letters, digits, - and _"); }
            let format = s(a, "format")?;
            if !matches!(format.as_str(), "anthropic" | "openai") { bail!("format must be anthropic or openai"); }
            let base_url = s(a, "base_url")?;
            if !base_url.starts_with("http") { bail!("the base URL must start with http:// or https://"); }
            let mut reg = ctx.registry()?;
            let models: Vec<String> = a["models"].as_str().unwrap_or("").split(',').map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect();
            if let Some(k) = opt(a, "key") { hrm_core::secrets::set_key(&hrm_core::secrets::provider_target(&id), k.trim())?; }
            else if hrm_core::secrets::get_key(&hrm_core::secrets::provider_target(&id)).is_none() { bail!("enter the provider's key"); }
            reg.providers.retain(|p| p.id != id);
            reg.providers.push(Provider { name: opt(a, "name").unwrap_or_else(|| id.clone()), id, base_url, format, models });
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "provider_remove" => {
            let id = s(a, "id")?;
            let mut reg = ctx.registry()?;
            reg.remove_provider(&id)?;
            hrm_core::secrets::delete(&hrm_core::secrets::provider_target(&id))?;
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "route_add" => {
            let id = s(a, "id")?;
            if !hrm_core::util::valid_name(&id) { bail!("'{id}' is not a valid id"); }
            let mode = s(a, "mode")?;
            let providers: Vec<String> = a["providers"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            let mut reg = ctx.registry()?;
            if providers.is_empty() { bail!("choose at least one provider"); }
            for p in &providers { if !reg.providers.iter().any(|x| &x.id == p) { bail!("no provider named '{p}'"); } }
            reg.routes.retain(|r| r.id != id);
            reg.routes.push(Route { id, mode, providers, default_provider: opt(a, "default") });
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "route_remove" => {
            let id = s(a, "id")?;
            let mut reg = ctx.registry()?;
            reg.remove_route(&id)?;
            ctx.save_registry(&reg)?;
            json!(true)
        }
        "router_start" => { service::router_token(ctx)?; service::ensure_router(ctx)?; json!(true) }
        "router_stop" => { service::router_stop(ctx)?; json!(true) }

        "backups" => json!({"list": hrm_core::backup::list(ctx), "dir": hrm_core::backup::dir(ctx), "data": ctx.data, "log": hrm_core::log::file(&ctx.data), "in_sync_folder": hrm_core::backup::in_sync_folder(ctx)}),
        "backup_create" => json!(hrm_core::backup::create_with(ctx, a["label"].as_str().unwrap_or("Manual"), a["transcripts"].as_bool().unwrap_or(ctx.settings.backup_transcripts))?),
        "backup_delete" => { hrm_core::backup::delete(ctx, &s(a, "name")?)?; json!(true) }
        "backup_rename" => json!(hrm_core::backup::rename(ctx, &s(a, "name")?, &s(a, "label")?)?),
        "backup_export" => { hrm_core::backup::export(ctx, std::path::Path::new(&s(a, "path")?))?; json!(true) }
        // a backup file from elsewhere is kept with the others, then put back
        "backup_restore" | "backup_import" => {
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
            json!({"restored": r, "sync": synced})
        }
        "log_open" => {
            let f = hrm_core::log::file(&ctx.data);
            if f.exists() { std::process::Command::new("explorer").arg(format!("/select,{}", f.display())).spawn()?; }
            else { std::fs::create_dir_all(f.parent().unwrap())?; std::process::Command::new("explorer").arg(f.parent().unwrap()).spawn()?; }
            json!(true)
        }
        "settings_set" => {
            let key = s(a, "key")?;
            let mut v = serde_json::to_value(&ctx.settings)?;
            if v.get(&key).is_none() { bail!("no setting named {key}"); }
            v[&key] = a["value"].clone();
            let settings = serde_json::from_value(v).context("that value does not fit the setting")?;
            let mut fresh = Ctx::open_at(&ctx.data, &ctx.home)?;
            fresh.settings = settings;
            fresh.save_settings()?;
            *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(fresh);
            *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
            json!(true)
        }
        "live_quota_set" => { ctx.db.kv_set("live_quota", if a["on"] == true { "1" } else { "0" })?; json!(true) }
        "import_ps" => {
            let mut reg = ctx.registry()?;
            let mut rep = hrm_core::import::ImportReport::default();
            let path = opt(a, "path").map(PathBuf::from).unwrap_or_else(|| hrm_core::import::default_ps_registry(ctx));
            hrm_core::import::import_powershell(ctx, &mut reg, &path, &mut rep)?;
            ctx.save_registry(&reg)?;
            for p in &reg.profiles { let _ = harness::of(p).prepare(ctx, p); }
            json!(rep)
        }
        "detect" => {
            let mut reg = ctx.registry()?;
            let mut rep = hrm_core::import::ImportReport::default();
            hrm_core::import::detect_main_profiles(ctx, &mut reg, &mut rep);
            ctx.save_registry(&reg)?;
            json!(rep)
        }
        // resume here (phase 3): a session from another computer as a fork in this computer's clone
        "session_resume_here" => hrm_core::transcripts::resume_here(ctx, &s(a, "device")?, &s(a, "harness")?, &s(a, "profile")?, &s(a, "id")?)?,
        "session_fetch" => {
            let t = hrm_core::transcripts::fetch(ctx, &s(a, "device")?, &s(a, "harness")?, &s(a, "profile")?, &s(a, "id")?)?;
            json!({"path": t.path, "bytes": t.bytes})
        }
        "transcripts_status" => hrm_core::transcripts::status(ctx)?,
        // the hub (phase 5): joining with a code or the admin token, inviting, leaving, the recovery key
        "hub_join" | "hub_leave" => {
            let fresh = Arc::new(if op == "hub_join" { hrm_core::sync::hub::join(ctx, &s(a, "url")?, opt(a, "code").as_deref(), opt(a, "admin_token").as_deref())? } else { hrm_core::sync::hub::leave(ctx)? });
            *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh.clone();
            *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if hrm_core::journal::on(&fresh) { hrm_core::journal::round(&fresh)? } else { hrm_core::journal::status(&fresh)? }
        }
        "hub_invite" => hrm_core::sync::hub::invite(ctx)?,
        "hub_status" => hrm_core::sync::hub::status(ctx),
        "ring_key_show" => json!({"key": hrm_core::sync::hub::ring_show(ctx)?}),
        "ring_key_enter" => {
            hrm_core::sync::hub::ring_enter(ctx, &s(a, "key")?)?;
            if ctx.settings.sync_mode == "hub" { hrm_core::journal::round(ctx)? } else { hrm_core::journal::status(ctx)? }
        }
        // phase 4: restore as a merge (a backup zip or a sync folder), Welcome back, and the archive
        "restore_preview" => hrm_core::restore::preview(ctx, &hrm_core::restore::Source::parse(a)?)?,
        "restore_run" => {
            let r = hrm_core::restore::run(ctx, &hrm_core::restore::Source::parse(a)?, a["adopt"] == true)?;
            hrm_core::restore::welcome_done(ctx)?;
            // with adopt the settings changed: the identity follows now
            let fresh = Arc::new(Ctx::open_at(&ctx.data, &ctx.home)?);
            *state.ctx.lock().unwrap_or_else(|e| e.into_inner()) = fresh;
            *state.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None;
            r
        }
        "restore_welcome" => if a["done"] == true { hrm_core::restore::welcome_done(ctx)?; json!(true) } else { hrm_core::restore::welcome(ctx)? },
        "archive_status" => hrm_core::archive::status(ctx)?,
        "archive_now" => hrm_core::archive::archive_now(ctx)?,
        other => bail!("unknown operation {other}"),
    })
}

/// Harness Manager's own newer version, from the newest release's latest.json (HRM_UPDATE_URL in tests).
/// `install` downloads and installs it, then restarts (the Windows installer closes the app itself).
async fn self_update(app: &tauri::AppHandle, install: bool) -> Result<Value> {
    use tauri_plugin_updater::UpdaterExt;
    let mut b = app.updater_builder();
    if let Ok(u) = std::env::var("HRM_UPDATE_URL") { b = b.endpoints(vec![u.parse()?])?; }
    let current = app.package_info().version.to_string();
    let found = b.build()?.check().await?;
    let seen = json!({"at": hrm_core::util::now_ms(), "current": current, "version": found.as_ref().map(|u| u.version.clone()), "notes": found.as_ref().and_then(|u| u.body.clone())});
    let _ = app.state::<Arc<AppState>>().ctx().db.kv_set("self:latest", &seen.to_string());
    let Some(up) = found else { return Ok(json!({"current": current})) };
    if install {
        up.download_and_install(|_, _| {}, || {}).await?;
        app.restart();
    }
    Ok(json!({"current": current, "version": up.version, "notes": up.body}))
}

/// Look for a new Harness Manager at launch and then every hour; the window offers it, or drops an old offer.
/// The tests' copy looks only when they give it a release to find.
fn watch_self_update(app: tauri::AppHandle) {
    if std::env::var("HRM_DEVTOOLS_PORT").is_ok() && std::env::var("HRM_UPDATE_URL").is_err() { return; }
    std::thread::spawn(move || loop {
        match tauri::async_runtime::block_on(self_update(&app, false)) {
            Ok(v) => { let _ = app.emit("self-update", v); }
            Err(e) => hrm_core::log::warn(&format!("update check: {e:#}")),
        }
        std::thread::sleep(std::time::Duration::from_secs(3600));
    });
}

#[tauri::command]
async fn call(app: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>, op: String, args: Value) -> Result<Value, String> {
    if op == "self_update_check" || op == "self_update_install" {
        return self_update(&app, op == "self_update_install").await.map_err(|e| format!("{e:#}"));
    }
    // the limits popup: a ring opens the window at that account
    if op == "open_main" {
        show_main(&app);
        if let Some(h) = args["hash"].as_str() { let _ = app.emit_to("main", "navigate", h); }
        return Ok(Value::Null);
    }
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !NO_CHANGE.contains(&op.as_str()) { st.epoch.fetch_add(1, Ordering::SeqCst); st.pages.lock().unwrap_or_else(|e| e.into_inner()).clear(); }
        let r = if PAGE_CALLS.contains(&op.as_str()) { cached(&st, &op, &args) } else { dispatch(&st, &op, &args) };
        r.map_err(|e| format!("{e:#}"))
    })
        .await
        .map_err(|e| e.to_string())?
}

/// The app has no console, so a startup error is shown in a message box instead of vanishing.
fn fatal(msg: &str) -> ! {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        unsafe { MessageBoxW(std::ptr::null_mut(), w(msg).as_ptr(), w("Harness Manager").as_ptr(), MB_OK | MB_ICONERROR) };
    }
    eprintln!("{msg}");
    std::process::exit(1)
}

/// Bring the main window back from the tray (or from behind other windows).
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The limits popup: every account's ring in a small frameless window by the tray icon. A click on the icon opens
/// it and a second one closes it; so does losing the focus. The window itself opens from the icon's menu or a double-click.
fn limits_popup(app: &tauri::AppHandle, at: tauri::PhysicalPosition<f64>, icon: tauri::Rect) {
    use std::time::{Duration, Instant};
    use tauri::{PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};
    static HID: Mutex<Option<Instant>> = Mutex::new(None);
    // the click that took its focus (and so hid it) must not open it again
    if HID.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() < Duration::from_millis(400)) { return; }
    let w = match app.get_webview_window("limits") {
        Some(w) => { if w.is_visible().unwrap_or(false) { let _ = w.hide(); return; } w }
        None => {
            // three rings to a row, a row per three shown accounts. shortcut: sized once, at its first open; an account added later scrolls, until the app restarts
            let n = { let ctx = app.state::<Arc<AppState>>().ctx(); let hidden = &ctx.settings.hidden_tools; ctx.registry().map(|r| r.profiles.iter().filter(|p| !hidden.contains(&p.harness)).count()).unwrap_or(0) };
            let w = match WebviewWindowBuilder::new(app, "limits", WebviewUrl::App("limits.html".into()))
                .title("Limits").decorations(false).resizable(false).always_on_top(true).skip_taskbar(true).visible(false)
                .inner_size(372.0, 20.0 + n.max(1).div_ceil(3) as f64 * 104.0).build() {
                Ok(w) => w,
                Err(e) => { hrm_core::log::warn(&format!("limits popup: {e:#}")); return }
            };
            let w2 = w.clone();
            w.on_window_event(move |e| if let WindowEvent::Focused(false) = e { let _ = w2.hide(); *HID.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now()); });
            w
        }
    };
    // by the icon: above a tray at the foot of the screen (Windows), under one at the top (macOS), and within the screen
    let size = w.outer_size().unwrap_or_default();
    let (sw, sh) = (size.width as f64, size.height as f64);
    let m = app.monitor_from_point(at.x, at.y).ok().flatten();
    let scale = m.as_ref().map_or(1.0, |m| m.scale_factor());
    let (mx, my, mw, mh) = m.map(|m| (m.position().x as f64, m.position().y as f64, m.size().width as f64, m.size().height as f64)).unwrap_or((0.0, 0.0, at.x + sw, at.y + sh));
    let top = match icon.position { tauri::Position::Physical(p) => p.y as f64, tauri::Position::Logical(p) => p.y * scale };
    let bottom = top + match icon.size { tauri::Size::Physical(s) => s.height as f64, tauri::Size::Logical(s) => s.height * scale };
    let x = (at.x - sw / 2.0).clamp(mx, (mx + mw - sw).max(mx));
    let y = (if at.y > my + mh / 2.0 { top - sh - 6.0 } else { bottom + 6.0 }).clamp(my, (my + mh - sh).max(my));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
}

/// The tray icon: a click shows the limits, a double-click the window; its menu opens the window or quits.
fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItem::with_id(app, "open", "Open Harness Manager", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut t = TrayIconBuilder::with_id("main").tooltip("Harness Manager").menu(&menu).show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|t, e| match e {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, rect, .. } => limits_popup(t.app_handle(), position, rect),
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => show_main(t.app_handle()),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() { t = t.icon(icon.clone()); }
    t.build(app)?;
    Ok(())
}

fn main() {
    hrm_core::log::log_panics();
    // before any thread starts: set_var is not safe while others read the environment
    #[cfg(not(windows))]
    hrm_core::util::adopt_shell_path();
    let ctx = Arc::new(Ctx::open().unwrap_or_else(|e| fatal(&format!("Harness Manager could not start:\n\n{e:#}"))));
    let first = { let _l = ctx.lock_registry(); hrm_core::import::first_run(&ctx).ok().flatten().map(|r| serde_json::to_value(r).unwrap()) };
    let state = Arc::new(AppState { ctx: Mutex::new(ctx), pages: Mutex::new(HashMap::new()), epoch: AtomicU64::new(0), first_run: Mutex::new(first), settings_mtime: Mutex::new(None), peers: std::sync::OnceLock::new() });
    let mut builder = tauri::Builder::default();
    // one copy runs: opening it again (Start menu, a shortcut) shows the window the tray keeps. The tests' copy
    // skips this, so they can run while the installed app sits in the tray.
    if std::env::var("HRM_DEVTOOLS_PORT").is_err() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state.clone())
        // closing hides the window; the app keeps refreshing in the tray until Quit
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if w.app_handle().state::<Arc<AppState>>().ctx().settings.close_to_tray {
                    api.prevent_close();
                    let _ = w.hide();
                } else {
                    w.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![call])
        .setup(move |app| {
            // background refresh: new usage and (opted-in) live limits, then tell the window
            let handle = app.handle().clone();
            let st = state.clone();
            let mut first = true;
            // each loop's round is guarded: a panic is logged and the next round runs, where it once ended the thread
            std::thread::spawn(move || loop {
                hrm_core::log::round("background refresh", || {
                    let ctx = st.ctx();
                    // the servers the tools already have are in the list from the first start, without an Import click
                    if std::mem::take(&mut first) {
                        let _l = ctx.lock_registry();
                        if let Err(e) = import_mcp_now(&ctx) { hrm_core::log::warn(&format!("reading the tools' MCP servers: {e:#}")); }
                        // a router left running by an older version or a dev build is replaced by this build's
                        if service::router_running(&ctx) { if let Err(e) = service::ensure_router(&ctx) { hrm_core::log::warn(&format!("router: {e:#}")); } }
                    }
                    let n = service::import_usage(&ctx).unwrap_or(0);
                    let _ = service::refresh_quotas(&ctx, false);
                    if let Err(e) = hrm_core::backup::auto(&ctx) { hrm_core::log::warn(&format!("automatic backup: {e:#}")); }
                    if let Err(e) = hrm_core::archive::daily(&ctx) { hrm_core::log::warn(&format!("archive: {e:#}")); }
                    // the index the pages read: every account's folders walked (the watcher's safety net)
                    if let Err(e) = hrm_core::index::refresh(&ctx, hrm_core::index::Scope::All) { hrm_core::log::warn(&format!("index: {e:#}")); }
                    // across computers: out with what is new here, in with what the others published
                    if hrm_core::journal::on(&ctx) {
                        match hrm_core::journal::round(&ctx) {
                            Ok(v) => { let _ = handle.emit("computers", v); }
                            Err(e) => hrm_core::log::warn(&format!("computers: {e:#}")),
                        }
                    }
                    let _ = handle.emit("refreshed", json!({"new_rows": n}));
                });
                let mins = st.ctx().settings.refresh_minutes.max(1) as u64;
                std::thread::sleep(std::time::Duration::from_secs(mins * 60));
            });
            // what runs here, for the other computers' live views, once a minute
            let st2 = state.clone();
            std::thread::spawn(move || loop {
                hrm_core::log::round("presence", || {
                    if let Err(e) = hrm_core::journal::presence_tick(&st2.ctx()) { hrm_core::log::warn(&format!("presence: {e:#}")); }
                    if let Err(e) = hrm_core::index::mark_open(&st2.ctx()) { hrm_core::log::warn(&format!("index: open sessions: {e:#}")); }
                });
                std::thread::sleep(std::time::Duration::from_secs(60));
            });
            // the folder watcher: a changed session file is in the index within a second, and the pages redraw
            let (h6, st6) = (app.handle().clone(), state.clone());
            hrm_core::watch::start(Arc::new(move || st6.ctx()), Arc::new(move |r| { let _ = h6.emit("refreshed", json!({"new_rows": 0, "index": r})); }));
            // the pages the window opened last time, computed while it starts
            let st8 = state.clone();
            std::thread::spawn(move || prefetch_pages(&st8));
            // the direct connection: the listener while it is on, the paired computers pulled every ten seconds
            let (h3, st3) = (app.handle().clone(), state.clone());
            let on_event: hrm_core::sync::peer::OnEvent = Arc::new(move |name, v| { let _ = h3.emit(name, v); });
            match hrm_core::sync::peer::Runtime::new(std::net::Ipv4Addr::UNSPECIFIED.into(), on_event) {
                Ok(rt) => {
                    let rt = Arc::new(rt);
                    let _ = state.peers.set(rt.clone());
                    let h4 = app.handle().clone();
                    std::thread::spawn(move || loop {
                        hrm_core::log::round("direct connection", || {
                            let ctx = st3.ctx();
                            rt.tick(&ctx);
                            if ctx.settings.direct_connection {
                                if let Ok(v) = hrm_core::journal::status(&st3.ctx()) { let _ = h4.emit("computers", v); }
                            }
                        });
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    });
                }
                Err(e) => hrm_core::log::warn(&format!("computers: the direct connection could not start: {e:#}")),
            }
            // the hub's events socket while the app runs (phase 5): a round on each change, the window told
            let (h5, st5) = (app.handle().clone(), state.clone());
            hrm_core::sync::hub::watch(Arc::new(move || st5.ctx()), Arc::new(move |v| { let _ = h5.emit("computers", v); }));
            let mut win = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                .title("Harness Manager")
                .inner_size(1280.0, 820.0)
                .min_inner_size(980.0, 640.0)
                // started at sign-in (--tray): the window waits in the tray
                .visible(!std::env::args().any(|a| a == "--tray"));
            // end-to-end tests drive the window over the DevTools protocol; only when they ask for it
            if let Ok(port) = std::env::var("HRM_DEVTOOLS_PORT") {
                win = win.additional_browser_args(&format!("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"))
                    // its own browser profile: WebView2 cannot start a second, differently configured browser in the
                    // profile an open Harness Manager window is using, and the test would never get its window
                    .data_directory(state.ctx().data.join("webview"));
            }
            let w = win.build()?;
            if w.is_visible().unwrap_or(false) { w.set_focus()?; }
            tray(app)?;
            watch_self_update(app.handle().clone());
            // newer versions of the tools, when asked to look at launch: the Apps item shows how many
            let (handle, st) = (app.handle().clone(), state.clone());
            std::thread::spawn(move || {
                let ctx = st.ctx();
                if ctx.settings.check_updates_at_launch {
                    if let Ok(v) = hrm_core::apps::check(&ctx) { let _ = handle.emit("app-updates", v["updates"].clone()); }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Harness Manager");
}
