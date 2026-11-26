//! A phone's identity: an Ed25519 key that signs its TLS certificate and what it seals, and an X25519 key that the
//! computer seals the mailbox to. The certificate is made as the desktop makes its own (`core/sync/peer/identity.rs`):
//! self-signed, the id as its common name, a fixed serial and fixed dates, so the same key always gives the same
//! certificate and losing it breaks no pairing.

use anyhow::{bail, Result};
use curve25519_dalek::montgomery::MontgomeryPoint;
use ed25519_dalek::{Signer, SigningKey};

use crate::{b64, random};

/// PKCS#8 v1 wrapping of an Ed25519 seed (RFC 8410): a fixed 16-byte prefix before the 32-byte seed.
const PKCS8_PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];

#[derive(Clone)]
pub struct PhoneIdentity {
    /// The phone's id, as the computer stores it (`phones.phone_id`).
    pub id: String,
    key: SigningKey,
    x25519: [u8; 32],
}

/// The certificate's signer: rcgen without a crypto backend of its own, signing with ed25519-dalek.
pub(crate) struct Ed(pub(crate) SigningKey);

impl rcgen::PublicKeyData for Ed {
    fn der_bytes(&self) -> &[u8] { self.0.as_ref().as_bytes() }
    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm { &rcgen::PKCS_ED25519 }
}

impl rcgen::SigningKey for Ed {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> { Ok(self.0.sign(msg).to_bytes().to_vec()) }
}

/// A PEM certificate's DER bytes.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    crate::unb64(&body).map_err(|_| anyhow::anyhow!("not a PEM certificate"))
}

impl PhoneIdentity {
    /// A new phone: a random id and both keys.
    pub fn generate() -> Result<PhoneIdentity> {
        let id = random::<16>()?;
        PhoneIdentity::from_secrets(&format!("phone-{}", hex::encode(id)), &random::<32>()?, &random::<32>()?)
    }

    /// The identity kept in the platform's key store: the id, the Ed25519 seed and the X25519 secret.
    pub fn from_secrets(id: &str, seed: &[u8], x25519: &[u8]) -> Result<PhoneIdentity> {
        if id.is_empty() || id.len() > 80 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { bail!("a phone id is up to 80 letters, digits, - or _"); }
        let (Ok(seed), Ok(x25519)) = (<[u8; 32]>::try_from(seed), <[u8; 32]>::try_from(x25519)) else { bail!("a phone key is 32 bytes") };
        Ok(PhoneIdentity { id: id.to_string(), key: SigningKey::from_bytes(&seed), x25519 })
    }

    /// What to keep: the Ed25519 seed and the X25519 secret.
    pub fn secrets(&self) -> ([u8; 32], [u8; 32]) { (self.key.to_bytes(), self.x25519) }

    pub fn signing_key(&self) -> &SigningKey { &self.key }

    /// The Ed25519 public key, base64.
    pub fn pubkey(&self) -> String { b64(&self.key.verifying_key().to_bytes()) }

    /// The X25519 public key, base64: what the computer seals to.
    pub fn x25519_public(&self) -> String { b64(&MontgomeryPoint::mul_base_clamped(self.x25519).to_bytes()) }

    pub(crate) fn x25519_secret(&self) -> [u8; 32] { self.x25519 }

    /// The private key as PKCS#8 DER, as TLS libraries take it.
    pub fn key_der(&self) -> Vec<u8> { [PKCS8_PREFIX.as_slice(), &self.key.to_bytes()].concat() }

    /// The self-signed certificate, PEM and DER.
    pub fn cert(&self) -> Result<(String, Vec<u8>)> {
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())?;
        p.distinguished_name.push(rcgen::DnType::CommonName, self.id.as_str());
        p.serial_number = Some(rcgen::SerialNumber::from(1u64));
        p.not_before = rcgen::date_time_ymd(2026, 1, 1);
        p.not_after = rcgen::date_time_ymd(2036, 1, 1);
        let cert = p.self_signed(&Ed(self.key.clone()))?;
        Ok((cert.pem(), cert.der().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_secrets_give_the_same_certificate_and_keys() {
        let a = PhoneIdentity::generate().unwrap();
        let (seed, x) = a.secrets();
        let b = PhoneIdentity::from_secrets(&a.id, &seed, &x).unwrap();
        assert_eq!(a.cert().unwrap().1, b.cert().unwrap().1);
        assert_eq!((a.pubkey(), a.x25519_public()), (b.pubkey(), b.x25519_public()));
        let (pem, der) = a.cert().unwrap();
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        // the public key is in the certificate, as the desktop's check of its own certificate file expects
        assert!(der.windows(32).any(|w| w == a.signing_key().verifying_key().to_bytes()));
        assert_eq!(a.key_der().len(), 48);
    }

    #[test]
    fn a_bad_id_or_key_is_refused() {
        assert!(PhoneIdentity::from_secrets("a/b", &[1; 32], &[2; 32]).is_err());
        assert!(PhoneIdentity::from_secrets("ok", &[1; 31], &[2; 32]).is_err());
    }
}
