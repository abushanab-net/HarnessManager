//! Restore is a merge: the history in a backup zip, or in a sync folder (every computer's journal, the backups and
//! the archive kept there), goes into this database by the rules the sync merge uses, so restoring twice, or over a
//! live database, adds nothing. With `adopt`, this computer takes the restored computer's id and name and carries
//! on with a new incarnation; without it, the restored computer stays a separate past computer.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::params;
use serde_json::{json, Value};

use crate::db::{quota_point_in, upsert_session, upsert_usage};
use crate::model::{SessionMeta, UsageRow};
use crate::util::{now_ms, walk_files, write_atomic};
use crate::Ctx;

/// A backup zip, or a sync folder.
#[derive(Debug, Clone)]
pub enum Source { Zip(PathBuf), Folder(PathBuf) }

impl Source {
    /// `{zip?, folder?}` as the ops and the CLI give it.
    pub fn parse(a: &Value) -> Result<Source> {
        match (a["zip"].as_str().filter(|s| !s.is_empty()), a["folder"].as_str().filter(|s| !s.is_empty())) {
            (Some(z), _) => Ok(Source::Zip(PathBuf::from(z))),
            (_, Some(f)) => Ok(Source::Folder(PathBuf::from(f))),
            _ => bail!("give a backup zip or a sync folder"),
        }
    }

    /// A path: a folder, or a zip file.
    pub fn of_path(p: &Path) -> Result<Source> {
        if p.is_dir() { Ok(Source::Folder(p.to_path_buf())) }
        else if p.is_file() { Ok(Source::Zip(p.to_path_buf())) }
        else { bail!("{} is not a folder or a zip", p.display()) }
    }
}

/// Everything a source holds, keyed so that reading it twice (a journal and the backup of the same computer) keeps one.
#[derive(Default)]
struct History {
    /// request_id -> the row with `device`, `cost_usd`, `price_rev`.
    usage: HashMap<String, Value>,
    /// journal key -> (meta, device); the newest `last_active_at` wins.
    sessions: HashMap<String, (SessionMeta, String)>,
    /// (profile, tier, used, resets_at, at)
    quotas: Vec<(String, String, f64, Option<i64>, i64)>,
    /// "kind/key" -> tags
    tags: HashMap<String, Vec<String>>,
    peers: Vec<Value>,
    /// (id, name) of every computer the source describes
    devices: Vec<(String, String)>,
    /// rows of the `archived` table: {key, harness, profile, id, device, json, path, bytes, at}
    archived: HashMap<String, Value>,
    /// the zip's account list
    profiles: Vec<Value>,
    /// (device, incarnation, last seq) of every journal read from a folder
    cursors: Vec<(String, String, i64)>,
}

impl History {
    fn usage(&mut self, row: Value) {
        if let Some(id) = row["request_id"].as_str() { self.usage.insert(id.to_string(), row); }
    }
    fn session(&mut self, key: &str, m: SessionMeta, device: &str) {
        match self.sessions.get(key) {
            Some((old, _)) if old.last_active_at >= m.last_active_at => {}
            _ => { self.sessions.insert(key.to_string(), (m, device.to_string())); }
        }
    }
    fn archived(&mut self, row: Value) {
        if let Some(k) = row["key"].as_str() { self.archived.entry(k.to_string()).or_insert(row); }
    }
    fn device(&mut self, id: &str, name: &str) {
        if !id.is_empty() && !self.devices.iter().any(|(i, _)| i == id) { self.devices.push((id.to_string(), name.to_string())); }
    }
}

fn lines(text: &str) -> impl Iterator<Item = Value> + '_ { text.lines().filter_map(|l| serde_json::from_str(l).ok()) }

/// A backup zip's `history/*` and its account list.
fn read_zip(path: &Path, h: &mut History) -> Result<()> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?).context("not a Harness Manager backup (not a zip)")?;
    let mut text = |name: &str| -> Option<String> {
        let mut f = z.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let dev = text("history/device.json").and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
    if dev.is_null() && text("history/usage.jsonl").is_none() { bail!("{} holds no history (a backup from before 1.6 only carries settings and the account list)", path.display()); }
    h.device(dev["id"].as_str().unwrap_or(""), dev["name"].as_str().unwrap_or(""));
    let me = dev["id"].as_str().unwrap_or("").to_string();
    for row in lines(&text("history/usage.jsonl").unwrap_or_default()) { h.usage(row); }
    for row in lines(&text("history/sessions.jsonl").unwrap_or_default()) {
        // the scan's cache rows (key with '|') name the file on that computer; restored under their journal key, as sessions of it
        let Ok(m) = serde_json::from_str::<SessionMeta>(row["json"].as_str().unwrap_or("")) else { continue };
        if m.id.is_empty() { continue; }
        let device = row["device"].as_str().filter(|d| !d.is_empty()).unwrap_or(&me).to_string();
        let key = crate::db::session_key(&m);
        h.session(&key, m, &device);
    }
    for row in lines(&text("history/quota_hist.jsonl").unwrap_or_default()) {
        h.quotas.push((row["profile"].as_str().unwrap_or("").into(), row["tier"].as_str().unwrap_or("").into(), row["used"].as_f64().unwrap_or(0.0), row["resets_at"].as_i64(), row["ts"].as_i64().unwrap_or(0)));
    }
    for row in lines(&text("history/tags.jsonl").unwrap_or_default()) {
        h.tags.entry(format!("{}/{}", row["kind"].as_str().unwrap_or(""), row["key"].as_str().unwrap_or(""))).or_default().push(row["tag"].as_str().unwrap_or("").into());
    }
    for row in lines(&text("history/archived.jsonl").unwrap_or_default()) { h.archived(row); }
    if let Some(p) = text("history/peers.json").and_then(|s| serde_json::from_str::<Vec<Value>>(&s).ok()) {
        for peer in p { if let Some(n) = peer["name"].as_str() { h.device(peer["device_id"].as_str().unwrap_or(""), n); } h.peers.push(peer); }
    }
    if let Some(reg) = text("profiles.json").and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
        h.profiles.extend(reg["profiles"].as_array().cloned().unwrap_or_default());
    }
    Ok(())
}

/// A sync folder: every computer's journal in manifest order, the newest backup of each, and the archive's metas.
fn read_folder(root: &Path, h: &mut History) -> Result<()> {
    let devices = root.join("devices");
    if !devices.is_dir() && !root.join("backups").is_dir() && !root.join("archive").is_dir() { bail!("{} is not a sync folder (no devices, backups or archive in it)", root.display()); }
    for e in std::fs::read_dir(&devices).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
        let (id, dir) = (e.file_name().to_string_lossy().to_string(), e.path());
        let dev = crate::util::read_json(&dir.join("device.json")).unwrap_or(Value::Null);
        h.device(&id, dev["name"].as_str().unwrap_or(""));
        let Some(man) = crate::sync::folder::read_manifest(&dir) else { continue };
        let inc = man["incarnation"].as_str().unwrap_or("").to_string();
        // how far the sync merge need not go again: up to the first chunk not read here, which it must still fetch
        let (mut last, mut gap) = (0, false);
        for ch in man["chunks"].as_array().into_iter().flatten() {
            let file = ch["file"].as_str().unwrap_or("");
            if file.is_empty() || file.contains(['/', '\\']) { continue; }
            // a OneDrive placeholder, a half-copied file: not this chunk, by what its manifest says of it
            let bytes = std::fs::read(dir.join("journal").join(file)).ok().filter(|b| crate::util::sha256_hex(b) == ch["sha256"].as_str().unwrap_or(""));
            let Some(text) = bytes.and_then(|b| String::from_utf8(b).ok()) else {
                crate::log::warn(&format!("restore: {file} of {id} could not be read yet; the next sync rounds bring it"));
                gap = true;
                continue;
            };
            for e in text.lines().filter_map(|l| serde_json::from_str::<crate::journal::Entry>(l).ok()) {
                if !gap { last = last.max(e.seq); }
                match e.op.as_str() {
                    "usage" => { let mut d = e.data; d["device"] = json!(id); h.usage(d); }
                    "session" => { if let Ok(m) = serde_json::from_value::<SessionMeta>(e.data) { h.session(&e.key, m, &id); } }
                    "quota" => { if let Some((p, t)) = e.key.split_once('/') { h.quotas.push((p.into(), t.into(), e.data["used"].as_f64().unwrap_or(0.0), e.data["resets_at"].as_i64(), e.data["at"].as_i64().unwrap_or(e.ts))); } }
                    "tag" => { h.tags.insert(e.key.clone(), e.data["tags"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).collect()); }
                    "tombstone" => { if let Some(k) = e.key.strip_prefix("session/") { h.sessions.remove(k); } }
                    _ => {}
                }
            }
        }
        if !inc.is_empty() { h.cursors.push((id.clone(), inc, last)); }
    }
    for e in std::fs::read_dir(root.join("backups")).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
        // the newest backup of each computer: what it had before sync, its limit history and its archive table
        let mut zips: Vec<PathBuf> = std::fs::read_dir(e.path()).into_iter().flatten().flatten().map(|f| f.path()).filter(|p| p.extension().map(|x| x == "zip").unwrap_or(false)).collect();
        zips.sort();
        if let Some(z) = zips.last() { if let Err(err) = read_zip(z, h) { crate::log::warn(&format!("restore: {}: {err:#}", z.display())); } }
    }
    let mut metas = vec![];
    walk_files(&root.join("archive"), &|f| f.extension().map(|e| e == "json").unwrap_or(false), &mut metas);
    for f in metas {
        let Some(v) = crate::util::read_json(&f) else { continue };
        let Ok(mut m) = serde_json::from_value::<SessionMeta>(v["session"].clone()) else { continue };
        let copy = f.with_extension("jsonl.zst");
        if m.id.is_empty() || !copy.is_file() { continue; }
        m.source_path = copy.display().to_string();
        m.archived = true;
        let key = crate::db::session_key(&m);
        h.archived(json!({"key": key, "harness": m.harness, "profile": m.profile, "id": m.id, "device": v["device"].as_str().unwrap_or(""), "json": serde_json::to_string(&m).unwrap_or_default(),
            "path": m.source_path, "bytes": v["bytes"].as_i64().unwrap_or(0), "at": v["archived_at"].as_i64().unwrap_or(0)}));
    }
    Ok(())
}

fn read(src: &Source) -> Result<History> {
    let mut h = History::default();
    match src {
        Source::Zip(p) => read_zip(p, &mut h)?,
        Source::Folder(p) => read_folder(p, &mut h)?,
    }
    Ok(h)
}

/// The record in the source that is this computer's past self: another id with this computer's name. A record
/// with this computer's own id is this computer as it is (it writes one into the folder the moment it is chosen).
fn this_computer(ctx: &Ctx, h: &History) -> Option<(String, String)> {
    h.devices.iter().find(|(id, n)| *id != ctx.settings.device_id && n.eq_ignore_ascii_case(&ctx.settings.device_name)).cloned()
}

fn counts(ctx: &Ctx, h: &History, src: &Source) -> Value {
    let mut per: HashMap<&str, (i64, i64)> = HashMap::new();
    for r in h.usage.values() { per.entry(r["device"].as_str().unwrap_or("")).or_default().0 += 1; }
    for (_, d) in h.sessions.values() { per.entry(d.as_str()).or_default().1 += 1; }
    let computers: Vec<Value> = h.devices.iter().map(|(id, name)| { let c = per.get(id.as_str()).copied().unwrap_or_default(); json!({"id": id, "name": name, "requests": c.0, "sessions": c.1}) }).collect();
    let tokens: i64 = h.usage.values().map(|r| ["input", "output", "cache_read", "cache_write"].iter().map(|k| r[k].as_i64().unwrap_or(0)).sum::<i64>()).sum();
    let mut accounts: Vec<&str> = h.quotas.iter().map(|q| q.0.as_str()).collect();
    accounts.sort();
    accounts.dedup();
    let newest = h.usage.values().filter_map(|r| r["ts"].as_i64()).chain(h.sessions.values().map(|(m, _)| m.last_active_at)).max();
    let (kind, path) = match src { Source::Zip(p) => ("zip", p), Source::Folder(p) => ("folder", p) };
    json!({"source": kind, "path": path, "computers": computers, "requests": h.usage.len(), "tokens": tokens, "sessions": h.sessions.len(), "accounts": accounts.len(),
        "archived": h.archived.len(), "newest": newest, "this": this_computer(ctx, h).map(|(id, name)| json!({"id": id, "name": name, "can_adopt": crate::sync::peer::identity::has_key(&id)})),
        "empty": h.usage.is_empty() && h.sessions.is_empty() && h.archived.is_empty() && h.quotas.is_empty()})
}

/// What a source holds, before anything is merged.
pub fn preview(ctx: &Ctx, src: &Source) -> Result<Value> { Ok(counts(ctx, &read(src)?, src)) }

/// Another computer answers to this id now: heard from or connected within five minutes.
fn live_peer(ctx: &Ctx, id: &str) -> Result<bool> {
    let seen: Option<i64> = ctx.db.with(|c| Ok(c.query_row("select max(coalesce(connected, 0), coalesce(last_seen, 0)) from peers where device_id=?", [id], |r| r.get(0)).ok()))?;
    Ok(seen.map(|t| now_ms() - t < 300_000).unwrap_or(false))
}

/// The merge. Returns what was added: `{usage, sessions, quota, tags, peers, archived, profiles, adopted, device}`.
pub fn run(ctx: &Ctx, src: &Source, adopt: bool) -> Result<Value> {
    let h = read(src)?;
    let me = ctx.settings.device_id.clone();
    let this = this_computer(ctx, &h);
    let adopted = if adopt {
        let (id, name) = this.clone().context("no record of this computer's name in it: restore as a new computer instead")?;
        if live_peer(ctx, &id)? { bail!("another computer uses this identity"); }
        // the key is not in any backup: under a new one every other computer would refuse this one for good
        if !crate::sync::peer::identity::has_key(&id) { bail!("this computer does not hold the signing key of {name}, so the other computers would not accept it as {name}: restore as a new computer instead"); }
        Some((id, name))
    } else { None };
    let mut quotas = h.quotas.clone();
    quotas.sort_by_key(|q| q.4);
    let mut added = ctx.db.with(|c| {
        crate::archive::tables(c)?;
        let tx = c.transaction()?;
        let mut n = [0usize; 6];
        for r in h.usage.values() {
            let row: UsageRow = serde_json::from_value(r.clone()).context("usage row")?;
            n[0] += upsert_usage(&tx, &row, r["device"].as_str().unwrap_or(""), r["cost_usd"].as_f64(), r["price_rev"].as_str())?;
        }
        for (key, (m, device)) in &h.sessions { n[1] += upsert_session(&tx, key, m, device)?; }
        for (p, t, used, resets, at) in &quotas {
            if quota_point_in(&tx, p, t, *used, *resets, *at, None)? { n[2] += 1; }
        }
        for (kk, tags) in &h.tags {
            let Some((kind, key)) = kk.split_once('/') else { continue };
            if tx.prepare("select 1 from tags where kind=? and key=?")?.exists([kind, key])? { continue; }
            for t in tags { n[3] += tx.execute("insert or ignore into tags values(?,?,?)", [kind, key, t.as_str()])?; }
        }
        for p in &h.peers {
            let id = p["device_id"].as_str().unwrap_or("");
            if id.is_empty() || id == me || adopted.as_ref().map(|(a, _)| a == id).unwrap_or(false) { continue; }
            n[4] += tx.execute("insert or ignore into peers(device_id, name, os, app_version, incarnation, address, port, cert_pem, pubkey, roots, transport) values(?,?,?,?,?,?,?,?,?,?,'folder')",
                params![id, p["name"].as_str(), p["os"].as_str(), p["app_version"].as_str(), p["incarnation"].as_str(), p["address"].as_str(), p["port"].as_i64(), p["cert_pem"].as_str(), p["pubkey"].as_str(), p["roots"].as_str()])?;
        }
        for (id, name) in &h.devices {
            if id == &me || adopted.as_ref().map(|(a, _)| a == id).unwrap_or(false) { continue; }
            n[4] += tx.execute("insert or ignore into peers(device_id, name, transport) values(?,?,'folder')", params![id, name])?;
        }
        for r in h.archived.values() {
            n[5] += tx.execute("insert or ignore into archived(key, harness, profile, id, device, json, path, bytes, at) values(?,?,?,?,?,?,?,?,?)",
                params![r["key"].as_str(), r["harness"].as_str(), r["profile"].as_str(), r["id"].as_str(), r["device"].as_str().unwrap_or(""), r["json"].as_str(), r["path"].as_str(), r["bytes"].as_i64().unwrap_or(0), r["at"].as_i64().unwrap_or(0)])?;
        }
        // the sync merge need not apply again what came in here
        for (dev, inc, seq) in &h.cursors {
            if dev != &me { tx.execute("insert into cursors2 values(?,?,?) on conflict(device_id, incarnation) do update set seq=max(seq, excluded.seq)", params![dev, inc, seq])?; }
        }
        tx.commit()?;
        Ok(json!({"usage": n[0], "sessions": n[1], "quota": n[2], "tags": n[3], "peers": n[4], "archived": n[5]}))
    })?;
    // the account list: accounts not here by name come back (no keys or logins: each asks to log in when first used)
    let mut profiles = 0;
    if !h.profiles.is_empty() {
        // the caller holds the registry lock, as for every command that changes the list
        let mut reg = ctx.registry()?;
        for p in &h.profiles {
            let Ok(p) = serde_json::from_value::<crate::registry::Profile>(p.clone()) else { continue };
            if reg.profiles.iter().any(|q| q.name == p.name) || crate::harness::by_id(&p.harness).is_none() { continue; }
            if reg.add_profile(p).is_ok() { profiles += 1; }
        }
        if profiles > 0 { ctx.save_registry(&reg)?; }
    }
    added["profiles"] = json!(profiles);
    added["adopted"] = json!(adopted.is_some());
    added["device"] = this.map(|(id, name)| json!({"id": id, "name": name})).unwrap_or(Value::Null);
    if let Some((id, name)) = adopted {
        let mut s = ctx.settings.clone();
        ctx.db.with(|c| {
            for t in ["usage", "sessions", "archived"] { c.execute(&format!("update {t} set device=?1 where device=?2"), [&id, &me])?; }
            c.execute("delete from peers where device_id=?", [&id])?;
            c.execute("delete from kv where k in ('device:pubkey', 'journal:published_seq')", [])?;
            // the sequence starts again under the new incarnation, so the others see a continuation
            c.execute("insert or replace into kv values('journal:backfilled', '0')", [])?;
            Ok(())
        })?;
        s.device_id = id.clone();
        s.device_name = name.clone();
        s.incarnation = crate::util::uuid4();
        write_atomic(&ctx.data.join("settings.json"), serde_json::to_string_pretty(&s)?.as_bytes())?;
        crate::log::info(&format!("restore: this computer continues as {name} ({id}), incarnation {}", s.incarnation));
    }
    crate::log::info(&format!("restore: {added}"));
    let _ = ctx.db.event("ok", "restore", &format!("Restored: {} requests, {} sessions and {} archived transcripts added.", added["usage"], added["sessions"], added["archived"]));
    Ok(added)
}

/// Welcome back: a sync folder is set and nothing has been published from here yet (`journal:backfilled` absent) and
/// the question was not answered before: the folder's preview when it holds history, else null.
pub fn welcome(ctx: &Ctx) -> Result<Value> {
    let s = &ctx.settings;
    if s.sync_mode != "folder" || s.sync_folder.is_empty() || !Path::new(&s.sync_folder).is_dir() { return Ok(Value::Null); }
    if ctx.db.kv_get("journal:backfilled")?.is_some() || ctx.db.kv_get("welcome:done")?.is_some() { return Ok(Value::Null); }
    let p = preview(ctx, &Source::Folder(PathBuf::from(&s.sync_folder)))?;
    Ok(if p["empty"] == true { Value::Null } else { p })
}

/// The question was answered (restored or not): not asked again at launch.
pub fn welcome_done(ctx: &Ctx) -> Result<()> { ctx.db.kv_set("welcome:done", "1") }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::Prices;

    fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| { crate::archive::tables(c)?; Ok(c.query_row(sql, [], |r| r.get(0))?) }).unwrap() }

    fn row(id: &str, ts: i64) -> UsageRow {
        UsageRow { request_id: id.into(), profile: "work".into(), harness: "claude".into(), ts, model: "claude-opus-5-5".into(), project: "C:\\p".into(),
            session_id: "s1".into(), input: 100, output: 10, cache_read: 50, cache_write: 0, ttft_ms: Some(900), gen_ms: None }
    }

    /// A computer named `name` with two requests, one session, a tag, two limit readings and an archived session.
    fn computer(dir: &Path, name: &str) -> Ctx {
        let mut ctx = Ctx::open_at(&dir.join(name), &dir.join("home")).unwrap();
        ctx.settings.device_name = name.into();
        ctx.save_settings().unwrap();
        let ctx = Ctx::open_at(&dir.join(name), &dir.join("home")).unwrap();
        let prices: Prices = [("claude-opus-5-5".to_string(), crate::pricing::Price { input: 10.0, output: 10.0, cache_read: 1.0, cache_write: 1.0 })].into_iter().collect();
        ctx.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &prices).unwrap();
        let m = SessionMeta { harness: "claude".into(), profile: "work".into(), id: "s1".into(), title: "fix the build".into(), cwd: "C:\\p".into(), started_at: 1, last_active_at: 5, source_path: "C:\\p\\s1.jsonl".into(), ..Default::default() };
        ctx.db.put_session("claude|work|C:\\p\\s1.jsonl", &m, 5, 10).unwrap();
        ctx.db.tag_set("project", "github.com/o/r", &["release".into()]).unwrap();
        ctx.db.quota_point("work", "session", 40.0, Some(9_000), 1_500).unwrap();
        ctx.db.quota_point("work", "session", 50.0, Some(9_000), 2_500).unwrap();
        let old = SessionMeta { id: "old".into(), title: "an old one".into(), source_path: "C:\\a\\old.jsonl.zst".into(), archived: true, ..m.clone() };
        ctx.db.with(|c| {
            crate::archive::tables(c)?;
            c.execute("insert into archived values('claude/work/old', 'claude', 'work', 'old', ?, ?, 'C:\\a\\old.jsonl.zst', 12, 1)", params![ctx.db.device, serde_json::to_string(&old).unwrap()])?;
            Ok(())
        }).unwrap();
        let mut reg = ctx.registry().unwrap();
        reg.add_profile(crate::registry::Profile { name: "work".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }).unwrap();
        ctx.save_registry(&reg).unwrap();
        ctx
    }

    #[test]
    fn a_restore_into_an_empty_database_reproduces_the_counts_and_a_second_restore_adds_nothing() {
        let t = tempfile::tempdir().unwrap();
        let a = computer(t.path(), "A");
        let zip = crate::backup::dir(&a).join(crate::backup::create(&a, "Manual").unwrap().name);
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        let src = Source::Zip(zip);
        let p = preview(&b, &src).unwrap();
        assert_eq!((p["requests"].as_i64(), p["tokens"].as_i64(), p["sessions"].as_i64(), p["accounts"].as_i64(), p["archived"].as_i64(), p["newest"].as_i64()), (Some(2), Some(320), Some(1), Some(1), Some(1), Some(2_000)), "{p}");
        assert_eq!(p["computers"][0]["name"], "A");
        assert_eq!(p["this"], Value::Null, "B is not named in A's backup");
        let r = run(&b, &src, false).unwrap();
        assert_eq!((r["usage"].as_i64(), r["sessions"].as_i64(), r["quota"].as_i64(), r["tags"].as_i64(), r["peers"].as_i64(), r["archived"].as_i64(), r["profiles"].as_i64()), (Some(2), Some(1), Some(2), Some(1), Some(1), Some(1), Some(1)), "{r}");
        let (dev, cost): (String, f64) = b.db.with(|c| Ok(c.query_row("select device, cost_usd from usage where request_id='claude:1'", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
        assert_eq!(dev, a.settings.device_id);
        assert!((cost - 0.00115).abs() < 1e-9, "the cost is the one frozen on A ({cost})");
        assert_eq!(count(&b, "select count(*) from quota_hist"), 2);
        assert_eq!(b.db.quota_points("work", "session", 0).unwrap().len(), 2);
        assert_eq!(b.db.tags_of("project").unwrap()["github.com/o/r"], vec!["release".to_string()]);
        // the restored computer is a past computer with A's name; its session is listed as A's, archived rows too
        let list = crate::service::sessions(&b, &Default::default()).unwrap();
        assert_eq!(list.iter().map(|s| (s.id.as_str(), s.device_name.as_str(), s.archived)).collect::<Vec<_>>(), vec![("s1", "A", false), ("old", "A", true)]);
        assert!(b.registry().unwrap().profiles.iter().any(|p| p.name == "work"));
        // again: nothing
        let r = run(&b, &src, false).unwrap();
        assert_eq!(r, json!({"usage": 0, "sessions": 0, "quota": 0, "tags": 0, "peers": 0, "archived": 0, "profiles": 0, "adopted": false, "device": null}), "{r}");
        assert_eq!((count(&b, "select count(*) from usage"), count(&b, "select count(*) from sessions"), count(&b, "select count(*) from quota_hist"), count(&b, "select count(*) from archived")), (2, 1, 2, 1));
    }

    #[test]
    fn a_sync_folder_restores_every_computers_journal_and_its_backups_and_archive() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let mut a = computer(t.path(), "A");
        a.settings.sync_mode = "folder".into();
        a.settings.sync_folder = shared.display().to_string();
        a.settings.archive_dir = t.path().join("chosen").display().to_string();
        a.save_settings().unwrap();
        let a = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        crate::journal::round(&a).unwrap();
        crate::backup::create(&a, "Automatic").unwrap();
        let _ = crate::secrets::delete(&crate::sync::peer::identity::target(&a.settings.device_id));
        assert!(shared.join("backups").join(&a.settings.device_id).read_dir().unwrap().count() == 1, "the backup went beside the journal");
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        let src = Source::Folder(shared.clone());
        let p = preview(&b, &src).unwrap();
        assert_eq!((p["requests"].as_i64(), p["accounts"].as_i64(), p["archived"].as_i64()), (Some(2), Some(1), Some(1)), "{p}");
        let r = run(&b, &src, false).unwrap();
        assert_eq!((r["usage"].as_i64(), r["quota"].as_i64(), r["archived"].as_i64()), (Some(2), Some(2), Some(1)), "{r}");
        let cur: i64 = b.db.with(|c| Ok(c.query_row("select seq from cursors2 where device_id=?", [&a.settings.device_id], |r| r.get(0))?)).unwrap();
        assert!(cur >= 4, "the sync merge starts after what came in here ({cur})");
        assert_eq!(run(&b, &src, false).unwrap()["usage"], 0);
    }

    #[test]
    fn a_chunk_not_read_yet_is_left_for_the_sync_merge() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let mut a = computer(t.path(), "A");
        a.settings.sync_mode = "folder".into();
        a.settings.sync_folder = shared.display().to_string();
        a.save_settings().unwrap();
        let a = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        crate::journal::round(&a).unwrap();
        // a second chunk, then the first as a placeholder that cannot be read yet
        a.db.insert_usage(&[row("claude:3", 3_000)], &Prices::new()).unwrap();
        crate::journal::round(&a).unwrap();
        let dir = shared.join("devices").join(&a.settings.device_id);
        let man = crate::sync::folder::read_manifest(&dir).unwrap();
        let (first, second) = (man["chunks"][0].clone(), man["chunks"][1].clone());
        let path = dir.join("journal").join(first["file"].as_str().unwrap());
        let good = std::fs::read(&path).unwrap();
        std::fs::write(&path, "not yet").unwrap();
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        let r = run(&b, &Source::Folder(shared.clone()), false).unwrap();
        assert_eq!(r["usage"], 1, "the chunk it could read came in: {r}");
        assert_eq!(count(&b, "select count(*) from cursors2"), 1);
        assert_eq!(count(&b, "select seq from cursors2"), 0, "the cursor waits before the gap");
        // once the file is there, the merge brings it, and the later chunk adds nothing twice
        std::fs::write(&path, good).unwrap();
        let cur = crate::journal::apply_manifest(&b, &a.settings.device_id, &man, &|f| Ok(std::fs::read(dir.join("journal").join(f))?)).unwrap();
        assert_eq!(cur, second["to"].as_i64().unwrap());
        assert_eq!(count(&b, "select count(*) from usage"), 3);
        let _ = crate::secrets::delete(&crate::sync::peer::identity::target(&a.settings.device_id));
    }

    #[test]
    fn adopt_needs_the_signing_key_of_that_computer() {
        let t = tempfile::tempdir().unwrap();
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let a = computer(t.path(), "A");
        let zip = crate::backup::dir(&a).join(crate::backup::create(&a, "Manual").unwrap().name);
        let src = Source::Zip(zip);
        let _ = crate::secrets::delete(&crate::sync::peer::identity::target(&a.settings.device_id));
        // a reinstalled computer with A's name: the backup is here, the key is not
        let mut b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        b.settings.device_name = "A".into();
        b.save_settings().unwrap();
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        assert_eq!(preview(&b, &src).unwrap()["this"]["can_adopt"], false);
        let id = b.settings.device_id.clone();
        assert!(run(&b, &src, true).unwrap_err().to_string().contains("signing key of A"));
        assert_eq!(Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap().settings.device_id, id, "it stays itself");
        // as a new computer the same backup is fine
        assert_eq!(run(&b, &src, false).unwrap()["usage"], 2);
    }

    #[test]
    fn adopt_takes_the_id_and_a_new_incarnation_and_is_refused_for_a_live_peers_id() {
        let t = tempfile::tempdir().unwrap();
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let a = computer(t.path(), "A");
        crate::sync::peer::identity::load(&a).unwrap();
        let zip = crate::backup::dir(&a).join(crate::backup::create(&a, "Manual").unwrap().name);
        let src = Source::Zip(zip);
        // a fresh computer with A's name
        let mut b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        b.settings.device_name = "a".into();
        b.save_settings().unwrap();
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        let (old_id, old_inc) = (b.settings.device_id.clone(), b.settings.incarnation.clone());
        assert_eq!(preview(&b, &src).unwrap()["this"]["name"], "A");
        // refused while another computer answers to that id
        b.db.with(|c| { c.execute("insert into peers(device_id, name, last_seen) values(?, 'A', ?)", params![a.settings.device_id, now_ms()])?; Ok(()) }).unwrap();
        assert_eq!(run(&b, &src, true).unwrap_err().to_string(), "another computer uses this identity");
        b.db.with(|c| { c.execute("update peers set last_seen = 1", [])?; Ok(()) }).unwrap();
        let r = run(&b, &src, true).unwrap();
        assert_eq!((r["adopted"].as_bool(), r["usage"].as_i64()), (Some(true), Some(2)));
        let b = Ctx::open_at(&t.path().join("B"), &t.path().join("home-b")).unwrap();
        assert_eq!((b.settings.device_id.as_str(), b.settings.device_name.as_str()), (a.settings.device_id.as_str(), "A"));
        assert!(b.settings.incarnation != old_inc && b.settings.incarnation != a.settings.incarnation, "a new incarnation");
        assert_eq!(count(&b, &format!("select count(*) from usage where device = '{old_id}'")), 0, "nothing is left under the old id");
        assert_eq!(count(&b, "select count(*) from peers"), 0, "the adopted id is not a peer");
        assert_eq!(b.db.kv_get("journal:backfilled").unwrap().as_deref(), Some("0"), "the journal starts again");
        // its history is this computer's now
        assert_eq!(crate::service::overview(&b, None, None, Some("here")).unwrap().requests, 2);
        // a fresh computer with another name cannot adopt
        let c = Ctx::open_at(&t.path().join("C"), &t.path().join("home-c")).unwrap();
        assert!(run(&c, &src, true).unwrap_err().to_string().starts_with("no record of this computer's name"));
        let _ = crate::secrets::delete(&crate::sync::peer::identity::target(&a.settings.device_id));
    }
}
