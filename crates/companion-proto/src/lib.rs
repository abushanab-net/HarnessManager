//! The phone's side of the companion: its identity, its side of pairing, the `/c1` types and the sealing of the hub
//! mailbox. The desktop uses the same crate for its side of pairing and in its tests (a fake phone), so both run one
//! derivation of the keys and of the three verification words. The phone's network is here too (`join` for pairing,
//! `client` for the companion API and its events): dart:io cannot verify the computer's Ed25519 certificate, so the
//! app only wraps these. Nothing here touches a file: the app keeps the secrets in the platform's key store.

pub mod api;
pub mod client;
pub mod identity;
pub mod join;
pub mod net;
pub mod pairing;
pub mod push;
pub mod seal;
mod words;

pub use client::{Client, Events};
pub use identity::PhoneIdentity;
pub use pairing::{Keyed, Pairing};

use base64::Engine;

pub(crate) fn b64(bytes: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(bytes) }
pub(crate) fn unb64(s: &str) -> anyhow::Result<Vec<u8>> { Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?) }

pub(crate) fn random<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| anyhow::anyhow!("random bytes: {e}"))?;
    Ok(b)
}
