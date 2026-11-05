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

use super::{identity, mdns};
use crate::journal::{self, Peer};
use crate::util::now_ms;
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
        Ok(Some(r.json()?))
    }

    pub fn bytes(&self, path: &str) -> Result<Vec<u8>> {
        let r = self.http.get(format!("{}{path}", self.base)).send()?;
        if !r.status().is_success() { bail!("{path} answered {}", r.status()); }
        Ok(r.bytes()?.to_vec())
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

/// One origin's chunks from a connected peer: since the cursor, the device record checked and learned, every
/// chunk verified and applied. `Ok(false)` when the peer holds nothing of that origin.
fn pull_origin(ctx: &Ctx, c: &Client, origin: &str) -> Result<bool> {
    let known: String = ctx.db.with(|c| Ok(c.query_row("select coalesce(incarnation, '') from peers where device_id=?", [origin], |r| r.get(0)).unwrap_or_default()))?;
    let cursor = |inc: &str| -> i64 { ctx.db.with(|c| Ok(c.query_row("select seq from cursors2 where device_id=? and incarnation=?", [origin, inc], |r| r.get(0)).unwrap_or(0))).unwrap_or(0) };
    let Some(mut r) = c.json(&format!("/v1/chunks/{origin}?since={}&incarnation={known}", cursor(&known)))? else { return Ok(false) };
    let inc = r["incarnation"].as_str().unwrap_or("").to_string();
    if inc != known {
        r = c.json(&format!("/v1/chunks/{origin}?since={}&incarnation={inc}", cursor(&inc)))?.context("the origin vanished")?;
    }
    if r["device"].is_object() { journal::learn_device(ctx, origin, &r["device"])?; }
    if journal::pubkey_of(ctx, origin)?.is_empty() { bail!("no key is known for {origin} yet; nothing of it is taken"); }
    let applied = journal::apply_manifest(ctx, origin, &r, &|file| c.bytes(&format!("/v1/chunk/{origin}/{file}")))?;
    ctx.db.with(|c| {
        c.execute("update peers set incarnation=?1, last_seq=max(coalesce(last_seq, 0), ?2), applied_seq=?3, last_pull=?4 where device_id=?5",
            params![inc, r["last_seq"].as_i64().unwrap_or(0), applied, now_ms(), origin])?;
        Ok(())
    })?;
    Ok(true)
}

/// Every paired computer, now: found by mDNS or its address, greeted, its journal and presence taken, then
/// whatever it holds of computers this one cannot reach (relay). Nothing here fails the caller: each is logged.
pub fn pull_all(ctx: &Ctx) {
    let id = match identity::load(ctx) { Ok(i) => i, Err(e) => { crate::log::warn(&format!("computers: no identity: {e:#}")); return } };
    let peers = journal::peers(ctx).unwrap_or_default();
    let me = ctx.settings.device_id.as_str();
    let mut via: Vec<(&Peer, Client, Value)> = vec![];
    for p in peers.iter().filter(|p| !p.cert_pem.is_empty()) {
        let found = mdns::lookup(&p.id).map(|(ip, port)| (ip.to_string(), port, "mdns")).or_else(|| address_of(p).map(|(h, port)| (h, port, "address")));
        let Some((host, port, path)) = found else { continue };
        match Client::connect(&id, &p.cert_pem, &host, port).and_then(|c| c.json("/v1/hello")?.context("no answer").map(|h| (c, h))) {
            Ok((c, h)) => {
                let _ = ctx.db.with(|db| {
                    db.execute("update peers set connected=?1, last_seen=?1, path=?2, relayed_by='', last_error='', name=coalesce(?3, name), incarnation=coalesce(?4, incarnation), last_seq=max(coalesce(last_seq, 0), coalesce(?5, 0)), app_version=coalesce(?6, app_version) where device_id=?7",
                        params![now_ms(), path, h["name"].as_str(), h["incarnation"].as_str(), h["last_seq"].as_i64(), h["app_version"].as_str(), p.id])?;
                    Ok(())
                });
                via.push((p, c, h));
            }
            Err(e) => {
                // kept on the peer, so the card (and a failing test) can say why
                let why = format!("{host}:{port}: {e:#}");
                let _ = ctx.db.with(|db| { db.execute("update peers set last_error=?1 where device_id=?2", params![why.chars().take(300).collect::<String>(), p.id])?; Ok(()) });
                crate::log::info(&format!("computers: {} at {host}:{port} did not answer: {e:#}", p.name));
            }
        }
    }
    for (p, c, _) in &via {
        if let Err(e) = pull_origin(ctx, c, &p.id) { crate::log::warn(&format!("computers: pulling {}: {e:#}", p.name)); }
        match c.json("/v1/presence") {
            Ok(Some(v)) => { let _ = ctx.db.with(|db| { db.execute("update peers set presence=?1, last_seen=max(coalesce(last_seen, 0), coalesce(json_extract(?1, '$.at'), 0)) where device_id=?2", params![v.to_string(), p.id])?; Ok(()) }); }
            Ok(None) => {}
            Err(e) => crate::log::info(&format!("computers: presence of {}: {e:#}", p.name)),
        }
    }
    let connected: HashSet<&str> = via.iter().map(|v| v.0.id.as_str()).collect();
    let mut known: BTreeSet<String> = peers.iter().map(|p| p.id.clone()).collect();
    for (_, _, h) in &via { known.extend(h["origins"].as_array().into_iter().flatten().filter_map(|o| o.as_str().map(String::from))); }
    for origin in known.iter().filter(|o| o.as_str() != me && !connected.contains(o.as_str())) {
        for (p, c, _) in &via {
            match pull_origin(ctx, c, origin) {
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
