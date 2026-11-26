//! Pairing a phone with a computer showing a code. As between two computers: SPAKE2 with the six digits as the
//! password and the two ids as the identities (the phone is A), then payloads sealed under the key both derived, the
//! phone's first. A phone that typed a wrong code cannot open what comes back, and the computer counts the failure
//! when it cannot open the phone's. Both sides show three words derived from the key, so the owner can see that the
//! phone on the screen is the one in their hand; the computer seals nothing back until the owner approves.

use anyhow::{anyhow, bail, Context, Result};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::api::Grants;
use crate::{b64, random, unb64};

/// The phone's side, between its first message and the computer's answer.
pub struct Pairing {
    st: Spake2<Ed25519Group>,
}

/// The key both sides hold once the exchange is done.
pub struct Keyed {
    key: Vec<u8>,
}

fn check_code(code: &str) -> Result<&str> {
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) { bail!("the code is six digits"); }
    Ok(code)
}

impl Pairing {
    /// The phone (A) starts: its message, base64, goes to `POST /pair/phone/start` with its id.
    pub fn start(code: &str, phone_id: &str, desktop_id: &str) -> Result<(Pairing, String)> {
        let (st, msg) = Spake2::<Ed25519Group>::start_a(&Password::new(check_code(code)?.as_bytes()), &Identity::new(phone_id.as_bytes()), &Identity::new(desktop_id.as_bytes()));
        Ok((Pairing { st }, b64(&msg)))
    }

    /// The computer's answer in: the key. A wrong code still gives a key here; it shows when nothing opens.
    pub fn finish(self, their_msg: &str) -> Result<Keyed> {
        let key = self.st.finish(&unb64(their_msg)?).map_err(|_| anyhow!("the pairing exchange failed"))?;
        Ok(Keyed { key })
    }
}

/// The computer's side (B): the phone's message in, the key and the computer's message (base64) out.
pub fn answer(code: &str, phone_id: &str, desktop_id: &str, their_msg: &str) -> Result<(Keyed, String)> {
    let (st, msg) = Spake2::<Ed25519Group>::start_b(&Password::new(check_code(code)?.as_bytes()), &Identity::new(phone_id.as_bytes()), &Identity::new(desktop_id.as_bytes()));
    let key = st.finish(&unb64(their_msg)?).map_err(|_| anyhow!("the pairing exchange failed"))?;
    Ok((Keyed { key }, b64(&msg)))
}

impl Keyed {
    /// `nonce ++ ciphertext`, base64, of the value's JSON.
    pub fn seal<T: Serialize>(&self, v: &T) -> Result<String> {
        let nonce = random::<12>()?;
        let ct = ChaCha20Poly1305::new_from_slice(&self.key)?.encrypt(&Nonce::from(nonce), serde_json::to_vec(v)?.as_slice()).map_err(|_| anyhow!("seal"))?;
        Ok(b64(&[nonce.as_slice(), &ct].concat()))
    }

    /// What the other side sealed; fails as "the code did not match" when the keys differ.
    pub fn open<T: for<'de> Deserialize<'de>>(&self, s: &str) -> Result<T> {
        let b = unb64(s)?;
        if b.len() < 12 { bail!("short payload"); }
        let nonce = Nonce::from(<[u8; 12]>::try_from(&b[..12])?);
        let pt = ChaCha20Poly1305::new_from_slice(&self.key)?.decrypt(&nonce, &b[12..]).map_err(|_| anyhow!("the code did not match"))?;
        serde_json::from_slice(&pt).context("the payload")
    }

    /// The three words both sides show: the same key gives the same words.
    pub fn words(&self) -> [&'static str; 3] {
        let h = Sha256::new().chain_update(b"harness-manager phone words v1").chain_update(&self.key).finalize();
        [crate::words::WORDS[h[0] as usize], crate::words::WORDS[h[1] as usize], crate::words::WORDS[h[2] as usize]]
    }
}

/// A certificate's fingerprint as the QR code carries it: SHA-256 of its DER, hex.
pub fn fingerprint(cert_der: &[u8]) -> String { hex::encode(Sha256::digest(cert_der)) }

/// What the QR code says: `hm-phone:1?h=<host:port>&c=<code>&f=<fingerprint>`.
pub fn qr(host_port: &str, code: &str, fingerprint: &str) -> String { format!("hm-phone:1?h={host_port}&c={code}&f={fingerprint}") }

/// A QR code read back: `(host:port, code, fingerprint)`.
pub fn parse_qr(s: &str) -> Result<(String, String, String)> {
    let q = s.trim().strip_prefix("hm-phone:1?").context("not a Harness Manager pairing code")?;
    let get = |k: &str| q.split('&').find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(String::from).with_context(|| format!("the pairing code has no {k}"));
    let (h, c, f) = (get("h")?, get("c")?, get("f")?);
    check_code(&c)?;
    Ok((h, c, f))
}

/// What the phone seals to the computer in `POST /pair/phone/finish`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhonePayload {
    pub id: String,
    pub name: String,
    /// "ios" or "android".
    pub platform: String,
    pub cert_pem: String,
    /// Ed25519, base64.
    pub pubkey: String,
    /// X25519, base64.
    pub x25519: String,
}

impl PhonePayload {
    pub fn of(id: &crate::PhoneIdentity, name: &str, platform: &str) -> Result<PhonePayload> {
        Ok(PhonePayload { id: id.id.clone(), name: name.into(), platform: platform.into(), cert_pem: id.cert()?.0, pubkey: id.pubkey(), x25519: id.x25519_public() })
    }
}

/// What the computer seals back once the owner approved (`GET /pair/phone/result`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesktopPayload {
    pub id: String,
    pub name: String,
    pub cert_pem: String,
    pub pubkey: String,
    /// Where the companion API answers: `host:port`, the pairing's host first.
    pub address_hints: Vec<String>,
    pub hub_url: Option<String>,
    pub hub_token: Option<String>,
    pub grants: Grants,
}

/// `GET /pair/phone/result`: `pending`, `approved` with the sealed `DesktopPayload`, or `refused`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairResult {
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Milliseconds until the code expires, for the "Waiting for approval" screen.
    #[serde(default)]
    pub expires_in: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn both_sides_derive_one_key_and_the_same_words() {
        let (p, msg) = Pairing::start("123456", "phone-1", "desk-1").unwrap();
        let (desk, back) = answer("123456", "phone-1", "desk-1", &msg).unwrap();
        let phone = p.finish(&back).unwrap();
        assert_eq!(phone.words(), desk.words());
        let sealed = phone.seal(&serde_json::json!({"a": 1})).unwrap();
        assert_eq!(desk.open::<Value>(&sealed).unwrap()["a"], 1);
    }

    #[test]
    fn a_wrong_code_opens_nothing_and_shows_other_words() {
        let (p, msg) = Pairing::start("123456", "phone-1", "desk-1").unwrap();
        let (desk, back) = answer("654321", "phone-1", "desk-1", &msg).unwrap();
        let phone = p.finish(&back).unwrap();
        let e = desk.open::<Value>(&phone.seal(&1).unwrap()).unwrap_err();
        assert!(format!("{e:#}").contains("did not match"));
        assert_ne!(phone.words(), desk.words());
        assert!(Pairing::start("12345", "p", "d").is_err());
    }

    #[test]
    fn the_qr_code_reads_back() {
        let s = qr("pc.local:4242", "012345", "ab12");
        assert_eq!(s, "hm-phone:1?h=pc.local:4242&c=012345&f=ab12");
        assert_eq!(parse_qr(&s).unwrap(), ("pc.local:4242".into(), "012345".into(), "ab12".into()));
        assert!(parse_qr("hm-phone:1?h=x&c=1&f=y").is_err());
        assert!(parse_qr("https://example.com").is_err());
    }
}
