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
use crate::util::{now_ms, safe_name, sha256_hex};
use crate::Ctx;

/// Chunks stay under this many bytes, so a cloud drive moves each in one go.
const CHUNK_BYTES: usize = 1_000_000;
/// This computer's newest chunks stay as they were cut, one whenever a tick finds news; the older ones are joined.
const SMALL_KEPT: usize = 100;
/// No more than this is read of a chunk another computer sends: four times the size they are cut at, as one long
/// entry may run over it.
pub const MAX_CHUNK_READ: u64 = 4 * 1024 * 1024;
/// And of a JSON answer (a chunk list, presence, the hub's devices).
pub const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
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
    /// When that presence was written, by this computer's clock; `presence.at` is by the other computer's.
    pub presence_seen: Option<i64>,
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
    /// Why the last attempt to reach it failed; empty once one succeeds.
    pub last_error: String,
    /// The `host:port` it last answered at.
    pub last_good: String,
    /// Entries of its journal that were passed because the list read did not hold them, `{incarnation, from, to}`,
    /// or null: one range, so after several holes it may name more than is missing.
    pub gap: Value,
}

/// Append one change to the outbox, inside the writer's own transaction.
pub fn record(c: &Connection, op: &str, key: &str, data: &Value) -> rusqlite::Result<()> { record_at(c, now_ms(), op, key, data) }

/// `record` for a change whose time the merge compares (a tag list): the entry carries the time kept here.
pub fn record_at(c: &Connection, ts: i64, op: &str, key: &str, data: &Value) -> rusqlite::Result<()> {
    c.execute("insert into outbox(ts, op, key, data) values(?,?,?,?)", params![ts, op, key, data.to_string()])?;
    Ok(())
}

/// Everything this computer has that its outbox does not, in one transaction: once when sync is turned on,
/// and again should the outbox be found empty beside a full usage table. Returns how many entries were added.
pub fn backfill(ctx: &Ctx) -> Result<usize> { record_missing(ctx, true) }

/// The backfill's work. The limit readings only when asked: `quota_point` journals each with its row, so later
/// the only rows without an entry are other computers' merged ones, which are not this computer's to publish.
/// Usage rows are only ever added, so the catch-up (not the backfill) looks at those added since the last look:
/// `journal:usage_mark` is the row it reached. A row below the mark becomes this computer's in two places only:
/// the stamp at the start, which removes the mark with it, and a restore that adopts a computer, which asks for
/// the backfill.
fn record_missing(ctx: &Ctx, quota: bool) -> Result<usize> {
    let n = ctx.db.with(|c| {
        let tx = c.transaction()?;
        let now = now_ms();
        let dev = &ctx.db.device;
        let top: i64 = tx.query_row("select coalesce(max(rowid), 0) from usage", [], |r| r.get(0))?;
        let mark: i64 = if quota { 0 } else { tx.query_row("select v from kv where k = 'journal:usage_mark'", [], |r| r.get::<_, String>(0)).optional()?.and_then(|v| v.parse().ok()).filter(|m| *m <= top).unwrap_or(0) };
        let mut n = tx.execute("insert into outbox(ts, op, key, data) select ?1, 'usage', request_id,
            json_object('request_id', request_id, 'profile', profile, 'harness', harness, 'ts', ts, 'model', model, 'project', project,
                'session_id', session_id, 'input', input, 'output', output, 'cache_read', cache_read, 'cache_write', cache_write,
                'ttft_ms', ttft_ms, 'gen_ms', gen_ms, 'cost_usd', cost_usd, 'price_rev', price_rev)
            from usage u where u.rowid > ?3 and device = ?2 and not exists (select 1 from outbox o where o.op = 'usage' and o.key = u.request_id)", params![now, dev, mark])?;
        tx.execute("insert or replace into kv values('journal:usage_mark', ?)", [top.to_string()])?;
        // sessions as the journal knows them (resolved, keyed harness/profile/id); the per-file cache rows have a '|' key
        n += tx.execute("insert into outbox(ts, op, key, data) select ?1, 'session', key, json from sessions s
            where device = ?2 and instr(key, '|') = 0 and not exists (select 1 from outbox o where o.op = 'session' and o.key = s.key)", params![now, dev])?;
        // `not in`, one lookup per reading: a correlated `not exists` parsed the JSON of every entry of that limit for each row
        if quota {
            n += tx.execute("insert into outbox(ts, op, key, data) select ?1, 'quota', profile || '/' || tier, json_object('used', used, 'resets_at', resets_at, 'at', ts, 'account', account)
                from (select distinct profile, tier, used, resets_at, ts, account from quota_hist where coalesce(device, ?2) = ?2)
                where (profile || '/' || tier, ts) not in (select key, coalesce(json_extract(data, '$.at'), 0) from outbox where op = 'quota')", params![now, dev])?;
        }
        // a tag list journaled late carries the time of its change, so it never passes for a newer one
        tx.execute("insert or ignore into tag_times select kind, key, ?1, ?2 from tags t
            group by kind, key having not exists (select 1 from outbox o where o.op = 'tag' and o.key = t.kind || '/' || t.key)", params![now, dev])?;
        n += tx.execute("insert into outbox(ts, op, key, data) select (select ts from tag_times x where x.kind = t.kind and x.key = t.key), 'tag', kind || '/' || key, json_object('tags', json_group_array(tag)) from tags t
            group by kind, key having not exists (select 1 from outbox o where o.op = 'tag' and o.key = t.kind || '/' || t.key)", [])?;
        tx.execute("insert or replace into kv values('journal:backfilled', '1')", [])?;
        tx.commit()?;
        Ok(n)
    })?;
    if n > 0 { crate::log::info(&format!("journal: {n} entries written from what this computer already had")); }
    Ok(n)
}

fn backfill_if_needed(ctx: &Ctx) -> Result<()> {
    let done = ctx.db.kv_get("journal:backfilled")?.as_deref() == Some("1");
    // this computer's own rows: merged ones must not trigger a backfill (which would journal merged tags as ours)
    let (outbox, usage): (i64, i64) = ctx.db.with(|c| Ok(c.query_row("select (select count(*) from outbox), (select count(*) from usage where device = ?)", [&ctx.db.device], |r| Ok((r.get(0)?, r.get(1)?)))?))?;
    if !done || (outbox == 0 && usage > 0) { backfill(ctx)?; }
    Ok(())
}

/// The outbox's entries after `after` up to `upto`, in order.
fn outbox(ctx: &Ctx, after: i64, upto: i64) -> Result<Vec<Entry>> {
    ctx.db.with(|c| {
        let mut st = c.prepare("select seq, ts, op, key, data from outbox where seq > ? and seq <= ? order by seq")?;
        let v = st.query_map([after, upto], |r| Ok(Entry { seq: r.get(0)?, ts: r.get(1)?, op: r.get(2)?, key: r.get(3)?,
            data: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(Value::Null) }))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })
}

/// An entry as its line of a chunk file.
fn line(e: &Entry) -> String { format!("{}\n", serde_json::to_string(e).unwrap_or_default()) }

/// Unpublished entries as signed chunk files in the local store, then its manifest: readers only ever see whole
/// chunks. The store's manifest is the water line; a store lost with the data folder is simply made again.
pub fn publish(ctx: &Ctx) -> Result<()> {
    // the refresh round, the 10 s tick and the hub's events thread all publish: one at a time
    let one = crate::util::NamedLock::take(&format!("journal-{}", ctx.settings.device_id));
    let me = &ctx.settings.device_id;
    let inc = &ctx.settings.incarnation;
    let published = store::manifest(ctx, me).filter(|m| m["incarnation"].as_str() == Some(inc)).and_then(|m| m["last_seq"].as_i64()).unwrap_or(0);
    let entries = outbox(ctx, published, i64::MAX)?;
    if entries.is_empty() {
        if store::manifest(ctx, me).is_none() { store::put_manifest_empty(ctx, me, inc)?; }
        ctx.db.kv_set("journal:published_seq", &published.to_string())?;
        return Ok(());
    }
    let id = identity::load(ctx)?;
    let mut chunks: Vec<(i64, i64, Vec<u8>)> = vec![];
    for e in &entries {
        let line = line(e);
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
    // not beside another publisher: after its 10 s wait this one goes on without the lock, and then only adds its chunks
    if one.held() {
        let sign = |from: i64, to: i64, sha: &str| id.sign(identity::chunk_message(inc, from, to, sha).as_bytes());
        let cut = |from: i64, to: i64| outbox(ctx, from - 1, to).ok().filter(|es| es.len() as i64 == to - from + 1).map(|es| es.iter().map(line).collect::<String>().into_bytes());
        if let Err(e) = store::mend(ctx, &cut, &sign).and_then(|_| store::join_old(ctx, SMALL_KEPT, CHUNK_BYTES, &sign)) { crate::log::warn(&format!("journal: joining old chunks: {e:#}")); }
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
    folder::write_manifest(&dir, &man)?;
    // what the folder's list named and this one no longer does: chunks joined since, or of an earlier incarnation
    let listed = |m: &Value, f: &Value| m["chunks"].as_array().into_iter().flatten().any(|c| c["file"] == *f);
    for f in there.iter().flat_map(|t| t["chunks"].as_array().into_iter().flatten()).map(|c| &c["file"]).filter(|f| !listed(&man, f)) {
        if let Some(f) = f.as_str().filter(|f| store::safe_file(f)) { let _ = std::fs::remove_file(dir.join("journal").join(f)); }
    }
    Ok(())
}

/// A session's journal key, `<tool>/<account>/<id>`, as another computer may send it: the three become path parts
/// and arguments here, so the tool is one this version knows and the other two are plain names.
pub(crate) fn session_key_ok(key: &str) -> bool {
    let mut p = key.splitn(3, '/');
    matches!((p.next(), p.next(), p.next()), (Some(h), Some(a), Some(i)) if crate::harness::by_id(h).is_some() && safe_name(a) && safe_name(i))
}

/// On a computer's card while an entry of a kind this version does not know holds its journal back.
const UPDATE_HERE: &str = "it sent a kind of entry this version does not know: update Harness Manager on this computer to take the rest of its journal";

/// Whether a change to an item's tags made at `ts` on `device` is later than the last one kept for it, and then it
/// is the one kept. Two computers that changed the same item apart both end with the later change; the
/// computer's id decides between equal times.
fn tag_newer(tx: &Connection, kind: &str, key: &str, ts: i64, device: &str) -> Result<bool> {
    Ok(tx.execute("insert into tag_times values(?1, ?2, ?3, ?4) on conflict(kind, key) do update set ts = ?3, device = ?4 where ?3 > ts or (?3 = ts and ?4 > device)", params![kind, key, ts, device])? > 0)
}

/// One entry of another computer's (`device`) journal into this database, on the computer `here`. `Ok(false)`:
/// its kind is one this version does not know (a newer version wrote it), and nothing was done.
fn apply(tx: &Connection, device: &str, here: &str, e: &Entry) -> Result<bool> {
    match e.op.as_str() {
        "usage" => {
            let r: UsageRow = serde_json::from_value(e.data.clone()).context("usage entry")?;
            upsert_usage(tx, &r, device, e.data["cost_usd"].as_f64(), e.data["price_rev"].as_str())?;
        }
        "session" => {
            let m: SessionMeta = serde_json::from_value(e.data.clone()).context("session entry")?;
            if !session_key_ok(&e.key) || e.key != crate::db::session_key(&m) { bail!("session entry with names that cannot be used here"); }
            // a session this computer holds as its own (one continued here) is never another computer's to take over
            if tx.query_row("select 1 from sessions where key=? and device=?", [e.key.as_str(), here], |_| Ok(())).optional()?.is_some() { bail!("session entry for a session of this computer"); }
            upsert_session(tx, &e.key, &m, device)?;
        }
        "quota" => {
            let (profile, tier) = e.key.split_once('/').context("quota key")?;
            quota_point_in(tx, profile, tier, e.data["used"].as_f64().unwrap_or(0.0), e.data["resets_at"].as_i64(), e.data["at"].as_i64().unwrap_or(e.ts), Some(device), e.data["account"].as_str())?;
        }
        "tag" => {
            let (kind, key) = e.key.split_once('/').context("tag key")?;
            if !matches!(kind, "session" | "project") { bail!("tag kind {kind}"); }
            if !tag_newer(tx, kind, key, e.ts, device)? { return Ok(true); }
            tx.execute("delete from tags where kind=? and key=?", [kind, key])?;
            for t in e.data["tags"].as_array().into_iter().flatten().filter_map(|t| t.as_str()) {
                tx.execute("insert or ignore into tags values(?,?,?)", [kind, key, t])?;
            }
        }
        // never a usage row: what was spent was spent; and of the sessions, only that computer's own
        "tombstone" => match e.key.split_once('/').context("tombstone key")? {
            ("session", k) => { tx.execute("delete from sessions where key=? and device=?", [k, device])?; }
            ("tag", k) => { let (kind, key) = k.split_once('/').context("tag key")?; if tag_newer(tx, kind, key, e.ts, device)? { tx.execute("delete from tags where kind=? and key=?", [kind, key])?; } }
            ("transcript", k) => crate::sync::transcripts::tombstone(tx, device, k)?,
            _ => return Ok(false),
        },
        // phase 3: the entry is stored, the file fetched when the session is read or resumed here
        "transcript" => crate::sync::transcripts::apply(tx, device, e)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/// The public key this computer knows for an origin, "" when none yet.
pub fn pubkey_of(ctx: &Ctx, id: &str) -> Result<String> {
    ctx.db.with(|c| Ok(c.query_row("select coalesce(pubkey, '') from peers where device_id=?", [id], |r| r.get(0)).optional()?.unwrap_or_default()))
}

/// Entries passed because the list read did not hold them, kept on the computer's row for its card: one range,
/// widened by each hole passed in the same incarnation; one passed in another incarnation starts it anew.
/// Nothing takes it off but forgetting the computer: what was passed is not read again.
fn note_gap(ctx: &Ctx, id: &str, inc: &str, from: i64, to: i64) -> Result<()> {
    ctx.db.with(|c| {
        let kept: Option<String> = c.query_row("select gap from peers where device_id=?", [id], |r| r.get::<_, Option<String>>(0)).optional()?.flatten();
        let kept = kept.and_then(|g| serde_json::from_str::<Value>(&g).ok()).filter(|g| g["incarnation"] == inc);
        let (from, to) = (kept.as_ref().and_then(|g| g["from"].as_i64()).unwrap_or(from).min(from), kept.as_ref().and_then(|g| g["to"].as_i64()).unwrap_or(to).max(to));
        c.execute("update peers set gap=?1 where device_id=?2", params![json!({"incarnation": inc, "from": from, "to": to}).to_string(), id])?;
        Ok(())
    })
}

/// One computer's new chunks, in manifest order, each read through `read`, checked against its sha256 and (when
/// the origin's key is known) its signature, kept in the local store for relay, and applied in one transaction
/// with the cursor. Stops at the first chunk that cannot be read or checked: order matters, and the next round
/// tries again. Stops too at the first entry of a kind this version does not know, with the cursor just before
/// it, and says so on that computer's row: the entry and all after it are taken once this computer is updated.
/// A list that starts beyond the cursor has lost entries (a relay and the hub drop their oldest chunks): what it
/// holds is taken all the same, as every version did, and the entries passed are kept on that computer's row.
/// Returns the sequence applied up to.
pub fn apply_manifest(ctx: &Ctx, id: &str, man: &Value, read: &dyn Fn(&str) -> Result<Vec<u8>>) -> Result<i64> {
    if !store::safe_origin(id) { bail!("{id} is not a computer's id"); }
    let inc = man["incarnation"].as_str().context("manifest without incarnation")?;
    let pubkey = pubkey_of(ctx, id)?;
    let mut cur: i64 = ctx.db.with(|c| Ok(c.query_row("select seq from cursors2 where device_id=? and incarnation=?", [id, inc], |r| r.get(0)).optional()?))?.unwrap_or(0);
    let (mut newer, mut through) = (false, false);
    // by where each starts, the longest first: a relay lists the small chunks it took beside a joined one that
    // covers them, and entries are missing only when no chunk left in the list reaches back to the cursor
    let mut chunks: Vec<&Value> = man["chunks"].as_array().into_iter().flatten().collect();
    chunks.sort_by_key(|c| (c["from"].as_i64().unwrap_or(0), std::cmp::Reverse(c["to"].as_i64().unwrap_or(0))));
    for ch in chunks {
        let (file, from, to) = (ch["file"].as_str().unwrap_or(""), ch["from"].as_i64().unwrap_or(0), ch["to"].as_i64().unwrap_or(0));
        if to <= cur || !store::safe_file(file) { continue; }
        let missing = (from > cur + 1).then_some((cur + 1, from - 1));
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
        let (mut n, mut bad, mut upto) = (0, 0, to);
        ctx.db.with(|c| {
            let tx = c.transaction()?;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let Ok(e) = serde_json::from_str::<Entry>(line) else { bad += 1; continue };
                if e.seq <= cur { continue; }
                match apply(&tx, id, &ctx.db.device, &e) {
                    Ok(true) => n += 1,
                    // skipped, it would be lost for good: it and all after it wait, the cursor just before it
                    Ok(false) => { upto = e.seq - 1; newer = true; break; }
                    Err(err) => { bad += 1; crate::log::warn(&format!("journal: entry {} of {file} from {id} skipped: {err:#}", e.seq)); }
                }
            }
            tx.execute("insert or replace into cursors2 values(?,?,?)", params![id, inc, upto])?;
            tx.commit()?;
            Ok(())
        })?;
        if let Err(e) = store::put_chunk(ctx, id, inc, ch, &bytes) { crate::log::warn(&format!("journal: keeping {file} from {id} for relay: {e:#}")); }
        if let Some((a, b)) = missing {
            crate::log::warn(&format!("journal: entries {a} to {b} of {id} were not in the list read; its journal goes on after them"));
            if let Err(e) = note_gap(ctx, id, inc, a, b) { crate::log::warn(&format!("journal: noting the entries of {id} that were passed: {e:#}")); }
        }
        cur = upto;
        if newer {
            crate::log::warn(&format!("journal: entry {} of {file} from {id} is of a kind this version does not know; it and what follows wait until Harness Manager is updated on this computer", upto + 1));
            break;
        }
        crate::log::info(&format!("journal: {n} entries from {id} applied ({file}{})", if bad > 0 { format!(", {bad} skipped") } else { String::new() }));
        through = true;
    }
    // said on that computer's card while it waits, and taken off once a chunk is read through again (after the update)
    ctx.db.with(|c| { c.execute("update peers set last_error = case when ?1 then ?2 else '' end where device_id = ?3 and (?1 or (?4 and last_error = ?2))", params![newer, UPDATE_HERE, id, through])?; Ok(()) })?;
    Ok(cur)
}

/// Where another computer's journal and presence are read: the sync folder, a paired computer or the hub.
pub trait Source {
    /// The origin's chunk list after `since`, of `incarnation` where the source tells them apart; `None` when it
    /// holds nothing of the origin.
    fn manifest(&self, origin: &str, incarnation: &str, since: i64) -> Result<Option<Value>>;
    /// One chunk's bytes, as the origin published them.
    fn read(&self, origin: &str, file: &str) -> Result<Vec<u8>>;
    /// What runs on the computers this source speaks for, each as `(id, presence)`.
    fn presence(&self) -> Result<Vec<(String, Value)>>;
    /// The origin's key must be known before anything of it is taken; not in the folder, which is the owner's.
    fn needs_key(&self) -> bool { true }
    /// Its presence is made as it is asked for, so it replaces what is held; a stored copy only when no older.
    fn live(&self) -> bool { false }
}

/// One origin's journal from a source: its chunk list since this computer's cursor (asked again when the source
/// names another incarnation than the one known here), its device record learned when the list carries one, and
/// every chunk checked and applied. `None` when the source holds nothing of the origin; else the list and the
/// sequence applied up to.
pub fn pull(ctx: &Ctx, src: &dyn Source, origin: &str) -> Result<Option<(Value, i64)>> {
    let known: String = ctx.db.with(|c| Ok(c.query_row("select coalesce(incarnation, '') from peers where device_id=?", [origin], |r| r.get(0)).unwrap_or_default()))?;
    let cursor = |inc: &str| -> i64 { ctx.db.with(|c| Ok(c.query_row("select seq from cursors2 where device_id=? and incarnation=?", [origin, inc], |r| r.get(0)).unwrap_or(0))).unwrap_or(0) };
    let Some(mut man) = src.manifest(origin, &known, cursor(&known))? else { return Ok(None) };
    let inc = man["incarnation"].as_str().unwrap_or("").to_string();
    if inc != known && !inc.is_empty() { man = src.manifest(origin, &inc, cursor(&inc))?.context("the origin vanished")?; }
    if man["device"].is_object() { learn_device(ctx, origin, &man["device"])?; }
    if src.needs_key() && pubkey_of(ctx, origin)?.is_empty() { bail!("no key is known for {origin} yet; nothing of it is taken"); }
    let applied = apply_manifest(ctx, origin, &man, &|file| src.read(origin, file))?;
    Ok(Some((man, applied)))
}

/// `pull` through a paired computer or the hub, and what it brought kept on the origin's row. `Ok(false)` when
/// the source holds nothing of the origin.
pub fn pull_noted(ctx: &Ctx, src: &dyn Source, origin: &str) -> Result<bool> {
    let Some((man, applied)) = pull(ctx, src, origin)? else { return Ok(false) };
    let last = man["last_seq"].as_i64().or_else(|| man["chunks"].as_array().and_then(|c| c.last()).and_then(|c| c["to"].as_i64())).unwrap_or(0);
    ctx.db.with(|c| {
        c.execute("update peers set incarnation=case when ?1 <> '' then ?1 else incarnation end, last_seq=max(coalesce(last_seq, 0), ?2), applied_seq=?3, last_pull=?4 where device_id=?5",
            params![man["incarnation"].as_str().unwrap_or(""), last, applied, now_ms(), origin])?;
        Ok(())
    })?;
    Ok(true)
}

/// What a source says runs on the other computers, kept on their rows. A presence not held yet is stamped with
/// when it was written by this computer's clock: the other computer's own time when that lies between the last
/// look and now, else the nearer of the two. The clocks of two computers are never compared after that.
pub fn take_presence(ctx: &Ctx, src: &dyn Source) -> Result<()> {
    for (id, p) in src.presence()? {
        ctx.db.with(|c| {
            c.execute("update peers set presence_seen=case when (?3 or coalesce(json_extract(?1, '$.at'), 0) >= coalesce(json_extract(presence, '$.at'), 0)) and json_extract(?1, '$.at') is not json_extract(presence, '$.at')
                    then min(?4, max(coalesce(json_extract(?1, '$.at'), 0), coalesce(presence_checked, 0))) else presence_seen end,
                presence=case when ?3 or coalesce(json_extract(?1, '$.at'), 0) >= coalesce(json_extract(presence, '$.at'), 0) then ?1 else presence end, presence_checked=?4,
                last_seen=max(coalesce(last_seen, 0), coalesce(json_extract(?1, '$.at'), 0)) where device_id=?2", params![p.to_string(), id, src.live(), now_ms()])?;
            Ok(())
        })?;
    }
    Ok(())
}

/// What another computer says about itself (a `device.json`, from the folder or a peer), when it is signed by
/// the key it carries: its name, incarnation, roots and key are kept, the record is cached for relay. The key
/// is learned once and never replaced by a record.
pub fn learn_device(ctx: &Ctx, id: &str, dev: &Value) -> Result<bool> {
    if !store::safe_origin(id) || dev["id"].as_str() != Some(id) || !identity::record_ok(dev) { return Ok(false); }
    let known = pubkey_of(ctx, id)?;
    if !known.is_empty() && dev["pubkey"].as_str() != Some(&known) {
        crate::log::warn(&format!("computers: {id} presents another key than the one paired; its record is ignored"));
        return Ok(false);
    }
    // that computer's own record too, from before the one kept (a relay's copy from before a rename or a restore):
    // it says nothing new, and taking it would set the name and the incarnation back
    if !known.is_empty() && store::device(ctx, id).is_some_and(|kept| kept["updated_at"].as_i64().unwrap_or(0) > dev["updated_at"].as_i64().unwrap_or(0)) { return Ok(true); }
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
    let src = folder::Folder { ctx, root };
    for (id, dir) in folder::others(ctx, root) {
        let dev = crate::util::read_json(&dir.join("device.json")).unwrap_or(Value::Null);
        if dev.is_object() { learn_device(ctx, &id, &dev)?; }
        let pulled = pull(ctx, &src, &id).unwrap_or_else(|e| { crate::log::warn(&format!("journal: merging {id}: {e:#}")); None });
        let (man, applied) = (pulled.as_ref().map(|p| &p.0), pulled.as_ref().map(|p| p.1));
        ctx.db.with(|c| {
            c.execute("insert into peers(device_id, name, os, app_version, incarnation, last_seen, last_seq, applied_seq, roots, transport) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,'folder')
                on conflict(device_id) do update set name=coalesce(?2, name), os=coalesce(?3, os), app_version=coalesce(?4, app_version), incarnation=coalesce(?5, incarnation),
                last_seen=max(coalesce(?6, 0), coalesce(last_seen, 0)), last_seq=max(coalesce(?7, 0), coalesce(last_seq, 0)), applied_seq=max(coalesce(?8, 0), coalesce(applied_seq, 0)), roots=coalesce(?9, roots), transport='folder'",
                params![id, dev["name"].as_str(), dev["os"].as_str(), dev["app_version"].as_str(), dev["incarnation"].as_str().or(man.and_then(|m| m["incarnation"].as_str())),
                    dev["updated_at"].as_i64(), man.and_then(|m| m["last_seq"].as_i64()), applied, dev["roots"].is_object().then(|| dev["roots"].to_string())])?;
            Ok(())
        })?;
    }
    take_presence(ctx, &src)
}

/// Sync in any form: through the folder or straight to paired computers.
pub fn on(ctx: &Ctx) -> bool { ctx.settings.sync_mode != "off" || ctx.settings.direct_connection }

/// One round: what this computer has, out; what the others have, in (through the folder, then from the paired
/// computers). What happened is kept for the status.
pub fn round(ctx: &Ctx) -> Result<Value> {
    let r = (|| -> Result<()> {
        if !on(ctx) { bail!("sync is off"); }
        backfill_if_needed(ctx)?;
        // and, now and then, rows that reached the database without being recorded (the backfill adds nothing twice)
        let last: i64 = ctx.db.kv_get("journal:catchup_at")?.and_then(|v| v.parse().ok()).unwrap_or(0);
        if now_ms() - last > 600_000 {
            let n = record_missing(ctx, false)?;
            if n > 0 { crate::log::info(&format!("journal: {n} entries caught up")); }
            ctx.db.kv_set("journal:catchup_at", &now_ms().to_string())?;
        }
        let root = if ctx.settings.sync_mode == "folder" { Some(folder::root(ctx)?) } else { None };
        // phase 3: changed transcripts go into the folder with their entries, in this round's chunks
        if let Some(root) = &root { if let Err(e) = crate::sync::transcripts::carry(ctx, root) { crate::log::warn(&format!("transcripts: {e:#}")); } }
        // phase 5: the same carry, to the hub
        if ctx.settings.sync_mode == "hub" { if let Err(e) = crate::sync::hub::carry(ctx) { crate::log::warn(&format!("transcripts: {e:#}")); } }
        publish(ctx)?;
        if let Some(root) = &root {
            mirror(ctx, root)?;
            folder::write_device(ctx, root)?;
            folder::write_presence(ctx, root)?;
            merge(ctx, root)?;
        }
        if ctx.settings.sync_mode == "hub" { crate::sync::hub::round(ctx)?; }
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
    match ctx.settings.sync_mode.as_str() {
        "folder" => folder::write_presence(ctx, &folder::root(ctx)?),
        "hub" => crate::sync::hub::push_presence(ctx),
        _ => Ok(()),
    }
}

pub fn peers(ctx: &Ctx) -> Result<Vec<Peer>> {
    ctx.db.read(|c| {
        let mut st = c.prepare("select device_id, name, os, app_version, last_seen, last_seq, applied_seq, transport, presence, incarnation, address, port, cert_pem, pubkey, paired_at, last_pull, path, connected, relayed_by, last_error, gap, presence_seen, last_good
            from peers order by name, device_id")?;
        let s = |r: &rusqlite::Row, i: usize| -> rusqlite::Result<String> { Ok(r.get::<_, Option<String>>(i)?.unwrap_or_default()) };
        let v = st.query_map([], |r| Ok(Peer { id: r.get(0)?, name: s(r, 1)?, os: s(r, 2)?, app_version: s(r, 3)?, last_seen: r.get(4)?, last_seq: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            applied_seq: r.get::<_, Option<i64>>(6)?.unwrap_or(0), transport: s(r, 7)?,
            presence: r.get::<_, Option<String>>(8)?.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or(Value::Null), presence_seen: r.get(21)?, incarnation: s(r, 9)?,
            address: s(r, 10)?, port: r.get::<_, Option<i64>>(11)?.unwrap_or(0) as u16, cert_pem: s(r, 12)?, pubkey: s(r, 13)?, paired_at: r.get(14)?, last_pull: r.get(15)?,
            path: s(r, 16)?, connected: r.get(17)?, relayed_by: s(r, 18)?, last_error: s(r, 19)?, last_good: s(r, 22)?,
            gap: r.get::<_, Option<String>>(20)?.and_then(|g| serde_json::from_str(&g).ok()).unwrap_or(Value::Null) }))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })
}

/// Every computer's name by id, this one included; an unknown id shows as itself.
pub fn device_names(ctx: &Ctx) -> HashMap<String, String> {
    let mut m: HashMap<String, String> = peers(ctx).unwrap_or_default().into_iter().map(|p| (p.id.clone(), if p.name.is_empty() { p.id } else { p.name })).collect();
    m.insert(ctx.db.device.clone(), ctx.settings.device_name.clone());
    m
}

/// A peer's presence written in the last five minutes by this computer's clock: what it says runs there still
/// counts. One kept by a version that did not stamp it goes by its own time until the next arrives.
pub(crate) fn fresh(p: &Peer, now: i64) -> bool { p.presence_seen.or_else(|| p.presence["at"].as_i64()).map(|t| now - t < 300_000).unwrap_or(false) }

/// A session id a peer's presence says is open there, by presence seen in the last five minutes.
pub fn open_elsewhere(peers: &[Peer], now: i64) -> std::collections::HashSet<String> {
    peers.iter().filter(|p| fresh(p, now))
        .flat_map(|p| p.presence["running"].as_array().into_iter().flatten().filter_map(|r| r["session_id"].as_str().map(String::from)).collect::<Vec<_>>()).collect()
}

/// The Computers page: this computer, the last round, and the others.
pub fn status(ctx: &Ctx) -> Result<Value> {
    let s = &ctx.settings;
    let now = now_ms();
    let last_seq: i64 = ctx.db.read(|c| Ok(c.query_row("select coalesce(max(seq), 0) from outbox", [], |r| r.get(0))?))?;
    let published: i64 = ctx.db.kv_get("journal:published_seq")?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let last = ctx.db.kv_get("computers:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null);
    let pairing = ctx.db.kv_get("pairing:current")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).filter(|p| p["expires_at"].as_i64().unwrap_or(0) > now).unwrap_or(Value::Null);
    let peers: Vec<Value> = peers(ctx)?.into_iter().map(|p| json!({"id": p.id, "name": p.name, "os": p.os, "app_version": p.app_version, "last_seen": p.last_seen,
        "last_seq": p.last_seq, "applied_seq": p.applied_seq, "transport": p.transport, "presence_at": p.presence["at"], "running": if fresh(&p, now) { p.presence["running"].as_array().cloned().unwrap_or_default() } else { vec![] },
        "address": p.address, "path": p.path, "connected": p.connected.map(|t| now - t < CONNECTED_MS).unwrap_or(false), "last_pull": p.last_pull, "paired_at": p.paired_at,
        "relayed_by": p.relayed_by, "direct": !p.cert_pem.is_empty(), "error": p.last_error, "gap": if p.gap.is_object() { json!({"from": p.gap["from"], "to": p.gap["to"]}) } else { Value::Null }})).collect();
    Ok(json!({"mode": s.sync_mode, "folder": s.sync_folder, "direct_connection": s.direct_connection,
        "this": {"id": s.device_id, "name": s.device_name, "incarnation": s.incarnation, "last_seq": last_seq, "published_seq": published, "port": s.listen_port, "pubkey": identity::pubkey(ctx), "pairing": pairing},
        "last_run": last["at"], "error": last["error"], "peers": peers, "hub": crate::sync::hub::status(ctx)}))
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
    if let Some(u) = a["hub_url"].as_str() { s.hub_url = if u.trim().is_empty() { String::new() } else { crate::sync::hub::normalize_url(u)? }; }
    if let Some(m) = a["sync_mode"].as_str() {
        match m {
            "off" | "folder" | "hub" => s.sync_mode = m.into(),
            _ => bail!("the mode is off, folder or hub"),
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
    if s.sync_mode == "hub" && (s.hub_url.is_empty() || crate::sync::hub::token(&ctx).is_none()) { bail!("join a hub first: Join with a code… on the hub card"); }
    ctx.save_settings()?;
    if on(&ctx) != before { ctx.db.kv_set("journal:backfilled", "0")?; }
    Ctx::open_at(data, home)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn two_computers_that_tag_the_same_item_before_syncing_end_with_the_same_tags() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let sync = || for c in [&a, &b, &a, &b] { round(c).unwrap(); };
        let tags = |c: &Ctx, kind: &str, key: &str| c.db.tags_of(kind).unwrap().remove(key).unwrap_or_default();
        // B's change is the later one: it stays on both, whichever entry each applies last
        a.db.tag_set("session", "work/s1", &["from-a".into()]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        b.db.tag_set("session", "work/s1", &["from-b".into()]).unwrap();
        sync();
        assert_eq!((tags(&a, "session", "work/s1"), tags(&b, "session", "work/s1")), (vec!["from-b".to_string()], vec!["from-b".to_string()]));
        // made in the same millisecond: the computer's id decides, the same way on both
        a.db.tag_set("project", "k", &["from-a".into()]).unwrap();
        b.db.tag_set("project", "k", &["from-b".into()]).unwrap();
        for c in [&a, &b] { c.db.with(|x| Ok(x.execute_batch("update outbox set ts = 5000 where key = 'project/k'; update tag_times set ts = 5000 where key = 'k';")?)).unwrap(); }
        sync();
        let winner = if a.settings.device_id > b.settings.device_id { "from-a" } else { "from-b" };
        assert_eq!((tags(&a, "project", "k"), tags(&b, "project", "k")), (vec![winner.to_string()], vec![winner.to_string()]));
        // a change made here after one from a computer whose clock runs ahead is still the later one everywhere
        a.db.with(|x| Ok(x.execute_batch("update outbox set ts = ts + 3600000 where key = 'session/work/s1'; update tag_times set ts = ts + 3600000 where key = 'work/s1';")?)).unwrap();
        a.db.tag_set("session", "work/s1", &["ahead".into()]).unwrap();
        sync();
        b.db.tag_set("session", "work/s1", &["after".into()]).unwrap();
        sync();
        assert_eq!((tags(&a, "session", "work/s1"), tags(&b, "session", "work/s1")), (vec!["after".to_string()], vec!["after".to_string()]));
    }

    #[test]
    fn what_runs_on_another_computer_goes_by_when_its_presence_came_not_by_its_clock() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (mut a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        // A's presence is written below, as its own clock would stamp it: `off` away from B's
        a.settings.sync_presence = false;
        let _k = keys(&[&a, &b]);
        round(&a).unwrap();
        let says = |off: i64, session: &str| std::fs::write(shared.join("devices").join(&a.settings.device_id).join("presence.json"), json!({"at": now_ms() + off, "running": [{"session_id": session}]}).to_string()).unwrap();
        let running = |at: i64| open_elsewhere(&peers(&b).unwrap(), at).into_iter().collect::<Vec<_>>();
        // written an hour before B first looks: not news
        says(-3_600_000, "s1");
        round(&b).unwrap();
        assert_eq!(running(now_ms()), Vec::<String>::new());
        // ten minutes behind, and heard a moment after B last looked: busy
        says(-600_000, "s2");
        round(&b).unwrap();
        assert_eq!(running(now_ms()), ["s2"]);
        // an hour ahead: busy now, and no longer six minutes after its last word, however often B looks
        says(3_600_000, "s3");
        round(&b).unwrap();
        assert_eq!(running(now_ms()), ["s3"]);
        round(&b).unwrap();
        assert_eq!(running(now_ms() + 360_000), Vec::<String>::new(), "it stopped: its clock does not keep it busy");
    }

    #[test]
    fn this_computers_old_chunks_are_joined_and_a_reader_that_holds_some_takes_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let me = &a.settings.device_id;
        // one chunk per row, as the ten-second tick cuts them
        let tick = |i: usize| { a.db.insert_usage(&[row(&format!("claude:{i}"), 1_000 + i as i64)], &Prices::new()).unwrap(); publish(&a).unwrap(); };
        let chunks = || store::manifest(&a, me).unwrap()["chunks"].as_array().unwrap().clone();
        (0..5).for_each(tick);
        round(&a).unwrap();
        round(&b).unwrap();
        assert_eq!((chunks().len(), count(&b, "select count(*) from usage")), (5, 5), "B holds the first five small chunks");
        let all = 2 * SMALL_KEPT + 5;
        (5..all).for_each(tick);
        let list = chunks();
        assert!(list.len() <= SMALL_KEPT + 5, "{} files for {all} chunks", list.len());
        assert_eq!((list[0]["from"].as_i64(), list[0]["file"].as_str()), (Some(1), Some(format!("{}-1-{}.jsonl", a.settings.incarnation, list[0]["to"]).as_str())));
        assert!(list[0]["to"].as_i64().unwrap() > 5 && list[0]["bytes"].as_u64().unwrap() <= CHUNK_BYTES as u64, "the joined file starts before B's cursor and ends after it: {}", list[0]);
        assert_eq!(list.last().unwrap()["to"], all as i64);
        let files = |dir: PathBuf| std::fs::read_dir(dir.join("journal")).unwrap().count();
        assert_eq!(files(store::dir(&a, me)), list.len(), "the small files it replaced are gone");
        // the folder follows; B takes what it lacks of the joined file, signed by A like any chunk; C reads it whole
        round(&a).unwrap();
        assert_eq!(files(shared.join("devices").join(me)), list.len());
        let c = computer(t.path(), "C", &shared);
        let _kc = keys(&[&c]);
        for x in [&b, &c] {
            let st = round(x).unwrap();
            assert_eq!(count(x, "select count(*) from usage"), all as i64, "{st}");
            assert_eq!(st["peers"].as_array().unwrap().iter().find(|p| p["id"] == me.as_str()).map(|p| (p["applied_seq"].as_i64(), &p["gap"])), Some((Some(all as i64), &Value::Null)));
        }
    }

    #[test]
    fn a_join_gives_way_to_a_publisher_that_changed_the_list_meanwhile_and_a_listed_file_that_is_gone_is_cut_again() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let (me, id) = (&a.settings.device_id, identity::load(&a).unwrap());
        let tick = |i: usize| { a.db.insert_usage(&[row(&format!("claude:{i}"), 1_000 + i as i64)], &Prices::new()).unwrap(); publish(&a).unwrap(); };
        let sign = |from: i64, to: i64, sha: &str| id.sign(identity::chunk_message(&a.settings.incarnation, from, to, sha).as_bytes());
        let list = || store::manifest(&a, me).unwrap()["chunks"].as_array().unwrap().iter().map(|c| (c["from"].as_i64().unwrap(), c["to"].as_i64().unwrap())).collect::<Vec<_>>();
        let whole = || store::manifest(&a, me).unwrap()["chunks"].as_array().unwrap().iter().all(|c| store::chunk(&a, me, c["file"].as_str().unwrap()).map(|b| sha256_hex(&b) == c["sha256"].as_str().unwrap()).unwrap_or(false));
        (1..=6).for_each(tick);
        let one = store::manifest(&a, me).unwrap()["chunks"][0]["bytes"].as_u64().unwrap() as usize;
        // one run a call, however many could be joined: the lock a publisher holds meanwhile stays short
        store::join_old(&a, 2, 2 * one, &sign).unwrap();
        assert_eq!(list(), [(1, 2), (3, 3), (4, 4), (5, 5), (6, 6)]);
        // T1 has read the list and the files of its run when T2 publishes a chunk and joins a run of its own
        let met = std::cell::Cell::new(false);
        store::join_old(&a, 2, CHUNK_BYTES, &|from, to, sha| {
            if !met.replace(true) {
                tick(7);
                store::join_old(&a, 2, CHUNK_BYTES, &sign).unwrap();
                assert_eq!(list(), [(1, 5), (6, 6), (7, 7)], "T2's join");
            }
            sign(from, to, sha)
        }).unwrap();
        assert!(met.get());
        assert_eq!(list(), [(1, 5), (6, 6), (7, 7)], "T1 wrote nothing from the list it read before: T2's chunk is listed");
        assert!(whole(), "and every listed file is there");
        assert_eq!(std::fs::read_dir(store::dir(&a, me).join("journal")).unwrap().count(), 3, "no file but the listed ones");
        // a listed file that is gone is cut again from the outbox at the next publish, with the same bytes
        let first = store::manifest(&a, me).unwrap()["chunks"][0].clone();
        std::fs::remove_file(store::dir(&a, me).join("journal").join(first["file"].as_str().unwrap())).unwrap();
        tick(8);
        assert!(whole());
        assert_eq!(store::manifest(&a, me).unwrap()["chunks"][0], first, "the same entries, so the same hash and signature");
        // and when the outbox no longer holds its entries it leaves the list: a reader says what is missing and goes on
        std::fs::remove_file(store::dir(&a, me).join("journal").join(first["file"].as_str().unwrap())).unwrap();
        a.db.with(|c| Ok(c.execute("delete from outbox where seq = 3", [])?)).unwrap();
        tick(9);
        assert_eq!(list(), [(6, 6), (7, 7), (8, 8), (9, 9)]);
        round(&a).unwrap();
        let st = round(&b).unwrap();
        // the row whose entry left the outbox is journaled again by A's backfill, as entry 10
        assert_eq!(count(&b, "select count(*) from usage"), 5, "{st}");
        assert_eq!((st["peers"][0]["applied_seq"].as_i64(), st["peers"][0]["gap"]["from"].as_i64(), st["peers"][0]["gap"]["to"].as_i64()), (Some(10), Some(1), Some(5)), "{st}");
    }

    #[test]
    fn a_publisher_that_could_not_take_the_lock_adds_its_chunk_and_joins_nothing() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let a = computer(t.path(), "A", &shared);
        let _k = keys(&[&a]);
        let tick = |i: usize| { a.db.insert_usage(&[row(&format!("claude:{i}"), 1_000 + i as i64)], &Prices::new()).unwrap(); publish(&a).unwrap(); };
        let chunks = || store::manifest(&a, &a.settings.device_id).unwrap()["chunks"].as_array().unwrap().len();
        (0..2 * SMALL_KEPT).for_each(tick);
        assert_eq!(chunks(), 2 * SMALL_KEPT, "one more and the old ones are joined");
        // another publisher holds the lock for longer than this one waits
        let (name, (taken, told), (done, wait)) = (format!("journal-{}", a.settings.device_id), std::sync::mpsc::channel::<()>(), std::sync::mpsc::channel::<()>());
        let holder = std::thread::spawn(move || { let _held = crate::util::NamedLock::take(&name); taken.send(()).unwrap(); let _ = wait.recv(); });
        told.recv().unwrap();
        tick(2 * SMALL_KEPT);
        assert_eq!(chunks(), 2 * SMALL_KEPT + 1, "its chunk is listed, nothing was joined beside the holder");
        done.send(()).unwrap();
        holder.join().unwrap();
        tick(2 * SMALL_KEPT + 1);
        assert!(chunks() <= SMALL_KEPT + 3, "with the lock, the join: {}", chunks());
    }

    #[test]
    fn the_catch_up_looks_at_the_usage_rows_added_since_its_last_look() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let a = computer(t.path(), "A", &shared);
        let _k = keys(&[&a]);
        a.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &Prices::new()).unwrap();
        round(&a).unwrap();
        let mark = || a.db.kv_get("journal:usage_mark").unwrap();
        assert_eq!(mark().as_deref(), Some("2"));
        // rows that reach the database without an entry: a process that still had sync off wrote them
        let quiet = crate::db::Db::open(&a.data.join("hm.db"), &a.settings.device_id, false).unwrap();
        quiet.insert_usage(&[row("claude:3", 3_000)], &Prices::new()).unwrap();
        let journaled = |id: &str| count(&a, &format!("select count(*) from outbox where op = 'usage' and key = '{id}'"));
        let catch_up = || { a.db.kv_set("journal:catchup_at", "0").unwrap(); round(&a).unwrap(); };
        catch_up();
        assert_eq!((journaled("claude:3"), mark().as_deref()), (1, Some("3")), "the new row is caught up, and the mark moves past it");
        // a row before the mark is not looked at again by the catch-up; the backfill still looks at all
        a.db.with(|c| Ok(c.execute("delete from outbox where key in ('claude:1', 'claude:2')", [])?)).unwrap();
        catch_up();
        assert_eq!((journaled("claude:1"), journaled("claude:2")), (0, 0));
        assert_eq!((backfill(&a).unwrap(), journaled("claude:1"), journaled("claude:2")), (2, 1, 1));
        // a row before the mark that becomes this computer's when the database is next opened takes the mark away with its stamp
        a.db.with(|c| Ok(c.execute_batch("delete from outbox where key = 'claude:1'; update usage set device = '' where request_id = 'claude:1';")?)).unwrap();
        let again = Ctx::open_at(&a.data, &a.home).unwrap();
        assert_eq!((count(&again, "select count(*) from usage where device = ''"), again.db.kv_get("journal:usage_mark").unwrap()), (0, None));
        again.db.kv_set("journal:catchup_at", "0").unwrap();
        round(&again).unwrap();
        assert_eq!(journaled("claude:1"), 1, "so the catch-up looks at it");
    }

    #[test]
    fn a_chunk_file_the_list_no_longer_names_is_removed_at_the_next_publish() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let a = computer(t.path(), "A", &shared);
        let _k = keys(&[&a]);
        let tick = |i: usize| { a.db.insert_usage(&[row(&format!("claude:{i}"), 1_000 + i as i64)], &Prices::new()).unwrap(); publish(&a).unwrap(); };
        (1..=2).for_each(tick);
        // as a join leaves them when it is stopped after its list was written and before the small files went
        let dir = store::dir(&a, &a.settings.device_id).join("journal");
        let left = dir.join(format!("{}-0-0.jsonl", a.settings.incarnation));
        std::fs::write(&left, b"{}\n").unwrap();
        std::fs::write(dir.join("notes.txt"), b"not a chunk").unwrap();
        tick(3);
        assert!(!left.exists(), "the chunk file no list names is gone");
        let names: Vec<String> = { let mut v: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(); v.sort(); v };
        let listed: Vec<String> = store::manifest(&a, &a.settings.device_id).unwrap()["chunks"].as_array().unwrap().iter().map(|c| c["file"].as_str().unwrap().to_string()).collect();
        assert_eq!(names.iter().filter(|n| n.ends_with(".jsonl")).cloned().collect::<Vec<_>>(), { let mut l = listed.clone(); l.sort(); l }, "the listed ones stay");
        assert!(names.contains(&"notes.txt".to_string()), "and what is not a chunk file is left alone");
    }

    /// A source that only says what runs on one computer: `(id, presence, live)`.
    struct Says(&'static str, Value, bool);
    impl Source for Says {
        fn manifest(&self, _: &str, _: &str, _: i64) -> Result<Option<Value>> { Ok(None) }
        fn read(&self, _: &str, _: &str) -> Result<Vec<u8>> { bail!("no chunks") }
        fn presence(&self) -> Result<Vec<(String, Value)>> { Ok(vec![(self.0.to_string(), self.1.clone())]) }
        fn live(&self) -> bool { self.2 }
    }

    #[test]
    fn a_presence_asked_for_live_is_taken_after_the_other_clock_was_set_back_and_an_older_stored_copy_is_not() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        ctx.db.with(|c| Ok(c.execute("insert into peers(device_id, name) values('peer-1', 'P')", [])?)).unwrap();
        let says = |off: i64, session: &str, live: bool| take_presence(&ctx, &Says("peer-1", json!({"at": now_ms() + off, "running": [{"session_id": session}]}), live)).unwrap();
        let running = |at: i64| open_elsewhere(&peers(&ctx).unwrap(), at).into_iter().collect::<Vec<_>>();
        // asked of the computer itself, whose clock is an hour ahead
        says(3_600_000, "s1", true);
        assert_eq!(running(now_ms()), ["s1"]);
        // its clock is set right, an hour back: what it says now replaces what is held, and counts from now
        says(0, "s2", true);
        assert_eq!((running(now_ms()), running(now_ms() + 360_000)), (vec!["s2".to_string()], vec![]));
        // the folder's or the hub's copy from before that is older by the computer's own clock: not taken
        says(-60_000, "s3", false);
        assert_eq!(running(now_ms()), ["s2"]);
        // a later one is
        says(1_000, "s4", false);
        assert_eq!(running(now_ms()), ["s4"]);
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
    fn a_limit_reading_goes_round_the_computers_once() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        let readings = |c: &Ctx| count(c, "select count(*) from quota_hist where tier = 'week'");
        // the account a reading was taken for goes with it to the other computer
        a.db.quota_point_of("work", "session", 10.0, Some(9_000), 500, Some("me@work.com")).unwrap();
        round(&a).unwrap();
        round(&b).unwrap();
        assert_eq!(count(&b, "select count(*) from quota_hist where tier = 'session' and account = 'me@work.com'"), 1);
        // the same account on both: each keeps offering its own cached reading on every page open
        a.db.quota_point("work", "week", 40.0, Some(9_000), 1_000).unwrap();
        b.db.quota_point("work", "week", 50.0, Some(9_000), 2_000).unwrap();
        for _ in 0..3 {
            round(&a).unwrap();
            round(&b).unwrap();
            a.db.quota_point("work", "week", 40.0, Some(9_000), 1_000).unwrap();
            b.db.quota_point("work", "week", 50.0, Some(9_000), 2_000).unwrap();
            // the ten-minute catch-up is due each time
            a.db.kv_set("journal:catchup_at", "0").unwrap();
            b.db.kv_set("journal:catchup_at", "0").unwrap();
        }
        assert_eq!((readings(&a), readings(&b)), (2, 2), "each computer holds the two readings, once");
        let outbox = |c: &Ctx| count(c, "select count(*) from outbox where op = 'quota' and key like '%/week'");
        assert_eq!((outbox(&a), outbox(&b)), (1, 1), "each journals its own reading only");
        // a new reading is still a new point, and one back to an earlier value too
        a.db.quota_point("work", "week", 60.0, Some(9_000), 3_000).unwrap();
        a.db.quota_point("work", "week", 40.0, Some(9_000), 4_000).unwrap();
        assert_eq!(readings(&a), 4);
    }

    #[test]
    fn an_idle_accounts_old_reading_is_stored_once_while_the_others_move_on() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        let (now, day) = (now_ms(), 86_400_000);
        let old = now - 61 * day;
        let n = |sql: &str| count(&ctx, sql);
        // what a page open does for the idle account, with the other account's readings coming in between
        for i in 0..4 {
            ctx.db.quota_point("idle", "week", 40.0, None, old).unwrap();
            ctx.db.quota_point("busy", "week", i as f64, None, now + i).unwrap();
        }
        assert_eq!(n("select count(*) from quota_hist where profile = 'idle'"), 1, "kept, not dropped and stored again");
        // the same account read elsewhere long since: this computer's stale reading is not news to anyone
        ctx.db.quota_point("idle", "week", 90.0, None, now).unwrap();
        ctx.db.quota_point("idle", "week", 40.0, None, old).unwrap();
        ctx.db.quota_point("idle", "week", 41.0, None, old - day).unwrap();
        assert_eq!(ctx.db.quota_points("idle", "week", 0).unwrap().len(), 1, "only the newest is left of that limit: {:?}", ctx.db.quota_points("idle", "week", 0).unwrap());
    }

    #[test]
    fn an_account_read_later_on_another_computer_shows_that_reading() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        let tier = |id: &str, used: f64| crate::model::QuotaTier { id: id.into(), label: "Week here".into(), used, resets_at: Some(9_000) };
        let here = crate::model::Quota { tiers: vec![tier("week", 0.0)], fetched_at: Some(1_000), source: "codex".into() };
        let me = Some("me@work.com");
        let ours = |c: &rusqlite::Connection, used: f64, at: i64, account: Option<&str>| crate::db::quota_point_in(c, "work", "week", used, Some(9_500), at, Some("other"), account);
        // nothing newer anywhere: this computer's own reading
        assert_eq!(crate::service::newer_elsewhere(&ctx, "work", me, Some(here.clone())).unwrap().source, "codex");
        // the same profile name on the other computer, signed in to another account: its limits are not this one's
        ctx.db.with(|c| Ok(ours(c, 13.0, 2_500, Some("someone@else.com"))?)).unwrap();
        // and a reading from a version that did not say whose it was: not taken either
        ctx.db.with(|c| Ok(ours(c, 14.0, 2_600, None)?)).unwrap();
        assert_eq!(crate::service::newer_elsewhere(&ctx, "work", me, Some(here.clone())).unwrap().source, "codex", "another account's limits never show here");
        // the other computer read this account later (the email in any case)
        ctx.db.with(|c| Ok(ours(c, 66.0, 2_000, Some("Me@Work.com"))?)).unwrap();
        let q = crate::service::newer_elsewhere(&ctx, "work", me, Some(here.clone())).unwrap();
        assert_eq!((q.tiers[0].used, q.tiers[0].resets_at, q.fetched_at, q.tiers[0].label.as_str()), (66.0, Some(9_500), Some(2_000), "Week here"), "its numbers, this computer's label");
        // a computer that never read it locally still shows it
        assert_eq!(crate::service::newer_elsewhere(&ctx, "work", me, None).unwrap().tiers[0].label, "Week");
        // without knowing which login this is, nothing from elsewhere
        assert_eq!(crate::service::newer_elsewhere(&ctx, "work", None, Some(here.clone())).unwrap().source, "codex");
        // read again here, later: this computer's reading again
        let later = crate::model::Quota { fetched_at: Some(3_000), ..here };
        assert_eq!(crate::service::newer_elsewhere(&ctx, "work", me, Some(later)).unwrap().fetched_at, Some(3_000));
    }

    #[test]
    fn turning_sync_off_and_on_publishes_this_computers_readings_only() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared), computer(t.path(), "B", &shared));
        let _k = keys(&[&a, &b]);
        a.db.quota_point("work", "week", 40.0, None, 1_000).unwrap();
        round(&a).unwrap();
        round(&b).unwrap();
        b.db.quota_point("work", "week", 50.0, None, 2_000).unwrap();
        assert_eq!(count(&b, "select count(*) from quota_hist"), 2, "A's reading merged, and B's own");
        b.db.with(|c| Ok(c.execute("delete from outbox", [])?)).unwrap();
        assert_eq!(backfill(&b).unwrap(), 1, "B's own reading, not A's");
        let at: String = b.db.with(|c| Ok(c.query_row("select data from outbox where op = 'quota'", [], |r| r.get(0))?)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&at).unwrap()["at"], 2_000);
    }

    #[test]
    fn a_name_with_a_path_in_it_is_no_computers_id() {
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("A"), &t.path().join("home")).unwrap();
        let evil = "..\\..\\evil";
        let id = identity::load(&ctx).unwrap();
        let mut rec = json!({"id": evil, "name": "x"});
        id.sign_record(&mut rec);
        assert!(identity::record_ok(&rec), "signed by the key it carries");
        assert!(!learn_device(&ctx, evil, &rec).unwrap());
        assert!(apply_manifest(&ctx, evil, &json!({"incarnation": "i", "chunks": []}), &|_| Ok(vec![])).is_err());
        assert!(!t.path().join("evil").exists() && !t.path().join("A").join("evil").exists());
        assert!(!store::dir(&ctx, evil).starts_with(t.path().join("evil")) && store::dir(&ctx, evil).starts_with(ctx.data.join("journal-cache")), "and the store stays in its folder");
        let _ = crate::secrets::delete(&identity::target(&ctx.settings.device_id));
    }

    #[test]
    fn readings_copied_many_times_are_kept_once_at_the_next_start() {
        let t = tempfile::tempdir().unwrap();
        let (data, home) = (t.path().join("A"), t.path().join("home"));
        let ctx = Ctx::open_at(&data, &home).unwrap();
        ctx.db.with(|c| {
            c.execute_batch("insert into quota_hist(ts, profile, tier, used, resets_at) values(1000, 'work', 'week', 40.0, 9000), (1000, 'work', 'week', 40.0, 9000), (2000, 'work', 'week', 50.0, 9000),
                (1000, 'work', 'week', 40.0, 9000), (1000, 'home', 'week', 40.0, 9000); delete from kv where k = 'migrated:quota-once';")?;
            Ok(())
        }).unwrap();
        drop(ctx);
        let ctx = Ctx::open_at(&data, &home).unwrap();
        assert_eq!(ctx.db.quota_points("work", "week", 0).unwrap(), vec![(1000, 40.0, Some(9000)), (2000, 50.0, Some(9000))]);
        assert_eq!(ctx.db.quota_points("home", "week", 0).unwrap().len(), 1);
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
        let live = crate::service::SessionFilter { open: Some(true), ..Default::default() };
        assert!(crate::service::sessions(&b, &live).unwrap().is_empty());
        // A's presence names it: B's Live now lists it
        let p = json!({"at": crate::util::now_ms(), "running": [{"session_id": "abc"}]}).to_string();
        b.db.with(|c| Ok(c.execute("update peers set presence = ?1 where device_id = ?2", rusqlite::params![p, a.settings.device_id])?)).unwrap();
        let list = crate::service::sessions(&b, &live).unwrap();
        assert_eq!((list.len(), list[0].open_elsewhere), (1, true));
        assert_eq!(status(&b).unwrap()["peers"][0]["running"].as_array().map(|r| r.len()), Some(1));
        // presence older than five minutes: A's console no longer counts anywhere
        let p = json!({"at": crate::util::now_ms() - 600_000, "running": [{"session_id": "abc"}]}).to_string();
        b.db.with(|c| Ok(c.execute("update peers set presence = ?1, presence_seen = json_extract(?1, '$.at') where device_id = ?2", rusqlite::params![p, a.settings.device_id])?)).unwrap();
        assert!(crate::service::sessions(&b, &live).unwrap().is_empty());
        assert_eq!(status(&b).unwrap()["peers"][0]["running"].as_array().map(|r| r.len()), Some(0));
        assert!(crate::service::sessions(&b, &crate::service::SessionFilter { scope: Some("here".into()), ..Default::default() }).unwrap().is_empty());
        let projects = crate::service::projects(&b, None, None).unwrap();
        assert_eq!((projects.len(), projects[0].here, projects[0].devices[0]["name"].as_str()), (1, false, Some("A")));
        a.db.forget_profile("work").unwrap();
        round(&a).unwrap();
        round(&b).unwrap();
        assert!(b.db.remote_sessions().unwrap().is_empty());
    }

    /// What another computer publishes, merged here: `entries` as one chunk of `id`'s journal. No key is known for
    /// it, so nothing is checked against a signature. Returns the sequence applied up to.
    fn merge_from(ctx: &Ctx, id: &str, entries: &[Value]) -> i64 {
        let bytes = entries.iter().map(|e| format!("{e}\n")).collect::<String>().into_bytes();
        let to = entries.last().and_then(|e| e["seq"].as_i64()).unwrap_or(0);
        let man = json!({"incarnation": "i", "chunks": [{"file": format!("i-1-{to}.jsonl"), "from": 1, "to": to, "sha256": sha256_hex(&bytes), "bytes": bytes.len()}]});
        apply_manifest(ctx, id, &man, &|_| Ok(bytes.clone())).unwrap()
    }

    fn entry(seq: i64, op: &str, key: &str, data: Value) -> Value { json!({"seq": seq, "ts": 1, "op": op, "key": key, "data": data}) }

    #[test]
    fn a_record_older_than_the_one_kept_does_not_set_a_computers_name_and_incarnation_back() {
        std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        let id = identity::load(&ctx).unwrap();
        let record = |name: &str, inc: &str, at: i64| { let mut r = json!({"id": "peer-1", "name": name, "incarnation": inc, "updated_at": at}); id.sign_record(&mut r); r };
        let kept = || -> (String, String) { ctx.db.with(|c| Ok(c.query_row("select name, incarnation from peers where device_id='peer-1'", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap() };
        assert!(learn_device(&ctx, "peer-1", &record("LAPTOP", "i1", 1_000)).unwrap());
        assert!(learn_device(&ctx, "peer-1", &record("LAPTOP-2", "i2", 2_000)).unwrap());
        // a relay still hands out the first one: it is that computer's, and it changes nothing
        assert!(learn_device(&ctx, "peer-1", &record("LAPTOP", "i1", 1_000)).unwrap());
        assert_eq!(kept(), ("LAPTOP-2".to_string(), "i2".to_string()));
        assert_eq!(store::device(&ctx, "peer-1").unwrap()["updated_at"], 2_000);
        assert!(learn_device(&ctx, "peer-1", &record("LAPTOP-3", "i2", 3_000)).unwrap());
        assert_eq!(kept().0, "LAPTOP-3");
        let _ = crate::secrets::delete(&identity::target(&ctx.settings.device_id));
    }

    #[test]
    fn a_list_that_names_a_small_chunk_before_the_joined_one_that_covers_it_has_no_gap() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        ctx.db.with(|c| Ok(c.execute("insert into peers(device_id, name) values('peer-1', 'P')", [])?)).unwrap();
        let usage = |seq: i64| entry(seq, "usage", &format!("claude:{seq}"), serde_json::to_value(row(&format!("claude:{seq}"), 1_000)).unwrap());
        let bytes = |seqs: std::ops::RangeInclusive<i64>| seqs.map(|s| format!("{}\n", usage(s))).collect::<String>().into_bytes();
        let chunk = |from: i64, to: i64| json!({"file": format!("i-{from}-{to}.jsonl"), "from": from, "to": to, "sha256": sha256_hex(&bytes(from..=to)), "bytes": bytes(from..=to).len()});
        // as a relay's store keeps them, by where each ends: the small chunk it took first, then the joined one
        let man = json!({"incarnation": "i", "chunks": [chunk(4, 4), chunk(1, 5)]});
        let read = |file: &str| -> Result<Vec<u8>> { let mut p = file.trim_end_matches(".jsonl").split('-').skip(1).map(|n| n.parse::<i64>().unwrap()); Ok(bytes(p.next().unwrap()..=p.next().unwrap())) };
        assert_eq!(apply_manifest(&ctx, "peer-1", &man, &read).unwrap(), 5);
        assert_eq!(count(&ctx, "select count(*) from usage"), 5, "the joined chunk held what the small one seemed to skip");
        assert_eq!(count(&ctx, "select count(*) from peers where gap is not null"), 0);
    }

    fn session(harness: &str, profile: &str, id: &str, title: &str, at: i64) -> Value {
        serde_json::to_value(SessionMeta { harness: harness.into(), profile: profile.into(), id: id.into(), title: title.into(), last_active_at: at, ..Default::default() }).unwrap()
    }

    fn keys_of(ctx: &Ctx, table: &str) -> Vec<String> {
        ctx.db.with(|c| { crate::transcripts::tables(c)?; Ok(c.prepare(&format!("select key from {table} order by key"))?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?) }).unwrap()
    }

    #[test]
    fn a_session_or_transcript_whose_names_are_not_plain_names_is_not_merged() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        let s = |seq: i64, h: &str, p: &str, id: &str| entry(seq, "session", &format!("{h}/{p}/{id}"), session(h, p, id, "t", 5));
        let tr = |seq: i64, key: &str| entry(seq, "transcript", key, json!({"file": "transcripts/claude/work/s1.jsonl.zst", "bytes": 1, "sha256": "x", "mtime": 1, "source_rel": "projects/p/s1.jsonl"}));
        let long = "a".repeat(200);
        let entries = [
            // a path, an option, a letter outside ASCII, nothing, a drive, a tool this version does not know, a name too long
            s(1, "claude", "work", "..\\..\\evil"), s(2, "claude", "work", "../evil"), s(3, "claude", "work", "--dangerously-skip-permissions"), s(4, "claude", "work", "é"),
            s(5, "claude", "work", ""), s(6, "claude", "..", "s1"), s(7, "claude", "C:work", "s1"), s(8, "nope", "work", "s1"), s(9, "claude", "work", &long), s(10, "claude", "work", ".hidden"),
            // the key names one session, the entry another
            entry(11, "session", "claude/work/s1", session("claude", "work", "..\\evil", "t", 5)),
            tr(12, "claude/work/..\\..\\evil"), tr(13, "claude/../s1"), tr(14, "nope/work/s1"), tr(15, "claude/work/-x"), tr(16, "claude/work"),
            // and the names the tools do give
            s(17, "claude", "work", "0b1c2d3e-aaaa-bbbb-cccc-1234567890ab"), s(18, "opencode", "oc_main", "ses_5f3a.1"), tr(19, "claude/work/s1"),
        ];
        assert_eq!(merge_from(&ctx, "peer-1", &entries), 19);
        assert_eq!(keys_of(&ctx, "sessions"), ["claude/work/0b1c2d3e-aaaa-bbbb-cccc-1234567890ab", "opencode/oc_main/ses_5f3a.1"]);
        assert_eq!(keys_of(&ctx, "transcripts"), ["claude/work/s1"]);
    }

    #[test]
    fn a_file_name_from_another_computer_that_is_not_a_plain_name_is_never_opened() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        // most of them without a slash: a hidden file, an option, a parent folder, a drive, a stream, a device, a name too long
        let long = "a".repeat(200);
        let asked = std::cell::RefCell::new(Vec::<String>::new());
        for name in [".i-1-1.jsonl", "-i-1-1.jsonl", "..", "../up.jsonl", "C:evil.jsonl", "i-1-1.jsonl:stream", "NUL", "nul.jsonl", long.as_str()] {
            let man = json!({"incarnation": "i", "chunks": [{"file": name, "from": 1, "to": 1, "sha256": sha256_hex(b""), "bytes": 0}]});
            assert_eq!(apply_manifest(&ctx, "peer-1", &man, &|f| { asked.borrow_mut().push(f.to_string()); bail!("not read") }).unwrap(), 0, "{name}");
            assert!(store::chunk(&ctx, "peer-1", name).is_err(), "{name}");
            assert!(store::put_chunk(&ctx, "peer-1", "i", &json!({"file": name, "from": 1, "to": 1, "bytes": 0}), b"").is_err(), "{name}");
        }
        assert_eq!(*asked.borrow(), Vec::<String>::new(), "none was asked for");
        assert!(!store::dir(&ctx, "peer-1").join("up.jsonl").exists() && store::manifest(&ctx, "peer-1").is_none(), "and none was kept");
        // a transcript's place in its computer's folder is transcripts/<tool>/<account>/<file>, each a plain name
        let tr = |file: &str| entry(1, "transcript", "claude/work/s1", json!({"file": file, "bytes": 1, "sha256": "x", "mtime": 1, "source_rel": "projects/p/s1.jsonl"}));
        for (i, file) in ["C:evil.zst", "transcripts/claude/work/C:evil.zst", "transcripts/claude/work/s1.jsonl.zst:stream", "transcripts\\claude\\work\\s1.jsonl.zst",
            "transcripts/claude/work/sub/s1.jsonl.zst", "elsewhere/claude/work/s1.jsonl.zst", "transcripts/claude/-work/s1.jsonl.zst"].into_iter().enumerate() {
            assert_eq!(merge_from(&ctx, &format!("peer-{}", i + 2), &[tr(file)]), 1);
        }
        assert_eq!(keys_of(&ctx, "transcripts"), Vec::<String>::new());
        merge_from(&ctx, "peer-9", &[tr("transcripts/claude/work/s1.jsonl.zst")]);
        assert_eq!(keys_of(&ctx, "transcripts"), ["claude/work/s1"]);
    }

    #[test]
    fn another_computers_entry_never_takes_or_removes_a_session_that_is_not_its_own() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        let mine: SessionMeta = serde_json::from_value(session("codex", "work", "abc", "mine", 5)).unwrap();
        ctx.db.with(|c| Ok(upsert_session(c, "codex/work/abc", &mine, &ctx.db.device)?)).unwrap();
        let row = |key: &str| -> Option<(String, String)> { ctx.db.with(|c| Ok(c.query_row("select device, title from sessions where key=?", [key], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)).unwrap() };
        // the same session, newer, as another computer's; then its removal
        merge_from(&ctx, "peer-1", &[entry(1, "session", "codex/work/abc", session("codex", "work", "abc", "theirs", 9)), entry(2, "session", "codex/work/other", session("codex", "work", "other", "theirs", 9)),
            entry(3, "tombstone", "session/codex/work/abc", json!({}))]);
        assert_eq!(row("codex/work/abc"), Some((ctx.db.device.clone(), "mine".to_string())), "this computer's session stays its own, and stays");
        assert_eq!(row("codex/work/other"), Some(("peer-1".to_string(), "theirs".to_string())));
        // a third computer cannot remove the second's; the second can
        merge_from(&ctx, "peer-2", &[entry(1, "tombstone", "session/codex/work/other", json!({}))]);
        assert!(row("codex/work/other").is_some());
        merge_from(&ctx, "peer-1", &[entry(4, "tombstone", "session/codex/work/other", json!({}))]);
        assert_eq!(row("codex/work/other"), None);
    }

    #[test]
    fn an_entry_of_a_kind_this_version_does_not_know_waits_for_the_update_with_all_that_follows() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("B"), &t.path().join("home")).unwrap();
        ctx.db.with(|c| Ok(c.execute("insert into peers(device_id, name) values('peer-1', 'P')", [])?)).unwrap();
        let usage = |seq: i64| entry(seq, "usage", &format!("claude:{seq}"), serde_json::to_value(row(&format!("claude:{seq}"), 1_000)).unwrap());
        let error = || -> String { ctx.db.with(|c| Ok(c.query_row("select coalesce(last_error, '') from peers where device_id='peer-1'", [], |r| r.get(0))?)).unwrap() };
        let newer = [usage(1), entry(2, "hologram", "k", json!({})), usage(3)];
        for _ in 0..2 {
            assert_eq!(merge_from(&ctx, "peer-1", &newer), 1, "it stops before the entry it cannot read");
            assert_eq!((count(&ctx, "select count(*) from usage"), count(&ctx, "select seq from cursors2 where device_id='peer-1'")), (1, 1));
            assert!(error().contains("update Harness Manager"), "said on that computer's card: {}", error());
        }
        // a broken entry of a kind it does know is skipped, as before, and what follows is taken
        assert_eq!(merge_from(&ctx, "peer-2", &[entry(1, "quota", "no-slash", json!({})), usage(7)]), 7);
        assert_eq!(count(&ctx, "select count(*) from usage"), 2);
        // updated (the kind is one it reads now): that entry and the one after it come in, and the card is clear
        assert_eq!(merge_from(&ctx, "peer-1", &[usage(1), usage(2), usage(3)]), 3);
        assert_eq!(count(&ctx, "select count(*) from usage"), 4);
        assert_eq!(error(), "");
    }
}
