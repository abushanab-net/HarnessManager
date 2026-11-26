//! Pairing from the phone, over the network: the computer's plain-HTTP pairing port (`/pair/hello`,
//! `/pair/phone/start`, `/pair/phone/finish`, then `/pair/phone/result` until the owner decides). Everything that
//! matters is sealed under the SPAKE2 key, and the computer's certificate is checked against the fingerprint the QR
//! code carries, so the plain HTTP carries nothing to trust.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::Refused;
use crate::pairing::{self, DesktopPayload, PairResult, PhonePayload};
use crate::{net, Keyed, Pairing, PhoneIdentity};

/// The phone's payload is in; the owner sees the three words and decides on the computer.
pub struct Joining {
    host: String,
    port: u16,
    phone_id: String,
    fingerprint: Option<String>,
    key: Keyed,
    /// The computer's id and name, as `/pair/hello` gave them.
    pub desktop_id: String,
    pub desktop_name: String,
}

/// What the owner decided.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Still deciding: ms left before the code expires.
    Pending(i64),
    /// Paired: the computer's payload (its certificate, addresses and the grants).
    Approved(DesktopPayload),
    Refused,
    Expired,
}

/// One plain-HTTP exchange on its own connection.
async fn exchange(host: &str, port: u16, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let mut s = net::handshake(net::tcp(host, port).await?).await?;
    let a = net::send(&mut s, net::request(method, host, path, body.map(|b| b.to_string().into_bytes()))?).await?;
    net::json(a)
}

#[derive(Deserialize)]
struct Hello {
    id: String,
    name: String,
}

/// Pair with the computer whose QR code says `qr` (`hm-phone:1?h=…&c=…&f=…`): this phone's payload is sent and
/// the owner is asked. `name` and `platform` ("ios" or "android") are what the owner sees.
pub async fn join_qr(qr: &str, me: &PhoneIdentity, name: &str, platform: &str) -> Result<Joining> {
    let (h, code, f) = pairing::parse_qr(qr)?;
    join(&h, &code, Some(&f), me, name, platform).await
}

/// The same with the code typed and `host:port` given; without a fingerprint the computer's certificate is the one
/// the sealed answer names, which only the holder of the code could seal.
pub async fn join(address: &str, code: &str, fingerprint: Option<&str>, me: &PhoneIdentity, name: &str, platform: &str) -> Result<Joining> {
    let (host, port) = net::split(address)?;
    let hello: Hello = serde_json::from_value(exchange(&host, port, "GET", "/pair/hello", None).await.with_context(|| format!("no computer shows a code at {address}"))?)?;
    let (p, msg) = Pairing::start(code, &me.id, &hello.id)?;
    let r = exchange(&host, port, "POST", "/pair/phone/start", Some(json!({"id": me.id, "msg": msg}))).await.context("the computer refused the pairing")?;
    let key = p.finish(r["msg"].as_str().unwrap_or(""))?;
    let mine = key.seal(&PhonePayload::of(me, name, platform)?)?;
    match exchange(&host, port, "POST", "/pair/phone/finish", Some(json!({"id": me.id, "payload": mine}))).await {
        Ok(_) => {}
        Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 403) => bail!("the code did not match; check it and try again"),
        Err(e) => return Err(e.context("the computer did not take this phone")),
    }
    Ok(Joining { host, port, phone_id: me.id.clone(), fingerprint: fingerprint.map(String::from), key, desktop_id: hello.id, desktop_name: hello.name })
}

impl Joining {
    /// The three words to compare with the computer's screen.
    pub fn words(&self) -> [&'static str; 3] { self.key.words() }

    /// The owner's decision as it stands now.
    pub async fn result(&self) -> Result<Outcome> {
        let r: PairResult = serde_json::from_value(exchange(&self.host, self.port, "GET", &format!("/pair/phone/result?id={}", self.phone_id), None).await?)?;
        Ok(match r.state.as_str() {
            "pending" => Outcome::Pending(r.expires_in),
            "approved" => {
                let p: DesktopPayload = self.key.open(r.payload.as_deref().context("the computer approved without its payload")?)?;
                let der = crate::identity::pem_to_der(&p.cert_pem)?;
                if self.fingerprint.as_deref().is_some_and(|f| f != pairing::fingerprint(&der)) { bail!("that is not the computer the QR code named"); }
                if p.id != self.desktop_id { bail!("the computer changed during the pairing"); }
                Outcome::Approved(p)
            }
            "refused" => Outcome::Refused,
            _ => Outcome::Expired,
        })
    }

    /// Asked every `every` until the owner decides or the code expires.
    pub async fn wait(&self, every: Duration) -> Result<Outcome> {
        loop {
            match self.result().await? {
                Outcome::Pending(_) => tokio::time::sleep(every).await,
                o => return Ok(o),
            }
        }
    }
}
