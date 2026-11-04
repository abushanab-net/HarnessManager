//! MCP servers, skills and prompts, managed once and written into every harness in its own format.
//! A write ledger records what Harness Manager wrote, and only those entries are ever removed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::harness;
use crate::registry::{McpServer, Prompt, Registry, Skill, SkillRepo, Target};
use crate::util::{read_json, valid_name, write_atomic};
use crate::Ctx;

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    /// Entries skipped because a different, hand-made entry with that name already exists.
    pub conflicts: Vec<String>,
    pub skipped: Vec<String>,
}

// ---------- MCP ----------

fn to_claude(s: &McpServer) -> Value {
    match s.transport.as_str() {
        "stdio" => {
            let mut v = json!({"type": "stdio", "command": s.command.clone().unwrap_or_default(), "args": s.args});
            if !s.env.is_empty() { v["env"] = json!(s.env); }
            v
        }
        t => {
            let mut v = json!({"type": t, "url": s.url.clone().unwrap_or_default()});
            if !s.headers.is_empty() { v["headers"] = json!(s.headers); }
            v
        }
    }
}

fn to_opencode(s: &McpServer) -> Value {
    if s.transport == "stdio" {
        let mut cmd = vec![s.command.clone().unwrap_or_default()];
        cmd.extend(s.args.iter().cloned());
        let mut v = json!({"type": "local", "command": cmd, "enabled": true});
        if !s.env.is_empty() { v["environment"] = json!(s.env); }
        v
    } else {
        let mut v = json!({"type": "remote", "url": s.url.clone().unwrap_or_default(), "enabled": true});
        if !s.headers.is_empty() { v["headers"] = json!(s.headers); }
        v
    }
}

fn to_agy(s: &McpServer) -> Option<Value> {
    match s.transport.as_str() {
        "stdio" => {
            let mut v = json!({"command": s.command.clone().unwrap_or_default(), "args": s.args});
            if !s.env.is_empty() { v["env"] = json!(s.env); }
            Some(v)
        }
        "http" => {
            let mut v = json!({"serverUrl": s.url.clone().unwrap_or_default()});
            if !s.headers.is_empty() { v["headers"] = json!(s.headers); }
            Some(v)
        }
        _ => None, // Antigravity has no SSE transport
    }
}

fn to_codex(s: &McpServer) -> toml_edit::Table {
    use toml_edit::{value, Array, InlineTable};
    let mut t = toml_edit::Table::new();
    if s.transport == "stdio" {
        t["command"] = value(s.command.clone().unwrap_or_default());
        let mut a = Array::new();
        for x in &s.args { a.push(x.as_str()); }
        t["args"] = value(a);
        if !s.env.is_empty() {
            let mut e = InlineTable::new();
            for (k, v) in &s.env { e.insert(k, v.as_str().into()); }
            t["env"] = value(e);
        }
    } else {
        t["url"] = value(s.url.clone().unwrap_or_default());
        if !s.headers.is_empty() {
            let mut h = InlineTable::new();
            for (k, v) in &s.headers { h.insert(k, v.as_str().into()); }
            t["http_headers"] = value(h);
        }
    }
    t
}

fn from_claude(name: &str, v: &Value) -> Option<McpServer> {
    let transport = v["type"].as_str().unwrap_or(if v.get("url").is_some() { "http" } else { "stdio" }).to_string();
    let map = |x: &Value| -> BTreeMap<String, String> { x.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default() };
    Some(McpServer {
        name: name.into(),
        transport,
        command: v["command"].as_str().map(String::from),
        args: v["args"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        env: map(&v["env"]),
        url: v["url"].as_str().or(v["serverUrl"].as_str()).map(String::from),
        headers: map(&v["headers"]),
        ..Default::default()
    })
}

/// Write `wanted` into one MCP config file, removing only entries the ledger says are ours.
fn write_mcp_file(ctx: &Ctx, path: &Path, format: &str, wanted: &[&McpServer], rep: &mut SyncReport) -> Result<()> {
    let target = path.display().to_string();
    let owned: BTreeSet<String> = ctx.db.ledger(&target, "mcp")?.into_iter().collect();
    let wanted_names: BTreeSet<String> = wanted.iter().map(|s| s.name.clone()).collect();
    if wanted.is_empty() && owned.is_empty() { return Ok(()); }
    // ownership changes wait until the file is written: a failed write must not forget a server still in it
    let mut ledger: Vec<(String, bool)> = vec![];
    let commit = |ledger: Vec<(String, bool)>| -> Result<()> {
        for (n, on) in ledger { ctx.db.ledger_set(&target, "mcp", &n, on)?; }
        Ok(())
    };
    if format == "codex-toml" {
        let text = crate::util::read_or_empty(path)?;
        let mut doc: toml_edit::DocumentMut = text.parse().with_context(|| format!("{} is not valid TOML; left unchanged", path.display()))?;
        if !doc.contains_key("mcp_servers") {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            doc["mcp_servers"] = toml_edit::Item::Table(t);
        }
        let servers = doc["mcp_servers"].as_table_mut().context("mcp_servers is not a table")?;
        for s in wanted {
            let new = to_codex(s);
            if let Some(existing) = servers.get(&s.name) {
                if !owned.contains(&s.name) && existing.to_string().trim() != toml_edit::Item::Table(new.clone()).to_string().trim() {
                    rep.conflicts.push(format!("{}: {}", path.display(), s.name));
                    continue;
                }
            }
            servers.insert(&s.name, toml_edit::Item::Table(new));
            ledger.push((s.name.clone(), true));
            rep.written.push(format!("{}: {}", path.display(), s.name));
        }
        for n in owned.difference(&wanted_names) {
            servers.remove(n);
            ledger.push((n.clone(), false));
            rep.removed.push(format!("{}: {}", path.display(), n));
        }
        write_atomic(path, doc.to_string().as_bytes())?;
        return commit(ledger);
    }
    let text = crate::util::read_or_empty(path)?;
    let mut root = if text.trim().is_empty() { json!({}) } else {
        serde_json::from_str::<Value>(&text).with_context(|| format!("{} is not valid JSON; left unchanged", path.display()))?
    };
    let key = if format == "opencode-json" { "mcp" } else { "mcpServers" };
    let obj = root.as_object_mut().context("config is not a JSON object")?;
    let servers = obj.entry(key).or_insert_with(|| json!({})).as_object_mut().context("server list is not an object")?;
    for s in wanted {
        let new = match format {
            "opencode-json" => to_opencode(s),
            "agy-json" => match to_agy(s) { Some(v) => v, None => { rep.skipped.push(format!("{}: {} (SSE is not supported there)", path.display(), s.name)); continue; } },
            _ => to_claude(s),
        };
        if let Some(existing) = servers.get(&s.name) {
            if !owned.contains(&s.name) && existing != &new {
                rep.conflicts.push(format!("{}: {}", path.display(), s.name));
                continue;
            }
        }
        servers.insert(s.name.clone(), new);
        ledger.push((s.name.clone(), true));
        rep.written.push(format!("{}: {}", path.display(), s.name));
    }
    for n in owned.difference(&wanted_names) {
        servers.remove(n);
        ledger.push((n.clone(), false));
        rep.removed.push(format!("{}: {}", path.display(), n));
    }
    write_atomic(path, serde_json::to_string_pretty(&root)?.as_bytes())?;
    commit(ledger)
}

/// Write every enabled MCP server and skill into every profile of every harness.
pub fn sync(ctx: &Ctx, reg: &Registry) -> Result<SyncReport> {
    let mut rep = SyncReport::default();
    let mut done_mcp: BTreeSet<PathBuf> = BTreeSet::new();
    let mut done_skills: BTreeSet<PathBuf> = BTreeSet::new();
    for p in &reg.profiles {
        let h = harness::of(p);
        let paths = h.item_paths(ctx, p);
        let applies = |t: &&Target, kind: &str| t.kind == kind && t.harness == p.harness && t.profile.as_deref().map(|x| x == p.name).unwrap_or(true);
        if let Some((file, fmt)) = paths.mcp {
            // Claude profiles other than main follow main through the shared layer, unless targeted directly
            let direct = reg.targets.iter().any(|t| t.kind == "mcp" && t.profile.as_deref() == Some(p.name.as_str()));
            let follows_main = p.harness == "claude" && !p.main && !direct;
            if !follows_main && done_mcp.insert(file.clone()) {
                let wanted: Vec<&McpServer> = reg.mcp.iter().filter(|s| reg.targets.iter().any(|t| applies(&t, "mcp") && t.item == s.name)).collect();
                if let Err(e) = write_mcp_file(ctx, &file, fmt, &wanted, &mut rep) { rep.skipped.push(format!("{e:#}")); }
            }
        }
        if let Some(dir) = paths.skills {
            let follows_main = p.harness == "claude" && !p.main && crate::shared::is_link(&dir);
            if !follows_main && done_skills.insert(dir.clone()) {
                let wanted: Vec<&Skill> = reg.skills.iter().filter(|s| reg.targets.iter().any(|t| applies(&t, "skill") && t.item == s.name)).collect();
                if let Err(e) = sync_skills_dir(ctx, &dir, &wanted, &mut rep) { rep.skipped.push(format!("{}: {e:#}", dir.display())); }
            }
        }
        if p.harness == "claude" && !p.main {
            let main = harness::claude::Claude;
            if let Err(e) = crate::shared::sync_claude(ctx, &harness::Harness::default_home(&main, ctx), &ctx.home.join(".claude.json"), &harness::Harness::home(&main, ctx, p)) {
                rep.skipped.push(format!("{}: shared settings not updated: {e:#}", p.name));
            }
        }
    }
    // for Overview: whether the tools are in step with the shared MCP servers and skills
    let _ = ctx.db.kv_set("sync:last", &json!({"at": crate::util::now_ms(), "written": rep.written.len(), "removed": rep.removed.len(),
        "conflicts": rep.conflicts.len(), "skipped": rep.skipped.len()}).to_string());
    let text = if rep.conflicts.is_empty() && rep.skipped.is_empty() { "MCP servers and skills are in step across the tools.".to_string() }
        else { format!("MCP servers and skills synced; {} left alone (a hand-made entry or an error).", rep.conflicts.len() + rep.skipped.len()) };
    let _ = ctx.db.event(if rep.conflicts.is_empty() && rep.skipped.is_empty() { "ok" } else { "warn" }, "sync", &text);
    Ok(rep)
}

/// Bring existing MCP servers of a harness's main profile under management.
/// Import the main profile's MCP servers: (added, kept). A server whose name the library already uses for a
/// different definition is kept as the tool's own: no target, no ownership, so sync never overwrites it.
pub fn import_mcp(ctx: &Ctx, reg: &mut Registry, harness_id: &str) -> Result<(Vec<String>, Vec<String>)> {
    let Some(main) = reg.profiles.iter().find(|p| p.harness == harness_id && p.main).cloned() else { bail!("no main {harness_id} profile") };
    import_mcp_from(ctx, reg, &main)
}

/// One account's MCP servers into the list. A main profile's are turned on for its tool; another account's
/// only for that account, so they do not spill into the tool's other accounts.
pub fn import_mcp_from(ctx: &Ctx, reg: &mut Registry, prof: &crate::registry::Profile) -> Result<(Vec<String>, Vec<String>)> {
    let harness_id = prof.harness.as_str();
    let h = harness::of(prof);
    let Some((file, fmt)) = h.item_paths(ctx, prof).mcp else { return Ok((vec![], vec![])) };
    let mut found: Vec<McpServer> = vec![];
    if fmt == "codex-toml" {
        let doc: toml_edit::DocumentMut = std::fs::read_to_string(&file).unwrap_or_default().parse()?;
        if let Some(t) = doc.get("mcp_servers").and_then(|t| t.as_table()) {
            for (name, item) in t.iter() {
                let Some(tt) = item.as_table() else { continue };
                let s = |k: &str| tt.get(k).and_then(|v| v.as_str()).map(String::from);
                let kv = |k: &str| -> BTreeMap<String, String> { tt.get(k).and_then(|v| v.as_inline_table()).map(|it| it.iter().map(|(k, v)| (k.to_string(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default() };
                found.push(McpServer {
                    name: name.into(),
                    transport: if s("url").is_some() { "http".into() } else { "stdio".into() },
                    command: s("command"),
                    args: tt.get("args").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    env: kv("env"),
                    url: s("url"),
                    headers: kv("http_headers"),
                    ..Default::default()
                });
            }
        }
    } else {
        let root = read_json(&file).unwrap_or(Value::Null);
        let key = if fmt == "opencode-json" { "mcp" } else { "mcpServers" };
        for (name, v) in root[key].as_object().cloned().unwrap_or_default() {
            let s = if fmt == "opencode-json" {
                let cmd: Vec<String> = v["command"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let mut s = from_claude(&name, &json!({"url": v["url"], "headers": v["headers"], "env": v["environment"]})).unwrap();
                s.transport = if v["type"] == "remote" { "http".into() } else { "stdio".into() };
                if let Some((c, rest)) = cmd.split_first() { s.command = Some(c.clone()); s.args = rest.to_vec(); }
                s
            } else {
                from_claude(&name, &v).unwrap()
            };
            found.push(s);
        }
    }
    let target = file.display().to_string();
    let (mut added, mut kept) = (vec![], vec![]);
    for s in found {
        match reg.mcp.iter().find(|m| m.name == s.name) {
            Some(m) if to_claude(m) != to_claude(&s) => { kept.push(s.name); continue; }
            Some(_) => {}
            None => reg.mcp.push(s.clone()),
        }
        let t = Target { kind: "mcp".into(), item: s.name.clone(), harness: harness_id.into(), profile: (!prof.main).then(|| prof.name.clone()) };
        // a server the whole tool has already needs no account tick
        let whole = reg.targets.iter().any(|x| x.kind == "mcp" && x.item == s.name && x.harness == harness_id && x.profile.is_none());
        if !reg.targets.contains(&t) && !(t.profile.is_some() && whole) { reg.targets.push(t); }
        ctx.db.ledger_set(&target, "mcp", &s.name, true)?;
        added.push(s.name);
    }
    Ok((added, kept))
}

pub fn set_target(reg: &mut Registry, kind: &str, item: &str, harness_id: &str, profile: Option<&str>, on: bool) -> Result<()> {
    if harness::by_id(harness_id).is_none() { bail!("unknown harness '{harness_id}'"); }
    let exists = match kind { "mcp" => reg.mcp.iter().any(|m| m.name == item), "skill" => reg.skills.iter().any(|s| s.name == item), _ => false };
    if !exists { bail!("no {kind} named '{item}'"); }
    let t = Target { kind: kind.into(), item: item.into(), harness: harness_id.into(), profile: profile.map(String::from) };
    reg.targets.retain(|x| x != &t);
    if on { reg.targets.push(t); }
    Ok(())
}

/// Import the MCP servers of every account (main profiles first), each config file once: per file, which
/// account and part of it was read, and what was added or kept.
pub fn import_mcp_all(ctx: &Ctx, reg: &mut Registry) -> Vec<Value> {
    let mut out = vec![];
    // "added" from import includes servers already in the list; the report tells new ones apart
    let known: BTreeSet<String> = reg.mcp.iter().map(|m| m.name.clone()).collect();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut profiles: Vec<_> = reg.profiles.iter().filter(|p| harness::of(p).caps().mcp).cloned().collect();
    profiles.sort_by_key(|p| (!p.main, harness::all().iter().position(|h| h.id() == p.harness)));
    for p in profiles {
        let h = harness::of(&p);
        let Some((file, fmt)) = h.item_paths(ctx, &p).mcp else { continue };
        if !seen.insert(file.clone()) { continue; }
        let part = match fmt { "codex-toml" => "[mcp_servers]", "opencode-json" => "\"mcp\"", _ => "top-level mcpServers" };
        let shown = file.strip_prefix(&ctx.home).map(|r| format!("~\\{}", r.display())).unwrap_or_else(|_| file.display().to_string());
        let label = if p.main { h.label().to_string() } else { format!("{} · {}", h.label(), p.label.clone().unwrap_or_else(|| p.name.clone())) };
        let base = json!({"harness": h.id(), "label": label, "profile": p.name, "file": shown, "part": part, "exists": file.exists()});
        let mut v = base.clone();
        match import_mcp_from(ctx, reg, &p) {
            Ok((added, kept)) => {
                let new: Vec<&String> = added.iter().filter(|n| !known.contains(*n)).collect();
                v["new"] = json!(new); v["added"] = json!(added); v["kept"] = json!(kept);
            }
            Err(e) => v["error"] = json!(format!("{e:#}")),
        }
        out.push(v);
    }
    out
}

pub fn add_mcp(reg: &mut Registry, s: McpServer) -> Result<()> {
    if !valid_name(&s.name) { bail!("'{}' is not a valid name", s.name); }
    if !matches!(s.transport.as_str(), "stdio" | "http" | "sse") { bail!("transport must be stdio, http or sse"); }
    if s.transport == "stdio" && s.command.as_deref().unwrap_or("").is_empty() { bail!("a stdio server needs a command"); }
    if s.transport != "stdio" && s.url.as_deref().unwrap_or("").is_empty() { bail!("an {} server needs a URL", s.transport); }
    reg.mcp.retain(|m| m.name != s.name);
    reg.mcp.push(s);
    Ok(())
}

pub fn remove_item(reg: &mut Registry, kind: &str, name: &str) -> Result<()> {
    let known = match kind {
        "mcp" => reg.mcp.iter().any(|m| m.name == name),
        "skill" => reg.skills.iter().any(|s| s.name == name),
        _ => bail!("unknown item kind '{kind}'"),
    };
    if !known { bail!("no {kind} named '{name}'"); }
    reg.mcp.retain(|m| !(kind == "mcp" && m.name == name));
    reg.skills.retain(|s| !(kind == "skill" && s.name == name));
    reg.targets.retain(|t| !(t.kind == kind && t.item == name));
    Ok(())
}

/// Delete a removed skill's folder in the store. The name must be a plain name, so `..` or a full path
/// can never point the delete outside the store.
pub fn delete_skill_files(ctx: &Ctx, name: &str) -> Result<()> {
    if !valid_name(name) { bail!("'{name}' is not a skill name; its folder was left alone"); }
    let d = skill_store(ctx).join(name);
    if d.exists() { std::fs::remove_dir_all(&d).with_context(|| format!("delete {}", d.display()))?; }
    Ok(())
}

// ---------- skills ----------

/// One store all harnesses link to.
pub fn skill_store(ctx: &Ctx) -> PathBuf { ctx.home.join(".agents").join("skills") }

/// A tool's own copy of a skill (a real folder where the store's link should be) is moved into `.hrm-backup`
/// beside it, so the next sync links the store's copy. Only skills ticked for that tool, and only the names
/// given (every ticked skill when none). Returns "<folder>: <name>" for each copy moved.
pub fn takeover_skills(ctx: &Ctx, reg: &Registry, names: &[String]) -> Result<Vec<String>> {
    let store = skill_store(ctx);
    let mut done = vec![];
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for p in &reg.profiles {
        let Some(dir) = harness::of(p).item_paths(ctx, p).skills else { continue };
        if (p.harness == "claude" && !p.main && crate::shared::is_link(&dir)) || !seen.insert(dir.clone()) { continue; }
        let applies = |t: &&Target| t.kind == "skill" && t.harness == p.harness && t.profile.as_deref().map(|x| x == p.name).unwrap_or(true);
        for s in reg.skills.iter().filter(|s| (names.is_empty() || names.contains(&s.name)) && reg.targets.iter().any(|t| applies(&t) && t.item == s.name)) {
            let dst = dir.join(&s.name);
            if !dst.exists() || crate::shared::is_link(&dst) || !store.join(&s.name).is_dir() { continue; }
            let backup = dir.join(".hrm-backup");
            std::fs::create_dir_all(&backup)?;
            let mut target = backup.join(&s.name);
            if target.exists() { target = backup.join(format!("{}-{}", s.name, crate::util::now_ms())); }
            std::fs::rename(&dst, &target).with_context(|| format!("move {} aside", dst.display()))?;
            done.push(format!("{}: {}", dir.display(), s.name));
        }
    }
    Ok(done)
}

fn sync_skills_dir(ctx: &Ctx, dir: &Path, wanted: &[&Skill], rep: &mut SyncReport) -> Result<()> {
    let target = dir.display().to_string();
    let owned: BTreeSet<String> = ctx.db.ledger(&target, "skill")?.into_iter().collect();
    if wanted.is_empty() && owned.is_empty() { return Ok(()); }
    std::fs::create_dir_all(dir)?;
    let store = skill_store(ctx);
    let names: BTreeSet<String> = wanted.iter().map(|s| s.name.clone()).collect();
    for s in wanted {
        let src = store.join(&s.name);
        let dst = dir.join(&s.name);
        if !src.is_dir() { rep.skipped.push(format!("{}: skill {} is missing from the store", dir.display(), s.name)); continue; }
        let link = crate::shared::is_link(&dst);
        if link || dst.exists() {
            let points_here = link && std::fs::canonicalize(&dst).ok() == std::fs::canonicalize(&src).ok();
            if points_here { ctx.db.ledger_set(&target, "skill", &s.name, true)?; continue; }
            // our own link that now points elsewhere or nowhere is replaced; anything else is the user's
            if !(link && owned.contains(&s.name)) { rep.conflicts.push(format!("{}: {}", dir.display(), s.name)); continue; }
            crate::shared::remove_link(&dst)?;
        }
        crate::shared::link_dir(&src, &dst)?;
        ctx.db.ledger_set(&target, "skill", &s.name, true)?;
        rep.written.push(format!("{}: {}", dir.display(), s.name));
    }
    for n in owned.difference(&names) {
        let p = dir.join(n);
        // only a link to the store's copy: the user may have put their own link there since
        let to_store = std::fs::canonicalize(&p).ok() == std::fs::canonicalize(store.join(n)).ok();
        if crate::shared::is_link(&p) && (to_store || !p.exists()) { crate::shared::remove_link(&p)?; }
        ctx.db.ledger_set(&target, "skill", n, false)?;
        rep.removed.push(format!("{}: {}", dir.display(), n));
    }
    Ok(())
}

pub fn dir_hash(dir: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut files = vec![];
    crate::util::walk_files(dir, &|_| true, &mut files);
    files.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in files {
        f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().replace('\\', "/").hash(&mut h);
        std::fs::read(&f).unwrap_or_default().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() { copy_dir(&e.path(), &to)?; } else { std::fs::copy(e.path(), to)?; }
    }
    Ok(())
}

/// Install a skill into the store from a local folder or a GitHub URL
/// (`https://github.com/owner/repo` or `.../tree/<branch>/<path>`).
pub fn install_skill(ctx: &Ctx, reg: &mut Registry, source: &str, name: Option<&str>) -> Result<Skill> {
    let store = skill_store(ctx);
    std::fs::create_dir_all(&store)?;
    let staged = ctx.data.join("skill-staging");
    if staged.exists() { std::fs::remove_dir_all(&staged)?; }
    // kept until the copy below is done: dropping it deletes the download
    let mut repos = Repos::new(staged.clone());
    let folder = if source.starts_with("https://github.com/") {
        fetch_github_dir(source, &mut repos)?
    } else {
        let p = PathBuf::from(source);
        if !p.join("SKILL.md").exists() { bail!("{} has no SKILL.md", p.display()); }
        p
    };
    let name = name.map(String::from).unwrap_or_else(|| folder.file_name().and_then(|n| n.to_str()).unwrap_or("skill").to_string());
    if !valid_name(&name) { bail!("'{name}' is not a valid skill name"); }
    let dst = store.join(&name);
    // the source may be the installed copy itself, or a link to it: replacing it would lose it
    if dst.exists() && std::fs::canonicalize(&folder).ok() == std::fs::canonicalize(&dst).ok() { bail!("{} is the installed copy of '{name}' already", folder.display()); }
    // copy beside the installed version, then swap: a copy that fails halfway leaves the old skill working
    let fresh = store.join(format!(".{name}.new"));
    let old = store.join(format!(".{name}.old"));
    for d in [&fresh, &old] { if d.exists() { std::fs::remove_dir_all(d)?; } }
    if let Err(e) = copy_dir(&folder, &fresh) {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(e.context(format!("copy {}; the installed '{name}' is unchanged", folder.display())));
    }
    if dst.exists() { std::fs::rename(&dst, &old).with_context(|| format!("replace {} (is a file in it open?)", dst.display()))?; }
    if let Err(e) = std::fs::rename(&fresh, &dst) {
        if old.exists() { let _ = std::fs::rename(&old, &dst); }
        return Err(anyhow::Error::from(e).context(format!("install {}", dst.display())));
    }
    if old.exists() { let _ = std::fs::remove_dir_all(&old); }
    let _ = std::fs::remove_dir_all(&staged);
    let skill = Skill { name: name.clone(), source: source.into(), hash: dir_hash(&dst) };
    reg.skills.retain(|s| s.name != name);
    reg.skills.push(skill.clone());
    Ok(skill)
}

/// Downloaded repositories, one download per repository and branch while checking or listing many skills.
pub struct Repos { dir: PathBuf, got: std::collections::HashMap<String, std::result::Result<PathBuf, String>> }
impl Repos {
    pub fn new(dir: PathBuf) -> Repos { let _ = std::fs::remove_dir_all(&dir); Repos { dir, got: Default::default() } }
    /// The extracted top folder of `owner/repo` at `branch` ("HEAD": the default branch).
    pub fn top(&mut self, owner: &str, repo: &str, branch: &str) -> Result<PathBuf> {
        let key = format!("{owner}/{repo}@{branch}");
        if !self.got.contains_key(&key) {
            let into = self.dir.join(self.got.len().to_string());
            let r = download_repo(owner, repo, branch, &into).map_err(|e| format!("{e:#}"));
            self.got.insert(key.clone(), r);
        }
        self.got[&key].clone().map_err(anyhow::Error::msg)
    }
}
impl Drop for Repos { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }

fn download_repo(owner: &str, repo: &str, branch: &str, into: &Path) -> Result<PathBuf> {
    // HRM_GITHUB_CODELOAD points the download at a mock server in tests
    let base = std::env::var("HRM_GITHUB_CODELOAD").unwrap_or_else(|_| "https://codeload.github.com".into());
    let zip_url = format!("{base}/{owner}/{repo}/zip/{}", if branch == "HEAD" { "HEAD".to_string() } else { format!("refs/heads/{branch}") });
    let bytes = reqwest::blocking::Client::new().get(&zip_url).header("User-Agent", "Harness Manager").send()?.error_for_status()?.bytes()?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))?;
    std::fs::create_dir_all(into)?;
    zip.extract(into)?;
    std::fs::read_dir(into)?.flatten().find(|e| e.path().is_dir()).map(|e| e.path()).context("empty archive")
}

fn fetch_github_dir(url: &str, repos: &mut Repos) -> Result<PathBuf> {
    let rest = url.trim_start_matches("https://github.com/").trim_end_matches('/');
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 { bail!("not a GitHub repository URL: {url}"); }
    let (owner, repo) = (parts[0], parts[1].trim_end_matches(".git"));
    // `tree/release/v2/skills/x`: the branch may be `release` or `release/v2`, so each split is tried in turn
    let splits: Vec<(String, String)> = if parts.len() >= 4 && parts[2] == "tree" {
        (4..=parts.len()).map(|k| (parts[3..k].join("/"), parts[k..].join("/"))).collect()
    } else {
        vec![("HEAD".into(), String::new())]
    };
    let mut first_err = None;
    for (branch, sub) in splits {
        match fetch_github_ref(owner, repo, &branch, &sub, repos) {
            Ok(dir) => return Ok(dir),
            Err(e) => { first_err.get_or_insert(e); }
        }
    }
    Err(first_err.unwrap().context(format!("could not fetch a skill from {url}")))
}

fn fetch_github_ref(owner: &str, repo: &str, branch: &str, sub: &str, repos: &mut Repos) -> Result<PathBuf> {
    let top = repos.top(owner, repo, branch)?;
    let dir = if sub.is_empty() { top } else { top.join(sub.replace('/', std::path::MAIN_SEPARATOR_STR)) };
    if !dir.join("SKILL.md").exists() { bail!("no SKILL.md in branch {branch} at {}", if sub.is_empty() { "the top" } else { sub }); }
    Ok(dir)
}

/// Skills whose source changed since install (GitHub sources are fetched again; local folders re-hashed).
pub fn check_skill_updates(ctx: &Ctx, reg: &Registry) -> Vec<(String, Option<bool>)> {
    let mut out = vec![];
    let mut repos = Repos::new(ctx.data.join("skill-check"));
    for s in &reg.skills {
        let fresh = if s.source.starts_with("https://github.com/") {
            fetch_github_dir(&s.source, &mut repos).map(|d| dir_hash(&d)).ok()
        } else {
            let p = PathBuf::from(&s.source);
            p.is_dir().then(|| dir_hash(&p))
        };
        let installed = dir_hash(&skill_store(ctx).join(&s.name));
        out.push((s.name.clone(), fresh.map(|h| h != installed)));
    }
    out
}

/// Skill folders in the store that the list does not have yet (put there by hand, another tool, or a list that
/// lost them), with the tools whose main profile already links to each.
pub fn unmanaged_skills(ctx: &Ctx, reg: &Registry) -> Vec<(String, Vec<String>)> {
    let store = skill_store(ctx);
    let Ok(rd) = std::fs::read_dir(&store) else { return vec![] };
    let mut names: Vec<String> = rd.flatten().filter(|e| e.path().join("SKILL.md").is_file())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| !n.starts_with('.') && valid_name(n) && !reg.skills.iter().any(|s| &s.name == n)).collect();
    names.sort();
    let dirs = linked_dirs(ctx, reg);
    names.into_iter().map(|n| {
        let target = std::fs::canonicalize(store.join(&n)).ok();
        let linked = dirs.iter().filter(|(_, d)| { let p = d.join(&n); crate::shared::is_link(&p) && std::fs::canonicalize(&p).ok() == target })
            .map(|(h, _)| h.clone()).collect();
        (n, linked)
    }).collect()
}

/// Each tool's main skills folder (the folder a tick of that tool writes to).
fn linked_dirs(ctx: &Ctx, reg: &Registry) -> Vec<(String, PathBuf)> {
    reg.profiles.iter().filter(|p| p.main).filter_map(|p| harness::of(p).item_paths(ctx, p).skills.map(|d| (p.harness.clone(), d))).collect()
}

/// Add store folders to the list. A tool that already links to one gets its tick, and that link becomes
/// Harness Manager's to keep in step (unticking removes it, as for any skill installed here).
pub fn adopt_skills(ctx: &Ctx, reg: &mut Registry, names: &[String]) -> Result<Vec<String>> {
    let found = unmanaged_skills(ctx, reg);
    let dirs = linked_dirs(ctx, reg);
    let mut done = vec![];
    for (name, linked) in found.into_iter().filter(|(n, _)| names.is_empty() || names.contains(n)) {
        reg.skills.push(Skill { name: name.clone(), source: "store".into(), hash: dir_hash(&skill_store(ctx).join(&name)) });
        for h in &linked {
            let t = Target { kind: "skill".into(), item: name.clone(), harness: h.clone(), profile: None };
            if !reg.targets.contains(&t) { reg.targets.push(t); }
            if let Some((_, d)) = dirs.iter().find(|(x, _)| x == h) { ctx.db.ledger_set(&d.display().to_string(), "skill", &name, true)?; }
        }
        done.push(name);
    }
    Ok(done)
}

/// The `name` and `description` in a SKILL.md's front matter (YAML between `---` lines; folded values joined).
pub fn skill_front(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") { return (None, None); }
    let body: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let field = |key: &str| -> Option<String> {
        let i = body.iter().position(|l| l.starts_with(&format!("{key}:")))?;
        let first = body[i][key.len() + 1..].trim();
        let v = if first.is_empty() || first == ">" || first == "|" || first == ">-" || first == "|-" {
            body[i + 1..].iter().take_while(|l| l.starts_with(' ') || l.starts_with('\t')).map(|l| l.trim()).collect::<Vec<_>>().join(" ")
        } else { first.to_string() };
        let v = v.trim_matches(|c| c == '"' || c == '\'').trim().to_string();
        (!v.is_empty()).then_some(v)
    };
    (field("name"), field("description"))
}

/// The repositories Discover lists: the user's, or these when they have none.
pub fn skill_repos(reg: &Registry) -> Vec<SkillRepo> {
    if !reg.skill_repos.is_empty() { return reg.skill_repos.clone(); }
    [("anthropics", "skills"), ("ComposioHQ", "awesome-claude-skills"), ("cexll", "myclaude")]
        .iter().map(|(o, n)| SkillRepo { owner: (*o).into(), name: (*n).into(), branch: "HEAD".into() }).collect()
}

/// Every skill in a repository: each folder with a SKILL.md, with its name, description and install URL.
pub fn repo_skills(repos: &mut Repos, r: &SkillRepo) -> Result<Vec<Value>> {
    let top = repos.top(&r.owner, &r.name, &r.branch)?;
    let mut files = vec![];
    crate::util::walk_files(&top, &|f: &Path| f.file_name().map(|n| n == "SKILL.md").unwrap_or(false), &mut files);
    let mut out = vec![];
    for f in files {
        let dir = f.parent().unwrap_or(&top);
        let sub = dir.strip_prefix(&top).unwrap_or(Path::new("")).to_string_lossy().replace('\\', "/");
        let (name, desc) = skill_front(&std::fs::read_to_string(&f).unwrap_or_default());
        let folder = dir.file_name().and_then(|n| n.to_str()).unwrap_or(&r.name).to_string();
        // the folder name is what it installs as; the front matter's name is only shown
        let url = if sub.is_empty() { format!("https://github.com/{}/{}", r.owner, r.name) } else { format!("https://github.com/{}/{}/tree/{}/{sub}", r.owner, r.name, r.branch) };
        out.push(json!({"folder": folder, "name": name.unwrap_or_else(|| folder.clone()), "description": desc, "path": sub, "url": url}));
    }
    out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(out)
}

/// Move a harness's own skill folders into the store and link them back, so they can be shared.
/// Returns the imported names, and the names left alone because the store has a different skill by that name.
pub fn import_skills(ctx: &Ctx, reg: &mut Registry, harness_id: &str) -> Result<(Vec<String>, Vec<String>)> {
    let Some(main) = reg.profiles.iter().find(|p| p.harness == harness_id && p.main).cloned() else { bail!("no main {harness_id} profile") };
    let Some(dir) = harness::of(&main).item_paths(ctx, &main).skills else { return Ok((vec![], vec![])) };
    let store = skill_store(ctx);
    std::fs::create_dir_all(&store)?;
    let target = dir.display().to_string();
    let (mut added, mut kept) = (vec![], vec![]);
    for e in std::fs::read_dir(&dir).map(|r| r.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let p = e.path();
        if !p.is_dir() || crate::shared::is_link(&p) || !p.join("SKILL.md").exists() { continue; }
        let name = e.file_name().to_string_lossy().to_string();
        if !valid_name(&name) { continue; }
        let dst = store.join(&name);
        if dst.exists() && dir_hash(&dst) != dir_hash(&p) {
            kept.push(name); // never replace the harness's own version with a different one
            continue;
        }
        if !dst.exists() { copy_dir(&p, &dst)?; }
        std::fs::remove_dir_all(&p)?;
        crate::shared::link_dir(&dst, &p)?;
        if !reg.skills.iter().any(|s| s.name == name) {
            reg.skills.push(Skill { name: name.clone(), source: "imported".into(), hash: dir_hash(&dst) });
        }
        let t = Target { kind: "skill".into(), item: name.clone(), harness: harness_id.into(), profile: None };
        if !reg.targets.contains(&t) { reg.targets.push(t); }
        ctx.db.ledger_set(&target, "skill", &name, true)?;
        added.push(name);
    }
    Ok((added, kept))
}

// ---------- prompts ----------

fn prompt_files(ctx: &Ctx, reg: &Registry, harness_id: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![];
    for p in reg.profiles.iter().filter(|p| p.harness == harness_id) {
        if let Some(f) = harness::of(p).item_paths(ctx, p).prompt {
            // a Claude profile whose CLAUDE.md imports the main one follows it
            let follows = std::fs::read_to_string(&f).map(|t| t.trim_start().starts_with('@') && t.lines().count() <= 2).unwrap_or(false);
            if !follows && !files.contains(&f) { files.push(f); }
        }
    }
    files
}

/// Enable a prompt. Every prompt file it replaces is saved into the library first unless the library
/// already holds that text or Harness Manager wrote it itself, so no edit and no account's own file is lost.
pub fn enable_prompt(ctx: &Ctx, reg: &mut Registry, id: &str) -> Result<()> {
    let harness_id = reg.prompts.iter().find(|p| p.id == id).map(|p| p.harness.clone()).context("no such prompt")?;
    let files = prompt_files(ctx, reg, &harness_id);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let mut cur_taken = false;
    for (n, f) in files.iter().enumerate() {
        let live = crate::util::read_or_empty(f)?;
        let ours = ctx.db.kv_get(&format!("prompt:{}", f.display()))?.as_deref() == Some(live.as_str());
        if live.trim().is_empty() || ours || reg.prompts.iter().any(|p| p.harness == harness_id && p.body == live) { continue; }
        // an edit made outside the app: the first one updates the prompt that was enabled, unless that is the one being written
        match reg.prompts.iter_mut().find(|p| p.harness == harness_id && p.enabled && p.id != id) {
            Some(cur) if !cur_taken => { cur.body = live; cur_taken = true; }
            _ => reg.prompts.push(Prompt { id: format!("saved-{}-{n}", crate::util::now_ms()), harness: harness_id.clone(),
                title: format!("Saved {stamp} from {}", f.display()), body: live, enabled: false }),
        }
    }
    let body = reg.prompts.iter().find(|p| p.id == id).map(|p| p.body.clone()).unwrap();
    for f in &files {
        write_atomic(f, body.as_bytes())?;
        ctx.db.kv_set(&format!("prompt:{}", f.display()), &body)?;
    }
    for p in reg.prompts.iter_mut().filter(|p| p.harness == harness_id) { p.enabled = p.id == id; }
    Ok(())
}

pub fn add_prompt(reg: &mut Registry, harness_id: &str, title: &str, body: &str) -> Result<String> {
    if harness::by_id(harness_id).is_none() { bail!("unknown harness '{harness_id}'"); }
    let id = format!("p{}", crate::util::now_ms());
    reg.prompts.push(Prompt { id: id.clone(), harness: harness_id.into(), title: title.into(), body: body.into(), enabled: false });
    Ok(id)
}
