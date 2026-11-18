//! The local journal store: `<data>/journal-cache/<origin>/` holds a manifest and chunk files in the folder
//! layout, for this computer (what it published) and for every origin it received, so a peer can pull any of
//! them (relay). Other origins' caches are pruned to the last 50 MB, oldest first, never below the newest chunk.
//! This computer's own journal is kept whole, its older chunks joined into files of up to 1 MB.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::util::{read_json, safe_name, sha256_hex, write_atomic, NamedLock};
use crate::Ctx;

const KEEP_BYTES: i64 = 50_000_000;
/// A chunk file no list names is left alone while it is younger than this: a publisher that could not take the
/// list's lock writes its file before the list.
const UNLISTED_KEPT: std::time::Duration = std::time::Duration::from_secs(120);

/// A computer's id, as it names a folder here: another computer can send any text as one, and `..\` must not
/// climb out of the store.
pub fn safe_origin(s: &str) -> bool { safe_name(s) }

/// A file name another computer sent (a chunk's, a transcript's): a plain name. A check for slashes alone lets a
/// drive or a stream through on Windows.
pub fn safe_file(s: &str) -> bool { safe_name(s) }

pub fn dir(ctx: &Ctx, origin: &str) -> PathBuf { ctx.data.join("journal-cache").join(if safe_origin(origin) { origin } else { ".invalid" }) }

pub fn manifest(ctx: &Ctx, origin: &str) -> Option<Value> { read_json(&dir(ctx, origin).join("manifest.json")) }

/// The same for a writer, which must tell a list that is not there (or is not one) from a list that could not be
/// read just now: starting over from the second would drop every chunk the list names.
pub fn read_list(ctx: &Ctx, origin: &str) -> std::io::Result<Option<Value>> {
    match std::fs::read_to_string(dir(ctx, origin).join("manifest.json")) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn device(ctx: &Ctx, origin: &str) -> Option<Value> { read_json(&dir(ctx, origin).join("device.json")) }

pub fn put_device(ctx: &Ctx, origin: &str, v: &Value) -> Result<()> { write_atomic(&dir(ctx, origin).join("device.json"), v.to_string().as_bytes()) }

/// A chunk file's bytes; the name is checked to be a plain file name first.
pub fn chunk(ctx: &Ctx, origin: &str, file: &str) -> Result<Vec<u8>> {
    if !safe_file(file) || !safe_origin(origin) { bail!("not a chunk name"); }
    Ok(std::fs::read(dir(ctx, origin).join("journal").join(file))?)
}

/// Every origin with a store here, this computer included.
pub fn origins(ctx: &Ctx) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(ctx.data.join("journal-cache")) else { return vec![] };
    let mut v: Vec<String> = rd.flatten().filter(|e| e.path().join("manifest.json").is_file()).filter_map(|e| e.file_name().to_str().map(String::from)).collect();
    v.sort();
    v
}

/// A manifest with no chunks yet, so a peer asking gets an answer with the incarnation.
pub fn put_manifest_empty(ctx: &Ctx, origin: &str, incarnation: &str) -> Result<()> {
    write_atomic(&dir(ctx, origin).join("manifest.json"), json!({"incarnation": incarnation, "last_seq": 0, "chunks": []}).to_string().as_bytes())
}

/// One writer of an origin's list at a time, across threads and processes: it is read, changed and written back
/// with the files it names, and a list written from a copy read earlier would drop what another writer added.
fn list_lock(origin: &str) -> NamedLock { NamedLock::take(&format!("journal-list-{origin}")) }

/// One run of this computer's chunks older than its newest `keep`, joined into one file of up to `max` bytes: a
/// run that is full, or the last one once it holds more than `keep`. The same entries in the same order, signed
/// anew by `sign(from, to, sha256)`; a reader skips the entries its cursor is past, so one that took some of the
/// small files takes the rest from the joined one. One run a call keeps the publish lock short however many
/// files wait. The list is read again under its lock before it is written: when another publisher changed it
/// meanwhile, nothing is written or removed this time, and the files removed are those the new list replaced.
pub fn join_old(ctx: &Ctx, keep: usize, max: usize, sign: &dyn Fn(i64, i64, &str) -> String) -> Result<()> {
    let me = &ctx.settings.device_id;
    let Some(man) = manifest(ctx, me) else { return Ok(()) };
    let (inc, list) = (man["incarnation"].as_str().unwrap_or("").to_string(), man["chunks"].as_array().cloned().unwrap_or_default());
    if list.len() <= keep { return Ok(()); }
    let old = &list[..list.len() - keep];
    let (seq, size) = (|c: &Value, k: &str| c[k].as_i64().unwrap_or(0), |c: &Value| c["bytes"].as_i64().unwrap_or(0) as usize);
    // runs of chunks that follow each other without a hole and fit one file together; the first worth joining
    let mut end = 0;
    let start = loop {
        if end == old.len() { return Ok(()); }
        let start = end;
        end += 1;
        let mut bytes = size(&old[start]);
        while end < old.len() && seq(&old[end], "from") == seq(&old[end - 1], "to") + 1 && bytes + size(&old[end]) <= max { bytes += size(&old[end]); end += 1; }
        if end - start >= 2 && (end < old.len() || end - start > keep) { break start; }
    };
    let mut bytes = vec![];
    for ch in &old[start..end] { bytes.extend(chunk(ctx, me, ch["file"].as_str().unwrap_or(""))?); }
    let (from, to, sha) = (seq(&old[start], "from"), seq(&old[end - 1], "to"), sha256_hex(&bytes));
    let joined = json!({"file": format!("{inc}-{from}-{to}.jsonl"), "from": from, "to": to, "sha256": sha, "sig": sign(from, to, &sha), "bytes": bytes.len()});
    let one = list_lock(me);
    if !one.held() || manifest(ctx, me).as_ref() != Some(&man) { return Ok(()); }
    let dir = dir(ctx, me).join("journal");
    write_atomic(&dir.join(joined["file"].as_str().unwrap_or("")), &bytes)?;
    let mut now = man.clone();
    now["chunks"] = json!(list[..start].iter().chain([&joined]).chain(&list[end..]).collect::<Vec<_>>());
    write_atomic(&dir.with_file_name("manifest.json"), serde_json::to_string_pretty(&now)?.as_bytes())?;
    for ch in &old[start..end] { let _ = std::fs::remove_file(dir.join(ch["file"].as_str().unwrap_or(""))); }
    crate::log::info(&format!("journal: {} old chunks joined into one of {} KB", end - start, bytes.len() / 1000));
    Ok(())
}

/// This computer's list against its files: a listed chunk whose file is gone is cut again from the outbox
/// (`cut(from, to)`, the same entries), or leaves the list when the outbox no longer holds them all. A reader of
/// that list then finds entries missing, says so and goes on, where it would ask for the file for good. And a
/// chunk file the list does not name goes once it is two minutes old: a join that was stopped between its list
/// and its removals left it. When the folder cannot be listed, nothing is judged missing or left over this time.
pub fn mend(ctx: &Ctx, cut: &dyn Fn(i64, i64) -> Option<Vec<u8>>, sign: &dyn Fn(i64, i64, &str) -> String) -> Result<()> {
    let me = &ctx.settings.device_id;
    let one = list_lock(me);
    let Some(mut man) = manifest(ctx, me).filter(|_| one.held()) else { return Ok(()) };
    let dir = dir(ctx, me).join("journal");
    let Ok(entries) = std::fs::read_dir(&dir).and_then(|rd| rd.collect::<std::io::Result<Vec<_>>>()) else { return Ok(()) };
    let have: HashSet<String> = entries.iter().filter_map(|e| e.file_name().to_str().map(String::from)).collect();
    let list = man["chunks"].as_array().cloned().unwrap_or_default();
    let listed: HashSet<&str> = list.iter().filter_map(|c| c["file"].as_str()).collect();
    let old = |e: &std::fs::DirEntry| e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age >= UNLISTED_KEPT);
    for e in entries.iter().filter(|e| e.file_name().to_str().is_some_and(|f| f.ends_with(".jsonl") && !listed.contains(f)) && old(e)) { let _ = std::fs::remove_file(e.path()); }
    if listed.iter().all(|f| have.contains(*f)) { return Ok(()); }
    let mut out = vec![];
    for ch in list {
        let (file, from, to) = (ch["file"].as_str().unwrap_or("").to_string(), ch["from"].as_i64().unwrap_or(0), ch["to"].as_i64().unwrap_or(0));
        if have.contains(&file) { out.push(ch); continue; }
        let Some(bytes) = cut(from, to).filter(|_| safe_file(&file)) else { crate::log::warn(&format!("journal: {file} is gone and the outbox no longer holds its entries; it leaves the list")); continue };
        write_atomic(&dir.join(&file), &bytes)?;
        let sha = sha256_hex(&bytes);
        crate::log::warn(&format!("journal: {file} was gone; cut again from the outbox"));
        out.push(json!({"file": file, "from": from, "to": to, "sha256": sha, "sig": sign(from, to, &sha), "bytes": bytes.len()}));
    }
    man["chunks"] = json!(out);
    write_atomic(&dir.with_file_name("manifest.json"), serde_json::to_string_pretty(&man)?.as_bytes())
}

/// Keep one chunk and list it in the origin's manifest (a new incarnation starts the list over). For this
/// computer's own journal nothing is written without the list's lock, or when its list could not be read: the
/// entries stay in the outbox and go out at the next publish. A list that is not there, or does not read as one,
/// starts over, and the publish that found it so cuts the whole outbox again. Another computer's chunks are kept
/// here for relay only, after they were merged: there the write goes on without the lock or the old list, since a
/// chunk left out of that copy and one lost to a second writer cost a reader the same, and refusing costs it always.
pub fn put_chunk(ctx: &Ctx, origin: &str, incarnation: &str, entry: &Value, bytes: &[u8]) -> Result<()> {
    let file = entry["file"].as_str().unwrap_or("");
    if !safe_file(file) || !safe_origin(origin) { bail!("not a chunk name"); }
    let (d, own) = (dir(ctx, origin), origin == ctx.settings.device_id);
    let one = list_lock(origin);
    if own && !one.held() { bail!("this computer's chunk list is held by another publisher; {file} goes out at the next publish"); }
    let kept = match read_list(ctx, origin) {
        Err(e) if own => return Err(e).context("this computer's chunk list could not be read; nothing is written this time"),
        r => r.unwrap_or(None),
    };
    let mut man = kept.filter(|m| m["incarnation"].as_str() == Some(incarnation)).unwrap_or_else(|| {
        let _ = std::fs::remove_dir_all(d.join("journal"));
        json!({"incarnation": incarnation, "last_seq": 0, "chunks": []})
    });
    write_atomic(&d.join("journal").join(file), bytes)?;
    let list = man["chunks"].as_array_mut().unwrap();
    list.retain(|c| c["file"] != entry["file"]);
    list.push(entry.clone());
    list.sort_by_key(|c| c["to"].as_i64().unwrap_or(0));
    if !own {
        let mut total: i64 = list.iter().map(|c| c["bytes"].as_i64().unwrap_or(0)).sum();
        while total > KEEP_BYTES && list.len() > 1 {
            let old = list.remove(0);
            total -= old["bytes"].as_i64().unwrap_or(0);
            let _ = std::fs::remove_file(d.join("journal").join(old["file"].as_str().unwrap_or("")));
        }
    }
    man["last_seq"] = json!(list.last().and_then(|c| c["to"].as_i64()).unwrap_or(0).max(man["last_seq"].as_i64().unwrap_or(0)));
    write_atomic(&d.join("manifest.json"), serde_json::to_string_pretty(&man)?.as_bytes())
}
