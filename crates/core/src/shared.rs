//! The shared layer: a Claude profile uses the main profile's skills, plugins, rules, CLAUDE.md, settings and
//! MCP servers, while its login, history and usage stay its own.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::util::write_atomic;
use crate::Ctx;

/// Folders linked to the main profile. `projects/` is deliberately not linked, so usage stays per account.
pub const LINKED: &[&str] = &["skills", "commands", "agents", "plugins", "rules", "output-styles"];

/// Settings that point at a login or another home; never mirrored into a profile.
const STRIP_ENV: &[&str] = &["CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_OAUTH_TOKEN"];
const STRIP_KEYS: &[&str] = &["apiKeyHelper", "forceLoginMethod", "forceLoginOrgUUID"];

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub linked: Vec<String>,
    /// Folders the profile already has with its own content; left alone.
    pub kept: Vec<String>,
    pub settings_keys: usize,
    pub mcp_servers: usize,
}

#[derive(Default, Serialize, Deserialize)]
struct MirrorState {
    #[serde(default)]
    settings: Map<String, Value>,
    #[serde(default)]
    mcp: Vec<String>,
    /// The config last mirrored for each of `mcp`: a server whose config differs now was changed by the profile.
    #[serde(default)]
    mcp_cfg: Map<String, Value>,
}

pub fn link_dir(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(windows)]
    { junction::create(src, dst)?; }
    #[cfg(unix)]
    { std::os::unix::fs::symlink(src, dst)?; }
    Ok(())
}

pub fn is_link(p: &Path) -> bool {
    #[cfg(windows)]
    { junction::exists(p).unwrap_or(false) || std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false) }
    #[cfg(unix)]
    { std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false) }
}

pub fn remove_link(p: &Path) -> Result<()> {
    #[cfg(windows)]
    { if junction::exists(p).unwrap_or(false) { junction::delete(p)?; } std::fs::remove_dir(p).or_else(|_| std::fs::remove_file(p))?; }
    #[cfg(unix)]
    { std::fs::remove_file(p)?; }
    Ok(())
}

fn strip(settings: &Map<String, Value>) -> Map<String, Value> {
    let mut s = settings.clone();
    for k in STRIP_KEYS { s.remove(*k); }
    if let Some(Value::Object(env)) = s.get_mut("env") {
        for k in STRIP_ENV { env.remove(*k); }
    }
    s
}

/// A JSON object file, None when it is missing or empty. Claude Code rewrites these files while it runs, so a
/// half-written file is read again for a moment; if it stays broken this fails instead of rebuilding the file
/// from scratch, which would lose the login and settings in it.
fn read_obj(path: &Path) -> Result<Option<Map<String, Value>>> {
    for _ in 0..10 {
        let text = crate::util::read_or_empty(path)?;
        if !text.trim().is_empty() {
            if let Ok(Value::Object(m)) = serde_json::from_str(&text) { return Ok(Some(m)); }
        } else if !path.exists() {
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let text = crate::util::read_or_empty(path)?;
    if text.trim().is_empty() { return Ok(None); }
    anyhow::bail!("{} is not a valid JSON object; left unchanged", path.display())
}

/// Bring one Claude profile home in step with the main profile. Safe to run again at any time.
pub fn sync_claude(_ctx: &Ctx, main_home: &Path, main_json: &Path, prof: &Path) -> Result<SyncReport> {
    let mut rep = SyncReport::default();
    std::fs::create_dir_all(prof)?;

    for d in LINKED {
        let src = main_home.join(d);
        let dst = prof.join(d);
        if !src.is_dir() || is_link(&dst) {
            if is_link(&dst) { rep.linked.push(d.to_string()); }
            continue;
        }
        let empty = std::fs::read_dir(&dst).map(|mut r| r.next().is_none()).unwrap_or(false);
        if dst.exists() && !empty {
            rep.kept.push(d.to_string());
            continue;
        }
        if empty { std::fs::remove_dir(&dst)?; }
        link_dir(&src, &dst)?;
        rep.linked.push(d.to_string());
    }

    // CLAUDE.md: one import line, unless the profile wrote its own
    let main_md = main_home.join("CLAUDE.md");
    let own_md = prof.join("CLAUDE.md");
    if main_md.exists() {
        let import = format!("@{}\n", main_md.display());
        let own = std::fs::read_to_string(&own_md).ok();
        let main_text = std::fs::read_to_string(&main_md).unwrap_or_default();
        if own.is_none() || own.as_deref() == Some(main_text.as_str()) {
            write_atomic(&own_md, import.as_bytes())?;
        }
    }

    let state_path = prof.join(".mirror-state.json");
    let mut state: MirrorState = match read_obj(&state_path)? {
        Some(m) => serde_json::from_value(Value::Object(m)).with_context(|| format!("{} is damaged; left unchanged", state_path.display()))?,
        None => MirrorState::default(),
    };

    // settings.json: per-key three-way merge; keys the profile changed itself win
    let main_settings = read_obj(&main_home.join("settings.json"))?.map(|m| strip(&m)).unwrap_or_default();
    let own_path = prof.join("settings.json");
    let own_sig = crate::util::file_sig(&own_path);
    let mut own = read_obj(&own_path)?.unwrap_or_default();
    let mut keys: Vec<String> = main_settings.keys().cloned().collect();
    keys.extend(state.settings.keys().cloned());
    keys.sort();
    keys.dedup();
    let mut changed = false;
    for k in keys {
        let base = state.settings.get(&k);
        let cur = own.get(&k);
        let fresh = cur.is_none() && base.is_none();
        if cur != base && !fresh { continue; }
        match main_settings.get(&k) {
            Some(v) => { if cur != Some(v) { own.insert(k.clone(), v.clone()); changed = true; } rep.settings_keys += 1; }
            None => { if own.remove(&k).is_some() { changed = true; } }
        }
    }
    let unchanged_since_read = crate::util::file_sig(&own_path) == own_sig;
    if (changed || !own_path.exists() && !own.is_empty()) && unchanged_since_read {
        write_atomic(&own_path, serde_json::to_string_pretty(&Value::Object(own))?.as_bytes())?;
    }
    if unchanged_since_read {
        state.settings = main_settings;
        // #100: saved now, so a failure in the MCP part below cannot leave these keys unsynced for good
        write_atomic(&state_path, serde_json::to_string_pretty(&state)?.as_bytes())?;
    }

    // MCP servers: mirror the main list; remove only names this mirror wrote before
    let main_mcp = read_obj(main_json)?.and_then(|v| v.get("mcpServers").and_then(|m| m.as_object().cloned())).unwrap_or_default();
    let pj_path = prof.join(".claude.json");
    let pj_sig = crate::util::file_sig(&pj_path);
    let pj_read = read_obj(&pj_path)?;
    // an existing file that reads as empty may be one Claude Code is about to write again: leave it this time
    if pj_read.is_none() && pj_path.exists() { return Ok(rep); }
    let mut pj = pj_read.unwrap_or_else(|| {
        let mut m = Map::new();
        m.insert("hasCompletedOnboarding".into(), Value::Bool(true));
        m
    });
    let mut servers = pj.get("mcpServers").and_then(|m| m.as_object().cloned()).unwrap_or_default();
    let before = servers.clone();
    // ours: this mirror wrote it and the profile has not changed it since (older states kept no configs)
    let ours = |name: &str, cur: &Value| state.mcp.iter().any(|n| n == name) && state.mcp_cfg.get(name).map(|l| l == cur).unwrap_or(true);
    let mut written = vec![];
    let mut written_cfg = Map::new();
    for (name, cfg) in &main_mcp {
        match servers.get(name) {
            // the profile's own server with that name, or its own copy of the same one: never claimed
            Some(existing) if !ours(name, existing) => continue,
            _ => {
                servers.insert(name.clone(), cfg.clone());
                written.push(name.clone());
                written_cfg.insert(name.clone(), cfg.clone());
            }
        }
    }
    for name in &state.mcp {
        if !main_mcp.contains_key(name) && servers.get(name).map(|cur| ours(name, cur)).unwrap_or(false) { servers.remove(name); }
    }
    rep.mcp_servers = written.len();
    if crate::util::file_sig(&pj_path) != pj_sig { return Ok(rep); } // Claude Code wrote it meanwhile: next time
    if servers != before || !pj_path.exists() {
        pj.insert("mcpServers".into(), Value::Object(servers));
        write_atomic(&pj_path, serde_json::to_string_pretty(&Value::Object(pj))?.as_bytes())?;
    }
    state.mcp = written;
    state.mcp_cfg = written_cfg;
    write_atomic(&state_path, serde_json::to_string_pretty(&state)?.as_bytes())?;
    Ok(rep)
}

/// Replace folders the old PowerShell launcher copied into a profile with links to the main profile.
/// Copies are moved to `<profile>/.hrm-backup/<name>` first.
pub fn relink_copies(main_home: &Path, prof: &Path) -> Result<Vec<String>> {
    let mut done = vec![];
    for d in LINKED {
        let src = main_home.join(d);
        let dst = prof.join(d);
        if !src.is_dir() || !dst.exists() || is_link(&dst) { continue; }
        let backup = prof.join(".hrm-backup");
        std::fs::create_dir_all(&backup)?;
        let mut target = backup.join(d);
        if target.exists() { target = backup.join(format!("{d}-{}", crate::util::now_ms())); }
        std::fs::rename(&dst, &target)?;
        link_dir(&src, &dst)?;
        done.push(d.to_string());
    }
    Ok(done)
}
