//! The direct connection's server: HTTPS on `listen_port` with mutual TLS, where the only client certificates
//! let in are those of paired computers and phones (read from `peers` and `phones` at each handshake, so a
//! forgotten computer or a revoked phone is refused at once). A computer gets the `/v1` routes: who this is, an
//! origin's chunk list, a chunk, what runs here now and a transcript. A phone gets only the companion API (`/c1`,
//! `crate::companion`): each connection carries who showed which certificate.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{CertificateError, DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{identity, store};
use crate::sync::folder;
use crate::Ctx;

/// Accepts a client certificate only when it is a paired computer's or a phone's that is not revoked.
struct Paired {
    ctx: Arc<Ctx>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl std::fmt::Debug for Paired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Paired") }
}

impl ClientCertVerifier for Paired {
    fn root_hint_subjects(&self) -> &[DistinguishedName] { &[] }

    fn verify_client_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _now: UnixTime) -> Result<ClientCertVerified, Error> {
        let pems: Vec<String> = self.ctx.db.with(|c| {
            let v = c.prepare("select cert_pem from peers where coalesce(cert_pem, '') <> '' union all select cert_pem from phones where revoked_at is null and coalesce(cert_pem, '') <> ''")?
                .query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            Ok(v)
        }).unwrap_or_default();
        if pems.iter().any(|p| identity::pem_to_der(p).ok().as_deref() == Some(end_entity.as_ref())) { Ok(ClientCertVerified::assertion()) }
        else { Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

pub struct Listener {
    handle: axum_server::Handle<SocketAddr>,
    pub port: u16,
    data: std::path::PathBuf,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

/// Every paired computer asks for this computer's record and presence every ten seconds, and each costs a scan
/// of usage, the credential store and a signature, or a look at the processes: one making serves them this long.
pub(super) const KEPT: Duration = Duration::from_secs(30);
pub(super) type Kept = Arc<Mutex<Option<(Instant, Value)>>>;

/// What the routes share: the context the listener started with, and the two answers it keeps.
#[derive(Clone)]
struct Served {
    ctx: Arc<Ctx>,
    record: Kept,
    presence: Kept,
}

impl axum::extract::FromRef<Served> for Arc<Ctx> {
    fn from_ref(s: &Served) -> Self { s.ctx.clone() }
}

/// The answer kept in `slot`, made again once it is older than `KEPT`; polls that come together wait for one making.
pub(super) fn kept(slot: &Kept, make: impl FnOnce() -> Result<Value>) -> Result<Value> {
    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, v)) = s.as_ref() { if at.elapsed() < KEPT { return Ok(v.clone()); } }
    let v = make()?;
    *s = Some((Instant::now(), v.clone()));
    Ok(v)
}

impl Listener {
    pub fn stop(&self) {
        self.stopped.store(true, std::sync::atomic::Ordering::SeqCst);
        crate::companion::conn::listening(&self.data, false);
        self.handle.shutdown();
    }
}

/// Serve on `bind:port` (0: any free port; the one taken is in the result) from the runtime's threads.
pub fn start(ctx: Arc<Ctx>, rt: &tokio::runtime::Handle, bind: IpAddr, port: u16) -> Result<Listener> {
    let id = identity::load(&ctx)?;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?
        .with_client_cert_verifier(Arc::new(Paired { ctx: ctx.clone(), provider }))
        .with_single_cert(vec![CertificateDer::from(id.cert_der.clone())], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key_der.clone())))?;
    let l = std::net::TcpListener::bind(SocketAddr::new(bind, port))?;
    l.set_nonblocking(true)?;
    let port = l.local_addr()?.port();
    let handle = axum_server::Handle::new();
    let app = Router::new()
        .route("/v1/hello", get(hello))
        .route("/v1/chunks/{origin}", get(chunks))
        .route("/v1/chunk/{origin}/{file}", get(chunk))
        .route("/v1/presence", get(presence))
        .route("/v1/transcript/{origin}/{harness}/{profile}/{id}", get(transcript))
        .route_layer(axum::middleware::from_fn(crate::companion::peers_only))
        .with_state(Served { ctx: ctx.clone(), record: Kept::default(), presence: Kept::default() })
        .merge(crate::companion::router(ctx.clone()));
    let _in_runtime = rt.enter();
    let server = axum_server::from_tcp_rustls(l, axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(cfg)))?
        .map(|inner| crate::companion::conn::Acceptor { inner, ctx: ctx.clone() })
        .handle(handle.clone());
    rt.spawn(async move {
        if let Err(e) = server.serve(app.into_make_service()).await { crate::log::warn(&format!("computers: the listener stopped: {e}")); }
    });
    // this listener holds the phones' connections: it cuts those revoked or paired again from any process
    crate::companion::conn::listening(&ctx.data, true);
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (s2, data) = (stopped.clone(), ctx.data.clone());
    rt.spawn(async move {
        while !s2.load(std::sync::atomic::Ordering::SeqCst) {
            let c = ctx.clone();
            let _ = tokio::task::spawn_blocking(move || crate::companion::conn::sweep(&c)).await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    Ok(Listener { handle, port, data, stopped })
}

/// The database and process scans are blocking work: off the runtime's threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.expect("blocking work")
}

/// Who this computer is, as `/v1/hello` answers: with the origins it holds chunks of (itself included).
pub fn hello_of(ctx: &Ctx) -> Value {
    let last_seq: i64 = ctx.db.with(|c| Ok(c.query_row("select coalesce(max(seq), 0) from outbox", [], |r| r.get(0))?)).unwrap_or(0);
    json!({"id": ctx.settings.device_id, "name": ctx.settings.device_name, "incarnation": ctx.settings.incarnation, "last_seq": last_seq,
        "app_version": env!("CARGO_PKG_VERSION"), "pubkey": identity::pubkey(ctx), "origins": store::origins(ctx)})
}

async fn hello(State(ctx): State<Arc<Ctx>>) -> Json<Value> { Json(blocking(move || hello_of(&ctx)).await) }

#[derive(Deserialize)]
struct Since {
    incarnation: Option<String>,
    since: Option<i64>,
}

/// An origin's chunks after `since` (of the asked incarnation; another one answers with no chunks and its own
/// name, so the asker can ask again), with the origin's signed device record when there is one.
async fn chunks(State(s): State<Served>, Path(origin): Path<String>, Query(q): Query<Since>) -> Response {
    blocking(move || {
        let ctx = &s.ctx;
        let Some(man) = store::manifest(ctx, &origin) else { return StatusCode::NOT_FOUND.into_response() };
        let inc = man["incarnation"].as_str().unwrap_or("").to_string();
        let same = q.incarnation.as_deref().map(|i| i == inc).unwrap_or(true);
        let since = q.since.unwrap_or(0);
        let list: Vec<Value> = if same { man["chunks"].as_array().into_iter().flatten().filter(|c| c["to"].as_i64().unwrap_or(0) > since).cloned().collect() } else { vec![] };
        let device = if origin == ctx.settings.device_id { kept(&s.record, || folder::device_record(ctx)).ok() } else { store::device(ctx, &origin) };
        Json(json!({"origin": origin, "incarnation": inc, "last_seq": man["last_seq"], "chunks": list, "device": device})).into_response()
    }).await
}

async fn chunk(State(ctx): State<Arc<Ctx>>, Path((origin, file)): Path<(String, String)>) -> Response {
    blocking(move || match store::chunk(&ctx, &origin, &file) {
        Ok(b) => (StatusCode::OK, b).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }).await
}

/// A transcript as it is carried (zstd): this computer's own from the tool's file, another origin's from the
/// copy fetched here. The asker checks the sha256 against the journal entry.
async fn transcript(State(ctx): State<Arc<Ctx>>, Path((origin, harness, profile, id)): Path<(String, String, String, String)>) -> Response {
    blocking(move || {
        if origin == ctx.settings.device_id && !switch(&ctx, "sync_transcripts", ctx.settings.sync_transcripts) { return StatusCode::NOT_FOUND.into_response(); }
        let r = if origin == ctx.settings.device_id { crate::sync::transcripts::own_zst(&ctx, &harness, &profile, &id) } else {
            let key = format!("{harness}/{profile}/{id}");
            ctx.db.with(|c| { crate::transcripts::tables(c)?; Ok(c.query_row("select file from transcripts where device=? and key=? and fetched_at is not null", [&origin, &key], |r| r.get::<_, String>(0))?) })
                .and_then(|file| Ok(std::fs::read(crate::transcripts::local_path(&ctx, &origin, &harness, &profile, &file)?)?))
                .and_then(|plain| Ok(zstd::encode_all(&plain[..], 3)?))
        };
        match r {
            Ok(b) => (StatusCode::OK, b).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    }).await
}

async fn presence(State(s): State<Served>) -> Response {
    blocking(move || {
        if !switch(&s.ctx, "sync_presence", s.ctx.settings.sync_presence) { return StatusCode::NOT_FOUND.into_response(); }
        match kept(&s.presence, || folder::presence(&s.ctx)) {
            Ok(v) => Json(v).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
        }
    }).await
}

/// A Computers switch as it is saved now: the listener keeps the context it started with, and a switch turned
/// off since then holds at once. `started` when the settings cannot be read.
fn switch(ctx: &Ctx, name: &str, started: bool) -> bool {
    crate::util::read_json(&ctx.data.join("settings.json")).and_then(|s| s[name].as_bool()).unwrap_or(started)
}
