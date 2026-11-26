//! Pairing from the phone, over the network: the computer's plain-HTTP pairing port (`/pair/hello`,
//! `/pair/phone/start`, `/pair/phone/finish`, then `/pair/phone/result` until the owner decides). Everything that
//! matters is sealed under the SPAKE2 key, and the computer's certificate is checked against the fingerprint the QR
//! code carries, so the plain HTTP carries nothing to trust.

use std::time::{Duration, Instant};

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

    /// Asked every `every` until the owner decides or the code expires. The phone keeps its own deadline, from the
    /// time left the computer first gave (at most `LONGEST_WAIT`): a computer that answers "pending" for ever, or one
    /// gone once the code expired, ends as `Expired`.
    /// Each exchange is bounded by that deadline too, and once the code's own time is up a failed exchange is the
    /// computer closing its port: `Expired` as well.
    pub async fn wait(&self, every: Duration) -> Result<Outcome> {
        // (the code's expiry, the deadline a moment later), from the first "pending"
        let mut times: Option<(Instant, Instant)> = None;
        loop {
            let asked = match times {
                Some((_, d)) => match tokio::time::timeout_at(d.into(), self.result()).await { Ok(r) => r, Err(_) => return Ok(Outcome::Expired) },
                None => self.result().await,
            };
            match asked {
                Ok(Outcome::Pending(left)) => {
                    let (_, d) = *times.get_or_insert_with(|| { let e = Instant::now() + Duration::from_millis(left.max(0) as u64).min(LONGEST_WAIT); (e, e + GRACE) });
                    let now = Instant::now();
                    if now >= d { return Ok(Outcome::Expired); }
                    tokio::time::sleep(every.min(d - now)).await;
                }
                Ok(o) => return Ok(o),
                Err(_) if times.is_some_and(|(e, _)| Instant::now() >= e) => return Ok(Outcome::Expired),
                Err(e) => return Err(e),
            }
        }
    }
}

/// The longest a code is shown (two minutes), and the moment more an answer may take to arrive.
const LONGEST_WAIT: Duration = Duration::from_secs(120);
const GRACE: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer whose first answer is "pending" with the code already expired, and whose later answers are `then`:
    /// "pending" again, "hang" (no answer for a minute), or "close" (the port shuts, as at the code's expiry).
    async fn computer(then: &'static str) -> Joining {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut first = true;
            loop {
                let Ok((mut s, _)) = l.accept().await else { return };
                let mode = if first { "pending" } else { then };
                first = false;
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 4096];
                    let _ = s.read(&mut buf).await;
                    match mode {
                        "hang" => { tokio::time::sleep(Duration::from_secs(60)).await; }
                        "close" => {}
                        _ => {
                            let body = r#"{"state":"pending","expires_in":0}"#;
                            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
                        }
                    }
                });
            }
        });
        let (p, msg) = Pairing::start("123456", "phone-1", "desk-1").unwrap();
        let (_, back) = pairing::answer("123456", "phone-1", "desk-1", &msg).unwrap();
        Joining { host: "127.0.0.1".into(), port, phone_id: "phone-1".into(), fingerprint: None, key: p.finish(&back).unwrap(), desktop_id: "desk-1".into(), desktop_name: "D".into() }
    }

    async fn waited(then: &'static str) -> (Result<Outcome>, Duration) {
        let j = computer(then).await;
        let started = Instant::now();
        let r = tokio::time::timeout(Duration::from_secs(30), j.wait(Duration::from_millis(200))).await.expect("the wait ended");
        (r, started.elapsed())
    }

    /// N4: a computer that answers "pending" for ever does not keep the phone waiting past the time it gave.
    #[tokio::test]
    async fn waiting_ends_at_the_phones_own_deadline() {
        let (r, took) = waited("pending").await;
        assert_eq!(r.unwrap(), Outcome::Expired);
        assert!(took < GRACE + Duration::from_secs(2), "{took:?}");
    }

    /// N4: an exchange already under way is held to the same deadline.
    #[tokio::test]
    async fn an_exchange_under_way_is_held_to_the_deadline() {
        let (r, took) = waited("hang").await;
        assert_eq!(r.unwrap(), Outcome::Expired);
        assert!(took < GRACE + Duration::from_secs(2), "{took:?}");
    }

    /// N4: once the code's time is up, the computer closing its port is the expiry, not an error.
    #[tokio::test]
    async fn the_port_closing_after_the_code_expired_is_the_expiry() {
        let (r, took) = waited("close").await;
        assert_eq!(r.unwrap(), Outcome::Expired);
        assert!(took < Duration::from_secs(2), "{took:?}");
    }
}
