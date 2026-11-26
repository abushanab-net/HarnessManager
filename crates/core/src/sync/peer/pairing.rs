//! Pairing two computers with a six-digit code. The computer showing the code serves a plain-HTTP route on a
//! port that lives two minutes; the joiner runs SPAKE2 with the code as the password (the two device ids as the
//! identities, the joiner as A), both derive a key, and exchange their identity payloads encrypted with it: the
//! joiner's first, so the host's is only sealed for someone who has shown the code. A wrong code fails at
//! decryption; three failures or two minutes end it. Both sides then hold the other's certificate and key.
//!
//! A phone pairs the same way on its own routes (`/pair/phone/*`, the exchange in `companion_proto::pairing`), with
//! one step more: once its payload opens, the owner sees its name and three words derived from the key, and grants
//! it what it may do; only then is this computer's payload sealed for it. It is stored in `phones`, never in `peers`,
//! and gets no ring key.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use rusqlite::params;
use serde_json::{json, Value};
use companion_proto::api::Grants;
use companion_proto::pairing::{DesktopPayload, PairResult, PhonePayload};
use companion_proto::Keyed;
use spake2::{Ed25519Group, Identity, Password, Spake2};

use super::{identity, mdns, OnEvent};
use crate::util::now_ms;
use crate::{journal, Ctx};

const LIVES_MS: i64 = 120_000;
const FAILURES: u32 = 3;
/// Exchanges begun and not finished, kept: the oldest goes when a flood of them comes.
const OPEN_STARTS: usize = 16;

fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
fn unb64(s: &str) -> Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

/// `{nonce ++ ciphertext}` base64 of the JSON, under the pairing key.
pub(super) fn seal(key: &[u8], v: &Value) -> Result<String> {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| anyhow!("random bytes: {e}"))?;
    let ct = ChaCha20Poly1305::new_from_slice(key)?.encrypt(&Nonce::from(nonce), v.to_string().as_bytes()).map_err(|_| anyhow!("seal"))?;
    Ok(b64(&[nonce.as_slice(), &ct].concat()))
}

fn open(key: &[u8], s: &str) -> Result<Value> {
    let b = unb64(s)?;
    if b.len() < 12 { bail!("short payload"); }
    let nonce = Nonce::from(<[u8; 12]>::try_from(&b[..12])?);
    let pt = ChaCha20Poly1305::new_from_slice(key)?.decrypt(&nonce, &b[12..]).map_err(|_| anyhow!("the code did not match"))?;
    Ok(serde_json::from_slice(&pt)?)
}

/// What each side tells the other: enough to store it as a peer and reach it.
pub fn payload(ctx: &Ctx) -> Result<Value> {
    let id = identity::load(ctx)?;
    let s = &ctx.settings;
    // phase 5: the ring key rides along, so a paired computer can read the hub at once
    Ok(json!({"id": s.device_id, "name": s.device_name, "os": std::env::consts::OS, "incarnation": s.incarnation, "pubkey": id.pubkey(), "cert_pem": id.cert_pem,
        "address_hint": format!("{}:{}", crate::util::hostname(), s.listen_port), "app_version": env!("CARGO_PKG_VERSION"), "ring": crate::sync::hub::ring_for_pairing(ctx)}))
}

/// The other side, as a peer: paired now, reached at `address` (empty when found by mDNS).
pub fn store_peer(ctx: &Ctx, p: &Value, address: &str) -> Result<Value> {
    let id = p["id"].as_str().filter(|s| !s.is_empty()).context("the payload has no id")?;
    if id == ctx.settings.device_id { bail!("that is this computer"); }
    let port: u16 = p["address_hint"].as_str().and_then(|a| a.rsplit_once(':')).and_then(|(_, p)| p.parse().ok()).unwrap_or(0);
    let Some(der) = p["cert_pem"].as_str().and_then(|c| identity::pem_to_der(c).ok()) else { bail!("the payload has no certificate") };
    ctx.db.with(|c| {
        // the listener tells a computer from a phone by the certificate alone: checked and stored in one transaction,
        // so a phone enrolled from another connection cannot come between
        let c = c.transaction()?;
        let phones: Vec<String> = c.prepare("select cert_pem from phones where revoked_at is null")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        if phones.iter().any(|pem| identity::pem_to_der(pem).ok().as_deref() == Some(der.as_slice())) { bail!("that certificate already belongs to a paired phone"); }
        c.execute("insert into peers(device_id, name, os, app_version, incarnation, cert_pem, pubkey, paired_at, address, port, transport) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'direct')
            on conflict(device_id) do update set name=?2, os=?3, app_version=?4, incarnation=?5, cert_pem=?6, pubkey=?7, paired_at=?8, address=?9, port=?10, transport='direct'",
            params![id, p["name"].as_str().unwrap_or(""), p["os"].as_str().unwrap_or(""), p["app_version"].as_str().unwrap_or(""), p["incarnation"].as_str().unwrap_or(""),
                p["cert_pem"].as_str().unwrap_or(""), p["pubkey"].as_str().unwrap_or(""), now_ms(), address, port])?;
        c.commit()?;
        Ok(())
    })?;
    crate::log::info(&format!("computers: paired with {} ({id})", p["name"].as_str().unwrap_or("")));
    crate::sync::hub::ring_adopt(ctx, &p["ring"]);
    Ok(json!({"id": id, "name": p["name"], "address": address}))
}

struct Shared {
    ctx: Arc<Ctx>,
    code: String,
    keys: Mutex<Vec<(String, Vec<u8>)>>,
    failures: AtomicU32,
    done: Mutex<Option<Value>>,
    handle: axum_server::Handle<SocketAddr>,
    on_event: OnEvent,
}

impl Shared {
    fn end(&self, v: Value) {
        *self.done.lock().unwrap_or_else(|e| e.into_inner()) = Some(v.clone());
        let _ = self.ctx.db.with(|c| { c.execute("delete from kv where k='pairing:current'", [])?; Ok(()) });
        (self.on_event)("computers-pairing", v);
        // the answer to the last request goes out first
        self.handle.graceful_shutdown(Some(Duration::from_secs(2)));
    }
}

/// The computer showing a code.
pub struct Host {
    pub code: String,
    pub expires_at: i64,
    pub port: u16,
    pub host_hint: String,
    shared: Arc<Shared>,
}

impl Host {
    pub fn cancel(&self) {
        let _ = self.shared.ctx.db.with(|c| { c.execute("delete from kv where k='pairing:current'", [])?; Ok(()) });
        self.shared.handle.shutdown();
    }

    /// `{ok, peer?, error?}` once it ended, by pairing or by failing.
    pub fn result(&self) -> Option<Value> { self.shared.done.lock().unwrap_or_else(|e| e.into_inner()).clone() }

    pub fn status(&self) -> Value { json!({"code": self.code, "expires_at": self.expires_at, "host_hint": self.host_hint, "port": self.port}) }
}

/// Show a code: a plain-HTTP pairing route on a free port for two minutes.
pub fn start(ctx: Arc<Ctx>, rt: &tokio::runtime::Handle, bind: IpAddr, host_name: Option<&str>, on_event: OnEvent) -> Result<Host> {
    identity::load(&ctx)?;
    let code = format!("{:06}", getrandom::u32().map_err(|e| anyhow!("random bytes: {e}"))? % 1_000_000);
    let l = std::net::TcpListener::bind(SocketAddr::new(bind, 0))?;
    l.set_nonblocking(true)?;
    let port = l.local_addr()?.port();
    let handle = axum_server::Handle::new();
    let shared = Arc::new(Shared { ctx: ctx.clone(), code: code.clone(), keys: Mutex::new(vec![]), failures: AtomicU32::new(0), done: Mutex::new(None), handle: handle.clone(), on_event });
    let app = Router::new().route("/pair/hello", get(hello)).route("/pair/start", post(start_route)).route("/pair/finish", post(finish)).with_state(shared.clone());
    let _in_runtime = rt.enter();
    let server = axum_server::from_tcp(l)?.handle(handle.clone());
    rt.spawn(async move { let _ = server.serve(app.into_make_service()).await; });
    let (h2, s2) = (handle.clone(), shared.clone());
    rt.spawn(async move {
        tokio::time::sleep(Duration::from_millis(LIVES_MS as u64)).await;
        if s2.done.lock().unwrap_or_else(|e| e.into_inner()).is_none() { s2.end(json!({"ok": false, "error": "the code expired"})); }
        h2.shutdown();
    });
    let expires_at = now_ms() + LIVES_MS;
    // the computer's name on the network, which the other computer can resolve; a VPN name or an IP when given
    let host_hint = format!("{}:{port}", host_name.map(String::from).unwrap_or_else(crate::util::hostname));
    ctx.db.kv_set("pairing:current", &json!({"code": code, "expires_at": expires_at, "host_hint": host_hint, "port": port}).to_string())?;
    Ok(Host { code, expires_at, port, host_hint, shared })
}

async fn hello(State(s): State<Arc<Shared>>) -> Json<Value> { Json(json!({"id": s.ctx.settings.device_id, "name": s.ctx.settings.device_name})) }

/// The joiner's SPAKE2 message in, ours out. Nothing sealed goes with it: anything sealed under the key would let
/// a stranger try one code per request at home, unseen, with `finish` never counting a failure.
async fn start_route(State(s): State<Arc<Shared>>, Json(a): Json<Value>) -> Response {
    let (joiner, msg) = (a["id"].as_str().unwrap_or("").to_string(), a["msg"].as_str().unwrap_or(""));
    let Ok(inbound) = unb64(msg) else { return StatusCode::BAD_REQUEST.into_response() };
    if joiner.is_empty() { return StatusCode::BAD_REQUEST.into_response(); }
    let (st, outbound) = Spake2::<Ed25519Group>::start_b(&Password::new(s.code.as_bytes()), &Identity::new(joiner.as_bytes()), &Identity::new(s.ctx.settings.device_id.as_bytes()));
    let Ok(key) = st.finish(&inbound) else { return StatusCode::BAD_REQUEST.into_response() };
    let mut keys = s.keys.lock().unwrap_or_else(|e| e.into_inner());
    keys.retain(|(id, _)| id != &joiner);
    if keys.len() >= OPEN_STARTS { keys.remove(0); }
    keys.push((joiner, key));
    Json(json!({"id": s.ctx.settings.device_id, "msg": b64(&outbound)})).into_response()
}

/// The joiner's sealed payload: opened with its key, the peer is stored and ours is sealed in the answer;
/// otherwise one failure more.
async fn finish(State(s): State<Arc<Shared>>, Json(a): Json<Value>) -> Response {
    let joiner = a["id"].as_str().unwrap_or("").to_string();
    let key = { let mut keys = s.keys.lock().unwrap_or_else(|e| e.into_inner()); keys.iter().position(|(id, _)| id == &joiner).map(|i| keys.remove(i).1) };
    let Some(key) = key else { return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "start first"}))).into_response() };
    match open(&key, a["payload"].as_str().unwrap_or("")) {
        Ok(p) => {
            // where to reach the joiner comes from the sealed payload; beside it, where anyone on the way can
            // rewrite it, only from a computer up to 1.11.0, which seals none
            let (ctx, address) = (s.ctx.clone(), p["address"].as_str().or(a["address"].as_str()).unwrap_or("").to_string());
            let c2 = ctx.clone();
            let sealed = match tokio::task::spawn_blocking(move || payload(&c2)).await.expect("payload").and_then(|m| seal(&key, &m)) {
                Ok(p) => p,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": format!("{e:#}")}))).into_response(),
            };
            match tokio::task::spawn_blocking(move || store_peer(&ctx, &p, &address)).await.expect("store") {
                Ok(peer) => { s.end(json!({"ok": true, "peer": peer})); Json(json!({"ok": true, "name": s.ctx.settings.device_name, "payload": sealed})).into_response() }
                Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": format!("{e:#}")}))).into_response(),
            }
        }
        Err(_) => {
            let n = s.failures.fetch_add(1, Ordering::SeqCst) + 1;
            crate::log::warn(&format!("computers: a pairing attempt with a wrong code ({n} of {FAILURES})"));
            if n >= FAILURES { s.end(json!({"ok": false, "error": "three wrong codes; show a new one"})); }
            (StatusCode::FORBIDDEN, Json(json!({"ok": false, "error": "the code did not match"}))).into_response()
        }
    }
}

/// Join the computer showing `code`: found by mDNS, or at `host` (`name:port`, the host hint it shows).
pub fn join(ctx: &Ctx, code: &str, host: Option<&str>) -> Result<Value> {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) { bail!("the code is six digits"); }
    let (addr, port, via_host) = match host.map(str::trim).filter(|h| !h.is_empty()) {
        Some(h) => {
            let (a, p) = h.rsplit_once(':').context("give the host as name:port, as the other computer shows it")?;
            (a.trim_matches(['[', ']']).to_string(), p.parse::<u16>().context("the port in name:port is a number")?, true)
        }
        None => {
            let (ip, p, _) = mdns::pairing_host().context("no computer on this network is showing a code; give its name:port")?;
            (ip.to_string(), p, false)
        }
    };
    let base = format!("http://{}:{port}", if addr.contains(':') { format!("[{addr}]") } else { addr.clone() });
    let http = reqwest::blocking::Client::builder().no_proxy().connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(20)).build()?;
    // the host is not known yet: its answers are a few kilobytes, and no more than this is read of one
    let json = |r: reqwest::blocking::Response| -> Result<Value> { Ok(serde_json::from_slice(&crate::util::body_capped(r, 1024 * 1024, "the other computer's answer")?)?) };
    let hello = json(http.get(format!("{base}/pair/hello")).send().with_context(|| format!("could not reach {addr}:{port}"))?.error_for_status()?)?;
    let their_id = hello["id"].as_str().filter(|s| !s.is_empty()).context("the other computer did not say who it is")?.to_string();
    let me = ctx.settings.device_id.as_str();
    let (st, outbound) = Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &Identity::new(me.as_bytes()), &Identity::new(their_id.as_bytes()));
    let r = json(http.post(format!("{base}/pair/start")).json(&json!({"id": me, "msg": b64(&outbound)})).send()?.error_for_status().context("the other computer refused the pairing")?)?;
    let key = st.finish(&unb64(r["msg"].as_str().unwrap_or(""))?).map_err(|_| anyhow!("the pairing exchange failed"))?;
    let mut mine = payload(ctx)?;
    // the other side learns of a wrong code only from a payload it cannot open
    let address = if via_host { mine["address_hint"].as_str().unwrap_or("").to_string() } else { String::new() };
    // sealed, so nobody on the way can change where the host will reach this computer; the copy beside the payload
    // is all a host up to 1.11.0 reads
    mine["address"] = json!(address);
    let f = http.post(format!("{base}/pair/finish")).json(&json!({"id": me, "payload": seal(&key, &mine)?, "address": address})).send()?;
    if f.status() == reqwest::StatusCode::FORBIDDEN { bail!("the code did not match; check it and try again"); }
    if !f.status().is_success() { bail!("the other computer did not accept the pairing: {}", json(f).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_default()); }
    let answer = json(f)?;
    // a computer from before 1.9.7 sent its payload with its first answer
    let theirs = open(&key, answer["payload"].as_str().or(r["payload"].as_str()).unwrap_or("")).map_err(|_| anyhow!("the code did not match; check it and try again"))?;
    let their_port = theirs["address_hint"].as_str().and_then(|a| a.rsplit_once(':')).map(|(_, p)| p).unwrap_or("0");
    let their_address = if via_host { format!("{addr}:{their_port}") } else { String::new() };
    store_peer(ctx, &theirs, &their_address)?;
    journal::status(ctx)
}

// ---------- a phone: the same exchange on its own routes; the owner approves before anything is sealed back ----------

/// How long an approved or refused phone can still fetch the answer after the code expired.
const RESULT_KEPT: Duration = Duration::from_secs(30);

/// Where a phone pairing stands, and what the owner is shown.
#[derive(Default)]
struct PhoneSt {
    /// "waiting", "confirm", "paired", "refused", "expired", "failed", or "cancelled" (shown to nobody).
    state: &'static str,
    phone: Option<PhonePayload>,
    key: Option<Keyed>,
    sealed: Option<String>,
    error: Option<String>,
    /// The phone fetched the owner's decision: whoever shows the code may close the port now.
    fetched: bool,
}

struct PhoneShared {
    ctx: Arc<Ctx>,
    code: String,
    keys: Mutex<Vec<(String, Keyed)>>,
    failures: AtomicU32,
    st: Mutex<PhoneSt>,
    handle: axum_server::Handle<SocketAddr>,
    on_event: OnEvent,
    /// When the code stops being good (ms).
    deadline: AtomicI64,
    /// `{attempt, code, expires_at, host_hint, port, qr}`.
    shown: Value,
}

impl PhoneShared {
    fn status_of(&self, st: &PhoneSt) -> Value {
        let mut v = self.shown.clone();
        v["state"] = json!(st.state);
        if let Some(p) = &st.phone { v["phone"] = json!({"name": p.name, "platform": p.platform}); }
        if let Some(k) = &st.key { v["words"] = json!(k.words()); }
        if let Some(e) = &st.error { v["error"] = json!(e); }
        if matches!(st.state, "paired" | "refused") { v["fetched"] = json!(st.fetched); }
        v
    }

    fn status(&self) -> Value { self.status_of(&self.st.lock().unwrap_or_else(|e| e.into_inner())) }

    /// A change of state, told as `phones-pairing`; false when the pairing is not at one of `from` any more.
    fn set(&self, from: &[&str], f: impl FnOnce(&mut PhoneSt)) -> bool {
        let v = {
            let mut st = self.st.lock().unwrap_or_else(|e| e.into_inner());
            if !from.contains(&st.state) { return false; }
            f(&mut st);
            self.status_of(&st)
        };
        (self.on_event)("phones-pairing", v);
        true
    }

    fn state(&self) -> &'static str { self.st.lock().unwrap_or_else(|e| e.into_inner()).state }

    /// The code's two minutes are up: a pairing still open expires (a cancelled one says nothing), and the port
    /// closes, after a moment more when a phone still has an answer to fetch.
    async fn expire(&self) {
        self.set(&["waiting", "confirm"], |st| { st.state = "expired"; st.error = Some("the code expired".into()); });
        if matches!(self.state(), "paired" | "refused") { tokio::time::sleep(RESULT_KEPT).await; }
        self.handle.shutdown();
    }
}

/// The computer showing a code to a phone.
pub struct PhoneHost {
    pub expires_at: i64,
    pub port: u16,
    shared: Arc<PhoneShared>,
}

impl PhoneHost {
    pub fn status(&self) -> Value { self.shared.status() }

    /// The id of this attempt: the owner's approve, refuse and cancel name it, so a decision taken on one phone's
    /// words never lands on another's.
    pub fn attempt(&self) -> &str { self.shared.shown["attempt"].as_str().unwrap_or("") }

    /// Stop showing the code; the attempt ends without an event.
    pub fn cancel(&self) {
        self.shared.st.lock().unwrap_or_else(|e| e.into_inner()).state = "cancelled";
        self.shared.handle.shutdown();
    }

    /// The owner approved the phone shown, with these grants: it is stored, and this computer's payload is sealed
    /// for it to fetch, naming `port`, where this computer's listener takes phones. Never the ring key: a phone is not
    /// a computer. Decided under the pairing's lock, with the deadline: the expiry cannot come between the check and
    /// the stored phone, and what is stored is what is told.
    pub fn approve(&self, ctx: &Ctx, grants: Grants, port: u16) -> Result<Value> {
        if port == 0 { bail!("the direct connection has no port yet; turn it on and pair again"); }
        let s = &self.shared;
        let v = {
            let mut st = s.st.lock().unwrap_or_else(|e| e.into_inner());
            if st.state != "confirm" { bail!("no phone waits for approval"); }
            if now_ms() >= s.deadline.load(Ordering::SeqCst) {
                st.state = "expired";
                st.error = Some("the code expired".into());
                let v = s.status_of(&st);
                drop(st);
                (s.on_event)("phones-pairing", v);
                bail!("the code expired before the approval; show a new one");
            }
            let (Some(phone), Some(key)) = (st.phone.clone(), st.key.as_ref()) else { bail!("no phone waits for approval") };
            let id = identity::load(ctx)?;
            let shown = s.shown["host_hint"].as_str().and_then(|h| h.rsplit_once(':')).map(|(h, _)| h.to_string()).unwrap_or_default();
            let mut hints = vec![format!("{shown}:{port}")];
            let me = format!("{}:{port}", crate::util::hostname());
            if !hints.contains(&me) { hints.push(me); }
            let p = DesktopPayload { id: ctx.settings.device_id.clone(), name: ctx.settings.device_name.clone(), cert_pem: id.cert_pem.clone(), pubkey: id.pubkey(),
                address_hints: hints, hub_url: None, hub_token: None, grants, x25519: Some(crate::companion::mailbox::keys(ctx)?.x_public) };
            let sealed = key.seal(&p)?;
            crate::phones::add(ctx, &phone, grants)?;
            st.state = "paired";
            st.sealed = Some(sealed);
            s.status_of(&st)
        };
        (s.on_event)("phones-pairing", v.clone());
        Ok(v)
    }

    /// The owner refused the phone, or stopped waiting for one.
    pub fn refuse(&self) -> Result<Value> {
        if !self.shared.set(&["waiting", "confirm"], |st| st.state = "refused") { bail!("the pairing already ended"); }
        Ok(self.status())
    }

    /// Tests: the code's time is up now.
    #[cfg(test)]
    pub(crate) fn run_out(&self) { self.shared.deadline.store(now_ms() - 1, Ordering::SeqCst); }

    /// Tests: what the expiry task does when the time is up, now.
    #[cfg(test)]
    pub(crate) async fn expire_now(&self) { self.run_out(); self.shared.expire().await; }
}

/// Show a code for a phone: the pairing routes on a free port for two minutes.
pub fn start_phone(ctx: Arc<Ctx>, rt: &tokio::runtime::Handle, bind: IpAddr, host_name: Option<&str>, on_event: OnEvent) -> Result<PhoneHost> {
    let id = identity::load(&ctx)?;
    let code = format!("{:06}", getrandom::u32().map_err(|e| anyhow!("random bytes: {e}"))? % 1_000_000);
    let l = std::net::TcpListener::bind(SocketAddr::new(bind, 0))?;
    l.set_nonblocking(true)?;
    let port = l.local_addr()?.port();
    let expires_at = now_ms() + LIVES_MS;
    let host_hint = format!("{}:{port}", host_name.map(String::from).unwrap_or_else(crate::util::hostname));
    let qr = companion_proto::pairing::qr(&host_hint, &code, &companion_proto::pairing::fingerprint(&id.cert_der));
    let handle = axum_server::Handle::new();
    let shown = json!({"attempt": crate::util::uuid4(), "code": code, "expires_at": expires_at, "host_hint": host_hint, "port": port, "qr": qr});
    let shared = Arc::new(PhoneShared { ctx: ctx.clone(), code, keys: Mutex::new(vec![]), failures: AtomicU32::new(0), st: Mutex::new(PhoneSt { state: "waiting", ..Default::default() }),
        handle: handle.clone(), on_event, deadline: AtomicI64::new(expires_at), shown });
    let app = Router::new().route("/pair/hello", get(phone_hello)).route("/pair/phone/start", post(phone_start)).route("/pair/phone/finish", post(phone_finish))
        .route("/pair/phone/result", get(phone_result)).with_state(shared.clone());
    let _in_runtime = rt.enter();
    let server = axum_server::from_tcp(l)?.handle(handle.clone());
    rt.spawn(async move { let _ = server.serve(app.into_make_service()).await; });
    let s2 = shared.clone();
    rt.spawn(async move {
        tokio::time::sleep(Duration::from_millis(LIVES_MS as u64)).await;
        s2.expire().await;
    });
    Ok(PhoneHost { expires_at, port, shared })
}

async fn phone_hello(State(s): State<Arc<PhoneShared>>) -> Json<Value> { Json(json!({"id": s.ctx.settings.device_id, "name": s.ctx.settings.device_name})) }

fn refused(code: StatusCode, why: &str) -> Response { (code, Json(json!({"ok": false, "error": why}))).into_response() }

/// The phone's SPAKE2 message in, ours out; nothing sealed with it, as for computers.
async fn phone_start(State(s): State<Arc<PhoneShared>>, Json(a): Json<Value>) -> Response {
    if s.state() != "waiting" { return refused(StatusCode::CONFLICT, "another phone is pairing"); }
    let phone = a["id"].as_str().unwrap_or("").to_string();
    if phone.is_empty() { return StatusCode::BAD_REQUEST.into_response(); }
    let Ok((key, msg)) = companion_proto::pairing::answer(&s.code, &phone, &s.ctx.settings.device_id, a["msg"].as_str().unwrap_or("")) else { return StatusCode::BAD_REQUEST.into_response() };
    let mut keys = s.keys.lock().unwrap_or_else(|e| e.into_inner());
    keys.retain(|(id, _)| id != &phone);
    if keys.len() >= OPEN_STARTS { keys.remove(0); }
    keys.push((phone, key));
    Json(json!({"id": s.ctx.settings.device_id, "msg": msg})).into_response()
}

/// The phone's sealed payload: opened, it waits for the owner with its words; otherwise one failure more.
async fn phone_finish(State(s): State<Arc<PhoneShared>>, Json(a): Json<Value>) -> Response {
    let phone = a["id"].as_str().unwrap_or("").to_string();
    let key = { let mut keys = s.keys.lock().unwrap_or_else(|e| e.into_inner()); keys.iter().position(|(id, _)| id == &phone).map(|i| keys.remove(i).1) };
    let Some(key) = key else { return refused(StatusCode::BAD_REQUEST, "start first") };
    match key.open::<PhonePayload>(a["payload"].as_str().unwrap_or("")) {
        Ok(p) => {
            if p.id != phone || identity::pem_to_der(&p.cert_pem).is_err() { return refused(StatusCode::BAD_REQUEST, "the phone's payload is not usable"); }
            let (name, platform) = (p.name.clone(), p.platform.clone());
            if !s.set(&["waiting"], |st| { st.state = "confirm"; st.phone = Some(p); st.key = Some(key); }) { return refused(StatusCode::CONFLICT, "another phone is pairing"); }
            crate::log::info(&format!("phones: {name} ({platform}) waits for approval"));
            Json(json!({"ok": true, "name": s.ctx.settings.device_name})).into_response()
        }
        Err(_) => {
            let n = s.failures.fetch_add(1, Ordering::SeqCst) + 1;
            crate::log::warn(&format!("phones: a pairing attempt with a wrong code ({n} of {FAILURES})"));
            // only the failure that ends a pairing still waiting closes the port, not one that lost to a phone at confirm
            if n >= FAILURES && s.set(&["waiting"], |st| { st.state = "failed"; st.error = Some("three wrong codes; show a new one".into()); }) {
                s.handle.graceful_shutdown(Some(Duration::from_secs(2)));
            }
            refused(StatusCode::FORBIDDEN, "the code did not match")
        }
    }
}

#[derive(serde::Deserialize)]
struct Of {
    id: String,
}

/// What the owner decided, for the phone that finished: `pending`, `approved` with the sealed payload, `refused`.
/// The answer can be fetched again until the attempt ends (a moment past the code's expiry), so one lost on the way
/// is not lost for good.
async fn phone_result(State(s): State<Arc<PhoneShared>>, Query(q): Query<Of>) -> Response {
    let (state, sealed, told) = {
        let mut st = s.st.lock().unwrap_or_else(|e| e.into_inner());
        if st.phone.as_ref().map(|p| p.id.as_str()) != Some(q.id.as_str()) { return refused(StatusCode::NOT_FOUND, "no such phone is pairing"); }
        // the decision is on its way to the phone: the first time, whoever shows the code is told
        let told = matches!(st.state, "paired" | "refused") && !st.fetched;
        if told { st.fetched = true; }
        (st.state, st.sealed.clone(), told.then(|| s.status_of(&st)))
    };
    if let Some(v) = told { (s.on_event)("phones-pairing", v); }
    let expires_in = (s.deadline.load(Ordering::SeqCst) - now_ms()).max(0);
    Json(match state {
        "confirm" | "waiting" => PairResult { state: "pending".into(), payload: None, expires_in },
        "paired" => PairResult { state: "approved".into(), payload: sealed, expires_in },
        "refused" => PairResult { state: "refused".into(), payload: None, expires_in },
        _ => PairResult { state: "expired".into(), payload: None, expires_in: 0 },
    }).into_response()
}
