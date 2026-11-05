//! Transcripts through the folder: on every publish round this computer's changed session files go to
//! `devices/<id>/transcripts/<harness>/<profile>/<session id>.<ext>.zst` with a `transcript` journal entry;
//! the other computers store the entry on merge and fetch the file only when they read or resume the session.

use std::path::Path;
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::harness::{self, Harness};
use crate::journal::{record, Entry};
use crate::transcripts::{tables, CAP_BYTES, MAX_TRANSCRIPT_BYTES};
use crate::util::{now_ms, sha256_hex, write_atomic};
use crate::Ctx;

/// The source of one session's transcript: its bytes, where the tool keeps it, and a stamp that changes
/// with it (the file's mtime; for OpenCode the session's last activity).
struct Source { bytes: Vec<u8>, ext: &'static str, source_rel: String, stamp: i64 }

/// What this computer would carry for one of its sessions, or None when the tool has nothing to copy.
fn source(ctx: &Ctx, h: &dyn Harness, p: &crate::registry::Profile, meta: &Value) -> Result<Option<Source>> {
    let home = h.home(ctx, p);
    let rel = |path: &Path| path.strip_prefix(&home).unwrap_or(path).display().to_string().replace('\\', "/");
    match h.id() {
        "claude" | "codex" | "pi" | "omp" => {
            let path = Path::new(meta["source_path"].as_str().unwrap_or(""));
            let Some((mtime, len)) = crate::util::file_sig(path) else { return Ok(None) };
            if len > MAX_TRANSCRIPT_BYTES { bail!("too big"); }
            // Codex keeps older rollouts compressed: carried plain, so every copy is zstd of the log itself
            let bytes = if path.extension().map(|e| e == "zst").unwrap_or(false) { harness::parse::read_text(path)?.into_bytes() } else { std::fs::read(path)? };
            Ok(Some(Source { bytes, ext: "jsonl", source_rel: rel(path), stamp: mtime }))
        }
        "opencode" => {
            let id = meta["id"].as_str().unwrap_or("");
            let stamp = meta["last_active_at"].as_i64().unwrap_or(0);
            // the export of the installed version: 2.x `session export`, 1.x `export`
            let mut out = None;
            for args in [vec!["session", "export", id], vec!["export", id]] {
                let mut c = h.launch(ctx, p, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>())?.command();
                c.stdin(Stdio::null());
                #[cfg(windows)]
                { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
                let o = c.output()?;
                if o.status.success() && serde_json::from_slice::<Value>(&o.stdout).is_ok() { out = Some(o.stdout); break; }
            }
            let Some(bytes) = out else { bail!("opencode export gave no session JSON") };
            if bytes.len() as u64 > MAX_TRANSCRIPT_BYTES { bail!("too big"); }
            Ok(Some(Source { bytes, ext: "json", source_rel: format!("export/{id}.json"), stamp }))
        }
        _ => Ok(None),
    }
}

/// This computer's changed transcripts into the folder, each with its journal entry, then the oldest out
/// when the folder holds more than the cap.
pub fn carry(ctx: &Ctx, root: &Path) -> Result<()> { carry_with_cap(ctx, root, CAP_BYTES) }

pub fn carry_with_cap(ctx: &Ctx, root: &Path, cap: i64) -> Result<()> {
    let dir = super::folder::mine(ctx, root);
    carry_to(ctx, cap, &|_, file, bytes| write_atomic(&dir.join(file), bytes), &|_, file| Ok(std::fs::remove_file(dir.join(file))?))
}

/// The compressed bytes of one of this computer's own transcripts, as it is carried (the same zstd level, so
/// the sha256 of the journal entry matches while the session is unchanged); for the peer transcript route.
pub fn own_zst(ctx: &Ctx, harness_id: &str, profile: &str, id: &str) -> Result<Vec<u8>> {
    let reg = ctx.registry()?;
    let key = format!("{harness_id}/{profile}/{id}");
    let json: String = ctx.db.with(|c| Ok(c.query_row("select json from sessions where key=? and device=?", [&key, &ctx.db.device], |r| r.get(0))?))?;
    let meta: Value = serde_json::from_str(&json)?;
    let (h, p) = (harness::by_id(harness_id).context("tool")?, reg.profiles.iter().find(|p| p.name == profile).context("account")?);
    let src = source(ctx, h, p, &meta)?.context("nothing to carry")?;
    Ok(zstd::encode_all(&src.bytes[..], 3)?)
}

/// The carry with any destination: `put(key, file, bytes)` stores a compressed transcript, `remove(key, file)` drops
/// one at the cap (`key` is the journal key, `file` the path under this computer's folder).
#[allow(clippy::type_complexity)]
pub fn carry_to(ctx: &Ctx, cap: i64, put: &dyn Fn(&str, &str, &[u8]) -> Result<()>, remove: &dyn Fn(&str, &str) -> Result<()>) -> Result<()> {
    if !ctx.settings.sync_transcripts { return Ok(()); }
    let reg = ctx.registry()?;
    // this computer's sessions as the journal knows them, with what was carried of each
    type Row = (String, String, Option<i64>, Option<String>);
    let rows: Vec<Row> = ctx.db.with(|c| {
        tables(c)?;
        let mut st = c.prepare("select s.key, s.json, t.mtime, t.skipped from sessions s left join transcripts_out t on t.key = s.key where s.device = ? and instr(s.key, '|') = 0")?;
        let v = st.query_map([&ctx.db.device], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let (mut n, mut pruned) = (0, 0);
    for (key, json, carried_stamp, _) in rows {
        let meta: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
        let (Some(h), Some(p)) = (harness::by_id(meta["harness"].as_str().unwrap_or("")), reg.profiles.iter().find(|p| p.name == meta["profile"].as_str().unwrap_or(""))) else { continue };
        let (harness_id, profile, id) = (h.id(), p.name.clone(), meta["id"].as_str().unwrap_or("").to_string());
        if id.is_empty() || id.contains(['/', '\\']) { continue; }
        // unchanged since it was carried, skipped or removed at the cap: nothing to do until it changes
        let stamp = match h.id() {
            "opencode" => meta["last_active_at"].as_i64().unwrap_or(0),
            "claude" | "codex" | "pi" | "omp" => match crate::util::file_sig(Path::new(meta["source_path"].as_str().unwrap_or(""))) { Some((m, _)) => m, None => continue },
            _ => continue,
        };
        if carried_stamp == Some(stamp) { continue; }
        let src = match source(ctx, h, p, &meta) {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(e) => {
                let why = format!("{e:#}");
                crate::log::warn(&format!("transcripts: {key} not carried: {why}"));
                ctx.db.with(|c| {
                    c.execute("insert or replace into transcripts_out(key, file, mtime, bytes, sha256, skipped, at) values(?,'',?,0,'',?,?)", params![key, stamp, why, now_ms()])?;
                    record(c, "transcript", &key, &json!({"skipped": why}))?;
                    Ok(())
                })?;
                continue;
            }
        };
        let file = format!("transcripts/{harness_id}/{profile}/{id}.{}.zst", src.ext);
        let packed = zstd::encode_all(&src.bytes[..], 3).context("compress the transcript")?;
        put(&key, &file, &packed)?;
        let data = json!({"file": file, "bytes": packed.len(), "sha256": sha256_hex(&packed), "mtime": src.stamp, "source_rel": src.source_rel});
        ctx.db.with(|c| {
            c.execute("insert or replace into transcripts_out(key, file, mtime, bytes, sha256, skipped, at) values(?,?,?,?,?,null,?)", params![key, file, src.stamp, packed.len() as i64, data["sha256"].as_str(), now_ms()])?;
            record(c, "transcript", &key, &data)?;
            Ok(())
        })?;
        n += 1;
    }
    // the cap: the oldest by mtime go first, never the newest one
    loop {
        let victim: Option<(String, String)> = ctx.db.with(|c| {
            let (total, count): (i64, i64) = c.query_row("select coalesce(sum(bytes), 0), count(*) from transcripts_out where skipped is null", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
            if total <= cap || count <= 1 { return Ok(None); }
            Ok(c.query_row("select key, file from transcripts_out where skipped is null order by mtime, at, key limit 1", [], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
        })?;
        let Some((key, file)) = victim else { break };
        if let Err(e) = remove(&key, &file) { crate::log::warn(&format!("transcripts: removing {file}: {e}")); }
        ctx.db.with(|c| {
            // the row stays, marked: the file is not carried again until the session changes
            c.execute("update transcripts_out set skipped='at the cap', bytes=0 where key=?", [&key])?;
            record(c, "tombstone", &format!("transcript/{key}"), &json!({}))?;
            Ok(())
        })?;
        pruned += 1;
    }
    if n > 0 || pruned > 0 { crate::log::info(&format!("transcripts: {n} carried, {pruned} removed at the cap")); }
    Ok(())
}

/// A `transcript` entry of another computer's journal: stored, fetched later when needed. A skipped one
/// (too big there) replaces nothing and removes what was known of it.
pub fn apply(tx: &Connection, device: &str, e: &Entry) -> Result<()> {
    tables(tx)?;
    if e.data.get("skipped").is_some() || e.data["file"].as_str().unwrap_or("").is_empty() {
        tx.execute("delete from transcripts where device=? and key=?", [device, &e.key])?;
        return Ok(());
    }
    let file = e.data["file"].as_str().context("transcript file")?;
    if file.contains("..") || Path::new(file).is_absolute() { bail!("transcript path {file}"); }
    tx.execute("insert or replace into transcripts(device, key, file, bytes, sha256, mtime, source_rel, fetched_at) values(?,?,?,?,?,?,?,null)",
        params![device, e.key, file, e.data["bytes"].as_i64().unwrap_or(0), e.data["sha256"].as_str().unwrap_or(""), e.data["mtime"].as_i64().unwrap_or(0), e.data["source_rel"].as_str().unwrap_or("")])?;
    Ok(())
}

/// The origin removed a transcript from the folder: the entry goes. shortcut: a fetched copy under
/// `<data>/transcripts` stays as a stray file (the merge has no data folder at hand); clear it when it matters.
pub fn tombstone(tx: &Connection, device: &str, key: &str) -> Result<()> {
    tables(tx)?;
    tx.execute("delete from transcripts where device=? and key=?", [device, key])?;
    Ok(())
}
