//! Backups of what Harness Manager owns: the account list, settings, what it wrote where (the ledger and
//! its saved state) and the folders of the skills it manages. Keys and logins stay in Windows Credential
//! Manager and are never in a backup. A backup is one zip in `<data>/backups`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::util::{now_ms, valid_name, write_atomic};
use crate::Ctx;

#[derive(Debug, Clone, Serialize)]
pub struct Backup {
    pub name: String,
    pub label: String,
    pub at: i64,
    pub size: u64,
}

pub fn dir(ctx: &Ctx) -> PathBuf { ctx.data.join("backups") }

/// State worth keeping; caches (discover, pricing, version checks), a running router's pid and its token are not:
/// the token is a secret, and a new one is made when the router starts without one.
fn keep_kv(k: &str) -> bool {
    !(k.starts_with("discover:") || k.starts_with("pricing:") || k.starts_with("apps:") || k.starts_with("self:")
        || k == "router:pid" || k == "router:token" || k.starts_with("breaker:"))
}

#[cfg(test)]
mod keep_tests {
    #[test]
    fn the_router_token_and_caches_stay_out_of_backups() {
        for k in ["router:token", "router:pid", "breaker:x", "apps:latest", "self:latest", "pricing:models"] { assert!(!super::keep_kv(k), "{k}"); }
        for k in ["live_quota", "migrated:codex-timing", "mcp:last_import"] { assert!(super::keep_kv(k), "{k}"); }
    }
}

/// "2026-11-02 03.15.20 Before restore.zip": the time it was made, then its label.
fn file_name(at: i64, label: &str) -> String {
    let t = chrono::DateTime::from_timestamp_millis(at).unwrap_or_default().with_timezone(&chrono::Local);
    format!("{} {}.zip", t.format("%Y-%m-%d %H.%M.%S"), label)
}

fn parse_name(name: &str) -> Option<(i64, String)> {
    let stem = name.strip_suffix(".zip")?;
    let (stamp, label) = (stem.get(..19)?, stem.get(20..).unwrap_or("").to_string());
    let t = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H.%M.%S").ok()?;
    let at = t.and_local_timezone(chrono::Local).single()?.timestamp_millis();
    Some((at, label))
}

fn clean_label(label: &str) -> Result<String> {
    let l = label.trim();
    if l.is_empty() || l.len() > 60 || l.chars().any(|c| "\\/:*?\"<>|".contains(c) || c.is_control()) {
        bail!("a label of up to 60 characters, without \\ / : * ? \" < > |");
    }
    Ok(l.to_string())
}

/// Newest first.
pub fn list(ctx: &Ctx) -> Vec<Backup> {
    let Ok(rd) = std::fs::read_dir(dir(ctx)) else { return vec![] };
    let mut out: Vec<Backup> = rd.flatten().filter_map(|e| {
        let name = e.file_name().to_str()?.to_string();
        let (at, label) = parse_name(&name)?;
        Some(Backup { size: e.metadata().map(|m| m.len()).unwrap_or(0), name, label, at })
    }).collect();
    out.sort_by(|a, b| b.at.cmp(&a.at).then(b.name.cmp(&a.name)));
    out
}

/// Write the zip of what Harness Manager owns now.
fn write_zip(ctx: &Ctx, out: &Path) -> Result<()> {
    let reg_text = crate::util::read_or_empty(&ctx.registry_path())?;
    let reg = ctx.registry()?;
    let state = ctx.db.with(|c| {
        let ledger: Vec<Value> = c.prepare("select target, kind, item from ledger")?
            .query_map([], |r| Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?])))?.collect::<rusqlite::Result<_>>()?;
        let kv: serde_json::Map<String, Value> = c.prepare("select k, v from kv")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter().filter(|(k, _)| keep_kv(k)).map(|(k, v)| (k, json!(v))).collect();
        Ok(json!({"ledger": ledger, "kv": kv}))
    })?;
    if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
    let tmp = out.with_extension("zip.part");
    {
        let f = std::fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("profiles.json", opts)?;
        z.write_all(reg_text.as_bytes())?;
        z.start_file("settings.json", opts)?;
        z.write_all(serde_json::to_string_pretty(&ctx.settings)?.as_bytes())?;
        z.start_file("state.json", opts)?;
        z.write_all(serde_json::to_string_pretty(&state)?.as_bytes())?;
        // the skills it manages, so a lost store folder can come back
        let store = crate::items::skill_store(ctx);
        for s in &reg.skills {
            let root = store.join(&s.name);
            let mut files = vec![];
            crate::util::walk_files(&root, &|_| true, &mut files);
            for f in files {
                let rel = f.strip_prefix(&root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
                z.start_file(format!("skills/{}/{rel}", s.name), opts)?;
                z.write_all(&std::fs::read(&f).unwrap_or_default())?;
            }
        }
        z.finish()?;
    }
    std::fs::rename(&tmp, out).with_context(|| format!("save {}", out.display()))?;
    Ok(())
}

/// Make a backup with this label ("Automatic", "Manual", "Before restore", ...).
pub fn create(ctx: &Ctx, label: &str) -> Result<Backup> {
    let label = clean_label(label)?;
    let mut at = now_ms();
    // two in the same second get different names
    while dir(ctx).join(file_name(at, &label)).exists() { at += 1000; }
    let name = file_name(at, &label);
    write_zip(ctx, &dir(ctx).join(&name))?;
    crate::log::info(&format!("backup made: {name}"));
    list(ctx).into_iter().find(|b| b.name == name).context("backup not found after writing it")
}

/// Keep the newest `keep`; delete the rest.
pub fn prune(ctx: &Ctx, keep: usize) -> Result<()> {
    for b in list(ctx).into_iter().skip(keep.max(1)) { std::fs::remove_file(dir(ctx).join(&b.name))?; }
    Ok(())
}

/// The automatic backup, when the newest is older than `hours`.
pub fn auto(ctx: &Ctx) -> Result<Option<Backup>> {
    let hours = ctx.settings.backup_hours as i64;
    if hours == 0 { return Ok(None); }
    if list(ctx).first().map(|b| now_ms() - b.at < hours * 3_600_000).unwrap_or(false) { return Ok(None); }
    let b = create(ctx, "Automatic")?;
    prune(ctx, ctx.settings.backup_keep as usize)?;
    let _ = ctx.db.event("ok", "backup", &format!("Automatic backup made: {:.1} MB, {} kept.", b.size as f64 / 1_048_576.0, ctx.settings.backup_keep));
    Ok(Some(b))
}

fn checked(ctx: &Ctx, name: &str) -> Result<PathBuf> {
    if parse_name(name).is_none() || name.contains(['/', '\\']) { bail!("no backup named {name}"); }
    let p = dir(ctx).join(name);
    if !p.is_file() { bail!("no backup named {name}"); }
    Ok(p)
}

pub fn delete(ctx: &Ctx, name: &str) -> Result<()> { Ok(std::fs::remove_file(checked(ctx, name)?)?) }

pub fn rename(ctx: &Ctx, name: &str, label: &str) -> Result<String> {
    let p = checked(ctx, name)?;
    let (at, _) = parse_name(name).unwrap();
    let to = file_name(at, &clean_label(label)?);
    if dir(ctx).join(&to).exists() { bail!("a backup named {to} exists"); }
    std::fs::rename(&p, dir(ctx).join(&to))?;
    Ok(to)
}

/// Put a backup back: the account list, settings, ledger and saved state, and the skill folders that are
/// missing from the store (folders still there are left as they are). The state before is backed up first.
/// Returns what was put back. The settings take effect when the context is opened again.
pub fn restore_file(ctx: &Ctx, zip_path: &Path) -> Result<Value> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path).with_context(|| format!("open {}", zip_path.display()))?)
        .context("not a Harness Manager backup (not a zip)")?;
    let mut text = |name: &str| -> Result<Option<String>> {
        match z.by_name(name) {
            Ok(mut f) => { let mut s = String::new(); f.read_to_string(&mut s)?; Ok(Some(s)) }
            Err(_) => Ok(None),
        }
    };
    let reg_text = text("profiles.json")?.context("not a Harness Manager backup: it has no profiles.json")?;
    // a list that does not parse is not written over the one in use
    let _: crate::registry::Registry = serde_json::from_str(&reg_text).context("the backup's account list is not valid")?;
    let settings = text("settings.json")?;
    let state: Value = text("state.json")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    create(ctx, "Before restore")?;
    write_atomic(&ctx.registry_path(), reg_text.as_bytes())?;
    if let Some(s) = settings {
        let _: crate::ctx::Settings = serde_json::from_str(&s).context("the backup's settings are not valid")?;
        write_atomic(&ctx.data.join("settings.json"), s.as_bytes())?;
    }
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        if let Some(rows) = state["ledger"].as_array() {
            tx.execute("delete from ledger", [])?;
            for r in rows {
                tx.execute("insert or replace into ledger values(?,?,?)", rusqlite::params![r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or(""), r[2].as_str().unwrap_or("")])?;
            }
        }
        for (k, v) in state["kv"].as_object().cloned().unwrap_or_default() {
            if keep_kv(&k) { tx.execute("insert or replace into kv values(?,?)", rusqlite::params![k, v.as_str().unwrap_or("")])?; }
        }
        tx.commit()?;
        Ok(())
    })?;
    // skill folders: only the missing ones come back
    let store = crate::items::skill_store(ctx);
    let mut skills = std::collections::BTreeSet::new();
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let Some(rest) = f.name().ok().and_then(|n| n.strip_prefix("skills/").map(String::from)) else { continue };
        let Some((skill, rel)) = rest.split_once('/') else { continue };
        if !valid_name(skill) || rel.is_empty() || rel.split('/').any(|p| p == ".." || p.is_empty()) { continue; }
        if store.join(skill).exists() && !skills.contains(skill) { continue; }
        let out = store.join(skill).join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
        let mut b = vec![];
        f.read_to_end(&mut b)?;
        std::fs::write(&out, b)?;
        skills.insert(skill.to_string());
    }
    crate::log::info(&format!("restored {} ({} skill folders put back)", zip_path.display(), skills.len()));
    Ok(json!({"skills": skills}))
}

pub fn restore(ctx: &Ctx, name: &str) -> Result<Value> { let p = checked(ctx, name)?; restore_file(ctx, &p) }

/// A backup of the current state, written to a file the user chose.
pub fn export(ctx: &Ctx, to: &Path) -> Result<()> { write_zip(ctx, to) }
