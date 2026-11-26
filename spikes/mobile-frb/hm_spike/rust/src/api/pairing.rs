//! The phone's half of the desktop pairing (`crates/core/src/sync/peer/pairing.rs` and `identity.rs`): the same
//! crates, the same SPAKE2 roles (the phone joins, so it is A), the same `nonce ++ ciphertext` base64 sealing and
//! the same deterministic Ed25519 self-signed certificate. HTTP stays in Dart; only the crypto crosses the bridge.

use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use ed25519_dalek::SigningKey;
use flutter_rust_bridge::frb;
use rustls::pki_types::PrivatePkcs8KeyDer;
use spake2::{Ed25519Group, Identity, Password, Spake2};

const PKCS8_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
fn unb64(s: &str) -> Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

#[frb(init)]
pub fn init_app() { flutter_rust_bridge::setup_default_user_utils(); }

pub struct PhoneIdentity {
    pub seed: Vec<u8>,
    pub pubkey: String,
    /// PKCS#8 PEM: what dart:io `SecurityContext.usePrivateKeyBytes` takes.
    pub key_pem: String,
    pub cert_pem: String,
    pub cert_der: Vec<u8>,
}

/// The identity from a stored seed, or a new one when `seed` is empty; the certificate as `identity::make_cert`.
#[frb(sync)]
pub fn phone_identity(seed: Vec<u8>, device_id: String) -> Result<PhoneIdentity> {
    let seed: [u8; 32] = if seed.is_empty() {
        let mut s = [0u8; 32];
        getrandom::fill(&mut s).map_err(|e| anyhow!("random bytes: {e}"))?;
        s
    } else { seed.try_into().map_err(|_| anyhow!("a seed is 32 bytes"))? };
    let key = SigningKey::from_bytes(&seed);
    let key_der = [PKCS8_PREFIX.as_slice(), &seed].concat();
    let kp = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&PrivatePkcs8KeyDer::from(key_der.clone()), &rcgen::PKCS_ED25519)?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())?;
    p.distinguished_name.push(rcgen::DnType::CommonName, device_id);
    p.serial_number = Some(rcgen::SerialNumber::from(1u64));
    p.not_before = rcgen::date_time_ymd(2026, 1, 1);
    p.not_after = rcgen::date_time_ymd(2036, 1, 1);
    let cert = p.self_signed(&kp)?;
    Ok(PhoneIdentity {
        seed: seed.to_vec(),
        pubkey: b64(&key.verifying_key().to_bytes()),
        key_pem: format!("-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n", b64(&key_der)),
        cert_pem: cert.pem(),
        cert_der: cert.der().to_vec(),
    })
}

/// A SPAKE2 exchange begun by the phone; `finish` consumes it once.
#[frb(opaque)]
pub struct SpakeA(Mutex<Option<Spake2<Ed25519Group>>>);

impl SpakeA {
    /// Side A: the phone's id and the host's id from `/pair/hello`, the six digits as the password. `msg` is base64.
    #[frb(sync)]
    pub fn start(code: String, me: String, them: String) -> (SpakeA, String) {
        let (st, msg) = Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &Identity::new(me.as_bytes()), &Identity::new(them.as_bytes()));
        (SpakeA(Mutex::new(Some(st))), b64(&msg))
    }

    /// The host's base64 message in, the shared key out.
    #[frb(sync)]
    pub fn finish(&self, inbound: String) -> Result<Vec<u8>> {
        let st = self.0.lock().unwrap().take().ok_or_else(|| anyhow!("already finished"))?;
        st.finish(&unb64(&inbound)?).map_err(|_| anyhow!("the pairing exchange failed"))
    }
}

/// `pairing::seal`: base64 of `nonce ++ ChaCha20-Poly1305(json)`.
#[frb(sync)]
pub fn seal(key: Vec<u8>, json: String) -> Result<String> {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| anyhow!("random bytes: {e}"))?;
    let ct = ChaCha20Poly1305::new_from_slice(&key)?.encrypt(&Nonce::from(nonce), json.as_bytes()).map_err(|_| anyhow!("seal"))?;
    Ok(b64(&[nonce.as_slice(), &ct].concat()))
}

/// `pairing::open`: the JSON text, or an error when the code did not match.
#[frb(sync)]
pub fn open(key: Vec<u8>, sealed: String) -> Result<String> {
    let b = unb64(&sealed)?;
    if b.len() < 12 { bail!("short payload"); }
    let nonce = Nonce::from(<[u8; 12]>::try_from(&b[..12])?);
    let pt = ChaCha20Poly1305::new_from_slice(&key)?.decrypt(&nonce, &b[12..]).map_err(|_| anyhow!("the code did not match"))?;
    Ok(String::from_utf8(pt)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_agree_and_a_wrong_code_fails() {
        let (a, msg_a) = SpakeA::start("123456".into(), "phone".into(), "host".into());
        let (b, msg_b) = Spake2::<Ed25519Group>::start_b(&Password::new(b"123456"), &Identity::new(b"phone"), &Identity::new(b"host"));
        let ka = a.finish(b64(&msg_b)).unwrap();
        let kb = b.finish(&unb64(&msg_a).unwrap()).unwrap();
        assert_eq!(ka, kb);
        assert_eq!(open(kb.clone(), seal(ka, "{\"x\":1}".into()).unwrap()).unwrap(), "{\"x\":1}");
        let (w, msg_w) = SpakeA::start("654321".into(), "phone".into(), "host".into());
        let (b2, msg_b2) = Spake2::<Ed25519Group>::start_b(&Password::new(b"123456"), &Identity::new(b"phone"), &Identity::new(b"host"));
        let kw = w.finish(b64(&msg_b2)).unwrap();
        let kb2 = b2.finish(&unb64(&msg_w).unwrap()).unwrap();
        assert!(open(kb2, seal(kw, "{}".into()).unwrap()).is_err());
        let id = phone_identity(vec![], "phone".into()).unwrap();
        assert!(id.cert_der.windows(32).any(|w| w == unb64(&id.pubkey).unwrap().as_slice()));
    }
}
