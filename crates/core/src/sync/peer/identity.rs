//! This computer's identity for the direct connection: an Ed25519 key pair whose private seed lives in the
//! credential store (never in `settings.json`, which backups carry) and a self-signed certificate at
//! `<data>/device.crt`. The certificate is deterministic (fixed serial and dates, Ed25519 signs deterministically),
//! so losing the file does not break the pairings that pinned it: it is simply made again.

use anyhow::{Context, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;

use crate::{secrets, Ctx};

/// PKCS#8 v1 wrapping of an Ed25519 seed (RFC 8410): a fixed 16-byte prefix before the 32-byte seed.
const PKCS8_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

pub struct Identity {
    pub key: SigningKey,
    /// The private key as PKCS#8 DER, as rustls and rcgen take it.
    pub key_der: Vec<u8>,
    pub cert_pem: String,
    pub cert_der: Vec<u8>,
}

fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
fn unb64(s: &str) -> Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

/// The credential holding the seed: the test prefix keeps tests apart, the id keeps two computers' data folders apart.
pub(crate) fn target(device_id: &str) -> String { format!("{}:device-key:{device_id}", secrets::prefix()) }

/// A PEM certificate's DER bytes.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    unb64(&body).context("certificate pem")
}

fn make_cert(key_der: &[u8], device_id: &str) -> Result<(String, Vec<u8>)> {
    let kp = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&rustls::pki_types::PrivatePkcs8KeyDer::from(key_der.to_vec()), &rcgen::PKCS_ED25519)?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())?;
    p.distinguished_name.push(rcgen::DnType::CommonName, device_id);
    p.serial_number = Some(rcgen::SerialNumber::from(1u64));
    p.not_before = rcgen::date_time_ymd(2026, 1, 1);
    p.not_after = rcgen::date_time_ymd(2036, 1, 1);
    let cert = p.self_signed(&kp)?;
    Ok((cert.pem(), cert.der().to_vec()))
}

/// This computer's identity, made at first use.
pub fn load(ctx: &Ctx) -> Result<Identity> {
    let t = target(&ctx.settings.device_id);
    let seed: [u8; 32] = match secrets::read(&t).and_then(|(_, b)| hex::decode(String::from_utf8_lossy(&b).trim()).ok()).and_then(|v| v.try_into().ok()) {
        Some(s) => s,
        None => {
            let mut s = [0u8; 32];
            getrandom::fill(&mut s).map_err(|e| anyhow::anyhow!("random bytes: {e}"))?;
            secrets::write(&t, "hrm", hex::encode(s).as_bytes()).context("save the device key")?;
            crate::log::info("computers: a device key was made for the direct connection");
            s
        }
    };
    let key = SigningKey::from_bytes(&seed);
    let key_der = [PKCS8_PREFIX.as_slice(), &seed].concat();
    let path = ctx.data.join("device.crt");
    let pubkey = key.verifying_key().to_bytes();
    // the file is used when it carries this key; otherwise (missing, another computer's backup) it is made again
    let cached = std::fs::read_to_string(&path).ok().and_then(|pem| pem_to_der(&pem).ok().filter(|der| der.windows(32).any(|w| w == pubkey)).map(|der| (pem, der)));
    let (cert_pem, cert_der) = match cached {
        Some(c) => c,
        None => {
            let c = make_cert(&key_der, &ctx.settings.device_id)?;
            crate::util::write_atomic(&path, c.0.as_bytes())?;
            c
        }
    };
    ctx.db.kv_set("device:pubkey", &b64(&pubkey))?;
    Ok(Identity { key, key_der, cert_pem, cert_der })
}

/// This computer's public key, base64, or "" before the identity exists.
pub fn pubkey(ctx: &Ctx) -> String { ctx.db.kv_get("device:pubkey").ok().flatten().unwrap_or_default() }

impl Identity {
    pub fn pubkey(&self) -> String { b64(&self.key.verifying_key().to_bytes()) }
    pub fn sign(&self, msg: &[u8]) -> String { b64(&self.key.sign(msg).to_bytes()) }
    /// `sig` over the record's compact JSON without `sig`; `pubkey` is in it, so a reader can check it stands alone.
    pub fn sign_record(&self, v: &mut Value) {
        v["pubkey"] = Value::String(self.pubkey());
        v.as_object_mut().map(|o| o.remove("sig"));
        let sig = self.sign(v.to_string().as_bytes());
        v["sig"] = Value::String(sig);
    }
}

/// What an origin signs for a chunk.
pub fn chunk_message(incarnation: &str, from: i64, to: i64, sha256: &str) -> String { format!("{incarnation}-{from}-{to}:{sha256}") }

pub fn verify(pubkey_b64: &str, msg: &[u8], sig_b64: &str) -> bool {
    let Ok(pk) = unb64(pubkey_b64).ok().and_then(|b| <[u8; 32]>::try_from(b).ok()).map(|b| VerifyingKey::from_bytes(&b)).unwrap_or(Err(ed25519_dalek::SignatureError::new())) else { return false };
    let Some(sig) = unb64(sig_b64).ok().and_then(|b| Signature::from_slice(&b).ok()) else { return false };
    pk.verify(msg, &sig).is_ok()
}

/// A device record that carries its own `pubkey` and a `sig` by it.
pub fn record_ok(v: &Value) -> bool {
    let (Some(pk), Some(sig)) = (v["pubkey"].as_str(), v["sig"].as_str()) else { return false };
    let mut c = v.clone();
    c.as_object_mut().map(|o| o.remove("sig"));
    verify(pk, c.to_string().as_bytes(), sig)
}
