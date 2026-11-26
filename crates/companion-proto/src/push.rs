//! Signed relay requests. The body is the exact byte sequence sent over HTTP.
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const TIMESTAMP: &str = "x-hrm-timestamp";
pub const SIGNATURE: &str = "x-hrm-signature";
pub const WAKE_PATH: &str = "/v1/wake";

#[derive(Serialize, Deserialize)]
pub struct Registration {
    pub relay_handle: String,
    pub secret: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Needs,
    Live,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wake {
    pub relay_handle: String,
    pub kind: Kind,
    pub collapse_id: String,
}

pub fn request_mac(key: &[u8; 32], method: &str, path: &str, timestamp: u64, body: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::new_from_slice(key).expect("HMAC accepts a 32-byte key");
    mac.update(format!("{method}\n{path}\n{timestamp}\n").as_bytes());
    mac.update(body);
    mac
}

pub fn signature(key: &[u8; 32], method: &str, path: &str, timestamp: u64, body: &[u8]) -> String {
    hex::encode(request_mac(key, method, path, timestamp, body).finalize().into_bytes())
}
