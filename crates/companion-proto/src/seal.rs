//! Sealing to one recipient, for the hub's phone mailbox: the hub carries what only the recipient can open and only
//! the sender can have written. A fresh X25519 key agrees a secret with the recipient's X25519 key; ChaCha20-Poly1305
//! encrypts under a key hashed from it; the sender's Ed25519 key signs everything the recipient reads.

use anyhow::{anyhow, bail, Context, Result};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use curve25519_dalek::montgomery::MontgomeryPoint;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{b64, random, unb64};

/// What travels: all base64.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sealed {
    pub v: u32,
    /// The sender's Ed25519 public key.
    pub from: String,
    /// The fresh X25519 public key.
    pub e: String,
    pub nonce: String,
    pub ct: String,
    /// The sender's signature over `signed()`.
    pub sig: String,
}

fn x25519_key(b64_key: &str) -> Result<[u8; 32]> { <[u8; 32]>::try_from(unb64(b64_key)?).map_err(|_| anyhow!("an X25519 key is 32 bytes")) }

/// What the signature covers: the version, the keys of both ends, the nonce and the ciphertext.
fn signed(v: u32, from: &[u8], e: &[u8], to: &[u8], nonce: &[u8], ct: &[u8]) -> Vec<u8> {
    [b"hm-seal".as_slice(), &v.to_be_bytes(), from, e, to, nonce, ct].concat()
}

fn cipher(shared: &MontgomeryPoint, e: &[u8], to: &[u8]) -> Result<ChaCha20Poly1305> {
    // a low-order key from a hostile sender gives an all-zero secret: refused, as RFC 7748 advises
    if shared.to_bytes() == [0u8; 32] { bail!("not a usable key"); }
    let k = Sha256::new().chain_update(b"hm-seal v1").chain_update(shared.to_bytes()).chain_update(e).chain_update(to).finalize();
    Ok(ChaCha20Poly1305::new_from_slice(&k)?)
}

/// The X25519 public key (base64) of a secret: what a sender seals to.
pub fn x25519_public(secret: &[u8; 32]) -> String { b64(&MontgomeryPoint::mul_base_clamped(*secret).to_bytes()) }

/// `plaintext` sealed to the holder of X25519 key `to` (base64), signed by `sender`.
pub fn seal_to(sender: &SigningKey, to: &str, plaintext: &[u8]) -> Result<Sealed> {
    let to = x25519_key(to)?;
    let secret = random::<32>()?;
    let e = MontgomeryPoint::mul_base_clamped(secret).to_bytes();
    let nonce = random::<12>()?;
    let ct = cipher(&MontgomeryPoint(to).mul_clamped(secret), &e, &to)?.encrypt(&Nonce::from(nonce), Payload { msg: plaintext, aad: &e }).map_err(|_| anyhow!("seal"))?;
    let from = sender.verifying_key().to_bytes();
    let sig = sender.sign(&signed(1, &from, &e, &to, &nonce, &ct));
    Ok(Sealed { v: 1, from: b64(&from), e: b64(&e), nonce: b64(&nonce), ct: b64(&ct), sig: b64(&sig.to_bytes()) })
}

/// Opened with our X25519 secret: the plaintext, when `from` signed it (the key the pairing stored, base64).
pub fn open_from(secret: &[u8; 32], from: &str, s: &Sealed) -> Result<Vec<u8>> {
    if s.v != 1 { bail!("sealed with version {}, which this version cannot open", s.v); }
    if s.from != from { bail!("sealed by someone else"); }
    let from_b = <[u8; 32]>::try_from(unb64(from)?).map_err(|_| anyhow!("an Ed25519 key is 32 bytes"))?;
    let pk = VerifyingKey::from_bytes(&from_b).context("the sender's key")?;
    let (e, nonce, ct) = (x25519_key(&s.e)?, unb64(&s.nonce)?, unb64(&s.ct)?);
    let me = MontgomeryPoint::mul_base_clamped(*secret).to_bytes();
    let sig = Signature::from_slice(&unb64(&s.sig)?).context("the signature")?;
    pk.verify(&signed(1, &from_b, &e, &me, &nonce, &ct), &sig).map_err(|_| anyhow!("the signature does not match"))?;
    let nonce = Nonce::from(<[u8; 12]>::try_from(nonce.as_slice()).map_err(|_| anyhow!("a nonce is 12 bytes"))?);
    cipher(&MontgomeryPoint(e).mul_clamped(*secret), &e, &me)?.decrypt(&nonce, Payload { msg: &ct, aad: &e }).map_err(|_| anyhow!("not sealed to this key"))
}

impl crate::PhoneIdentity {
    /// Sealed by this phone to the computer's X25519 key.
    pub fn seal_to(&self, to: &str, plaintext: &[u8]) -> Result<Sealed> { seal_to(self.signing_key(), to, plaintext) }

    /// Opened by this phone: sealed to it and signed by `from`.
    pub fn open_from(&self, from: &str, s: &Sealed) -> Result<Vec<u8>> { open_from(&self.x25519_secret(), from, s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PhoneIdentity;

    #[test]
    fn only_the_recipient_opens_and_only_from_the_sender() {
        let (a, b, c) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
        let s = a.seal_to(&b.x25519_public(), b"hello").unwrap();
        assert_eq!(b.open_from(&a.pubkey(), &s).unwrap(), b"hello");
        assert!(c.open_from(&a.pubkey(), &s).is_err(), "another recipient");
        assert!(b.open_from(&c.pubkey(), &s).is_err(), "another sender claimed");
        let mut t = s.clone();
        let mut ct = unb64(&t.ct).unwrap();
        ct[0] ^= 1;
        t.ct = b64(&ct);
        assert!(b.open_from(&a.pubkey(), &t).is_err(), "changed on the way");
        // a sender's key swapped in with a signature of its own still has to be the one expected
        let forged = c.seal_to(&b.x25519_public(), b"evil").unwrap();
        assert!(b.open_from(&a.pubkey(), &Sealed { from: a.pubkey(), ..forged }).is_err());
    }

    #[test]
    fn a_low_order_key_is_refused() {
        let a = PhoneIdentity::generate().unwrap();
        assert!(a.seal_to(&b64(&[0u8; 32]), b"x").is_err());
    }
}
