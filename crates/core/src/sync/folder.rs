//! The "folder" transport: a shared folder (OneDrive, Google Drive, SMB, a NAS) holding one folder per
//! computer: its journal chunks and manifest, what it knows (`device.json`) and what runs on it (`presence.json`).
//! Nothing in it is encrypted: the folder is the owner's.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::projects::Resolver;
use crate::util::{now_ms, read_json, write_atomic};
use crate::{harness, Ctx};

/// The sync folder, checked to be there: a drive that is not mounted must not look like an empty one.
pub fn root(ctx: &Ctx) -> Result<PathBuf> {
    let p = PathBuf::from(&ctx.settings.sync_folder);
    if ctx.settings.sync_folder.is_empty() || !p.is_absolute() { bail!("choose the folder to sync through in Settings > Computers"); }
    if !p.is_dir() { bail!("the sync folder {} is not reachable", p.display()); }
    Ok(p)
}

/// This computer's folder in it.
pub fn mine(ctx: &Ctx, root: &Path) -> PathBuf { root.join("devices").join(&ctx.settings.device_id) }

/// Every other computer's folder: (device id, folder).
pub fn others(ctx: &Ctx, root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(root.join("devices")) else { return vec![] };
    let mut v: Vec<(String, PathBuf)> = rd.flatten().filter(|e| e.path().is_dir()).filter_map(|e| {
        let id = e.file_name().to_str()?.to_string();
        (id != ctx.settings.device_id).then(|| (id, e.path()))
    }).collect();
    v.sort();
    v
}

pub fn write_chunk(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> { write_atomic(&dir.join("journal").join(name), bytes) }

pub fn read_manifest(dir: &Path) -> Option<Value> { read_json(&dir.join("manifest.json")) }

pub fn write_manifest(dir: &Path, m: &Value) -> Result<()> { write_atomic(&dir.join("manifest.json"), serde_json::to_string_pretty(m)?.as_bytes()) }

/// `device.json`: who this computer is and where its projects live, so "resume here" can map them later.
pub fn write_device(ctx: &Ctx, root: &Path) -> Result<()> {
    let reg = ctx.registry()?;
    let dirs: Vec<String> = ctx.db.with(|c| {
        let mut st = c.prepare("select distinct project from usage where device = ? and project <> ''")?;
        let v = st.query_map([&ctx.db.device], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let mut res = Resolver::new(&reg.projects);
    let mut roots = serde_json::Map::new();
    for d in dirs {
        let info = res.resolve(&d);
        roots.entry(info.key).or_insert(json!(info.root));
    }
    let v = json!({"id": ctx.settings.device_id, "name": ctx.settings.device_name, "os": std::env::consts::OS, "app_version": env!("CARGO_PKG_VERSION"),
        "incarnation": ctx.settings.incarnation, "roots": roots, "updated_at": now_ms()});
    write_atomic(&mine(ctx, root).join("device.json"), serde_json::to_string_pretty(&v)?.as_bytes())
}

/// What runs here now and the last five minutes' pace, for the other computers' live views.
pub fn presence(ctx: &Ctx) -> Result<Value> {
    let reg = ctx.registry()?;
    let mut res = Resolver::new(&reg.projects);
    // each account looks for its consoles side by side, as the account list does
    let found: Vec<(&crate::registry::Profile, Vec<crate::RunningConsole>)> = std::thread::scope(|s| {
        let hs: Vec<_> = reg.profiles.iter().map(|p| s.spawn(move || (p, harness::of(p).running(ctx, p)))).collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let running: Vec<Value> = found.iter().flat_map(|(p, rs)| rs.iter().map(|r| {
        let info = res.resolve(r.cwd.as_deref().unwrap_or(""));
        json!({"profile": p.name, "harness": p.harness, "project": info.key, "project_name": info.name, "session_id": r.session_id, "cwd": r.cwd, "started_at": r.started_at})
    }).collect::<Vec<_>>()).collect();
    let now = now_ms();
    let (accounts, total) = ctx.db.with(|c| {
        let mut st = c.prepare("select profile, sum(input + output + cache_read + cache_write), count(*) from usage where ts >= ? and device = ? group by profile")?;
        let mut total = 0i64;
        let accounts: serde_json::Map<String, Value> = st.query_map(rusqlite::params![now - 300_000, ctx.db.device], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?
            .flatten().map(|(p, t, n)| { total += t; (p, json!({"tokens_5m": t, "requests_5m": n})) }).collect();
        Ok((accounts, total))
    })?;
    Ok(json!({"at": now, "app_version": env!("CARGO_PKG_VERSION"), "running": running, "accounts": accounts, "tps_5m": total as f64 / 300.0}))
}

pub fn write_presence(ctx: &Ctx, root: &Path) -> Result<()> {
    if !ctx.settings.sync_presence { return Ok(()); }
    write_atomic(&mine(ctx, root).join("presence.json"), presence(ctx)?.to_string().as_bytes())
}
