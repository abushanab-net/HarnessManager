//! Spike host: the desktop's pairing route (`pairing.rs`: hello, start, finish, same JSON, same SPAKE2 roles and
//! sealing) on plain HTTP, and a mutual-TLS listener configured like `listener.rs` (aws-lc-rs provider, the host's
//! Ed25519 self-signed certificate, client certificates let in only when their DER equals a paired phone's).
//! The TLS side also serves a toy `/c1/requests` + `/c1/actions` for the timing spike, and the plain side a
//! `/spike/*` control surface for the driver script. Binds 127.0.0.1: the emulator reaches it as 10.0.2.2.
//!
//!   mobile-spike-host serve [--code 123456]
//!   mobile-spike-host client-cert <out-dir>     a client identity for the Dart CLI test, registered as a phone

use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use ed25519_dalek::SigningKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{CertificateError, DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};
use serde_json::{json, Value};
use spake2::{Ed25519Group, Identity, Password, Spake2};

const PAIR_PORT: u16 = 47801;
const TLS_PORT: u16 = 47802;
/// The same listener with an ECDSA P-256 server certificate instead: does dart:io get through when only the
/// server's key changes? (The phone's client certificate stays Ed25519.)
const P256_PORT: u16 = 47803;
const PKCS8_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

fn now_ms() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64 }
fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
fn unb64(s: &str) -> Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

// ---- copied from pairing.rs ----
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

// ---- copied from identity.rs ----
struct Ident { pubkey: String, key_der: Vec<u8>, cert_pem: String, cert_der: Vec<u8> }

fn ident_from_seed(seed: [u8; 32], cn: &str) -> Result<Ident> {
    let key = SigningKey::from_bytes(&seed);
    let key_der = [PKCS8_PREFIX.as_slice(), &seed].concat();
    let kp = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&PrivatePkcs8KeyDer::from(key_der.clone()), &rcgen::PKCS_ED25519)?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())?;
    p.distinguished_name.push(rcgen::DnType::CommonName, cn);
    p.serial_number = Some(rcgen::SerialNumber::from(1u64));
    p.not_before = rcgen::date_time_ymd(2026, 1, 1);
    p.not_after = rcgen::date_time_ymd(2036, 1, 1);
    let cert = p.self_signed(&kp)?;
    Ok(Ident { pubkey: b64(&key.verifying_key().to_bytes()), key_der, cert_pem: cert.pem(), cert_der: cert.der().to_vec() })
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    unb64(&body).context("certificate pem")
}

fn random32() -> [u8; 32] { let mut s = [0u8; 32]; getrandom::fill(&mut s).expect("random"); s }

fn data_dir() -> PathBuf { std::env::var_os("HOST_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("host-data")) }

/// The host's identity, kept in `host-data/` so the phone's pin survives a restart.
fn host_ident(dir: &FsPath) -> Result<Ident> {
    std::fs::create_dir_all(dir.join("phones"))?;
    let f = dir.join("host.seed");
    let seed: [u8; 32] = match std::fs::read_to_string(&f).ok().and_then(|s| hex::decode(s.trim()).ok()).and_then(|v| v.try_into().ok()) {
        Some(s) => s,
        None => { let s = random32(); std::fs::write(&f, hex::encode(s))?; s }
    };
    let id = ident_from_seed(seed, "spike-host")?;
    std::fs::write(dir.join("host.crt"), &id.cert_pem)?;
    Ok(id)
}

/// The pinned phone certificates, read at each handshake as `listener.rs` reads `peers`.
fn phone_ders(dir: &FsPath) -> Vec<Vec<u8>> {
    std::fs::read_dir(dir.join("phones")).into_iter().flatten().flatten().filter_map(|e| std::fs::read(e.path()).ok()).collect()
}

#[derive(Debug)]
struct Paired { dir: PathBuf, provider: Arc<rustls::crypto::CryptoProvider> }

impl ClientCertVerifier for Paired {
    fn root_hint_subjects(&self) -> &[DistinguishedName] { &[] }
    fn verify_client_cert(&self, end_entity: &CertificateDer<'_>, _i: &[CertificateDer<'_>], _now: UnixTime) -> Result<ClientCertVerified, Error> {
        if phone_ders(&self.dir).iter().any(|d| d.as_slice() == end_entity.as_ref()) { Ok(ClientCertVerified::assertion()) }
        else { eprintln!("tls: refused an unknown client certificate"); Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

struct Shared {
    dir: PathBuf,
    code: String,
    device_id: String,
    id: Ident,
    keys: Mutex<Vec<(String, Vec<u8>)>>,
    failures: AtomicU32,
    /// `{event, at}` lines for the driver: host clock, ms since the epoch.
    log: Mutex<Vec<Value>>,
    asks: Mutex<Vec<Value>>,
    receipts: Mutex<Vec<Value>>,
}

impl Shared {
    fn mark(&self, event: &str, extra: Value) {
        let mut v = json!({"event": event, "at": now_ms()});
        if let Value::Object(o) = extra { for (k, x) in o { v[k] = x; } }
        println!("{v}");
        self.log.lock().unwrap().push(v);
    }
}

type S = State<Arc<Shared>>;

// ---- pairing routes: the same bodies as pairing.rs ----
async fn hello(State(s): S) -> Json<Value> { s.mark("pair-hello", json!({})); Json(json!({"id": s.device_id, "name": "Spike host"})) }

async fn start_route(State(s): S, Json(a): Json<Value>) -> Response {
    let (joiner, msg) = (a["id"].as_str().unwrap_or("").to_string(), a["msg"].as_str().unwrap_or(""));
    let Ok(inbound) = unb64(msg) else { return StatusCode::BAD_REQUEST.into_response() };
    if joiner.is_empty() { return StatusCode::BAD_REQUEST.into_response(); }
    let (st, outbound) = Spake2::<Ed25519Group>::start_b(&Password::new(s.code.as_bytes()), &Identity::new(joiner.as_bytes()), &Identity::new(s.device_id.as_bytes()));
    let Ok(key) = st.finish(&inbound) else { return StatusCode::BAD_REQUEST.into_response() };
    let mut keys = s.keys.lock().unwrap();
    keys.retain(|(id, _)| id != &joiner);
    keys.push((joiner.clone(), key));
    s.mark("pair-start", json!({"joiner": joiner}));
    Json(json!({"id": s.device_id, "msg": b64(&outbound)})).into_response()
}

async fn finish(State(s): S, Json(a): Json<Value>) -> Response {
    let joiner = a["id"].as_str().unwrap_or("").to_string();
    let key = { let mut keys = s.keys.lock().unwrap(); keys.iter().position(|(id, _)| id == &joiner).map(|i| keys.remove(i).1) };
    let Some(key) = key else { return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "start first"}))).into_response() };
    match open(&key, a["payload"].as_str().unwrap_or("")) {
        Ok(p) => {
            let Some(der) = p["cert_pem"].as_str().and_then(|c| pem_to_der(c).ok()) else {
                return (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": "the payload has no certificate"}))).into_response();
            };
            let pid = p["id"].as_str().unwrap_or("phone").replace(['/', '\\', '.'], "_");
            if let Err(e) = std::fs::write(s.dir.join("phones").join(format!("{pid}.der")), &der) { return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(); }
            // the desktop's payload() minus `ring`: a phone never gets the ring key
            let mine = json!({"id": s.device_id, "name": "Spike host", "os": std::env::consts::OS, "incarnation": "spike", "pubkey": s.id.pubkey,
                "cert_pem": s.id.cert_pem, "address_hint": format!("10.0.2.2:{TLS_PORT}"), "app_version": "spike"});
            s.mark("pair-finish", json!({"ok": true, "phone": p["name"], "platform": p["platform"], "phone_pubkey": p["pubkey"]}));
            Json(json!({"ok": true, "name": "Spike host", "payload": seal(&key, &mine).unwrap()})).into_response()
        }
        Err(_) => {
            let n = s.failures.fetch_add(1, Ordering::SeqCst) + 1;
            s.mark("pair-wrong-code", json!({"n": n}));
            (StatusCode::FORBIDDEN, Json(json!({"ok": false, "error": "the code did not match"}))).into_response()
        }
    }
}

// ---- driver control (plain HTTP, the PC only) ----
async fn spike_ask(State(s): S, Json(a): Json<Value>) -> Json<Value> {
    let id = format!("ask-{}", now_ms());
    let ask = json!({"run": "run-1", "ask_id": id, "kind": "permission", "tool": "Bash", "what": a["what"].as_str().unwrap_or("rm -rf build/"),
        "expires_at": now_ms() + 60_000, "state_version": 1});
    s.asks.lock().unwrap().push(ask.clone());
    s.mark("ask-open", json!({"ask_id": id}));
    Json(ask)
}
async fn spike_mark(State(s): S, Json(a): Json<Value>) -> StatusCode { s.mark(a["event"].as_str().unwrap_or("mark"), a.clone()); StatusCode::NO_CONTENT }
async fn spike_log(State(s): S) -> Json<Value> { Json(Value::Array(s.log.lock().unwrap().clone())) }
async fn spike_reset(State(s): S) -> StatusCode { s.log.lock().unwrap().clear(); s.asks.lock().unwrap().clear(); StatusCode::NO_CONTENT }

// ---- the companion side, over mutual TLS ----
fn computer(s: &Shared) -> Value { json!({"id": s.device_id, "name": "Spike host"}) }

async fn c1_hello(State(s): S) -> Json<Value> { s.mark("c1-hello", json!({})); Json(json!({"computer": computer(&s), "observed_at": now_ms(), "api": 1})) }

async fn c1_requests(State(s): S) -> Json<Value> {
    let open: Vec<Value> = s.asks.lock().unwrap().iter().filter(|a| a["expires_at"].as_i64().unwrap_or(0) > now_ms() && a["answered"].is_null()).cloned().collect();
    s.mark("c1-requests", json!({"n": open.len()}));
    Json(json!({"computer": computer(&s), "observed_at": now_ms(), "requests": open}))
}

async fn c1_actions(State(s): S, Json(a): Json<Value>) -> Json<Value> {
    let action_id = a["action_id"].as_str().unwrap_or("").to_string();
    if let Some(r) = s.receipts.lock().unwrap().iter().find(|r| r["action_id"] == action_id.as_str()) { return Json(r.clone()); }
    let ask_id = a["target"]["ask_id"].as_str().unwrap_or("");
    let mut asks = s.asks.lock().unwrap();
    let status = match asks.iter_mut().find(|x| x["ask_id"] == ask_id) {
        None => "unavailable",
        Some(x) if !x["answered"].is_null() => "already_handled",
        Some(x) if x["expires_at"].as_i64().unwrap_or(0) <= now_ms() => "expired",
        Some(x) => { x["answered"] = a["payload"].clone(); "accepted" }
    };
    let r = json!({"action_id": action_id, "status": status, "detail": "", "at": now_ms()});
    s.receipts.lock().unwrap().push(r.clone());
    s.mark("c1-action", json!({"ask_id": ask_id, "status": status}));
    Json(r)
}

async fn c1_mark(State(s): S, Path(what): Path<String>, Json(a): Json<Value>) -> StatusCode { s.mark(&format!("app-{what}"), a); StatusCode::NO_CONTENT }

async fn serve(code: String) -> Result<()> {
    let dir = data_dir();
    let id = host_ident(&dir)?;
    let device_id = "spike-host-0001".to_string();
    let s = Arc::new(Shared { dir: dir.clone(), code: code.clone(), device_id, id, keys: Mutex::new(vec![]), failures: AtomicU32::new(0),
        log: Mutex::new(vec![]), asks: Mutex::new(vec![]), receipts: Mutex::new(vec![]) });

    let pair = Router::new().route("/pair/hello", get(hello)).route("/pair/start", post(start_route)).route("/pair/finish", post(finish))
        .route("/spike/ask", post(spike_ask)).route("/spike/mark", post(spike_mark)).route("/spike/log", get(spike_log)).route("/spike/reset", post(spike_reset))
        .with_state(s.clone());

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?
        .with_client_cert_verifier(Arc::new(Paired { dir: dir.clone(), provider }))
        .with_single_cert(vec![CertificateDer::from(s.id.cert_der.clone())], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(s.id.key_der.clone())))?;
    let tls = Router::new().route("/c1/hello", get(c1_hello)).route("/c1/requests", get(c1_requests)).route("/c1/actions", post(c1_actions))
        .route("/c1/mark/{what}", post(c1_mark)).with_state(s.clone());

    let p256 = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let p256_cert = rcgen::CertificateParams::new(Vec::<String>::new())?.self_signed(&p256)?;
    std::fs::write(dir.join("host-p256.der"), p256_cert.der())?;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let cfg256 = rustls::ServerConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?
        .with_client_cert_verifier(Arc::new(Paired { dir: dir.clone(), provider }))
        .with_single_cert(vec![p256_cert.der().clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(p256.serialize_der())))?;
    let p256_srv = axum_server::bind_rustls(SocketAddr::from(([127, 0, 0, 1], P256_PORT)), axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(cfg256)))
        .serve(tls.clone().into_make_service());
    println!("{}", json!({"event": "ready", "code": code, "pair_port": PAIR_PORT, "tls_port": TLS_PORT, "host_cert": dir.join("host.crt")}));
    let pair_srv = axum_server::bind(SocketAddr::from(([127, 0, 0, 1], PAIR_PORT))).serve(pair.into_make_service());
    let tls_srv = axum_server::bind_rustls(SocketAddr::from(([127, 0, 0, 1], TLS_PORT)), axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(cfg)))
        .serve(tls.into_make_service());
    tokio::try_join!(pair_srv, tls_srv, p256_srv)?;
    Ok(())
}

fn client_cert(out: &FsPath) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let id = ident_from_seed(random32(), "spike-dart-cli")?;
    let key_pem = format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", b64(&id.key_der));
    std::fs::write(out.join("client.key"), key_pem)?;
    std::fs::write(out.join("client.crt"), &id.cert_pem)?;
    let dir = data_dir();
    let host = host_ident(&dir)?;
    std::fs::write(out.join("host.der"), &host.cert_der)?;
    std::fs::write(dir.join("phones").join("dart-cli.der"), &id.cert_der)?;
    // a second identity the host does not know, for the refusal test
    let stranger = ident_from_seed(random32(), "stranger")?;
    std::fs::write(out.join("stranger.key"), format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", b64(&stranger.key_der)))?;
    std::fs::write(out.join("stranger.crt"), &stranger.cert_pem)?;
    println!("client identity in {}; registered as a phone; host certificate pinned in host.der", out.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") => {
            let code = args.iter().position(|a| a == "--code").and_then(|i| args.get(i + 1)).cloned()
                .unwrap_or_else(|| format!("{:06}", u32::from_le_bytes(random32()[..4].try_into().unwrap()) % 1_000_000));
            serve(code).await
        }
        Some("client-cert") => client_cert(FsPath::new(args.get(2).context("client-cert <out-dir>")?)),
        _ => bail!("usage: mobile-spike-host serve [--code 123456] | client-cert <out-dir>"),
    }
}
