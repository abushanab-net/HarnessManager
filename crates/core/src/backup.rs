//! Backups of what Harness Manager owns: the account list, settings, what it wrote where (the ledger and
//! its saved state) and the folders of the skills it manages. Keys and logins stay in Windows Credential
//! Manager and are never in a backup. A backup is one zip in `<data>/backups`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{types::ValueRef, Connection};
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
/// the token is a secret, and a new one is made when the router starts without one. The journal's marks describe
/// the outbox, which is not in a backup either.
fn keep_kv(k: &str) -> bool {
    !(k.starts_with("discover:") || k.starts_with("pricing:") || k.starts_with("apps:") || k.starts_with("self:")
        || k == "router:pid" || k == "router:token" || k.starts_with("breaker:") || k.starts_with("journal:") || k.starts_with("computers:"))
}

#[cfg(test)]
mod keep_tests {
    use super::*;

    #[test]
    fn the_router_token_and_caches_stay_out_of_backups() {
        for k in ["router:token", "router:pid", "breaker:x", "apps:latest", "self:latest", "pricing:models", "journal:published_seq", "journal:backfilled", "computers:last"] { assert!(!keep_kv(k), "{k}"); }
        for k in ["live_quota", "migrated:codex-timing", "mcp:last_import", "migrated:device"] { assert!(keep_kv(k), "{k}"); }
    }

    #[test]
    fn another_computers_backup_brings_its_preferences_but_not_its_identity() {
        let t = tempfile::tempdir().unwrap();
        let mut a = Ctx::open_at(&t.path().join("A"), &t.path().join("home-a")).unwrap();
        a.settings.theme = "dark".into();
        a.settings.sync_mode = "folder".into();
        std::fs::create_dir_all(t.path().join("A's folder")).unwrap();
        a.settings.sync_folder = t.path().join("A's folder").display().to_string();
        a.settings.device_name = "A".into();
        a.save_settings().unwrap();
        let a = Ctx::open_at(&t.path().join("A"), &t.path().join("home-a")).unwrap();
        a.save_registry(&a.registry().unwrap()).unwrap();
        let zip = dir(&a).join(create(&a, "Manual").unwrap().name);
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        let (id, inc, name) = (b.settings.device_id.clone(), b.settings.incarnation.clone(), b.settings.device_name.clone());
        restore_file(&b, &zip).unwrap();
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        assert_eq!(b.settings.theme, "dark", "the preference came");
        assert_eq!((b.settings.device_id, b.settings.incarnation, b.settings.device_name), (id, inc, name), "still itself");
        assert_eq!((b.settings.sync_mode.as_str(), b.settings.sync_folder.as_str()), ("off", ""), "and not in A's folder");
    }

    #[test]
    fn the_zip_holds_the_history_files_and_with_the_switch_the_transcripts() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let row = crate::model::UsageRow { request_id: "claude:1".into(), profile: "work".into(), harness: "claude".into(), ts: 1_000, model: "m".into(), project: "C:\\p".into(),
            session_id: "s1".into(), input: 100, output: 10, cache_read: 0, cache_write: 0, ttft_ms: None, gen_ms: None };
        ctx.db.insert_usage(&[row], &Default::default()).unwrap();
        ctx.db.tag_set("project", "k", &["x".into()]).unwrap();
        ctx.db.quota_point("work", "week", 10.0, None, 1_000).unwrap();
        let src = t.path().join("s1.jsonl");
        std::fs::write(&src, "{\"type\":\"user\"}
").unwrap();
        let m = crate::model::SessionMeta { harness: "claude".into(), profile: "work".into(), id: "s1".into(), source_path: src.display().to_string(), ..Default::default() };
        ctx.db.put_session(&format!("claude|work|{}", src.display()), &m, 1, 1).unwrap();
        let names = |b: &Backup| -> Vec<String> {
            let mut z = zip::ZipArchive::new(std::fs::File::open(dir(&ctx).join(&b.name)).unwrap()).unwrap();
            (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect()
        };
        let plain = create(&ctx, "Manual").unwrap();
        let n = names(&plain);
        for f in ["profiles.json", "settings.json", "state.json", "history/usage.jsonl", "history/sessions.jsonl", "history/quota_hist.jsonl", "history/tags.jsonl", "history/archived.jsonl", "history/peers.json", "history/device.json"] { assert!(n.contains(&f.to_string()), "{f} in {n:?}"); }
        assert!(!n.iter().any(|f| f.starts_with("transcripts/")), "no transcripts by default: {n:?}");
        let mut z = zip::ZipArchive::new(std::fs::File::open(dir(&ctx).join(&plain.name)).unwrap()).unwrap();
        let mut usage = String::new();
        z.by_name("history/usage.jsonl").unwrap().read_to_string(&mut usage).unwrap();
        let u: Value = serde_json::from_str(usage.lines().next().unwrap()).unwrap();
        assert_eq!((u["request_id"].as_str(), u["device"].as_str(), u["input"].as_i64()), (Some("claude:1"), Some(ctx.settings.device_id.as_str()), Some(100)));
        let mut dev = String::new();
        z.by_name("history/device.json").unwrap().read_to_string(&mut dev).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&dev).unwrap()["name"], ctx.settings.device_name);
        let with = create_with(&ctx, "Manual", true).unwrap();
        assert!(names(&with).contains(&"transcripts/claude/work/s1.jsonl.zst".to_string()), "{:?}", names(&with));
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
pub fn list(ctx: &Ctx) -> Vec<Backup> { list_in(&dir(ctx)) }

fn list_in(dir: &Path) -> Vec<Backup> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Backup> = rd.flatten().filter_map(|e| {
        let name = e.file_name().to_str()?.to_string();
        let (at, label) = parse_name(&name)?;
        Some(Backup { size: e.metadata().map(|m| m.len()).unwrap_or(0), name, label, at })
    }).collect();
    out.sort_by(|a, b| b.at.cmp(&a.at).then(b.name.cmp(&a.name)));
    out
}

/// Every row of a query as one JSON object per line, keyed by column name.
fn rows_jsonl(c: &Connection, sql: &str) -> rusqlite::Result<String> {
    let mut st = c.prepare(sql)?;
    let names: Vec<String> = st.column_names().map(String::from).collect();
    let mut out = String::new();
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let mut o = serde_json::Map::new();
        for (i, n) in names.iter().enumerate() {
            o.insert(n.clone(), match r.get_ref(i)? {
                ValueRef::Integer(i) => json!(i),
                ValueRef::Real(f) => json!(f),
                ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                _ => Value::Null,
            });
        }
        out.push_str(&Value::Object(o).to_string());
        out.push('\n');
    }
    Ok(out)
}

/// The history a restore merges: every usage row, session row, limit reading and tag this computer knows (its own
/// and the merged ones, each with its computer), the archived sessions, the other computers (names, certificates
/// and addresses, never keys) and this computer's record. Keys and logins stay out, as everywhere.
fn history(ctx: &Ctx) -> Result<Vec<(&'static str, String)>> {
    // read before the database is held below: the key is kept in it
    let device = json!({"id": ctx.settings.device_id, "name": ctx.settings.device_name, "incarnation": ctx.settings.incarnation, "pubkey": crate::sync::peer::identity::pubkey(ctx)});
    ctx.db.with(|c| {
        crate::archive::tables(c)?;
        let peers: Vec<Value> = rows_jsonl(c, "select device_id, name, os, app_version, incarnation, address, port, cert_pem, pubkey, roots from peers")?
            .lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        Ok(vec![
            ("history/usage.jsonl", rows_jsonl(c, "select request_id, profile, harness, ts, model, project, session_id, input, output, cache_read, cache_write, ttft_ms, gen_ms, device, cost_usd, price_rev from usage")?),
            ("history/sessions.jsonl", rows_jsonl(c, "select key, profile, harness, path, mtime, len, json, device from sessions")?),
            ("history/quota_hist.jsonl", rows_jsonl(c, "select ts, profile, tier, used, resets_at from quota_hist order by ts")?),
            ("history/tags.jsonl", rows_jsonl(c, "select kind, key, tag from tags order by rowid")?),
            ("history/archived.jsonl", rows_jsonl(c, "select key, harness, profile, id, device, json, path, bytes, at from archived")?),
            ("history/peers.json", serde_json::to_string_pretty(&peers)?),
            ("history/device.json", serde_json::to_string_pretty(&device)?),
        ])
    })
}

/// This computer's transcripts as the folder carries them: `transcripts/<harness>/<profile>/<id>.jsonl.zst`
/// for every session whose file is still in the tool's folder (archived ones are already compressed, in the archive).
fn transcripts(ctx: &Ctx) -> Result<Vec<(String, Vec<u8>)>> {
    let rows: Vec<String> = ctx.db.with(|c| {
        let mut st = c.prepare("select json from sessions where device = ? and instr(key, '|') > 0 and harness in ('claude', 'codex', 'pi', 'omp')")?;
        let v = st.query_map([&ctx.db.device], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let mut out = vec![];
    for j in rows {
        let m: Value = serde_json::from_str(&j).unwrap_or(Value::Null);
        let (h, p, id, path) = (m["harness"].as_str().unwrap_or(""), m["profile"].as_str().unwrap_or(""), m["id"].as_str().unwrap_or(""), Path::new(m["source_path"].as_str().unwrap_or("")));
        if id.is_empty() || id.contains(['/', '\\']) || !path.is_file() { continue; }
        let Ok(text) = crate::harness::parse::read_text(path) else { continue };
        out.push((format!("transcripts/{h}/{p}/{id}.jsonl.zst"), zstd::encode_all(text.as_bytes(), 3)?));
    }
    Ok(out)
}

/// Write the zip of what Harness Manager owns now; with `with_transcripts`, this computer's transcripts too.
fn write_zip(ctx: &Ctx, out: &Path, with_transcripts: bool) -> Result<()> {
    let reg_text = crate::util::read_or_empty(&ctx.registry_path())?;
    let reg = ctx.registry()?;
    let state = ctx.db.with(|c| {
        let ledger: Vec<Value> = c.prepare("select target, kind, item from ledger")?
            .query_map([], |r| Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?])))?.collect::<rusqlite::Result<_>>()?;
        let kv: serde_json::Map<String, Value> = c.prepare("select k, v from kv")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter().filter(|(k, _)| keep_kv(k)).map(|(k, v)| (k, json!(v))).collect();
        let tags: Vec<Value> = c.prepare("select kind, key, tag from tags order by rowid")?
            .query_map([], |r| Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?])))?.collect::<rusqlite::Result<_>>()?;
        Ok(json!({"ledger": ledger, "kv": kv, "tags": tags}))
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
        for (name, text) in history(ctx)? {
            z.start_file(name, opts)?;
            z.write_all(text.as_bytes())?;
        }
        if with_transcripts {
            for (name, bytes) in transcripts(ctx)? {
                z.start_file(name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
                z.write_all(&bytes)?;
            }
        }
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

/// Make a backup with this label ("Automatic", "Manual", "Before restore", ...), with transcripts as the setting says.
pub fn create(ctx: &Ctx, label: &str) -> Result<Backup> { create_with(ctx, label, ctx.settings.backup_transcripts) }

pub fn create_with(ctx: &Ctx, label: &str, with_transcripts: bool) -> Result<Backup> {
    let label = clean_label(label)?;
    let mut at = now_ms();
    // two in the same second get different names
    while dir(ctx).join(file_name(at, &label)).exists() { at += 1000; }
    let name = file_name(at, &label);
    write_zip(ctx, &dir(ctx).join(&name), with_transcripts)?;
    crate::log::info(&format!("backup made: {name}"));
    // a copy beside the journal, so a fresh computer finds it; a folder out of reach does not fail the backup
    if let Err(e) = copy_to_folder(ctx, &name) { crate::log::warn(&format!("backup: the copy into the sync folder: {e:#}")); }
    list(ctx).into_iter().find(|b| b.name == name).context("backup not found after writing it")
}

/// `<sync_folder>/backups/<device_id>`, when backups go there.
fn folder_dir(ctx: &Ctx) -> Option<PathBuf> {
    let s = &ctx.settings;
    (s.backup_in_sync_folder && s.sync_mode == "folder" && !s.sync_folder.is_empty() && Path::new(&s.sync_folder).is_dir())
        .then(|| Path::new(&s.sync_folder).join("backups").join(&s.device_id))
}

/// The zip into the sync folder, keeping `backup_keep` of this computer's there.
fn copy_to_folder(ctx: &Ctx, name: &str) -> Result<()> {
    let Some(d) = folder_dir(ctx) else { return Ok(()) };
    std::fs::create_dir_all(&d)?;
    let tmp = d.join(format!("{name}.part"));
    std::fs::copy(dir(ctx).join(name), &tmp)?;
    std::fs::rename(&tmp, d.join(name))?;
    for b in list_in(&d).into_iter().skip((ctx.settings.backup_keep as usize).max(1)) { std::fs::remove_file(d.join(&b.name))?; }
    Ok(())
}

/// Every computer's backups in the sync folder (`backups/<device_id>/*.zip`), newest first, with the computer's name.
pub fn in_sync_folder(ctx: &Ctx) -> Vec<Value> {
    let s = &ctx.settings;
    if s.sync_mode != "folder" || s.sync_folder.is_empty() { return vec![]; }
    let names = crate::journal::device_names(ctx);
    let Ok(rd) = std::fs::read_dir(Path::new(&s.sync_folder).join("backups")) else { return vec![] };
    let mut out: Vec<(i64, Value)> = rd.flatten().filter(|e| e.path().is_dir()).flat_map(|e| {
        let id = e.file_name().to_string_lossy().to_string();
        let name = names.get(&id).cloned().unwrap_or_else(|| id.clone());
        list_in(&e.path()).into_iter().map(|b| (b.at, json!({"name": b.name, "label": b.label, "at": b.at, "size": b.size, "device": id, "device_name": name, "path": e.path().join(&b.name)}))).collect::<Vec<_>>()
    }).collect();
    out.sort_by_key(|x| std::cmp::Reverse(x.0));
    out.into_iter().map(|(_, v)| v).collect()
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
/// Returns what was put back. The settings take effect when the context is opened again; who this computer is and
/// how it syncs stay as they are: a backup from another computer must not make this one a copy of it.
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
        let mut b: crate::ctx::Settings = serde_json::from_str(&s).context("the backup's settings are not valid")?;
        let here = ctx.settings.clone();
        (b.device_id, b.device_name, b.incarnation, b.registry_path) = (here.device_id, here.device_name, here.incarnation, here.registry_path);
        (b.sync_mode, b.sync_folder, b.hub_url, b.sync_transcripts, b.sync_presence, b.direct_connection, b.listen_port) =
            (here.sync_mode, here.sync_folder, here.hub_url, here.sync_transcripts, here.sync_presence, here.direct_connection, here.listen_port);
        write_atomic(&ctx.data.join("settings.json"), serde_json::to_string_pretty(&b)?.as_bytes())?;
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
        if let Some(rows) = state["tags"].as_array() {
            tx.execute("delete from tags", [])?;
            for r in rows { tx.execute("insert or ignore into tags values(?,?,?)", rusqlite::params![r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or(""), r[2].as_str().unwrap_or("")])?; }
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
pub fn export(ctx: &Ctx, to: &Path) -> Result<()> { write_zip(ctx, to, ctx.settings.backup_transcripts) }
