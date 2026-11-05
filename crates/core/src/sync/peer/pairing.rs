//! Pairing two computers with a six-digit code. The computer showing the code serves a plain-HTTP route on a
//! port that lives two minutes; the joiner runs SPAKE2 with the code as the password (the two device ids as the
//! identities, the joiner as A), both derive a key, and exchange their identity payloads encrypted with it. A
//! wrong code fails at decryption; three failures or two minutes end it. Both sides then hold the other's
//! certificate and key.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use rusqlite::params;
use serde_json::{json, Value};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use super::{identity, mdns, OnEvent};
use crate::util::now_ms;
use crate::{journal, Ctx};

const LIVES_MS: i64 = 120_000;
const FAILURES: u32 = 3;

fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
fn unb64(s: &str) -> Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

/// `{nonce ++ ciphertext}` base64 of the JSON, under the pairing key.
fn seal(key: &[u8], v: &Value) -> Result<String> {
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
    Ok(json!({"id": s.device_id, "name": s.device_name, "os": std::env::consts::OS, "incarnation": s.incarnation, "pubkey": id.pubkey(), "cert_pem": id.cert_pem,
        "address_hint": format!("{}:{}", crate::util::hostname(), s.listen_port), "app_version": env!("CARGO_PKG_VERSION")}))
}

/// The other side, as a peer: paired now, reached at `address` (empty when found by mDNS).
pub fn store_peer(ctx: &Ctx, p: &Value, address: &str) -> Result<Value> {
    let id = p["id"].as_str().filter(|s| !s.is_empty()).context("the payload has no id")?;
    if id == ctx.settings.device_id { bail!("that is this computer"); }
    let port: u16 = p["address_hint"].as_str().and_then(|a| a.rsplit_once(':')).and_then(|(_, p)| p.parse().ok()).unwrap_or(0);
    if p["cert_pem"].as_str().map(|c| identity::pem_to_der(c).is_err()).unwrap_or(true) { bail!("the payload has no certificate"); }
    ctx.db.with(|c| {
        c.execute("insert into peers(device_id, name, os, app_version, incarnation, cert_pem, pubkey, paired_at, address, port, transport) values(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'direct')
            on conflict(device_id) do update set name=?2, os=?3, app_version=?4, incarnation=?5, cert_pem=?6, pubkey=?7, paired_at=?8, address=?9, port=?10, transport='direct'",
            params![id, p["name"].as_str().unwrap_or(""), p["os"].as_str().unwrap_or(""), p["app_version"].as_str().unwrap_or(""), p["incarnation"].as_str().unwrap_or(""),
                p["cert_pem"].as_str().unwrap_or(""), p["pubkey"].as_str().unwrap_or(""), now_ms(), address, port])?;
        Ok(())
    })?;
    crate::log::info(&format!("computers: paired with {} ({id})", p["name"].as_str().unwrap_or("")));
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
        *self.done.lock().unwrap() = Some(v.clone());
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
    pub fn result(&self) -> Option<Value> { self.shared.done.lock().unwrap().clone() }

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
        if s2.done.lock().unwrap().is_none() { s2.end(json!({"ok": false, "error": "the code expired"})); }
        h2.shutdown();
    });
    let expires_at = now_ms() + LIVES_MS;
    // the computer's name on the network, which the other computer can resolve; a VPN name or an IP when given
    let host_hint = format!("{}:{port}", host_name.map(String::from).unwrap_or_else(crate::util::hostname));
    ctx.db.kv_set("pairing:current", &json!({"code": code, "expires_at": expires_at, "host_hint": host_hint, "port": port}).to_string())?;
    Ok(Host { code, expires_at, port, host_hint, shared })
}

async fn hello(State(s): State<Arc<Shared>>) -> Json<Value> { Json(json!({"id": s.ctx.settings.device_id, "name": s.ctx.settings.device_name})) }

/// The joiner's SPAKE2 message in, ours and our sealed payload out.
async fn start_route(State(s): State<Arc<Shared>>, Json(a): Json<Value>) -> Response {
    let (joiner, msg) = (a["id"].as_str().unwrap_or("").to_string(), a["msg"].as_str().unwrap_or(""));
    let Ok(inbound) = unb64(msg) else { return StatusCode::BAD_REQUEST.into_response() };
    if joiner.is_empty() { return StatusCode::BAD_REQUEST.into_response(); }
    let (st, outbound) = Spake2::<Ed25519Group>::start_b(&Password::new(s.code.as_bytes()), &Identity::new(joiner.as_bytes()), &Identity::new(s.ctx.settings.device_id.as_bytes()));
    let Ok(key) = st.finish(&inbound) else { return StatusCode::BAD_REQUEST.into_response() };
    let ctx = s.ctx.clone();
    let mine = tokio::task::spawn_blocking(move || payload(&ctx)).await.expect("payload");
    let sealed = match mine.and_then(|m| seal(&key, &m)) { Ok(p) => p, Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response() };
    let mut keys = s.keys.lock().unwrap();
    keys.retain(|(id, _)| id != &joiner);
    keys.push((joiner, key));
    Json(json!({"id": s.ctx.settings.device_id, "msg": b64(&outbound), "payload": sealed})).into_response()
}

/// The joiner's sealed payload: opened with its key, the peer is stored; otherwise one failure more.
async fn finish(State(s): State<Arc<Shared>>, Json(a): Json<Value>) -> Response {
    let joiner = a["id"].as_str().unwrap_or("").to_string();
    let key = { let mut keys = s.keys.lock().unwrap(); keys.iter().position(|(id, _)| id == &joiner).map(|i| keys.remove(i).1) };
    let Some(key) = key else { return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "start first"}))).into_response() };
    match open(&key, a["payload"].as_str().unwrap_or("")) {
        Ok(p) => {
            let (ctx, address) = (s.ctx.clone(), a["address"].as_str().unwrap_or("").to_string());
            match tokio::task::spawn_blocking(move || store_peer(&ctx, &p, &address)).await.expect("store") {
                Ok(peer) => { s.end(json!({"ok": true, "peer": peer})); Json(json!({"ok": true, "name": s.ctx.settings.device_name})).into_response() }
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
    let hello: Value = http.get(format!("{base}/pair/hello")).send().with_context(|| format!("could not reach {addr}:{port}"))?.error_for_status()?.json()?;
    let their_id = hello["id"].as_str().filter(|s| !s.is_empty()).context("the other computer did not say who it is")?.to_string();
    let me = ctx.settings.device_id.as_str();
    let (st, outbound) = Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &Identity::new(me.as_bytes()), &Identity::new(their_id.as_bytes()));
    let r: Value = http.post(format!("{base}/pair/start")).json(&json!({"id": me, "msg": b64(&outbound)})).send()?.error_for_status().context("the other computer refused the pairing")?.json()?;
    let key = st.finish(&unb64(r["msg"].as_str().unwrap_or(""))?).map_err(|_| anyhow!("the pairing exchange failed"))?;
    let theirs = open(&key, r["payload"].as_str().unwrap_or(""));
    let mine = payload(ctx)?;
    // the other side learns of a wrong code only from a payload it cannot open: send ours either way
    let address = if via_host { mine["address_hint"].as_str().unwrap_or("").to_string() } else { String::new() };
    let f = http.post(format!("{base}/pair/finish")).json(&json!({"id": me, "payload": seal(&key, &mine)?, "address": address})).send()?;
    let theirs = theirs.map_err(|_| anyhow!("the code did not match; check it and try again"))?;
    if !f.status().is_success() { bail!("the other computer did not accept the pairing: {}", f.json::<Value>().ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_default()); }
    let their_port = theirs["address_hint"].as_str().and_then(|a| a.rsplit_once(':')).map(|(_, p)| p).unwrap_or("0");
    let their_address = if via_host { format!("{addr}:{their_port}") } else { String::new() };
    store_peer(ctx, &theirs, &their_address)?;
    journal::status(ctx)
}
