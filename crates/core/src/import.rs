//! First-run setup: main profiles for installed harnesses, and the account list of the old
//! `claude-accounts.ps1` PowerShell launcher.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::harness;
use crate::registry::{Kind, Profile, Registry};
use crate::util::{read_json, valid_name};
use crate::Ctx;

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub main_profiles: Vec<String>,
    pub accounts: Vec<String>,
    pub keys: Vec<String>,
    pub skipped: Vec<String>,
}

/// Add a main profile for each harness whose home exists on this computer.
pub fn detect_main_profiles(ctx: &Ctx, reg: &mut Registry, rep: &mut ImportReport) {
    for h in harness::all() {
        if reg.profiles.iter().any(|p| p.harness == h.id() && p.main) { continue; }
        if !h.default_home(ctx).exists() { continue; }
        let name = format!("{}-main", h.id());
        if reg.profile(&name).is_some() { continue; }
        let p = Profile { name: name.clone(), label: None, harness: h.id().into(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 };
        if reg.add_profile(p).is_ok() { rep.main_profiles.push(name); }
    }
}

pub fn default_ps_registry(ctx: &Ctx) -> PathBuf {
    ctx.home.join("OneDrive").join("Documents").join("PowerShell").join("claude-accounts.json")
}

/// Import `{accounts:[{name,email}], apiKeys:[{name,computer,key}]}` from the PowerShell launcher.
/// Its account folders (`~/.claude-<name>`) are the same ones Harness Manager uses, so logins carry over.
pub fn import_powershell(_ctx: &Ctx, reg: &mut Registry, path: &Path, rep: &mut ImportReport) -> Result<()> {
    let Some(v) = read_json(path) else { anyhow::bail!("could not read {}", path.display()) };
    for a in v["accounts"].as_array().cloned().unwrap_or_default() {
        let Some(name) = a["name"].as_str() else { continue };
        if reg.profile(name).is_some() { rep.skipped.push(format!("{name}: already exists")); continue; }
        let p = Profile { name: name.into(), label: None, harness: "claude".into(), kind: Kind::Subscription, email: a["email"].as_str().map(String::from).filter(|e| !e.is_empty()), main: false, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 };
        match reg.add_profile(p) {
            Ok(()) => rep.accounts.push(name.into()),
            Err(e) => rep.skipped.push(format!("{name}: {e}")),
        }
    }
    let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
    for k in v["apiKeys"].as_array().cloned().unwrap_or_default() {
        let raw = k["name"].as_str().unwrap_or("");
        let name: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        if !valid_name(&name) { rep.skipped.push(format!("key {raw}: not a usable account name")); continue; }
        if k["computer"].as_str() != Some(computer.as_str()) {
            rep.skipped.push(format!("key {raw}: saved on another computer"));
            continue;
        }
        let secret = match crate::secrets::dpapi_unprotect_hex(k["key"].as_str().unwrap_or("")) {
            Ok(s) => s,
            Err(e) => { rep.skipped.push(format!("key {raw}: {e}")); continue; }
        };
        if reg.profile(&name).is_some() { rep.skipped.push(format!("key {raw}: a profile named {name} exists")); continue; }
        if let Err(e) = crate::secrets::set_key(&crate::secrets::key_target(&name), &secret) {
            rep.skipped.push(format!("key {raw}: {e:#}"));
            continue;
        }
        reg.add_profile(Profile { name: name.clone(), label: None, harness: "claude".into(), kind: Kind::ApiKey, email: None, main: false, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 })?;
        rep.keys.push(name);
    }
    Ok(())
}

/// Run once when the registry is empty: main profiles plus the PowerShell launcher's accounts if found.
pub fn first_run(ctx: &Ctx) -> Result<Option<ImportReport>> {
    let mut reg = ctx.registry()?;
    if !reg.profiles.is_empty() { return Ok(None); }
    let mut rep = ImportReport::default();
    detect_main_profiles(ctx, &mut reg, &mut rep);
    let ps = default_ps_registry(ctx);
    if ps.exists() {
        if let Err(e) = import_powershell(ctx, &mut reg, &ps, &mut rep) { rep.skipped.push(format!("{e:#}")); }
    }
    ctx.save_registry(&reg)?;
    for p in &reg.profiles {
        let _ = harness::of(p).prepare(ctx, p);
    }
    Ok(Some(rep))
}
