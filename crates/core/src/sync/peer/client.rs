//! The direct connection's client: this computer's certificate shown, the peer's pinned as the only one accepted
//! (so a name or an address that changes does not matter), and the pull of every origin a peer holds.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rusqlite::params;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use serde_json::Value;

use super::{identity, mdns, store};
use crate::journal::{self, Peer};
use crate::util::{body_capped, now_ms};
use crate::Ctx;

/// Accepts exactly one certificate: the paired computer's.
#[derive(Debug)]
struct Pinned {
    der: Vec<u8>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _server_name: &ServerName<'_>, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, Error> {
        if end_entity.as_ref() == self.der.as_slice() { Ok(ServerCertVerified::assertion()) } else { Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

pub struct Client {
    http: reqwest::blocking::Client,
    base: String,
}

impl Client {
    /// A client for one peer: our identity, its certificate pinned.
    pub fn connect(id: &identity::Identity, peer_cert_pem: &str, host: &str, port: u16) -> Result<Client> {
        let der = identity::pem_to_der(peer_cert_pem)?;
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?
            .dangerous().with_custom_certificate_verifier(Arc::new(Pinned { der, provider }))
            .with_client_auth_cert(vec![CertificateDer::from(id.cert_der.clone())], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key_der.clone())))?;
        let http = reqwest::blocking::Client::builder().no_proxy().use_preconfigured_tls(cfg).connect_timeout(Duration::from_secs(3)).timeout(Duration::from_secs(60)).build()?;
        let host = if host.contains(':') && !host.starts_with('[') { format!("[{host}]") } else { host.to_string() };
        Ok(Client { http, base: format!("https://{host}:{port}") })
    }

    /// A JSON answer; `None` when the peer has nothing under that path.
    pub fn json(&self, path: &str) -> Result<Option<Value>> {
        let r = self.http.get(format!("{}{path}", self.base)).send()?;
        if r.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
        if !r.status().is_success() { bail!("{path} answered {}", r.status()); }
        Ok(Some(serde_json::from_slice(&body_capped(r, journal::MAX_JSON_BYTES, path)?)?))
    }

    /// An answer's bytes, when they are no more than `max`.
    pub fn bytes(&self, path: &str, max: u64) -> Result<Vec<u8>> {
        let r = self.http.get(format!("{}{path}", self.base)).send()?;
        if !r.status().is_success() { bail!("{path} answered {}", r.status()); }
        body_capped(r, max, path)
    }
}

/// `host` or `host:port` (an IPv6 literal in brackets) as the owner or the pairing gave it; without a port, the
/// one learned at pairing.
pub fn address_of(p: &Peer) -> Option<(String, u16)> {
    let a = p.address.trim();
    if a.is_empty() { return None; }
    let split = match a.rsplit_once(':') {
        Some((h, port)) if (!h.contains(':') || h.ends_with(']')) => port.parse::<u16>().ok().map(|port| (h.trim_matches(['[', ']']).to_string(), port)),
        _ => None,
    };
    split.or_else(|| (p.port > 0).then(|| (a.trim_matches(['[', ']']).to_string(), p.port)))
}

/// Where a paired computer may answer, in the order tried: the hub's rendezvous addresses, mDNS, then the address
/// the owner or the pairing gave; each as `(host, port, path)`.
fn candidates(ctx: &Ctx, p: &Peer) -> Vec<(String, u16, &'static str)> {
    let mut v: Vec<(String, u16, &'static str)> = crate::sync::hub::rendezvous_pairs(ctx, &p.id).into_iter().map(|(h, port)| (h, port, "rendezvous")).collect();
    if let Some((ip, port)) = mdns::lookup(&p.id) { v.push((ip.to_string(), port, "mdns")); }
    if let Some((h, port)) = address_of(p) { v.push((h, port, "address")); }
    v
}

/// A paired computer greeted at the first place that answers: the client, its `hello` and how it was reached.
fn reach(ctx: &Ctx, id: &identity::Identity, p: &Peer) -> Option<(Client, Value, &'static str)> {
    let mut why = String::new();
    for (host, port, path) in candidates(ctx, p) {
        match Client::connect(id, &p.cert_pem, &host, port).and_then(|c| c.json("/v1/hello")?.context("no answer").map(|h| (c, h))) {
            Ok((c, h)) => return Some((c, h, path)),
            Err(e) => { why = format!("{host}:{port}: {e:#}"); crate::log::info(&format!("computers: {} at {host}:{port} did not answer: {e:#}", p.name)); }
        }
    }
    // kept on the peer, so the card (and a failing test) can say why
    if !why.is_empty() { let _ = ctx.db.with(|db| { db.execute("update peers set last_error=?1 where device_id=?2", params![why.chars().take(300).collect::<String>(), p.id])?; Ok(()) }); }
    None
}

/// Another computer's transcript from any paired computer that answers and holds it (the origin itself, or one
/// that fetched it): the compressed bytes when their sha256 is the journal entry's. Without an entry (`sha`
/// None) only the origin is asked, for its current copy.
pub fn transcript_from_peers(ctx: &Ctx, origin: &str, harness: &str, profile: &str, session: &str, sha: Option<&str>) -> Option<Vec<u8>> {
    let id = identity::load(ctx).ok()?;
    let peers = journal::peers(ctx).ok()?;
    // the origin first, then the rest
    let mut order: Vec<&Peer> = peers.iter().filter(|p| !p.cert_pem.is_empty()).collect();
    order.sort_by_key(|p| p.id != origin);
    for p in order.into_iter().filter(|p| sha.is_some() || p.id == origin) {
        let Some((c, _, _)) = reach(ctx, &id, p) else { continue };
        match c.bytes(&format!("/v1/transcript/{origin}/{harness}/{profile}/{session}"), crate::transcripts::MAX_FETCH_BYTES) {
            Ok(b) if sha.is_none_or(|s| crate::util::sha256_hex(&b) == s) => { crate::log::info(&format!("transcripts: {harness}/{profile}/{session} came from {}", p.name)); return Some(b); }
            Ok(_) => crate::log::info(&format!("transcripts: {} holds another version of {harness}/{profile}/{session}", p.name)),
            Err(_) => {}
        }
    }
    None
}

/// A connected paired computer (`id`) as a source: what it holds of any origin, and its own presence.
struct Via<'a> {
    c: &'a Client,
    id: &'a str,
}

impl journal::Source for Via<'_> {
    fn manifest(&self, origin: &str, incarnation: &str, since: i64) -> Result<Option<Value>> { self.c.json(&format!("/v1/chunks/{origin}?since={since}&incarnation={incarnation}")) }

    fn read(&self, origin: &str, file: &str) -> Result<Vec<u8>> { self.c.bytes(&format!("/v1/chunk/{origin}/{file}"), journal::MAX_CHUNK_READ) }

    fn presence(&self) -> Result<Vec<(String, Value)>> { Ok(self.c.json("/v1/presence")?.map(|v| (self.id.to_string(), v)).into_iter().collect()) }

    fn live(&self) -> bool { true }

    fn is_origin(&self, origin: &str) -> bool { origin == self.id }
}

/// Every paired computer, now: found by mDNS or its address, greeted, its journal and presence taken, then
/// whatever it holds of computers this one cannot reach (relay). Nothing here fails the caller: each is logged.
pub fn pull_all(ctx: &Ctx) {
    let id = match identity::load(ctx) { Ok(i) => i, Err(e) => { crate::log::warn(&format!("computers: no identity: {e:#}")); return } };
    let peers = journal::peers(ctx).unwrap_or_default();
    let me = ctx.settings.device_id.as_str();
    let mut via: Vec<(&Peer, Client, Value)> = vec![];
    for p in peers.iter().filter(|p| !p.cert_pem.is_empty()) {
        let Some((c, h, path)) = reach(ctx, &id, p) else { continue };
        let _ = ctx.db.with(|db| {
            db.execute("update peers set connected=?1, last_seen=?1, path=?2, relayed_by='', last_error='', name=coalesce(?3, name), incarnation=coalesce(?4, incarnation), last_seq=max(coalesce(last_seq, 0), coalesce(?5, 0)), app_version=coalesce(?6, app_version) where device_id=?7",
                params![now_ms(), path, h["name"].as_str(), h["incarnation"].as_str(), h["last_seq"].as_i64(), h["app_version"].as_str(), p.id])?;
            Ok(())
        });
        via.push((p, c, h));
    }
    for (p, c, _) in &via {
        let src = Via { c, id: &p.id };
        if let Err(e) = journal::pull_noted(ctx, &src, &p.id) { crate::log::warn(&format!("computers: pulling {}: {e:#}", p.name)); }
        if let Err(e) = journal::take_presence(ctx, &src) { crate::log::info(&format!("computers: presence of {}: {e:#}", p.name)); }
    }
    let connected: HashSet<&str> = via.iter().map(|v| v.0.id.as_str()).collect();
    let mut known: BTreeSet<String> = peers.iter().map(|p| p.id.clone()).collect();
    for (_, _, h) in &via { known.extend(h["origins"].as_array().into_iter().flatten().filter_map(|o| o.as_str().filter(|o| store::safe_origin(o)).map(String::from))); }
    for origin in known.iter().filter(|o| o.as_str() != me && !connected.contains(o.as_str())) {
        for (p, c, _) in &via {
            match journal::pull_noted(ctx, &Via { c, id: &p.id }, origin) {
                Ok(true) => {
                    let _ = ctx.db.with(|db| { db.execute("update peers set path='relay', relayed_by=?1 where device_id=?2", params![p.id, origin])?; Ok(()) });
                    break;
                }
                Ok(false) => {}
                Err(e) => crate::log::info(&format!("computers: {origin} through {}: {e:#}", p.name)),
            }
        }
    }
}
