//! The companion API client in Rust: rustls with the phone's Ed25519 certificate shown and the computer's pinned by
//! exact DER, as `crates/core/src/sync/peer/client.rs` does for computers. dart:io cannot do this handshake: its
//! BoringSSL never offers `ed25519` in signature_algorithms, so a server holding only an Ed25519 certificate
//! answers alert 40 (handshake_failure). See spikes/mobile-tls/README.md.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use flutter_rust_bridge::frb;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};

const PKCS8_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

/// Accepts exactly one certificate: the paired computer's.
#[derive(Debug)]
struct Pinned {
    der: Vec<u8>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _i: &[CertificateDer<'_>], _n: &ServerName<'_>, _o: &[u8], _now: UnixTime) -> Result<ServerCertVerified, Error> {
        if end_entity.as_ref() == self.der.as_slice() { Ok(ServerCertVerified::assertion()) } else { Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    base64::engine::general_purpose::STANDARD.decode(body.trim()).context("certificate pem")
}

pub struct C1Answer {
    pub status: u16,
    pub body: String,
    pub ms: u32,
}

/// One computer's companion API, kept open: the connection is reused between calls.
#[frb(opaque)]
pub struct C1 {
    http: reqwest::blocking::Client,
    base: String,
}

impl C1 {
    /// `seed` and `cert_der` are the phone's identity; `pin_pem` the computer's certificate from pairing.
    #[frb(sync)]
    pub fn connect(seed: Vec<u8>, cert_der: Vec<u8>, pin_pem: String, base: String) -> Result<C1> {
        let seed: [u8; 32] = seed.try_into().map_err(|_| anyhow!("a seed is 32 bytes"))?;
        let key_der = [PKCS8_PREFIX.as_slice(), &seed].concat();
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?
            .dangerous().with_custom_certificate_verifier(Arc::new(Pinned { der: pem_to_der(&pin_pem)?, provider }))
            .with_client_auth_cert(vec![CertificateDer::from(cert_der)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der)))?;
        let http = reqwest::blocking::Client::builder().no_proxy().use_preconfigured_tls(cfg)
            .connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(20)).build()?;
        Ok(C1 { http, base: base.trim_end_matches('/').to_string() })
    }

    pub fn get(&self, path: String) -> Result<C1Answer> { self.send(self.http.get(format!("{}{path}", self.base))) }

    pub fn post(&self, path: String, json: String) -> Result<C1Answer> {
        self.send(self.http.post(format!("{}{path}", self.base)).header("content-type", "application/json").body(json))
    }

    fn send(&self, r: reqwest::blocking::RequestBuilder) -> Result<C1Answer> {
        let t = std::time::Instant::now();
        let r = r.send().map_err(|e| anyhow!("{e:?}"))?;
        let status = r.status().as_u16();
        let body = r.text()?;
        Ok(C1Answer { status, body, ms: t.elapsed().as_millis() as u32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs `mobile-spike-host serve` running and `client-cert ../mobile-tls/dart_cli/certs` made: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn against_the_spike_host() {
        let d = "../../../mobile-tls/dart_cli/certs";
        let rd = |f: &str| std::fs::read_to_string(format!("{d}/{f}")).unwrap();
        let seed = pem_to_der(&rd("client.key")).unwrap()[16..].to_vec();
        let host = std::fs::read(format!("{d}/host.der")).unwrap();
        let pin = format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n", base64::engine::general_purpose::STANDARD.encode(&host));
        let c = C1::connect(seed.clone(), pem_to_der(&rd("client.crt")).unwrap(), pin.clone(), "https://127.0.0.1:47802".into()).unwrap();
        let a = c.get("/c1/hello".into()).unwrap();
        assert_eq!(a.status, 200, "{}", a.body);
        let b = c.get("/c1/hello".into()).unwrap();
        println!("first {} ms, reused {} ms: {}", a.ms, b.ms, b.body);
        // a stranger's certificate is refused by the host; a wrong pin by the phone
        let s = C1::connect(pem_to_der(&rd("stranger.key")).unwrap()[16..].to_vec(), pem_to_der(&rd("stranger.crt")).unwrap(), pin, "https://127.0.0.1:47802".into()).unwrap();
        println!("stranger: {}", s.get("/c1/hello".into()).err().map(|e| e.to_string()).unwrap_or_default().chars().take(160).collect::<String>());
        let mut wrong = host.clone();
        *wrong.last_mut().unwrap() ^= 1;
        let wpin = format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n", base64::engine::general_purpose::STANDARD.encode(&wrong));
        let w = C1::connect(seed, pem_to_der(&rd("client.crt")).unwrap(), wpin, "https://127.0.0.1:47802".into()).unwrap();
        let e = w.get("/c1/hello".into()).err().expect("wrong pin refused").to_string();
        println!("wrong pin: {}", e.chars().take(160).collect::<String>());
        assert!(e.contains("ApplicationVerificationFailure"));
    }
}
