//! The setup pages: MCP servers, skills, system prompts, API keys and routing, and the tools themselves (Apps).

use anyhow::{bail, Context, Result};
use hrm_core::registry::{Kind, McpServer, Provider, Route};
use hrm_core::{harness, items, service, Ctx};
use serde_json::{json, Value};

use crate::{import_mcp_now, opt, s, sync_items, sync_soft, AppState};

pub fn items(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
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
    Ok(json!({"mcp": reg.mcp, "skills": reg.skills, "skill_info": info, "unmanaged": unmanaged, "skill_repos": items::skill_repos(&reg), "prompts": reg.prompts, "targets": reg.targets, "skill_store": store,
              "profiles": reg.profiles.iter().map(|p| json!({"name": p.name, "harness": p.harness})).collect::<Vec<_>>()}))
}

pub fn item_remove(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { Ok(json!(service::item_remove(ctx, &s(a, "kind")?, &s(a, "name")?)?)) }

pub fn target(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    items::set_target(&mut reg, &s(a, "kind")?, &s(a, "item")?, &s(a, "harness")?, opt(a, "profile").as_deref(), a["on"] == true)?;
    ctx.save_registry(&reg)?;
    sync_items(ctx)
}

// a column's header tick: every listed item on or off for one tool, written once
pub fn target_bulk(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let (kind, h) = (s(a, "kind")?, s(a, "harness")?);
    for it in a["items"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
        items::set_target(&mut reg, &kind, it, &h, None, a["on"] == true)?;
    }
    ctx.save_registry(&reg)?;
    sync_items(ctx)
}

// add (refused when the name exists) or edit a server, and set the tools it is written to
pub fn mcp_save(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let server: McpServer = serde_json::from_value(a["server"].clone())?;
    let tools: Option<Vec<String>> = a["targets"].as_array().map(|on| on.iter().filter_map(|v| v.as_str().map(String::from)).collect());
    Ok(json!(service::mcp_save(ctx, server, a["replace"] == true, tools.as_deref())?))
}

pub fn mcp_import_all(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(json!(import_mcp_now(ctx)?)) }

pub fn mcp_last_import(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    Ok(ctx.db.kv_get("mcp:last_import")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null))
}

pub fn sync(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let relinked = if a["relink"] == true { service::relink(ctx)? } else { vec![] };
    Ok(json!({"items": sync_items(ctx)?, "relinked": relinked}))
}

pub fn sync_last(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(ctx.db.kv_get("sync:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null)) }

pub fn skill_install(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let enable: Vec<String> = a["enable"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
    let (sk, sync) = service::skill_install(ctx, &s(a, "source")?, opt(a, "name").as_deref(), &enable)?;
    let mut v = serde_json::to_value(&sk)?;
    // the sync report, or `{"error": ...}`: a link that fails must not undo the answer
    v["sync"] = sync.map_or_else(|e| json!({"error": format!("{e:#}")}), |rep| json!(rep));
    Ok(v)
}

// the skills in each Discover repository; a day's cache unless asked to refresh
pub fn discover(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
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
    Ok(json!(out))
}

pub fn skill_repo_add(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
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
    Ok(json!(true))
}

pub fn skill_repo_remove(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    reg.skill_repos = items::skill_repos(&reg);
    let (o, n) = (s(a, "owner")?, s(a, "name")?);
    reg.skill_repos.retain(|x| !(x.owner == o && x.name == n));
    if reg.skill_repos.is_empty() { bail!("keep at least one repository"); }
    ctx.save_registry(&reg)?;
    Ok(json!(true))
}

// install again from where it came from; its ticks stay
pub fn skill_update(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let name = s(a, "name")?;
    let src = reg.skills.iter().find(|x| x.name == name).map(|x| x.source.clone()).context("no such skill")?;
    if !src.starts_with("https://github.com/") && !std::path::Path::new(&src).is_dir() { bail!("{name} has no source to update from ({src})"); }
    let sk = items::install_skill(ctx, &mut reg, &src, Some(&name))?;
    ctx.save_registry(&reg)?;
    Ok(json!(sk))
}

// skill folders in the store that the list does not have: added, with the ticks their links already show
pub fn skill_adopt(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let names: Vec<String> = a["names"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let done = items::adopt_skills(ctx, &mut reg, &names)?;
    ctx.save_registry(&reg)?;
    Ok(json!({"adopted": done, "sync": sync_soft(ctx)}))
}

// the tools' own copies of ticked skills go to .hrm-backup, then the sync links the store's
pub fn skill_takeover(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let reg = ctx.registry()?;
    let names: Vec<String> = a["names"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let moved = items::takeover_skills(ctx, &reg, &names)?;
    Ok(json!({"moved": moved, "sync": sync_soft(ctx)}))
}

pub fn skill_import(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let (added, kept, sync) = service::skill_import(ctx, &s(a, "harness")?)?;
    Ok(json!({"added": added, "kept": kept, "sync": sync}))
}

pub fn skill_check(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    let reg = ctx.registry()?;
    Ok(json!(items::check_skill_updates(ctx, &reg).into_iter().map(|(n, u)| json!({"name": n, "update": u})).collect::<Vec<_>>()))
}

pub fn prompt_add(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let id = items::add_prompt(&mut reg, &s(a, "harness")?, &s(a, "title")?, a["body"].as_str().unwrap_or(""))?;
    ctx.save_registry(&reg)?;
    Ok(json!(id))
}

pub fn prompt_save(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let id = s(a, "id")?;
    let p = reg.prompts.iter_mut().find(|p| p.id == id).context("no such prompt")?;
    if let Some(t) = a["title"].as_str() { p.title = t.into(); }
    if let Some(b) = a["body"].as_str() { p.body = b.into(); }
    let enabled = p.enabled;
    ctx.save_registry(&reg)?;
    if enabled { items::enable_prompt(ctx, &mut reg, &id)?; ctx.save_registry(&reg)?; }
    Ok(json!(true))
}

pub fn prompt_enable(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    items::enable_prompt(ctx, &mut reg, &s(a, "id")?)?;
    ctx.save_registry(&reg)?;
    Ok(json!(true))
}

pub fn prompt_delete(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let mut reg = ctx.registry()?;
    let id = s(a, "id")?;
    reg.prompts.retain(|p| p.id != id);
    ctx.save_registry(&reg)?;
    Ok(json!(true))
}

pub fn prompt_live(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    // what the tool's prompt file holds right now
    let reg = ctx.registry()?;
    let h = s(a, "harness")?;
    let p = reg.profiles.iter().find(|p| p.harness == h && p.main).or_else(|| reg.profiles.iter().find(|p| p.harness == h)).context("no profile for this tool")?;
    let f = harness::of(p).item_paths(ctx, p).prompt.context("this tool has no prompt file")?;
    // `own`: the text without the hrm block, which is what a saved prompt is compared with
    let body = std::fs::read_to_string(&f).unwrap_or_default();
    Ok(json!({"path": f, "own": items::strip_hint(&body), "body": body, "hint": ctx.settings.agent_hint}))
}

pub fn routing(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    let reg = ctx.registry()?;
    let breakers: Value = reg.providers.iter().map(|p| (p.id.clone(), ctx.db.kv_get(&format!("breaker:{}", p.id)).ok().flatten()
        .and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(json!({"state": "closed", "failures": 0})))).collect::<serde_json::Map<_, _>>().into();
    let keys: Value = reg.providers.iter().map(|p| (p.id.clone(), json!(hrm_core::secrets::get_key(&hrm_core::secrets::provider_target(&p.id)).is_some()))).collect::<serde_json::Map<_, _>>().into();
    Ok(json!({"providers": reg.providers, "routes": reg.routes, "breakers": breakers, "keys": keys,
              "running": service::router_running(ctx), "port": ctx.settings.router_port, "log": ctx.db.route_log(40)?,
              "stats": hrm_core::dash::router_stats(ctx, hrm_core::util::now_ms())?, "now": hrm_core::util::now_ms(),
              "routed_profiles": reg.profiles.iter().filter(|p| p.kind == Kind::Routed).map(|p| json!({"name": p.name, "route": p.route})).collect::<Vec<_>>()}))
}

pub fn provider_add(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let (id, format, base_url) = (s(a, "id")?, s(a, "format")?, s(a, "base_url")?);
    let models: Vec<String> = a["models"].as_str().unwrap_or("").split(',').map(String::from).collect();
    service::provider_add(ctx, Provider { name: opt(a, "name").unwrap_or_else(|| id.clone()), id, base_url, format, models }, a["key"].as_str())?;
    Ok(json!(true))
}

pub fn provider_remove(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::provider_remove(ctx, &s(a, "id")?)?; Ok(json!(true)) }

pub fn route_add(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let (id, mode) = (s(a, "id")?, s(a, "mode")?);
    let providers: Vec<String> = a["providers"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    service::route_add(ctx, Route { id, mode, providers, default_provider: opt(a, "default") })?;
    Ok(json!(true))
}

pub fn route_remove(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> { service::route_remove(ctx, &s(a, "id")?)?; Ok(json!(true)) }

pub fn router_start(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { service::router_token(ctx)?; service::ensure_router(ctx)?; Ok(json!(true)) }

pub fn router_stop(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { service::router_stop(ctx)?; Ok(json!(true)) }

// the tools themselves: installed where, which version, the newest one (last checked), install or update
pub fn apps(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> {
    let apps = hrm_core::apps::list();
    let mut latest = ctx.db.kv_get("apps:latest")?.and_then(|v| serde_json::from_str::<Value>(&v).ok());
    let updates = hrm_core::apps::recount(ctx, &apps, &mut latest)?;
    Ok(json!({"apps": apps, "latest": latest, "updates": updates}))
}

pub fn apps_latest(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { hrm_core::apps::check(ctx) }

pub fn app_update(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::apps::update(&s(a, "id")?)?)) }

pub fn app_install(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::apps::install(&s(a, "id")?, &s(a, "method")?)?)) }

pub fn app_remove_copy(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::apps::remove_copy(&s(a, "id")?, &s(a, "path")?)?)) }

pub fn app_uninstall(_: &AppState, _: &Ctx, a: &Value) -> Result<Value> { Ok(json!(hrm_core::apps::uninstall(&s(a, "id")?)?)) }
