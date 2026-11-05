//! Harness Manager desktop app: the window is a view over hrm-core. Every action goes through one
//! command, `call(op, args)`, which runs the same core functions the `hrm` CLI uses.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use hrm_core::registry::{Kind, McpServer, Provider, Route};
use hrm_core::service::{self, AddRequest, SessionFilter};
use hrm_core::{harness, items, Ctx};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

struct AppState {
    ctx: Mutex<Arc<Ctx>>,
    first_run: Mutex<Option<Value>>,
    settings_mtime: Mutex<Option<std::time::SystemTime>>,
}

impl AppState {
    /// The context, reopened when settings.json changed on disk (for example through `hrm settings`).
    fn ctx(&self) -> Arc<Ctx> {
        let mut cur = self.ctx.lock().unwrap();
        let m = std::fs::metadata(cur.data.join("settings.json")).and_then(|m| m.modified()).ok();
        let mut seen = self.settings_mtime.lock().unwrap();
        if m != *seen {
            if let Ok(fresh) = Ctx::open_at(&cur.data, &cur.home) { *cur = Arc::new(fresh); }
            *seen = m;
        }
        cur.clone()
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
    "login_poll", "login_cancel", "login_terminal", "refresh", "home", "requests", "today", "usage", "sessions", "projects", "tag_set", "tag_list", "tag_rename", "transcript", "file_stamp", "resume",
    "items", "skill_check", "discover", "apps", "apps_latest", "app_update", "app_install", "app_remove_copy", "autostart_set", "backups", "backup_export", "log_open", "prompt_live", "routing", "router_start", "router_stop", "live_quota_set",
    "limits_extra", "account_stats", "session_stats", "sync_last", "self_latest", "mcp_last_import", "usage_csv"];

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
                "first_run": state.first_run.lock().unwrap().take(),
                "now": hrm_core::util::now_ms(),
            })
        }
        "detail" => {
            let name = s(a, "name")?;
            let reg = ctx.registry()?;
            let p = reg.get(&name)?;
            let f = SessionFilter { profile: Some(name.clone()), days: Some(30), ..Default::default() };
            let mut sessions = service::sessions(ctx, &f)?;
            sessions.truncate(40);
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
        "home" => {
            let days = a["days"].as_i64();
            let h = a["harness"].as_str().filter(|s| !s.is_empty());
            let since = a["hours"].as_i64().map(|x| hrm_core::util::now_ms() - x * 3_600_000);
            let (total, rows) = service::request_log_since(ctx, days, since, h, a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
            json!({"overview": service::overview(ctx, days, h)?, "requests": {"total": total, "rows": rows},
                   "dash": hrm_core::dash::dashboard(ctx, a["hours"].as_i64().unwrap_or(24), h)?})
        }
        "requests" => {
            let since = a["hours"].as_i64().map(|x| hrm_core::util::now_ms() - x * 3_600_000);
            let (total, rows) = service::request_log_since(ctx, a["days"].as_i64(), since, a["harness"].as_str().filter(|s| !s.is_empty()), a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
            json!({"total": total, "rows": rows})
        }
        "today" => json!(service::overview(ctx, Some(1), None)?.today_cost),
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
            let f: SessionFilter = serde_json::from_value(a["filter"].clone()).unwrap_or_default();
            json!(service::grouped_sessions(ctx, &f, a["group"].as_str().unwrap_or("project"))?)
        }
        "transcript" => json!(service::transcript(ctx, &s(a, "profile")?, &s(a, "id")?)?),
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
        "projects" => json!(service::projects(ctx, a["days"].as_i64())?),
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
            *state.ctx.lock().unwrap() = Arc::new(fresh);
            *state.settings_mtime.lock().unwrap() = None;
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
        "apps" => json!({"apps": hrm_core::apps::list(), "latest": ctx.db.kv_get("apps:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok())}),
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

        "backups" => json!({"list": hrm_core::backup::list(ctx), "dir": hrm_core::backup::dir(ctx), "data": ctx.data, "log": hrm_core::log::file(&ctx.data)}),
        "backup_create" => json!(hrm_core::backup::create(ctx, a["label"].as_str().unwrap_or("Manual"))?),
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
            *state.ctx.lock().unwrap() = fresh.clone();
            *state.settings_mtime.lock().unwrap() = None;
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
            *state.ctx.lock().unwrap() = Arc::new(fresh);
            *state.settings_mtime.lock().unwrap() = None;
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
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || dispatch(&st, &op, &args).map_err(|e| format!("{e:#}")))
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

/// The tray icon: a click opens the window; its menu opens it or quits.
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
        .on_tray_icon_event(|t, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e { show_main(t.app_handle()); }
        });
    if let Some(icon) = app.default_window_icon() { t = t.icon(icon.clone()); }
    t.build(app)?;
    Ok(())
}

fn main() {
    // before any thread starts: set_var is not safe while others read the environment
    #[cfg(not(windows))]
    hrm_core::util::adopt_shell_path();
    let ctx = Arc::new(Ctx::open().unwrap_or_else(|e| fatal(&format!("Harness Manager could not start:\n\n{e:#}"))));
    let first = { let _l = ctx.lock_registry(); hrm_core::import::first_run(&ctx).ok().flatten().map(|r| serde_json::to_value(r).unwrap()) };
    let state = Arc::new(AppState { ctx: Mutex::new(ctx), first_run: Mutex::new(first), settings_mtime: Mutex::new(None) });
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
            std::thread::spawn(move || loop {
                let ctx = st.ctx();
                // the servers the tools already have are in the list from the first start, without an Import click
                if std::mem::take(&mut first) {
                    let _l = ctx.lock_registry();
                    if let Err(e) = import_mcp_now(&ctx) { hrm_core::log::warn(&format!("reading the tools' MCP servers: {e:#}")); }
                }
                let n = service::import_usage(&ctx).unwrap_or(0);
                let _ = service::refresh_quotas(&ctx, false);
                if let Err(e) = hrm_core::backup::auto(&ctx) { hrm_core::log::warn(&format!("automatic backup: {e:#}")); }
                let _ = handle.emit("refreshed", json!({"new_rows": n}));
                let mins = ctx.settings.refresh_minutes.max(1) as u64;
                drop(ctx);
                std::thread::sleep(std::time::Duration::from_secs(mins * 60));
            });
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
