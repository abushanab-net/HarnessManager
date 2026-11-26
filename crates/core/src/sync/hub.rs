//! The "hub" transport: a small service the owner hosts (`crates/hub`, behind Caddy) that every computer pushes
//! its journal chunks to and pulls the others' from, with presence fan-out, rendezvous addresses for the direct
//! connection and the transcripts. The hub never sees a key: chunks, presence and transcripts go up encrypted
//! with the **ring key**, a 32-byte secret shared by the owner's computers (carried in the pairing payload, or
//! typed as the recovery key). Only the signed device records are plaintext, by design.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use rusqlite::params;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::sync::folder;
use crate::sync::peer::store;
use crate::util::{body_capped, now_ms};
use crate::{journal, secrets, Ctx};

// ---------- the ring key ----------

/// The recovery key is 20 random bytes: 32 base32 characters, shown as 8 groups of 4. The ring key is its SHA-256.
const SEED_BYTES: usize = 20;

/// The credentials carry the device id so two data folders on one computer (tests, `HRM_HOME`) stay apart.
pub(crate) fn ring_target(device_id: &str) -> String { format!("{}:ring-key:{device_id}", secrets::prefix()) }
/// Where a version before tokens were kept per hub kept this computer's one hub token.
pub(crate) fn token_target(device_id: &str) -> String { format!("{}:hub-token:{device_id}", secrets::prefix()) }

/// The token hub `hub` gave this computer, kept under that hub's own address: one write holds the token and the hub
/// it belongs to, so a context for one hub never reads another's.
pub(crate) fn token_target_for(device_id: &str, hub: &str) -> String { format!("{}:hub-token:{device_id}:{}", secrets::prefix(), &crate::util::sha256_hex(hub.as_bytes())[..16]) }

/// A file name no version kept a token in: the per-hub tokens have no file to move from.
const NO_FILE: &str = "hub.token.per-hub";

/// `file`: where a version before the credential store on macOS and Linux kept this secret, in the data folder.
fn read_secret(ctx: &Ctx, target: &str, file: &str) -> Option<String> {
    secrets::read_moving(target, &ctx.data.join(file)).map(|b| String::from_utf8_lossy(&b).trim().to_string()).filter(|s| !s.is_empty())
}

/// The store takes the new value, and the file an earlier version kept the one before it in goes.
fn write_secret(ctx: &Ctx, target: &str, file: &str, value: &str) -> Result<()> {
    secrets::write(target, "hrm", value.as_bytes())?;
    let _ = std::fs::remove_file(ctx.data.join(file));
    Ok(())
}

/// The seed as stored: 32 base32 characters, or None when this computer has no ring key.
fn ring_seed(ctx: &Ctx) -> Option<String> {
    read_secret(ctx, &ring_target(&ctx.settings.device_id), "ring.key").filter(|s| s.len() == 32)
}

pub fn has_ring(ctx: &Ctx) -> bool { ring_seed(ctx).is_some() }

/// The recovery form: "ABCD EFGH IJKL MNOP QRST UVWX YZ23 4567".
pub fn ring_display(seed: &str) -> String { seed.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join(" ") }

/// The recovery key to show the owner, once, or when asked again behind a confirm; marks it shown.
pub fn ring_show(ctx: &Ctx) -> Result<String> {
    let seed = ring_seed(ctx).context("no ring key on this computer yet: join a hub, pair with a computer that has one, or enter the recovery key")?;
    ctx.db.kv_set("ring:shown", "1")?;
    Ok(ring_display(&seed))
}

/// The 32-byte key the hub's blobs are sealed with.
pub fn ring_key(ctx: &Ctx) -> Result<[u8; 32]> {
    let seed = ring_seed(ctx).context("no ring key on this computer: enter the recovery key on the Computers page")?;
    let raw = data_encoding::BASE32_NOPAD.decode(seed.as_bytes()).context("the ring key kept here is not valid")?;
    Ok(Sha256::digest(&raw).into())
}

fn store_seed(ctx: &Ctx, seed: &str) -> Result<()> { write_secret(ctx, &ring_target(&ctx.settings.device_id), "ring.key", seed) }

/// A new ring key, on the first computer that turns the hub on. Returns the recovery form.
pub fn ring_make(ctx: &Ctx) -> Result<String> {
    let mut s = [0u8; SEED_BYTES];
    getrandom::fill(&mut s).map_err(|e| anyhow!("random bytes: {e}"))?;
    let seed = data_encoding::BASE32_NOPAD.encode(&s);
    store_seed(ctx, &seed)?;
    ctx.db.kv_set("ring:shown", "0")?;
    crate::log::info("hub: a ring key was made on this computer");
    Ok(ring_display(&seed))
}

/// The recovery key as typed: spaces, dashes and case do not matter.
pub fn ring_enter(ctx: &Ctx, key: &str) -> Result<()> {
    let seed: String = key.to_ascii_uppercase().chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| match c { '0' => 'O', '1' => 'I', '8' => 'B', c => c }).collect();
    if seed.len() != 32 || data_encoding::BASE32_NOPAD.decode(seed.as_bytes()).is_err() { bail!("the recovery key is 8 groups of 4 letters and digits (A to Z, 2 to 7)"); }
    store_seed(ctx, &seed)?;
    ctx.db.kv_set("ring:shown", "1")?;
    Ok(())
}

/// What the pairing payload carries, so a paired computer shares the ring at once.
pub fn ring_for_pairing(ctx: &Ctx) -> Value { ring_seed(ctx).map(Value::String).unwrap_or(Value::Null) }

/// A ring that came with a pairing payload: taken when this computer has none, else kept as it is.
pub fn ring_adopt(ctx: &Ctx, seed: &Value) {
    let Some(s) = seed.as_str().filter(|s| s.len() == 32 && data_encoding::BASE32_NOPAD.decode(s.as_bytes()).is_ok()) else { return };
    match ring_seed(ctx) {
        None => { if store_seed(ctx, s).is_ok() { let _ = ctx.db.kv_set("ring:shown", "1"); crate::log::info("hub: the ring key came with the pairing"); } }
        Some(mine) if mine != s => crate::log::warn("hub: the paired computer has another ring key; this computer keeps its own (enter its recovery key here to use that one)"),
        _ => {}
    }
}

/// What a sealed blob belongs to: the computer, the kind (`chunk`, `presence`, `transcript`) and the name it is
/// kept under. Sealed in as associated data, a blob opens only as that: the hub cannot hand one computer's out
/// under another's name.
fn bound(origin: &str, kind: &str, name: &str) -> Vec<u8> { format!("{origin}\n{kind}\n{name}").into_bytes() }

/// Off in this version: 1.11 opens only a blob sealed without associated data, and would wait for good on every
/// chunk sealed the bound way. This version opens both; the next one turns this on.
const SEAL_BOUND: bool = false;

fn seal(key: &[u8; 32], plain: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| anyhow!("random bytes: {e}"))?;
    let ct = ChaCha20Poly1305::new_from_slice(key)?.encrypt(&Nonce::from(nonce), Payload { msg: plain, aad }).map_err(|_| anyhow!("seal"))?;
    Ok([nonce.as_slice(), &ct].concat())
}

/// `nonce ++ ciphertext`, bound to what it belongs to (`bound`) once `SEAL_BOUND` is on.
pub fn encrypt(key: &[u8; 32], plain: &[u8], aad: &[u8]) -> Result<Vec<u8>> { seal(key, plain, if SEAL_BOUND { aad } else { &[] }) }

/// A blob bound to `aad`, or one sealed before blobs were bound, which is read for one more version.
pub fn decrypt(key: &[u8; 32], bytes: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < 12 { bail!("short blob"); }
    let (nonce, cipher) = (Nonce::from(<[u8; 12]>::try_from(&bytes[..12])?), ChaCha20Poly1305::new_from_slice(key)?);
    cipher.decrypt(&nonce, Payload { msg: &bytes[12..], aad }).or_else(|_| cipher.decrypt(&nonce, &bytes[12..])).map_err(|_| anyhow!("the blob does not open with this ring key"))
}

// ---------- the client ----------

pub fn normalize_url(u: &str) -> Result<String> {
    let u = u.trim().trim_end_matches('/').to_string();
    if !(u.starts_with("https://") || u.starts_with("http://")) || u.len() < 10 { bail!("the hub address starts with https:// (or http:// on a private network)"); }
    Ok(u)
}

/// The token of the hub these settings name, and of no other.
pub fn token(ctx: &Ctx) -> Option<String> {
    let base = normalize_url(&ctx.settings.hub_url).ok()?;
    if let Some(t) = read_secret(ctx, &token_target_for(&ctx.settings.device_id, &base), NO_FILE) { return Some(t); }
    // one from before tokens were kept per hub is the hub's it was written for (named, or else these settings' hub,
    // as it always was): a join since keeps its own and removes it
    let legacy = read_secret(ctx, &token_target(&ctx.settings.device_id), "hub.token")?;
    let of = ctx.db.kv_get(TOKEN_OF).ok().flatten().unwrap_or_default();
    (of.is_empty() || of == base).then_some(legacy)
}

/// The hub the stored token was given by.
const TOKEN_OF: &str = "hub:token_of";

#[cfg(test)]
thread_local! {
    /// Test seam: runs during a join, just after its credential is written.
    pub(crate) static JOIN_SEAM: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}

pub struct Hub {
    http: reqwest::blocking::Client,
    pub base: String,
    token: String,
}

impl Hub {
    /// This computer's hub, with its token: the one that hub gave, read under that hub's address, so a context still
    /// naming another hub (a pairing begun before this computer joined a new one) only ever sends that hub its own.
    pub fn open(ctx: &Ctx) -> Result<Hub> {
        let base = normalize_url(&ctx.settings.hub_url).context("no hub address")?;
        let token = token(ctx).context("this computer has not joined the hub yet: Join with a code… on the Computers page")?;
        Hub::at(&base, &token)
    }

    fn at(base: &str, token: &str) -> Result<Hub> {
        let http = reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(120)).build()?;
        Ok(Hub { http, base: base.into(), token: token.into() })
    }

    fn req(&self, m: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        let r = self.http.request(m, format!("{}{path}", self.base));
        if self.token.is_empty() { r } else { r.bearer_auth(&self.token) }
    }

    fn checked(r: reqwest::blocking::Response, path: &str) -> Result<reqwest::blocking::Response> {
        let st = r.status();
        if st.is_success() { return Ok(r); }
        let body = body_capped(r, 65_536, path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or(body);
        match st.as_u16() {
            401 | 403 => bail!("the hub refused this computer ({msg}); join it again"),
            _ => bail!("{path} answered {st}{}", if msg.trim().is_empty() { String::new() } else { format!(": {}", crate::util::truncate(msg.trim(), 200)) }),
        }
    }

    /// A JSON answer; `None` when the hub has nothing under that path.
    pub fn get_json(&self, path: &str) -> Result<Option<Value>> {
        let r = self.req(reqwest::Method::GET, path).send().with_context(|| format!("could not reach the hub at {}", self.base))?;
        if r.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
        Ok(Some(serde_json::from_slice(&body_capped(Self::checked(r, path)?, journal::MAX_JSON_BYTES, path)?)?))
    }

    /// An answer's bytes, when they are no more than `max`.
    pub fn get_bytes(&self, path: &str, max: u64) -> Result<Vec<u8>> {
        let r = self.req(reqwest::Method::GET, path).send().with_context(|| format!("could not reach the hub at {}", self.base))?;
        body_capped(Self::checked(r, path)?, max, path)
    }

    /// An answer's bytes, at most `max`; `None` when the hub has nothing under that path.
    pub fn get_opt_bytes(&self, path: &str, max: u64) -> Result<Option<Vec<u8>>> {
        let r = self.req(reqwest::Method::GET, path).send().with_context(|| format!("could not reach the hub at {}", self.base))?;
        if r.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
        Ok(Some(body_capped(Self::checked(r, path)?, max, path)?))
    }

    pub fn put(&self, path: &str, body: Vec<u8>, headers: &[(&str, String)]) -> Result<()> {
        let mut r = self.req(reqwest::Method::PUT, path).body(body);
        for (k, v) in headers { r = r.header(*k, v); }
        Self::checked(r.send().with_context(|| format!("could not reach the hub at {}", self.base))?, path).map(|_| ())
    }

    pub fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let r = self.req(reqwest::Method::POST, path).json(body).send().with_context(|| format!("could not reach the hub at {}", self.base))?;
        Ok(serde_json::from_slice(&body_capped(Self::checked(r, path)?, journal::MAX_JSON_BYTES, path)?)?)
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        let r = self.req(reqwest::Method::DELETE, path).send().with_context(|| format!("could not reach the hub at {}", self.base))?;
        if r.status() == reqwest::StatusCode::NOT_FOUND { return Ok(()); }
        Self::checked(r, path).map(|_| ())
    }
}

// ---------- joining ----------

/// Join the hub at `url` with a code from another computer's Invite, or the admin token for the first one; the
/// token is kept in the credential store and the settings switch to the hub. The first computer makes the ring key.
/// Returns the context to go on with.
pub fn join(ctx: &Ctx, url: &str, code: Option<&str>, admin_token: Option<&str>) -> Result<Ctx> {
    let base = normalize_url(url)?;
    let code = code.map(|c| c.chars().filter(|c| c.is_ascii_digit()).collect::<String>()).filter(|c| !c.is_empty());
    let admin = admin_token.map(str::trim).filter(|a| !a.is_empty());
    if code.is_none() && admin.is_none() { bail!("give the six-digit code from a computer already on the hub, or the hub's admin token for the first computer"); }
    if let Some(c) = &code { if c.len() != 6 { bail!("the code is six digits"); } }
    let mut body = json!({"device": folder::device_record(ctx)?});
    if let Some(c) = &code { body["code"] = json!(c); } else { body["admin_token"] = json!(admin); }
    let r = Hub::at(&base, "")?.post_json("/v1/join", &body).map_err(|e| {
        let s = format!("{e:#}");
        if s.starts_with("the hub refused this computer") { anyhow!("the hub did not accept the {}; check it and try again", if code.is_some() { "code" } else { "admin token" }) } else { e }
    })?;
    let tok = r["token"].as_str().filter(|t| !t.is_empty()).context("the hub answered without a token")?;
    write_secret(ctx, &token_target_for(&ctx.settings.device_id, &base), NO_FILE, tok)?;
    #[cfg(test)]
    if let Some(f) = JOIN_SEAM.with(|s| s.borrow_mut().take()) { f(); }
    // the one token of a version before is not this hub's to send any more
    let _ = secrets::delete(&token_target(&ctx.settings.device_id));
    let _ = std::fs::remove_file(ctx.data.join("hub.token"));
    let _ = ctx.db.kv_set(TOKEN_OF, "");
    // the first computer makes the ring; any other gets it by pairing or the recovery key, whichever way it joined:
    // with a ring of its own it could read nothing of theirs, nor they of it
    let others = r["devices"].as_array().into_iter().flatten().any(|d| split_device(d).0["id"].as_str().is_some_and(|i| i != ctx.settings.device_id));
    if admin.is_some() && !others && !has_ring(ctx) { ring_make(ctx)?; }
    crate::log::info(&format!("hub: joined {base} ({} computers there)", r["devices"].as_array().map(|d| d.len()).unwrap_or(0)));
    journal::configure(&ctx.data, &ctx.home, &json!({"sync_mode": "hub", "hub_url": base}))
}

/// A one-time six-digit code for another computer, good for ten minutes: `{code, expires_at}`.
pub fn invite(ctx: &Ctx) -> Result<Value> { Hub::open(ctx)?.post_json("/v1/invite", &json!({})) }

/// A one-use code for a phone to join through the hub, and the phone's mailbox there: `{code, phone_id, expires_at}`.
pub fn phone_invite(ctx: &Ctx) -> Result<Value> { Hub::open(ctx)?.post_json("/v1/phones/invite", &json!({})) }

/// A phone's token and mailbox gone from the hub (already gone counts).
pub fn delete_phone(ctx: &Ctx, mailbox: &str) -> Result<()> { Hub::open(ctx)?.delete(&format!("/v1/phones/{mailbox}")) }

/// Leave the hub: this computer's record and token go there, the token here; the settings fall back to off.
pub fn leave(ctx: &Ctx) -> Result<Ctx> {
    if let Ok(h) = Hub::open(ctx) { if let Err(e) = h.delete(&format!("/v1/devices/{}", ctx.settings.device_id)) { crate::log::warn(&format!("hub: leaving: {e:#}")); } }
    if let Ok(base) = normalize_url(&ctx.settings.hub_url) { let _ = secrets::delete(&token_target_for(&ctx.settings.device_id, &base)); }
    let _ = secrets::delete(&token_target(&ctx.settings.device_id));
    let _ = ctx.db.kv_set(TOKEN_OF, "");
    let _ = std::fs::remove_file(ctx.data.join("hub.token"));
    journal::configure(&ctx.data, &ctx.home, &json!({"sync_mode": "off"}))
}

// ---------- the round ----------

/// The hub as a source: the chunk lists it keeps, each chunk and presence blob opened with the ring key.
struct Sealed<'a> {
    hub: &'a Hub,
    key: &'a [u8; 32],
    me: &'a str,
}

impl journal::Source for Sealed<'_> {
    fn manifest(&self, origin: &str, incarnation: &str, since: i64) -> Result<Option<Value>> { self.hub.get_json(&format!("/v1/chunks/{origin}?since={since}&incarnation={incarnation}")) }

    fn read(&self, origin: &str, file: &str) -> Result<Vec<u8>> { decrypt(self.key, &self.hub.get_bytes(&format!("/v1/chunk/{origin}/{file}"), journal::MAX_CHUNK_READ)?, &bound(origin, "chunk", file)) }

    /// `{<id>: {at, blob}}`, each blob sealed by its computer with the same ring.
    fn presence(&self) -> Result<Vec<(String, Value)>> {
        let Some(p) = self.hub.get_json("/v1/presence")? else { return Ok(vec![]) };
        Ok(p.as_object().into_iter().flatten().filter(|(id, _)| *id != self.me).filter_map(|(id, v)| {
            let blob = v["blob"].as_str().and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())?;
            let Ok(plain) = decrypt(self.key, &blob, &bound(id, "presence", "")) else { crate::log::warn(&format!("hub: the presence of {id} does not open with this ring key")); return None };
            Some((id.clone(), serde_json::from_slice(&plain).ok()?))
        }).collect())
    }
}

/// This computer's chunks the hub lacks, sealed, with the plaintext's sha256 and this computer's signature.
fn push_chunks(ctx: &Ctx, hub: &Hub, key: &[u8; 32]) -> Result<usize> {
    let me = &ctx.settings.device_id;
    let Some(man) = store::manifest(ctx, me) else { return Ok(0) };
    let inc = man["incarnation"].as_str().unwrap_or("");
    let there = hub.get_json(&format!("/v1/chunks/{me}?since=0&incarnation={inc}"))?.unwrap_or(Value::Null);
    // up to its newest chunk the hub has them, or dropped them by its retention: chunks go up in order, so a gap
    // below that is its choice, and sending those again would push out others, round after round
    let top = if there["incarnation"].as_str() == Some(inc) { there["chunks"].as_array().into_iter().flatten().filter_map(|c| c["to"].as_i64()).max().unwrap_or(0) } else { 0 };
    let mut n = 0;
    for ch in man["chunks"].as_array().into_iter().flatten() {
        let file = ch["file"].as_str().unwrap_or("");
        if file.is_empty() || ch["to"].as_i64().unwrap_or(0) <= top { continue; }
        let bytes = store::chunk(ctx, me, file)?;
        hub.put(&format!("/v1/chunks/{me}/{file}"), encrypt(key, &bytes, &bound(me, "chunk", file))?, &[("X-From", ch["from"].to_string()), ("X-To", ch["to"].to_string()),
            ("X-Sha256", ch["sha256"].as_str().unwrap_or("").into()), ("X-Sig", ch["sig"].as_str().unwrap_or("").into())])?;
        n += 1;
    }
    Ok(n)
}

/// A hub device entry as `[{...device.json, last_seen, last_seq}]` or `{device, last_seen, last_seq}`: the signed
/// record alone, and what the hub added.
fn split_device(d: &Value) -> (Value, Option<i64>, Option<i64>) {
    if d["device"].is_object() { return (d["device"].clone(), d["last_seen"].as_i64(), d["last_seq"].as_i64()); }
    let mut rec = d.clone();
    let (seen, seq) = (rec["last_seen"].as_i64(), rec["last_seq"].as_i64());
    if !crate::sync::peer::identity::record_ok(&rec) { if let Some(o) = rec.as_object_mut() { o.remove("last_seen"); o.remove("last_seq"); } }
    (rec, seen, seq)
}

/// This computer's presence, sealed, to the hub.
pub fn push_presence(ctx: &Ctx) -> Result<()> {
    if !ctx.settings.sync_presence { return Ok(()); }
    let hub = Hub::open(ctx)?;
    hub.put("/v1/presence", encrypt(&ring_key(ctx)?, folder::presence(ctx)?.to_string().as_bytes(), &bound(&ctx.settings.device_id, "presence", ""))?, &[])?;
    note(ctx, "hub:presence_at");
    Ok(())
}

/// The record up when it says something new (its time and signature change with every making). Each write to the
/// hub is an event to every computer, and each answers an event with a round: a round that always writes never ends.
fn push_device(ctx: &Ctx, hub: &Hub) -> Result<()> {
    let rec = folder::device_record(ctx)?;
    let mut said = rec.clone();
    if let Some(o) = said.as_object_mut() { o.remove("updated_at"); o.remove("sig"); }
    let said = crate::util::sha256_hex(said.to_string().as_bytes());
    if ctx.db.kv_get("hub:device_sent")?.as_deref() == Some(&said) { return Ok(()); }
    hub.put(&format!("/v1/devices/{}", ctx.settings.device_id), rec.to_string().into_bytes(), &[("Content-Type", "application/json".into())])?;
    ctx.db.kv_set("hub:device_sent", &said)
}

/// A round's presence: at most once in a while, for the same reason. The minute's tick in the app keeps it fresh.
const PRESENCE_EVERY_MS: i64 = 55_000;
fn presence_due(ctx: &Ctx) -> bool {
    now_ms() - ctx.db.kv_get("hub:presence_at").ok().flatten().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) >= PRESENCE_EVERY_MS
}

/// Where this computer can be reached for the direct connection: its LAN addresses with the listen port.
fn my_addresses(ctx: &Ctx) -> Vec<String> {
    let port = ctx.settings.listen_port;
    if !ctx.settings.direct_connection || port == 0 { return vec![]; }
    let mut v: Vec<String> = if_addrs::get_if_addrs().unwrap_or_default().into_iter().filter(|i| !i.is_loopback()).filter_map(|i| match i.ip() { std::net::IpAddr::V4(ip) => Some(format!("{ip}:{port}")), _ => None }).collect();
    v.push(format!("{}:{port}", crate::util::hostname()));
    v
}

/// The addresses the hub holds for a paired computer, newest first.
pub fn rendezvous_of(ctx: &Ctx, id: &str) -> Vec<String> {
    ctx.db.kv_get("hub:rendezvous").ok().flatten().and_then(|v| serde_json::from_str::<Value>(&v).ok())
        .map(|m| m[id]["addresses"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn note(ctx: &Ctx, k: &str) { let _ = ctx.db.kv_set(k, &now_ms().to_string()); }

/// One hub round: this computer's new chunks and record up; every device's record learned and its chunks
/// down; presence both ways; the rendezvous addresses when the direct connection is on.
pub fn round(ctx: &Ctx) -> Result<()> {
    let hub = Hub::open(ctx)?;
    let key = ring_key(ctx)?;
    let me = ctx.settings.device_id.clone();
    let n = push_chunks(ctx, &hub, &key)?;
    push_device(ctx, &hub)?;
    if ctx.settings.sync_presence && presence_due(ctx) {
        hub.put("/v1/presence", encrypt(&key, folder::presence(ctx)?.to_string().as_bytes(), &bound(&me, "presence", ""))?, &[])?;
        note(ctx, "hub:presence_at");
    }
    note(ctx, "hub:last_push");
    if n > 0 { crate::log::info(&format!("hub: {n} chunks pushed")); }
    // every other computer on the hub: its record, then its journal
    let devices = hub.get_json("/v1/devices")?.unwrap_or(json!([]));
    let list: Vec<Value> = match &devices { Value::Array(a) => a.clone(), Value::Object(o) => o.values().cloned().collect(), _ => vec![] };
    let src = Sealed { hub: &hub, key: &key, me: &me };
    for d in &list {
        let (rec, seen, last_seq) = split_device(d);
        let Some(id) = rec["id"].as_str().map(String::from).filter(|i| *i != me && !i.contains(['/', '\\'])) else { continue };
        if !journal::learn_device(ctx, &id, &rec)? { crate::log::warn(&format!("hub: the record of {id} is not signed by its key; skipped")); continue; }
        let seen = [seen, rec["updated_at"].as_i64()].into_iter().flatten().max();
        ctx.db.with(|c| {
            c.execute("update peers set transport='hub', last_seen=max(coalesce(?1, 0), coalesce(last_seen, 0)), last_seq=max(coalesce(?2, 0), coalesce(last_seq, 0)) where device_id=?3", params![seen, last_seq, id])?;
            Ok(())
        })?;
        if let Err(e) = journal::pull_noted(ctx, &src, &id) { crate::log::warn(&format!("hub: pulling {id}: {e:#}")); }
    }
    journal::take_presence(ctx, &src)?;
    // rendezvous: where this computer can be reached, and where the others can
    if ctx.settings.direct_connection {
        let mine = my_addresses(ctx);
        if !mine.is_empty() { hub.put("/v1/rendezvous", json!({"addresses": mine}).to_string().into_bytes(), &[("Content-Type", "application/json".into())])?; }
        if let Some(r) = hub.get_json("/v1/rendezvous")? { ctx.db.kv_set("hub:rendezvous", &r.to_string())?; }
    }
    note(ctx, "hub:last_pull");
    Ok(())
}

// ---------- transcripts ----------

fn transcript_path(origin: &str, key: &str) -> String { format!("/v1/transcripts/{origin}/{key}") }

/// Phase 3's carry with the hub as its destination: each compressed transcript sealed, with the sha256 and size
/// of what was sealed, so the receiver checks the journal entry against what it opens.
pub fn carry(ctx: &Ctx) -> Result<()> {
    if !ctx.settings.sync_transcripts { return Ok(()); }
    let hub = Hub::open(ctx)?;
    let key = ring_key(ctx)?;
    let me = ctx.settings.device_id.clone();
    super::transcripts::carry_to(ctx, crate::transcripts::CAP_BYTES,
        &|k, _, bytes| hub.put(&transcript_path(&me, k), encrypt(&key, bytes, &bound(&me, "transcript", k))?, &[("X-Sha256", crate::util::sha256_hex(bytes)), ("X-Bytes", bytes.len().to_string())]),
        &|k, _| hub.delete(&transcript_path(&me, k)))
}

/// A transcript of another computer from the hub, opened with the ring key (still compressed).
pub fn fetch_transcript(ctx: &Ctx, origin: &str, key: &str) -> Result<Vec<u8>> {
    decrypt(&ring_key(ctx)?, &Hub::open(ctx)?.get_bytes(&transcript_path(origin, key), crate::transcripts::MAX_FETCH_BYTES)?, &bound(origin, "transcript", key))
}

// ---------- status ----------

/// `{url, joined, devices, last_push, last_pull, ws, error, ring, ring_shown}`.
pub fn status(ctx: &Ctx) -> Value {
    let kv = |k: &str| ctx.db.kv_get(k).ok().flatten();
    let devices: i64 = ctx.db.read(|c| Ok(c.query_row("select count(*) from peers where transport='hub'", [], |r| r.get(0))?)).unwrap_or(0);
    let joined = token(ctx).is_some() && !ctx.settings.hub_url.is_empty();
    let last = kv("computers:last").and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null);
    json!({"url": ctx.settings.hub_url, "joined": joined, "devices": if joined { devices + 1 } else { 0 },
        "last_push": kv("hub:last_push").and_then(|v| v.parse::<i64>().ok()), "last_pull": kv("hub:last_pull").and_then(|v| v.parse::<i64>().ok()),
        "ws": kv("hub:ws").as_deref() == Some("1"), "error": if ctx.settings.sync_mode == "hub" { last["error"].clone() } else { Value::Null },
        "ring": has_ring(ctx), "ring_shown": kv("ring:shown").as_deref() != Some("0")})
}

// ---------- the events socket ----------

pub type CtxOf = Arc<dyn Fn() -> Arc<Ctx> + Send + Sync>;
pub type OnStatus = Arc<dyn Fn(Value) + Send + Sync>;

/// A hub event about another computer; one about this computer is the echo of its own write. Mail is not a change
/// of any computer's.
fn from_another(event: &str, me: &str) -> bool { serde_json::from_str::<Value>(event).map(|v| v["kind"] != "mail" && v["id"].as_str() != Some(me)).unwrap_or(true) }

/// A phone this computer invited has left it mail: the mailbox pass looks at once.
fn mail_for_me(event: &str, me: &str) -> bool {
    let mine = serde_json::from_str::<Value>(event).is_ok_and(|v| v["kind"] == "mail" && v["to"].as_str() == Some(me));
    if mine { crate::companion::mailbox::wake(); }
    mine
}

/// One connection to `/v1/events`: another computer's message runs a round (what changed is pulled, and what is
/// new here goes up), a burst of them one round, told to `on_status`. Returns when the socket closes or fails.
async fn listen(ctx: &Ctx, base: &str, token: &str, ctx_of: &CtxOf, on_status: &OnStatus) -> Result<()> {
    use futures_util::StreamExt;
    let url = format!("{}/v1/events", base.replacen("http", "ws", 1));
    let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url.as_str())?;
    req.headers_mut().insert("Authorization", format!("Bearer {token}").parse()?);
    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(req)).await.context("the hub's events socket did not answer")??;
    ctx.db.kv_set("hub:ws", "1")?;
    crate::log::info("hub: live events on");
    let mut quiet = 0;
    loop {
        // the hub says what changed (presence, chunks, a device); a quiet socket is checked with a ping
        match tokio::time::timeout(Duration::from_secs(60), ws.next()).await {
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) => {
                quiet = 0;
                if mail_for_me(t.as_str(), &ctx.settings.device_id) || !from_another(t.as_str(), &ctx.settings.device_id) { continue; }
                // what else arrives in the next second goes with this round
                let settle = tokio::time::Instant::now() + Duration::from_secs(1);
                while let Ok(m) = tokio::time::timeout_at(settle, ws.next()).await {
                    match m {
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => bail!("the hub closed the events socket"),
                        Some(Err(e)) => bail!("events socket: {e}"),
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) => { mail_for_me(t.as_str(), &ctx.settings.device_id); }
                        Some(Ok(_)) => {}
                    }
                }
                let ctx = ctx_of();
                let (c2, o2) = (ctx.clone(), on_status.clone());
                let _ = tokio::task::spawn_blocking(move || { if let Ok(v) = journal::round(&c2) { o2(v); } }).await;
            }
            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))) | Ok(None) => bail!("the hub closed the events socket"),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => bail!("events socket: {e}"),
            Err(_) => {
                use futures_util::SinkExt;
                quiet += 1;
                if quiet > 2 { bail!("the events socket went quiet"); }
                ws.send(tokio_tungstenite::tungstenite::Message::Ping(vec![].into())).await?;
            }
        }
    }
}

/// Whether the events socket is up, for the status: written when it changes, not every two seconds by a computer
/// that is not on a hub.
fn set_ws(ctx: &Ctx, v: &str) {
    if ctx.db.kv_get("hub:ws").ok().flatten().as_deref() != Some(v) { let _ = ctx.db.kv_set("hub:ws", v); }
}

/// Keeps the events socket open while the app runs, on its own thread; without it, a round every 30 s is the
/// fallback. Nothing happens while the hub is not this computer's transport. The phones paired through the hub are
/// served beside it (`companion::mailbox::watch`).
pub fn watch(ctx_of: CtxOf, on_status: OnStatus) {
    crate::companion::mailbox::watch(ctx_of.clone());
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio");
        // after a failure the next try comes soon, then later: 1, 2, 4 ... 30 seconds
        let mut fails: u32 = 0;
        loop {
            let ctx = ctx_of();
            if ctx.settings.sync_mode != "hub" || token(&ctx).is_none() {
                set_ws(&ctx, "0");
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
            // the blocking client is made outside the runtime: reqwest's own runtime must not be dropped inside it
            let r = Hub::open(&ctx).and_then(|hub| rt.block_on(listen(&ctx, &hub.base, &hub.token, &ctx_of, &on_status)));
            let wait = match &r { Err(e) => { fails += 1; crate::log::info(&format!("hub: {e:#}; trying again")); (1u64 << fails.min(5)).min(30) } Ok(_) => { fails = 0; 1 } };
            set_ws(&ctx, "0");
            std::thread::sleep(Duration::from_secs(wait));
            let ctx = ctx_of();
            if ctx.settings.sync_mode == "hub" { if let Ok(v) = journal::round(&ctx) { on_status(v); } }
        }
    });
}

/// A connected peer's rendezvous addresses as `(host, port)` pairs, for the client.
pub fn rendezvous_pairs(ctx: &Ctx, id: &str) -> Vec<(String, u16)> {
    rendezvous_of(ctx, id).iter().filter_map(|a| {
        let (h, p) = a.rsplit_once(':')?;
        Some((h.trim_matches(['[', ']']).to_string(), p.parse().ok()?))
    }).collect()
}

#[cfg(test)]
pub(crate) fn keys_of(ctx: &Ctx) -> Vec<String> { vec![ring_target(&ctx.settings.device_id), token_target(&ctx.settings.device_id), token_target_of(ctx)] }

/// Tests: where the token of `ctx`'s hub is kept.
#[cfg(test)]
pub(crate) fn token_target_of(ctx: &Ctx) -> String { token_target_for(&ctx.settings.device_id, &normalize_url(&ctx.settings.hub_url).unwrap_or_default()) }

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;
