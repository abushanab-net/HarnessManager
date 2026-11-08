//! The local journal store: `<data>/journal-cache/<origin>/` holds a manifest and chunk files in the folder
//! layout, for this computer (what it published) and for every origin it received, so a peer can pull any of
//! them (relay). Other origins' caches are pruned to the last 50 MB, oldest first, never below the newest chunk.

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::util::{read_json, write_atomic};
use crate::Ctx;

const KEEP_BYTES: i64 = 50_000_000;

/// A computer's id, as it names a folder here: another computer can send any text as one, and `..\` must not
/// climb out of the store.
pub fn safe_origin(s: &str) -> bool { !s.is_empty() && !s.starts_with('.') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_') }

pub fn dir(ctx: &Ctx, origin: &str) -> PathBuf { ctx.data.join("journal-cache").join(if safe_origin(origin) { origin } else { ".invalid" }) }

pub fn manifest(ctx: &Ctx, origin: &str) -> Option<Value> { read_json(&dir(ctx, origin).join("manifest.json")) }

pub fn device(ctx: &Ctx, origin: &str) -> Option<Value> { read_json(&dir(ctx, origin).join("device.json")) }

pub fn put_device(ctx: &Ctx, origin: &str, v: &Value) -> Result<()> { write_atomic(&dir(ctx, origin).join("device.json"), v.to_string().as_bytes()) }

/// A chunk file's bytes; the name is checked to be a plain file name first.
pub fn chunk(ctx: &Ctx, origin: &str, file: &str) -> Result<Vec<u8>> {
    if file.is_empty() || file.contains(['/', '\\']) || origin.contains(['/', '\\']) { bail!("not a chunk name"); }
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

/// Keep one chunk and list it in the origin's manifest (a new incarnation starts the list over).
pub fn put_chunk(ctx: &Ctx, origin: &str, incarnation: &str, entry: &Value, bytes: &[u8]) -> Result<()> {
    let d = dir(ctx, origin);
    let mut man = manifest(ctx, origin).filter(|m| m["incarnation"].as_str() == Some(incarnation)).unwrap_or_else(|| {
        let _ = std::fs::remove_dir_all(d.join("journal"));
        json!({"incarnation": incarnation, "last_seq": 0, "chunks": []})
    });
    let file = entry["file"].as_str().unwrap_or("");
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
