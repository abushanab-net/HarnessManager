//! Across computers: this computer's changes go to an append-only outbox, are published as immutable, signed
//! chunk files in the local journal store (mirrored to the sync folder, served to paired computers), and every
//! other computer's journal is merged into this database. Usage rows keep their request id, so a merge never
//! double counts; a chunk that cannot be read yet (a OneDrive placeholder, a half-copied file) is tried again
//! next round. Nothing here is fatal to the app.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{quota_point_in, upsert_session, upsert_usage};
use crate::model::{SessionMeta, UsageRow};
use crate::sync::peer::{identity, store};
use crate::sync::{folder, peer};
use crate::util::{now_ms, sha256_hex};
use crate::Ctx;

/// Chunks stay under this many bytes, so a cloud drive moves each in one go.
const CHUNK_BYTES: usize = 1_000_000;
/// A peer whose `hello` answered within this long is connected.
pub const CONNECTED_MS: i64 = 30_000;

/// One change, as a line of the journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: i64,
    pub ts: i64,
    pub op: String,
    pub key: String,
    pub data: Value,
}

/// Another computer, as this one last saw it.
#[derive(Debug, Clone, Serialize)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub os: String,
    pub app_version: String,
    pub incarnation: String,
    pub last_seen: Option<i64>,
    pub last_seq: i64,
    pub applied_seq: i64,
    pub transport: String,
    pub presence: Value,
    pub address: String,
    pub port: u16,
    pub cert_pem: String,
    pub pubkey: String,
    pub paired_at: Option<i64>,
    pub last_pull: Option<i64>,
    pub path: String,
    /// When the last `hello` answered.
    pub connected: Option<i64>,
    pub relayed_by: String,
}

/// Append one change to the outbox, inside the writer's own transaction.
pub fn record(c: &Connection, op: &str, key: &str, data: &Value) -> rusqlite::Result<()> {
    c.execute("insert into outbox(ts, op, key, data) values(?,?,?,?)", params![now_ms(), op, key, data.to_string()])?;
    Ok(())
}

/// Everything this computer has that its outbox does not, in one transaction: once when sync is turned on,
/// and again should the outbox be found empty beside a full usage table. Returns how many entries were added.
pub fn backfill(ctx: &Ctx) -> Result<usize> {
    let n = ctx.db.with(|c| {
        let tx = c.transaction()?;
        let now = now_ms();
        let dev = &ctx.db.device;
        let mut n = tx.execute("insert into outbox(ts, op, key, data) select ?1, 'usage', request_id,
            json_object('request_id', request_id, 'profile', profile, 'harness', harness, 'ts', ts, 'model', model, 'project', project,
                'session_id', session_id, 'input', input, 'output', output, 'cache_read', cache_read, 'cache_write', cache_write,
                'ttft_ms', ttft_ms, 'gen_ms', gen_ms, 'cost_usd', cost_usd, 'price_rev', price_rev)
            from usage u where device = ?2 and not exists (select 1 from outbox o where o.op = 'usage' and o.key = u.request_id)", params![now, dev])?;
        // sessions as the journal knows them (resolved, keyed harness/profile/id); the per-file cache rows have a '|' key
        n += tx.execute("insert into outbox(ts, op, key, data) select ?1, 'session', key, json from sessions s
            where device = ?2 and instr(key, '|') = 0 and not exists (select 1 from outbox o where o.op = 'session' and o.key = s.key)", params![now, dev])?;
        n += tx.execute("insert into outbox(ts, op, key, data) select ?1, 'quota', profile || '/' || tier, json_object('used', used, 'resets_at', resets_at, 'at', ts)
            from quota_hist q where not exists (select 1 from outbox o where o.op = 'quota' and o.key = q.profile || '/' || q.tier and json_extract(o.data, '$.at') = q.ts)", [now])?;
        n += tx.execute("insert into outbox(ts, op, key, data) select ?1, 'tag', kind || '/' || key, json_object('tags', json_group_array(tag)) from tags t
            group by kind, key having not exists (select 1 from outbox o where o.op = 'tag' and o.key = t.kind || '/' || t.key)", [now])?;
        tx.execute("insert or replace into kv values('journal:backfilled', '1')", [])?;
        tx.commit()?;
        Ok(n)
    })?;
    if n > 0 { crate::log::info(&format!("journal: {n} entries written from what this computer already had")); }
    Ok(n)
}

fn backfill_if_needed(ctx: &Ctx) -> Result<()> {
    let done = ctx.db.kv_get("journal:backfilled")?.as_deref() == Some("1");
    let (outbox, usage): (i64, i64) = ctx.db.with(|c| Ok(c.query_row("select (select count(*) from outbox), (select count(*) from usage)", [], |r| Ok((r.get(0)?, r.get(1)?)))?))?;
    if !done || (outbox == 0 && usage > 0) { backfill(ctx)?; }
    Ok(())
}

/// Unpublished entries as signed chunk files in the local store, then its manifest: readers only ever see whole
/// chunks. The store's manifest is the water line; a store lost with the data folder is simply made again.
pub fn publish(ctx: &Ctx) -> Result<()> {
    let me = &ctx.settings.device_id;
    let inc = &ctx.settings.incarnation;
    let published = store::manifest(ctx, me).filter(|m| m["incarnation"].as_str() == Some(inc)).and_then(|m| m["last_seq"].as_i64()).unwrap_or(0);
    let entries: Vec<Entry> = ctx.db.with(|c| {
        let mut st = c.prepare("select seq, ts, op, key, data from outbox where seq > ? order by seq")?;
        let v = st.query_map([published], |r| Ok(Entry { seq: r.get(0)?, ts: r.get(1)?, op: r.get(2)?, key: r.get(3)?,
            data: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(Value::Null) }))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    if entries.is_empty() {
        if store::manifest(ctx, me).is_none() { store::put_manifest_empty(ctx, me, inc)?; }
        ctx.db.kv_set("journal:published_seq", &published.to_string())?;
        return Ok(());
    }
    let id = identity::load(ctx)?;
    let mut chunks: Vec<(i64, i64, Vec<u8>)> = vec![];
    for e in &entries {
        let line = format!("{}\n", serde_json::to_string(e)?);
        match chunks.last_mut() {
            Some((_, to, bytes)) if bytes.len() + line.len() <= CHUNK_BYTES => { *to = e.seq; bytes.extend_from_slice(line.as_bytes()); }
            _ => chunks.push((e.seq, e.seq, line.into_bytes())),
        }
    }
    for (from, to, bytes) in &chunks {
        let (file, sha) = (format!("{inc}-{from}-{to}.jsonl"), sha256_hex(bytes));
        let sig = id.sign(identity::chunk_message(inc, *from, *to, &sha).as_bytes());
        store::put_chunk(ctx, me, inc, &json!({"file": file, "from": from, "to": to, "sha256": sha, "sig": sig, "bytes": bytes.len()}), bytes)?;
    }
    let last = entries.last().map(|e| e.seq).unwrap_or(published);
    ctx.db.kv_set("journal:published_seq", &last.to_string())?;
    crate::log::info(&format!("journal: published {} entries in {} chunks (to seq {last})", entries.len(), chunks.len()));
    Ok(())
}

/// The local store into this computer's folder in the sync folder: chunks the folder lacks, then the manifest.
fn mirror(ctx: &Ctx, root: &Path) -> Result<()> {
    let me = &ctx.settings.device_id;
    let Some(man) = store::manifest(ctx, me) else { return Ok(()) };
    let dir = folder::mine(ctx, root);
    let there = folder::read_manifest(&dir);
    if there.as_ref().map(|t| t["incarnation"] == man["incarnation"] && t["last_seq"] == man["last_seq"] && t["chunks"].as_array().map(|c| c.len()) == man["chunks"].as_array().map(|c| c.len())).unwrap_or(false) { return Ok(()); }
    for ch in man["chunks"].as_array().into_iter().flatten() {
        let file = ch["file"].as_str().unwrap_or("");
        if dir.join("journal").join(file).is_file() { continue; }
        folder::write_chunk(&dir, file, &store::chunk(ctx, me, file)?)?;
    }
    folder::write_manifest(&dir, &man)
}

/// One entry of another computer's journal into this database.
fn apply(tx: &Connection, device: &str, e: &Entry) -> Result<()> {
    match e.op.as_str() {
        "usage" => {
            let r: UsageRow = serde_json::from_value(e.data.clone()).context("usage entry")?;
            upsert_usage(tx, &r, device, e.data["cost_usd"].as_f64(), e.data["price_rev"].as_str())?;
        }
        "session" => {
            let m: SessionMeta = serde_json::from_value(e.data.clone()).context("session entry")?;
            upsert_session(tx, &e.key, &m, device)?;
        }
        "quota" => {
            let (profile, tier) = e.key.split_once('/').context("quota key")?;
            quota_point_in(tx, profile, tier, e.data["used"].as_f64().unwrap_or(0.0), e.data["resets_at"].as_i64(), e.data["at"].as_i64().unwrap_or(e.ts))?;
        }
        "tag" => {
            let (kind, key) = e.key.split_once('/').context("tag key")?;
            if !matches!(kind, "session" | "project") { bail!("tag kind {kind}"); }
            tx.execute("delete from tags where kind=? and key=?", [kind, key])?;
            for t in e.data["tags"].as_array().into_iter().flatten().filter_map(|t| t.as_str()) {
                tx.execute("insert or ignore into tags values(?,?,?)", [kind, key, t])?;
            }
        }
        // never a usage row: what was spent was spent
        "tombstone" => match e.key.split_once('/').context("tombstone key")? {
            ("session", k) => { tx.execute("delete from sessions where key=?", [k])?; }
            ("tag", k) => { let (kind, key) = k.split_once('/').context("tag key")?; tx.execute("delete from tags where kind=? and key=?", [kind, key])?; }
            (other, _) => bail!("tombstone of {other}"),
        },
        other => bail!("unknown op {other}"),
    }
    Ok(())
}

/// The public key this computer knows for an origin, "" when none yet.
pub fn pubkey_of(ctx: &Ctx, id: &str) -> Result<String> {
    ctx.db.with(|c| Ok(c.query_row("select coalesce(pubkey, '') from peers where device_id=?", [id], |r| r.get(0)).optional()?.unwrap_or_default()))
}

/// One computer's new chunks, in manifest order, each read through `read`, checked against its sha256 and (when
/// the origin's key is known) its signature, kept in the local store for relay, and applied in one transaction
/// with the cursor. Stops at the first chunk that cannot be read or checked: order matters, and the next round
/// tries again. Returns the sequence applied up to.
pub fn apply_manifest(ctx: &Ctx, id: &str, man: &Value, read: &dyn Fn(&str) -> Result<Vec<u8>>) -> Result<i64> {
    let inc = man["incarnation"].as_str().context("manifest without incarnation")?;
    let pubkey = pubkey_of(ctx, id)?;
    let mut cur: i64 = ctx.db.with(|c| Ok(c.query_row("select seq from cursors2 where device_id=? and incarnation=?", [id, inc], |r| r.get(0)).optional()?))?.unwrap_or(0);
    for ch in man["chunks"].as_array().into_iter().flatten() {
        let (file, from, to) = (ch["file"].as_str().unwrap_or(""), ch["from"].as_i64().unwrap_or(0), ch["to"].as_i64().unwrap_or(0));
        if to <= cur || file.is_empty() || file.contains(['/', '\\']) { continue; }
        let bytes = match read(file) {
            Ok(b) => b,
            Err(e) => { crate::log::info(&format!("journal: {file} from {id} is waiting for the sync service ({e:#})")); break; }
        };
        let sha = sha256_hex(&bytes);
        if sha != ch["sha256"].as_str().unwrap_or("") {
            crate::log::warn(&format!("journal: {file} from {id} does not match its manifest yet; tried again next round"));
            break;
        }
        if !pubkey.is_empty() && !identity::verify(&pubkey, identity::chunk_message(inc, from, to, &sha).as_bytes(), ch["sig"].as_str().unwrap_or("")) {
            crate::log::warn(&format!("journal: {file} from {id} is not signed by that computer; skipped"));
            break;
        }
        let text = String::from_utf8_lossy(&bytes);
        let (mut n, mut bad) = (0, 0);
        ctx.db.with(|c| {
            let tx = c.transaction()?;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let Ok(e) = serde_json::from_str::<Entry>(line) else { bad += 1; continue };
                if e.seq <= cur { continue; }
                match apply(&tx, id, &e) {
                    Ok(()) => n += 1,
                    Err(err) => { bad += 1; crate::log::warn(&format!("journal: entry {} of {file} from {id} skipped: {err:#}", e.seq)); }
                }
            }
            tx.execute("insert or replace into cursors2 values(?,?,?)", params![id, inc, to])?;
            tx.commit()?;
            Ok(())
        })?;
        if let Err(e) = store::put_chunk(ctx, id, inc, ch, &bytes) { crate::log::warn(&format!("journal: keeping {file} from {id} for relay: {e:#}")); }
        crate::log::info(&format!("journal: {n} entries from {id} applied ({file}{})", if bad > 0 { format!(", {bad} skipped") } else { String::new() }));
        cur = to;
    }
    Ok(cur)
}

/// What another computer says about itself (a `device.json`, from the folder or a peer), when it is signed by
/// the key it carries: its name, incarnation, roots and key are kept, the record is cached for relay. The key
/// is learned once and never replaced by a record.
pub fn learn_device(ctx: &Ctx, id: &str, dev: &Value) -> Result<bool> {
    if dev["id"].as_str() != Some(id) || !identity::record_ok(dev) { return Ok(false); }
    let known = pubkey_of(ctx, id)?;
    if !known.is_empty() && dev["pubkey"].as_str() != Some(&known) {
        crate::log::warn(&format!("computers: {id} presents another key than the one paired; its record is ignored"));
        return Ok(false);
    }
    ctx.db.with(|c| {
        c.execute("insert into peers(device_id, name, os, app_version, incarnation, roots, pubkey, transport) values(?1,?2,?3,?4,?5,?6,?7,'relay')
            on conflict(device_id) do update set name=coalesce(?2, name), os=coalesce(?3, os), app_version=coalesce(?4, app_version), incarnation=coalesce(?5, incarnation),
            roots=coalesce(?6, roots), pubkey=?7",
            params![id, dev["name"].as_str(), dev["os"].as_str(), dev["app_version"].as_str(), dev["incarnation"].as_str(), dev["roots"].is_object().then(|| dev["roots"].to_string()), dev["pubkey"].as_str()])?;
        Ok(())
    })?;
    store::put_device(ctx, id, dev)?;
    Ok(true)
}

/// Every other computer's journal, and what each says about itself, into this database. Per computer: an
/// error is logged and the next one is tried.
fn merge(ctx: &Ctx, root: &Path) -> Result<()> {
    for (id, dir) in folder::others(ctx, root) {
        let dev = crate::util::read_json(&dir.join("device.json")).unwrap_or(Value::Null);
        if dev.is_object() { learn_device(ctx, &id, &dev)?; }
        let presence = std::fs::read_to_string(dir.join("presence.json")).ok().filter(|s| serde_json::from_str::<Value>(s).is_ok());
        let man = folder::read_manifest(&dir);
        let applied = match &man {
            Some(m) => match apply_manifest(ctx, &id, m, &|file| Ok(std::fs::read(dir.join("journal").join(file))?)) {
                Ok(seq) => Some(seq),
                Err(e) => { crate::log::warn(&format!("journal: merging {id}: {e:#}")); None }
            },
            None => None,
        };
        let seen = [dev["updated_at"].as_i64(), presence.as_deref().and_then(|p| serde_json::from_str::<Value>(p).ok()).and_then(|p| p["at"].as_i64())].into_iter().flatten().max();
        ctx.db.with(|c| {
            c.execute("insert into peers(device_id, name, os, app_version, incarnation, last_seen, last_seq, applied_seq, roots, transport, presence) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,'folder',?10)
                on conflict(device_id) do update set name=coalesce(?2, name), os=coalesce(?3, os), app_version=coalesce(?4, app_version), incarnation=coalesce(?5, incarnation),
                last_seen=max(coalesce(?6, 0), coalesce(last_seen, 0)), last_seq=max(coalesce(?7, 0), coalesce(last_seq, 0)), applied_seq=max(coalesce(?8, 0), coalesce(applied_seq, 0)), roots=coalesce(?9, roots), transport='folder',
                presence=case when coalesce(json_extract(?10, '$.at'), 0) >= coalesce(json_extract(presence, '$.at'), 0) then coalesce(?10, presence) else presence end",
                params![id, dev["name"].as_str(), dev["os"].as_str(), dev["app_version"].as_str(), dev["incarnation"].as_str().or(man.as_ref().and_then(|m| m["incarnation"].as_str())),
                    seen, man.as_ref().and_then(|m| m["last_seq"].as_i64()), applied, dev["roots"].is_object().then(|| dev["roots"].to_string()), presence])?;
            Ok(())
        })?;
    }
    Ok(())
}

/// Sync in any form: through the folder or straight to paired computers.
pub fn on(ctx: &Ctx) -> bool { ctx.settings.sync_mode != "off" || ctx.settings.direct_connection }

/// One round: what this computer has, out; what the others have, in (through the folder, then from the paired
/// computers). What happened is kept for the status.
pub fn round(ctx: &Ctx) -> Result<Value> {
    let r = (|| -> Result<()> {
        if !on(ctx) { bail!("sync is off"); }
        backfill_if_needed(ctx)?;
        publish(ctx)?;
        if ctx.settings.sync_mode == "folder" {
            let root = folder::root(ctx)?;
            mirror(ctx, &root)?;
            folder::write_device(ctx, &root)?;
            folder::write_presence(ctx, &root)?;
            merge(ctx, &root)?;
        }
        if ctx.settings.direct_connection { peer::pull_all(ctx); }
        Ok(())
    })();
    let error = r.err().map(|e| format!("{e:#}"));
    if let Some(e) = &error { crate::log::warn(&format!("computers: {e}")); }
    ctx.db.kv_set("computers:last", &json!({"at": now_ms(), "error": error}).to_string())?;
    status(ctx)
}

/// `presence.json` alone, once a minute while the app runs.
pub fn presence_tick(ctx: &Ctx) -> Result<()> {
    if ctx.settings.sync_mode != "folder" { return Ok(()); }
    folder::write_presence(ctx, &folder::root(ctx)?)
}

pub fn peers(ctx: &Ctx) -> Result<Vec<Peer>> {
    ctx.db.with(|c| {
        let mut st = c.prepare("select device_id, name, os, app_version, last_seen, last_seq, applied_seq, transport, presence, incarnation, address, port, cert_pem, pubkey, paired_at, last_pull, path, connected, relayed_by
            from peers order by name, device_id")?;
        let s = |r: &rusqlite::Row, i: usize| -> rusqlite::Result<String> { Ok(r.get::<_, Option<String>>(i)?.unwrap_or_default()) };
        let v = st.query_map([], |r| Ok(Peer { id: r.get(0)?, name: s(r, 1)?, os: s(r, 2)?, app_version: s(r, 3)?, last_seen: r.get(4)?, last_seq: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            applied_seq: r.get::<_, Option<i64>>(6)?.unwrap_or(0), transport: s(r, 7)?,
            presence: r.get::<_, Option<String>>(8)?.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or(Value::Null), incarnation: s(r, 9)?,
            address: s(r, 10)?, port: r.get::<_, Option<i64>>(11)?.unwrap_or(0) as u16, cert_pem: s(r, 12)?, pubkey: s(r, 13)?, paired_at: r.get(14)?, last_pull: r.get(15)?,
            path: s(r, 16)?, connected: r.get(17)?, relayed_by: s(r, 18)? }))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })
}

/// Every computer's name by id, this one included; an unknown id shows as itself.
pub fn device_names(ctx: &Ctx) -> HashMap<String, String> {
    let mut m: HashMap<String, String> = peers(ctx).unwrap_or_default().into_iter().map(|p| (p.id.clone(), if p.name.is_empty() { p.id } else { p.name })).collect();
    m.insert(ctx.db.device.clone(), ctx.settings.device_name.clone());
    m
}

/// A session id a peer's presence says is open there, by presence seen in the last five minutes.
pub fn open_elsewhere(peers: &[Peer], now: i64) -> std::collections::HashSet<String> {
    peers.iter().filter(|p| p.presence["at"].as_i64().map(|t| now - t < 300_000).unwrap_or(false))
        .flat_map(|p| p.presence["running"].as_array().into_iter().flatten().filter_map(|r| r["session_id"].as_str().map(String::from)).collect::<Vec<_>>()).collect()
}

/// The Computers page: this computer, the last round, and the others.
pub fn status(ctx: &Ctx) -> Result<Value> {
    let s = &ctx.settings;
    let now = now_ms();
    let last_seq: i64 = ctx.db.with(|c| Ok(c.query_row("select coalesce(max(seq), 0) from outbox", [], |r| r.get(0))?))?;
    let published: i64 = ctx.db.kv_get("journal:published_seq")?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let last = ctx.db.kv_get("computers:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null);
    let pairing = ctx.db.kv_get("pairing:current")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).filter(|p| p["expires_at"].as_i64().unwrap_or(0) > now).unwrap_or(Value::Null);
    let peers: Vec<Value> = peers(ctx)?.into_iter().map(|p| json!({"id": p.id, "name": p.name, "os": p.os, "app_version": p.app_version, "last_seen": p.last_seen,
        "last_seq": p.last_seq, "applied_seq": p.applied_seq, "transport": p.transport, "presence_at": p.presence["at"], "running": p.presence["running"].as_array().cloned().unwrap_or_default(),
        "address": p.address, "path": p.path, "connected": p.connected.map(|t| now - t < CONNECTED_MS).unwrap_or(false), "last_pull": p.last_pull, "paired_at": p.paired_at,
        "relayed_by": p.relayed_by, "direct": !p.cert_pem.is_empty()})).collect();
    Ok(json!({"mode": s.sync_mode, "folder": s.sync_folder, "direct_connection": s.direct_connection,
        "this": {"id": s.device_id, "name": s.device_name, "incarnation": s.incarnation, "last_seq": last_seq, "published_seq": published, "port": s.listen_port, "pubkey": identity::pubkey(ctx), "pairing": pairing},
        "last_run": last["at"], "error": last["error"], "peers": peers}))
}

/// Change the Computers settings from `{device_name?, sync_mode?, sync_folder?, sync_transcripts?, sync_presence?,
/// direct_connection?, listen_port?}`, saved and reopened so the journal follows the mode; turning sync on queues
/// a backfill for the next round.
pub fn configure(data: &Path, home: &Path, a: &Value) -> Result<Ctx> {
    let mut ctx = Ctx::open_at(data, home)?;
    let before = on(&ctx);
    let s = &mut ctx.settings;
    if let Some(n) = a["device_name"].as_str() {
        let n = n.trim();
        if n.is_empty() || n.chars().count() > 60 { bail!("give this computer a name of up to 60 characters"); }
        s.device_name = n.into();
    }
    if let Some(f) = a["sync_folder"].as_str() { s.sync_folder = f.trim().trim_end_matches(['/', '\\']).into(); }
    if let Some(m) = a["sync_mode"].as_str() {
        match m {
            "off" | "folder" => s.sync_mode = m.into(),
            "hub" => bail!("the hub comes with a later version; choose a folder for now"),
            _ => bail!("the mode is off or folder"),
        }
    }
    if let Some(b) = a["sync_transcripts"].as_bool() { s.sync_transcripts = b; }
    if let Some(b) = a["sync_presence"].as_bool() { s.sync_presence = b; }
    if let Some(b) = a["direct_connection"].as_bool() { s.direct_connection = b; }
    if let Some(p) = a["listen_port"].as_i64() {
        if !(0..=65535).contains(&p) { bail!("the port is a number up to 65535, or 0 to pick one"); }
        s.listen_port = p as u16;
    }
    if s.sync_mode == "folder" {
        let p = Path::new(&s.sync_folder);
        if s.sync_folder.is_empty() || !p.is_absolute() || !p.is_dir() { bail!("choose an existing folder (its full path) to sync through"); }
    }
    ctx.save_settings()?;
    if on(&ctx) != before { ctx.db.kv_set("journal:backfilled", "0")?; }
    Ctx::open_at(data, home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::Prices;

    /// A computer with sync on through `shared`, its data in `dir/<name>`; its device key under the test prefix,
    /// deleted when the context is dropped.
    fn computer(dir: &Path, name: &str, shared: &Path) -> Ctx {
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let data = dir.join(name);
        let mut ctx = Ctx::open_at(&data, &dir.join("home")).unwrap();
        ctx.settings.device_name = name.into();
        ctx.settings.sync_mode = "folder".into();
        ctx.settings.sync_folder = shared.display().to_string();
        ctx.save_settings().unwrap();
        Ctx::open_at(&data, &dir.join("home")).unwrap()
    }

    fn row(id: &str, ts: i64) -> UsageRow {
        UsageRow { request_id: id.into(), profile: "work".into(), harness: "claude".into(), ts, model: "claude-opus-5-5".into(), project: "C:\\p".into(),
            session_id: "s1".into(), input: 100, output: 10, cache_read: 50, cache_write: 0, ttft_ms: Some(900), gen_ms: None }
    }

    fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

    /// Deletes the computers' device keys from the credential store when the test ends.
    struct Keys(Vec<String>);
    impl Drop for Keys {
        fn drop(&mut self) { for id in &self.0 { let _ = crate::secrets::delete(&identity::target(id)); } }
    }
    fn keys(ctxs: &[&Ctx]) -> Keys { Keys(ctxs.iter().map(|c| c.settings.device_id.clone()).collect()) }

    #[test]
    fn what_a_records_b_merges_once_with_a_as_its_device() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let prices: Prices = [("claude-opus-5-5".to_string(), crate::pricing::Price { input: 10.0, output: 10.0, cache_read: 1.0, cache_write: 1.0 })].into_iter().collect();
        assert_eq!(a.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &prices).unwrap(), 2);
        a.db.tag_set("project", "github.com/o/r", &["release".into()]).unwrap();
        a.db.quota_point("work", "session", 40.0, Some(9_000), 1_500).unwrap();
        assert_eq!(count(&a, "select count(*) from outbox"), 4);
        let st = round(&a).unwrap();
        assert_eq!(st["this"]["published_seq"], 4, "{st}");
        assert!(shared.join("devices").join(&a.settings.device_id).join("manifest.json").exists());

        let st = round(&b).unwrap();
        assert_eq!(st["error"], Value::Null, "{st}");
        assert_eq!(st["peers"][0]["name"], "A");
        assert_eq!(st["peers"][0]["applied_seq"], 4);
        assert_eq!(count(&b, "select count(*) from usage"), 2);
        let (dev, cost, rev): (String, f64, String) = b.db.with(|c| Ok(c.query_row("select device, cost_usd, price_rev from usage where request_id='claude:1'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
        assert_eq!(dev, a.settings.device_id);
        assert!((cost - 0.00115).abs() < 1e-9, "the cost is the one frozen on A ({cost})");
        assert_eq!(rev, crate::pricing::rev(&prices));
        assert_eq!(b.db.tags_of("project").unwrap()["github.com/o/r"], vec!["release".to_string()]);
        assert_eq!(b.db.quota_points("work", "session", 0).unwrap(), vec![(1_500, 40.0, Some(9_000))]);
        // the views: all computers, or this one
        assert_eq!(crate::service::overview(&b, None, None, None).unwrap().requests, 2);
        assert_eq!(crate::service::overview(&b, None, None, Some("here")).unwrap().requests, 0);
        let home = crate::dash::dashboard(&b, 24 * 365 * 60, None, None).unwrap();
        assert_eq!(home["tiles"]["requests"]["by_device"][0], json!({"id": a.settings.device_id, "name": "A", "value": 2}), "{}", home["tiles"]);
        // B's own outbox stays B's: nothing merged bounces back
        assert_eq!(count(&b, "select count(*) from outbox"), 0);

        // merging again adds nothing
        round(&b).unwrap();
        assert_eq!(count(&b, "select count(*) from usage"), 2);
        assert_eq!(count(&b, "select count(*) from quota_hist"), 1);
        assert_eq!(count(&b, "select count(*) from cursors2"), 1);
    }

    #[test]
    fn a_chunk_that_does_not_match_its_manifest_waits_and_is_applied_once_fixed() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        a.db.insert_usage(&[row("claude:1", 1_000)], &Prices::new()).unwrap();
        round(&a).unwrap();
        let dir = shared.join("devices").join(&a.settings.device_id);
        let mut man = folder::read_manifest(&dir).unwrap();
        let good = man["chunks"][0]["sha256"].clone();
        man["chunks"][0]["sha256"] = json!("0000");
        folder::write_manifest(&dir, &man).unwrap();
        let st = round(&b).unwrap();
        assert_eq!(st["error"], Value::Null, "a bad chunk is not an error, it is retried: {st}");
        assert_eq!(count(&b, "select count(*) from usage"), 0);
        assert_eq!(st["peers"][0]["applied_seq"], 0);
        man["chunks"][0]["sha256"] = good;
        folder::write_manifest(&dir, &man).unwrap();
        round(&b).unwrap();
        assert_eq!(count(&b, "select count(*) from usage"), 1);
        round(&b).unwrap();
        assert_eq!(count(&b, "select count(*) from usage"), 1);
    }

    #[test]
    fn backfill_writes_what_was_there_before_sync_once() {
        let t = tempfile::tempdir().unwrap();
        // sync off: rows are stored, nothing is journaled
        let ctx = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        assert!(!ctx.db.journal);
        ctx.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &Prices::new()).unwrap();
        ctx.db.tag_set("session", "work/s1", &["x".into(), "y".into()]).unwrap();
        ctx.db.quota_point("work", "week", 10.0, None, 1_000).unwrap();
        ctx.db.quota_point("work", "week", 12.0, None, 2_000).unwrap();
        assert_eq!(count(&ctx, "select count(*) from outbox"), 0);
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let ctx = computer(t.path(), "A", &shared);
        let _k = keys(&[&ctx]);
        ctx.db.insert_usage(&[row("claude:3", 3_000)], &Prices::new()).unwrap();
        assert_eq!(count(&ctx, "select count(*) from outbox"), 1, "sync on: the new row is journaled as it comes");
        assert_eq!(backfill(&ctx).unwrap(), 5, "two usage rows, one tag list, two readings");
        assert_eq!(count(&ctx, "select count(*) from outbox"), 6);
        assert_eq!(backfill(&ctx).unwrap(), 0);
        let tags: String = ctx.db.with(|c| Ok(c.query_row("select data from outbox where op='tag'", [], |r| r.get(0))?)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&tags).unwrap()["tags"], json!(["x", "y"]));
    }

    #[test]
    fn a_database_from_before_gets_its_columns_and_this_computers_stamp() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("A");
        std::fs::create_dir_all(&data).unwrap();
        {
            let c = Connection::open(data.join("hm.db")).unwrap();
            c.execute_batch("create table usage(request_id text primary key, profile text not null, harness text not null, ts integer not null,
                day text not null, model text not null, project text not null, session_id text not null,
                input integer not null, output integer not null, cache_read integer not null, cache_write integer not null, ttft_ms integer, gen_ms integer);
                create table sessions(key text primary key, profile text not null, harness text not null, path text not null, mtime integer not null, len integer not null, json text not null);
                create table kv(k text primary key, v text not null);
                insert into kv values('migrated:session-failed', '1');
                insert into usage values('claude:old', 'work', 'claude', 1000, '2026-01-01', 'm', 'C:\\p', 's', 1, 1, 0, 0, null, null);
                insert into sessions values('claude|work|C:\\p\\s.jsonl', 'work', 'claude', 'C:\\p\\s.jsonl', 1, 1, '{}');").unwrap();
        }
        let ctx = Ctx::open_at(&data, &t.path().join("home")).unwrap();
        assert!(!ctx.settings.device_id.is_empty() && !ctx.settings.incarnation.is_empty() && !ctx.settings.device_name.is_empty());
        let (dev, cost): (String, Option<f64>) = ctx.db.with(|c| Ok(c.query_row("select device, cost_usd from usage", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
        assert_eq!(dev, ctx.settings.device_id);
        assert_eq!(cost, None);
        assert_eq!(count(&ctx, "select count(*) from sessions where device <> ''"), 1);
        // the ids stay the same when opened again
        let again = Ctx::open_at(&data, &t.path().join("home")).unwrap();
        assert_eq!((again.settings.device_id, again.settings.incarnation), (ctx.settings.device_id.clone(), ctx.settings.incarnation.clone()));
        // old rows priced once a table is known
        let prices: Prices = [("m".to_string(), crate::pricing::Price { input: 1.0, output: 1.0, cache_read: 1.0, cache_write: 1.0 })].into_iter().collect();
        assert_eq!(ctx.db.price_unpriced(&prices).unwrap(), 1);
        assert_eq!(ctx.db.price_unpriced(&prices).unwrap(), 0);
    }

    #[test]
    fn a_merged_session_and_its_tombstone() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let m = SessionMeta { harness: "codex".into(), profile: "work".into(), id: "abc".into(), title: "fix the build".into(), cwd: "C:\\p".into(), started_at: 1, last_active_at: 5,
            project_key: "github.com/o/r".into(), project_name: "r".into(), source_path: "C:\\p\\abc.jsonl".into(), ..Default::default() };
        a.db.journal_session(&m).unwrap();
        a.db.journal_session(&m).unwrap();
        assert_eq!(count(&a, "select count(*) from outbox"), 1, "the same session again is not journaled twice");
        round(&a).unwrap();
        round(&b).unwrap();
        let remote = b.db.remote_sessions().unwrap();
        assert_eq!(remote.len(), 1);
        assert_eq!((remote[0].0.as_str(), remote[0].1.title.as_str(), remote[0].1.project_key.as_str()), (a.settings.device_id.as_str(), "fix the build", "github.com/o/r"));
        let list = crate::service::sessions(&b, &crate::service::SessionFilter::default()).unwrap();
        assert_eq!((list.len(), list[0].device_name.as_str(), list[0].open_elsewhere), (1, "A", false));
        assert!(crate::service::sessions(&b, &crate::service::SessionFilter { scope: Some("here".into()), ..Default::default() }).unwrap().is_empty());
        let projects = crate::service::projects(&b, None, None).unwrap();
        assert_eq!((projects.len(), projects[0].here, projects[0].devices[0]["name"].as_str()), (1, false, Some("A")));
        a.db.forget_profile("work").unwrap();
        round(&a).unwrap();
        round(&b).unwrap();
        assert!(b.db.remote_sessions().unwrap().is_empty());
    }
}
