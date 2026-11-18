//! Backups of what Harness Manager owns: the account list, settings, what it wrote where (the ledger and
//! its saved state) and the folders of the skills it manages. Keys and logins stay in the system
//! credential store and are never in a backup. A backup is one zip in `<data>/backups`.

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

/// State a backup carries and a restore puts back: the live-limits choice, what was written into each prompt file
/// and the last MCP import. Everything else describes this computer as it is now (which account owns the Antigravity
/// login, the router's token, its identity, caches, marks of what was read) and is neither kept nor put back: a
/// restored owner would have the next start save the login in the slot under the wrong account.
fn keep_kv(k: &str) -> bool { k == "live_quota" || k == "mcp:last_import" || k.starts_with("prompt:") }

#[cfg(test)]
mod keep_tests {
    use super::*;

    #[test]
    fn the_router_token_and_caches_stay_out_of_backups() {
        for k in ["router:token", "router:pid", "breaker:x", "apps:latest", "self:latest", "pricing:models", "journal:published_seq", "journal:backfilled", "computers:last",
            "agy:active", "device:pubkey", "pairing:current", "migrated:device", "opencode:usage:work"] { assert!(!keep_kv(k), "{k}"); }
        for k in ["live_quota", "mcp:last_import", "prompt:C:\\p\\AGENTS.md"] { assert!(keep_kv(k), "{k}"); }
    }

    #[test]
    fn a_restore_does_not_say_who_owns_the_antigravity_login() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        ctx.db.kv_set("agy:active", "agy-main").unwrap();
        // a backup made by a version that kept the owner, from a time another account had the login
        let zip = t.path().join("b.zip");
        zip_of(&zip, &[("profiles.json", "{}"), ("state.json", r#"{"kv": {"agy:active": "agy-work", "live_quota": "1"}}"#)]);
        restore_file(&ctx, &zip).unwrap();
        assert_eq!(ctx.db.kv_get("agy:active").unwrap().as_deref(), Some("agy-main"), "the owner is the one this computer recorded");
        assert_eq!(ctx.db.kv_get("live_quota").unwrap().as_deref(), Some("1"), "a choice is put back");
    }

    /// A zip with these entries, named as given: what a backup made by another program can hold.
    fn zip_of(path: &Path, entries: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, text) in entries {
            z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(text.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn a_skill_file_named_outside_its_folder_is_not_restored() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let (zip, elsewhere) = (t.path().join("b.zip"), t.path().join("elsewhere.txt"));
        zip_of(&zip, &[("profiles.json", "{}"), ("skills/demo/SKILL.md", "ok"), ("skills/demo/..\\..\\up.txt", "x"), ("skills/demo/sub/..\\..\\..\\up.txt", "x"),
            (&format!("skills/demo/{}", elsewhere.display()), "x")]);
        restore_file(&ctx, &zip).unwrap();
        let demo = crate::items::skill_store(&ctx).join("demo");
        assert_eq!(std::fs::read_to_string(demo.join("SKILL.md")).unwrap(), "ok", "a plain name is restored");
        let mut written = vec![];
        crate::util::walk_files(t.path(), &|f| !f.starts_with(&ctx.data) && f != zip, &mut written);
        assert!(written.iter().all(|f| f.starts_with(&demo)), "every restored file is in the skill's folder: {written:?}");
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

    #[test]
    fn a_backup_made_now_holds_the_transcripts_when_asked_for_or_when_the_setting_says_so() {
        let t = tempfile::tempdir().unwrap();
        let mut ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let src = t.path().join("s1.jsonl");
        std::fs::write(&src, "{\"type\":\"user\"}\n").unwrap();
        let m = crate::model::SessionMeta { harness: "claude".into(), profile: "work".into(), id: "s1".into(), source_path: src.display().to_string(), ..Default::default() };
        ctx.db.put_session(&format!("claude|work|{}", src.display()), &m, 1, 1).unwrap();
        let made = |ctx: &Ctx, label: Option<&str>, transcripts: Option<bool>| {
            let (b, with) = create_now(ctx, label, transcripts).unwrap();
            let mut z = zip::ZipArchive::new(std::fs::File::open(dir(ctx).join(&b.name)).unwrap()).unwrap();
            let held = (0..z.len()).any(|i| z.by_index(i).unwrap().name().unwrap().starts_with("transcripts/"));
            assert_eq!(held, with, "{}", b.name);
            (b.label, with)
        };
        assert_eq!(made(&ctx, None, None), ("Manual".to_string(), false));
        assert_eq!(made(&ctx, Some("Before the move"), Some(true)), ("Before the move".to_string(), true));
        // the setting decides when nothing is said: `hrm backup` without its flag left them out here
        ctx.settings.backup_transcripts = true;
        assert!(made(&ctx, None, None).1);
        assert!(!made(&ctx, None, Some(false)).1, "a plain no is kept");
    }

    #[test]
    fn a_session_id_in_two_files_does_not_fail_the_backup_and_a_failed_one_leaves_no_part_file() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        // the same session id under two project folders, as a tool leaves it after a copy or a fork
        for (folder, text, mtime) in [("a", "{\"type\":\"older\"}\n", 1), ("b", "{\"type\":\"newer\"}\n", 2)] {
            let src = t.path().join(folder).join("s1.jsonl");
            std::fs::create_dir_all(src.parent().unwrap()).unwrap();
            std::fs::write(&src, text).unwrap();
            let m = crate::model::SessionMeta { harness: "claude".into(), profile: "work".into(), id: "s1".into(), source_path: src.display().to_string(), ..Default::default() };
            ctx.db.put_session(&format!("claude|work|{}", src.display()), &m, mtime, 1).unwrap();
        }
        let b = create_with(&ctx, "Manual", true).expect("a zip refuses a name twice: the backup failed on this");
        let mut z = zip::ZipArchive::new(std::fs::File::open(dir(&ctx).join(&b.name)).unwrap()).unwrap();
        let mut packed = vec![];
        z.by_name("transcripts/claude/work/s1.jsonl.zst").unwrap().read_to_end(&mut packed).unwrap();
        assert_eq!(String::from_utf8(zstd::decode_all(&packed[..]).unwrap()).unwrap(), "{\"type\":\"newer\"}\n", "the newest file is the one kept");
        let parts = |d: &Path| std::fs::read_dir(d).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".part")).count();
        assert_eq!(parts(&dir(&ctx)), 0);

        // a write that cannot finish (its name is taken by a folder) leaves nothing behind
        let taken = dir(&ctx).join("taken.zip");
        std::fs::create_dir_all(&taken).unwrap();
        assert!(write_zip(&ctx, &taken, true).is_err());
        assert_eq!(parts(&dir(&ctx)), 0, "no part file after a failed write");

        // what an older version left is removed by the automatic backup's round, and a fresh one is not
        std::fs::write(dir(&ctx).join("2026-11-12 08.41.00 Automatic.zip.part"), b"half").unwrap();
        assert_eq!(sweep_parts(&dir(&ctx), 3_600_000), 0, "one that may still be written stays");
        assert_eq!(sweep_parts(&dir(&ctx), 0), 1);
        assert_eq!(parts(&dir(&ctx)), 0);
    }

    #[test]
    fn a_failed_automatic_backup_is_tried_again_in_an_hour_not_at_every_round() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        // the backups folder cannot be made: a file has its name
        std::fs::write(dir(&ctx), b"in the way").unwrap();
        assert!(auto(&ctx).is_err(), "the first try fails and says so");
        assert!(matches!(auto(&ctx), Ok(None)), "the next round does not try again");
        std::fs::remove_file(dir(&ctx)).unwrap();
        assert!(matches!(auto(&ctx), Ok(None)), "still within the hour");
        ctx.db.kv_set("backup:failed_at", &(now_ms() - 3_700_000).to_string()).unwrap();
        assert!(auto(&ctx).unwrap().is_some(), "after the hour it is made");
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
    ctx.db.with(|c| Ok(crate::archive::tables(c)?))?;
    // on a read connection, in one transaction: every table as of the same moment, and no write waits while the
    // whole usage table is put into text
    ctx.db.read(|c| {
        let c = &c.transaction()?;
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
    let rows: Vec<String> = ctx.db.read(|c| {
        let mut st = c.prepare("select json from sessions where device = ? and instr(key, '|') > 0 and harness in ('claude', 'codex', 'pi', 'omp') order by mtime desc")?;
        let v = st.query_map([&ctx.db.device], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let (mut out, mut seen) = (vec![], std::collections::HashSet::new());
    for j in rows {
        let m: Value = serde_json::from_str(&j).unwrap_or(Value::Null);
        let (h, p, id, path) = (m["harness"].as_str().unwrap_or(""), m["profile"].as_str().unwrap_or(""), m["id"].as_str().unwrap_or(""), Path::new(m["source_path"].as_str().unwrap_or("")));
        if id.is_empty() || id.contains(['/', '\\']) || !path.is_file() { continue; }
        let name = format!("transcripts/{h}/{p}/{id}.jsonl.zst");
        // one session id can be in two files (a copy in another project folder, a fork): the newest goes in, since a
        // zip refuses a name twice and the whole backup failed on it
        if !seen.insert(name.clone()) { continue; }
        let Ok(text) = crate::harness::parse::read_text(path) else { continue };
        out.push((name, zstd::encode_all(text.as_bytes(), 3)?));
    }
    Ok(out)
}

/// Write the zip of what Harness Manager owns now; with `with_transcripts`, this computer's transcripts too.
fn write_zip(ctx: &Ctx, out: &Path, with_transcripts: bool) -> Result<()> {
    let reg_text = crate::util::read_or_empty(&ctx.registry_path())?;
    let reg = ctx.registry()?;
    // keys only (each `null`): no env or header value is in a backup, also where the list file still holds one
    let reg_text = if reg_text.trim().is_empty() { reg_text } else { reg.without_mcp_values().file_text()? };
    let state = ctx.db.read(|c| {
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
    let write = || -> Result<()> {
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
        Ok(())
    };
    // a write that fails leaves no part file: with transcripts one is over a gigabyte, and the next try makes another
    let done = write().and_then(|()| std::fs::rename(&tmp, out).with_context(|| format!("save {}", out.display())));
    if done.is_err() { let _ = std::fs::remove_file(&tmp); }
    done
}

/// Part files a failed or interrupted backup left in `d`, once they are older than `older_ms` (one being written now is not).
fn sweep_parts(d: &Path, older_ms: i64) -> usize {
    let Ok(rd) = std::fs::read_dir(d) else { return 0 };
    rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".zip.part")).filter(|e| {
        let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|a| a.as_millis() as i64).unwrap_or(i64::MAX);
        age >= older_ms && std::fs::remove_file(e.path()).is_ok()
    }).count()
}

/// Make a backup with this label ("Automatic", "Manual", "Before restore", ...), with transcripts as the setting says.
pub fn create(ctx: &Ctx, label: &str) -> Result<Backup> { create_with(ctx, label, ctx.settings.backup_transcripts) }

/// The backup the window's button and `hrm backup` make: labelled "Manual" unless told otherwise, with transcripts
/// when asked for, else as the setting says. Returns it and whether the transcripts are in it.
pub fn create_now(ctx: &Ctx, label: Option<&str>, transcripts: Option<bool>) -> Result<(Backup, bool)> {
    let with = transcripts.unwrap_or(ctx.settings.backup_transcripts);
    Ok((create_with(ctx, label.unwrap_or("Manual"), with)?, with))
}

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
    if let Err(e) = std::fs::copy(dir(ctx).join(name), &tmp).and_then(|_| std::fs::rename(&tmp, d.join(name))) { let _ = std::fs::remove_file(&tmp); return Err(e.into()); }
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
    // what earlier failures left behind goes first, also when no backup is due
    let swept = sweep_parts(&dir(ctx), 3_600_000) + folder_dir(ctx).map(|d| sweep_parts(&d, 3_600_000)).unwrap_or(0);
    if swept > 0 { crate::log::info(&format!("backup: removed {swept} part files that failed backups left")); }
    if list(ctx).first().map(|b| now_ms() - b.at < hours * 3_600_000).unwrap_or(false) { return Ok(None); }
    // a backup that failed is tried again in an hour, not at every refresh round
    let failed_at = ctx.db.kv_get("backup:failed_at").ok().flatten().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    if now_ms() - failed_at < 3_600_000 { return Ok(None); }
    let b = match create(ctx, "Automatic") {
        Ok(b) => b,
        Err(e) => { let _ = ctx.db.kv_set("backup:failed_at", &now_ms().to_string()); return Err(e); }
    };
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
    let mut reg = crate::registry::Registry::from_text(&reg_text).context("the backup's account list is not valid")?;
    let settings = text("settings.json")?;
    let state: Value = text("state.json")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    create(ctx, "Before restore")?;
    // a backup holds no env or header value (`null`, read as empty): this computer's value stays
    let here = ctx.registry()?;
    for s in &mut reg.mcp { if let Some(h) = here.mcp.iter().find(|h| h.name == s.name) { s.fill_unset(h); } }
    reg.save_restored(&ctx.registry_path())?;
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
        // the name as the zip library confines it: no part is `..`, a drive, or holds a separator or a stream
        let Some(path) = f.enclosed_name() else { continue };
        let Ok(rest) = path.strip_prefix("skills") else { continue };
        let mut parts = rest.components();
        let Some(skill) = parts.next().and_then(|c| c.as_os_str().to_str()) else { continue };
        let rel = parts.as_path();
        // and checked here too: every part is a plain name, so the file stays under the skill's folder
        if !valid_name(skill) || rel.as_os_str().is_empty() || !rel.components().all(|c| matches!(c, std::path::Component::Normal(_))) { continue; }
        if store.join(skill).exists() && !skills.contains(skill) { continue; }
        let out = store.join(skill).join(rel);
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
