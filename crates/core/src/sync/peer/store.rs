//! The local journal store: `<data>/journal-cache/<origin>/` holds a manifest and chunk files in the folder
//! layout, for this computer (what it published) and for every origin it received, so a peer can pull any of
//! them (relay). Other origins' caches are pruned to the last 50 MB, oldest first, never below the newest chunk.
//! This computer's own journal is kept whole, its older chunks joined into files of up to 1 MB.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::util::{read_json, safe_name, sha256_hex, write_atomic};
use crate::Ctx;

const KEEP_BYTES: i64 = 50_000_000;

/// A computer's id, as it names a folder here: another computer can send any text as one, and `..\` must not
/// climb out of the store.
pub fn safe_origin(s: &str) -> bool { safe_name(s) }

/// A file name another computer sent (a chunk's, a transcript's): a plain name. A check for slashes alone lets a
/// drive or a stream through on Windows.
pub fn safe_file(s: &str) -> bool { safe_name(s) }

pub fn dir(ctx: &Ctx, origin: &str) -> PathBuf { ctx.data.join("journal-cache").join(if safe_origin(origin) { origin } else { ".invalid" }) }

pub fn manifest(ctx: &Ctx, origin: &str) -> Option<Value> { read_json(&dir(ctx, origin).join("manifest.json")) }

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

/// This computer's chunks older than its newest `keep`, joined into files of up to `max` bytes once that saves
/// `keep` files: the same entries in the same order, each joined file signed anew by `sign(from, to, sha256)`. A
/// reader skips the entries its cursor is past, so one that took some of the small files takes the rest from the
/// joined one. The list is written before the files it replaces go: a reader that still asks for one tries again.
pub fn join_old(ctx: &Ctx, keep: usize, max: usize, sign: &dyn Fn(i64, i64, &str) -> String) -> Result<()> {
    let me = &ctx.settings.device_id;
    let Some(mut man) = manifest(ctx, me) else { return Ok(()) };
    let (inc, list) = (man["incarnation"].as_str().unwrap_or("").to_string(), man["chunks"].as_array().cloned().unwrap_or_default());
    if list.len() <= keep { return Ok(()); }
    let (old, kept) = list.split_at(list.len() - keep);
    let (seq, size) = (|c: &Value, k: &str| c[k].as_i64().unwrap_or(0), |c: &Value| c["bytes"].as_i64().unwrap_or(0) as usize);
    // runs of chunks that follow each other without a hole and fit one file together
    let mut runs: Vec<Vec<&Value>> = vec![];
    for ch in old {
        match runs.last_mut() {
            Some(r) if seq(ch, "from") == seq(r[r.len() - 1], "to") + 1 && r.iter().map(|c| size(c)).sum::<usize>() + size(ch) <= max => r.push(ch),
            _ => runs.push(vec![ch]),
        }
    }
    if old.len() - runs.len() < keep { return Ok(()); }
    let dir = dir(ctx, me).join("journal");
    let (mut out, mut gone): (Vec<Value>, Vec<&str>) = (vec![], vec![]);
    for r in runs {
        if r.len() == 1 { out.push(r[0].clone()); continue; }
        let mut bytes = vec![];
        for ch in &r { bytes.extend(chunk(ctx, me, ch["file"].as_str().unwrap_or(""))?); }
        let (from, to, sha) = (seq(r[0], "from"), seq(r[r.len() - 1], "to"), sha256_hex(&bytes));
        let file = format!("{inc}-{from}-{to}.jsonl");
        write_atomic(&dir.join(&file), &bytes)?;
        out.push(json!({"file": file, "from": from, "to": to, "sha256": sha, "sig": sign(from, to, &sha), "bytes": bytes.len()}));
        gone.extend(r.iter().filter_map(|c| c["file"].as_str()));
    }
    out.extend(kept.iter().cloned());
    man["chunks"] = json!(out);
    write_atomic(&dir.with_file_name("manifest.json"), serde_json::to_string_pretty(&man)?.as_bytes())?;
    crate::log::info(&format!("journal: {} old chunks joined into files of up to {} KB", gone.len(), max / 1000));
    for f in gone { let _ = std::fs::remove_file(dir.join(f)); }
    Ok(())
}

/// Keep one chunk and list it in the origin's manifest (a new incarnation starts the list over).
pub fn put_chunk(ctx: &Ctx, origin: &str, incarnation: &str, entry: &Value, bytes: &[u8]) -> Result<()> {
    let file = entry["file"].as_str().unwrap_or("");
    if !safe_file(file) || !safe_origin(origin) { bail!("not a chunk name"); }
    let d = dir(ctx, origin);
    let mut man = manifest(ctx, origin).filter(|m| m["incarnation"].as_str() == Some(incarnation)).unwrap_or_else(|| {
        let _ = std::fs::remove_dir_all(d.join("journal"));
        json!({"incarnation": incarnation, "last_seq": 0, "chunks": []})
    });
    write_atomic(&d.join("journal").join(file), bytes)?;
    let list = man["chunks"].as_array_mut().unwrap();
    list.retain(|c| c["file"] != entry["file"]);
    list.push(entry.clone());
    list.sort_by_key(|c| c["to"].as_i64().unwrap_or(0));
    if origin != ctx.settings.device_id {
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
