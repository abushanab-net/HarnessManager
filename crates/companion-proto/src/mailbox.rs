//! The phone away from home: the owner's hub carries numbered messages between the phone and one computer
//! (`/v1/mail/{phone}/up|down/{seq}`, docs/plans/mobile/PLAN.md, "the hub, for phones"). A computer on the hub
//! invites the phone with a one-use code; the phone redeems it for a token that reaches only its own mailbox and
//! pairs through it as it would on the LAN: SPAKE2 with the code, the three words, and nothing sealed back before the
//! owner approves on the computer. The approval is signed by the computer's own identity (the one the invite's
//! fingerprint pins) over the exchange, the mailbox, both ends' keys and the grants. From then on every message is
//! sealed to its recipient and signed by its sender (`seal`), and names its mailbox, direction and number, so the hub
//! sees only sizes and times and cannot replay or move one; each side reads numbers only upwards. The computer posts
//! snapshots (hello, overview, accounts, requests) after each change; the phone posts the same action bodies as
//! `POST /c1/actions` and the receipts come back the same way. `Mailbox` answers the same typed calls as the direct
//! `Client`, so the app can switch routes, and says how fresh what it shows is (`freshness`).

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use http_body_util::Full;
use rustls::pki_types::ServerName;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::api::{AccountOne, Accounts, Action, Computer, ConnectError, Hello, Overview, Receipt, Refused, Requests};
use crate::join::Outcome;
use crate::pairing::{self, DesktopPayload, PhonePayload};
use crate::seal::{open_from, seal_to, Sealed};
use crate::{b64, net, unb64, Keyed, Pairing, PhoneIdentity};

fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }

// ---------- the sealed envelope, the same on both sides ----------

/// `body` sealed by `sender` to X25519 key `to`, for slot `seq` of `mailbox`'s `dir`: what is put on the hub.
pub fn seal_mail(sender: &SigningKey, to: &str, mailbox: &str, dir: &str, seq: i64, body: &Value) -> Result<Vec<u8>> {
    let env = json!({"mailbox": mailbox, "dir": dir, "seq": seq, "at": now_ms(), "body": body});
    Ok(serde_json::to_vec(&seal_to(sender, to, &serde_json::to_vec(&env)?)?)?)
}

/// A message read from slot `seq` of `mailbox`'s `dir`, opened with our X25519 secret: its body, when `from` sealed it
/// for that very slot. Changed on the way, sealed by anyone else, or copied from another slot (a replay): an error.
pub fn open_mail(secret: &[u8; 32], from: &str, mailbox: &str, dir: &str, seq: i64, bytes: &[u8]) -> Result<Value> {
    let s: Sealed = serde_json::from_slice(bytes).context("not a sealed message")?;
    let env: Value = serde_json::from_slice(&open_from(secret, from, &s)?)?;
    if env["mailbox"] != mailbox || env["dir"] != dir || env["seq"] != seq { bail!("a message not written for this place: replayed or moved"); }
    Ok(env["body"].clone())
}

/// The numbers of a listing to read, in order: within the hub's range (1 to 2^53), above `cursor`, each above the
/// one before. A hub that lists an older message after a newer one, one twice, or one out of range, is not followed.
pub fn ahead(list: &Value, cursor: i64) -> Vec<i64> {
    let mut last = cursor;
    list["mail"].as_array().into_iter().flatten().filter_map(|m| m["seq"].as_i64()).filter(|s| (1..=1i64 << 53).contains(s))
        .filter(|&s| { let up = s > last; if up { last = s; } up }).collect()
}

/// The next number after `max`, as the hub listed it; none past the hub's own range (2^53).
pub fn next_seq(max: i64) -> Result<i64> { max.checked_add(1).filter(|n| (1..=1i64 << 53).contains(n)).context("the mailbox has no number left; pair again") }

// ---------- the approval, signed by the computer ----------

/// The Ed25519 SubjectPublicKeyInfo prefix (RFC 8410): a certificate made for key K carries it followed by K.
const ED25519_SPKI: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// What the computer's identity signs when it approves a phone through the hub: the exchange as each side saw it
/// (the phone's first message, the computer's answer, the phone's sealed payload), the mailbox, the phone's id and
/// both of its keys, the computer's id and both of its keys, and the grants. A hub that ran either half of the
/// exchange itself, or swapped a key, makes the phone's copy differ from what the computer signed.
pub fn approval_binding(mailbox: &str, phone_id: &str, phone_pubkey: &str, phone_x25519: &str, desk: &DesktopPayload, transcript: [&[u8]; 3]) -> Vec<u8> {
    let mut h = Sha256::new();
    for t in transcript { h.update((t.len() as u64).to_be_bytes()); h.update(t); }
    format!("hm hub approval v1\n{mailbox}\n{phone_id}\n{phone_pubkey}\n{phone_x25519}\n{}\n{}\n{}\n{}\n{}", desk.id, desk.pubkey, desk.x25519.as_deref().unwrap_or(""),
        serde_json::to_string(&desk.grants).unwrap_or_default(), hex::encode(h.finalize())).into_bytes()
}

/// The signature checks out under the computer's key, and that key is the one its certificate is made for.
fn approval_ok(desk: &DesktopPayload, cert_der: &[u8], binding: &[u8], sig: &str) -> bool {
    let Ok(pk) = unb64(&desk.pubkey) else { return false };
    let spki = [ED25519_SPKI.as_slice(), &pk].concat();
    // the certificate's own public-key field, parsed: not the key's bytes found anywhere in it
    let der = rustls::pki_types::CertificateDer::from(cert_der);
    let Ok(cert) = webpki::EndEntityCert::try_from(&der) else { return false };
    if pk.len() != 32 || cert.subject_public_key_info().as_ref() != spki.as_slice() { return false; }
    let (Ok(vk), Ok(sig)) = (VerifyingKey::from_bytes(&pk.try_into().unwrap_or([0; 32])), unb64(sig).and_then(|s| Ok(Signature::from_slice(&s)?))) else { return false };
    vk.verify(binding, &sig).is_ok()
}

/// What a refused phone signs to say it read the refusal (it has no key of the computer's to seal to).
pub fn fetched_refusal(mailbox: &str) -> Vec<u8> { format!("hm hub fetched v1\n{mailbox}\nrefused").into_bytes() }

// ---------- the hub over HTTP ----------

/// A loopback or private address (10/8, 172.16/12, 192.168/16, fc00::/7): the only places plain HTTP goes.
pub fn private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => a.is_loopback() || a.is_private(),
        IpAddr::V6(a) => a.is_loopback() || (a.segments()[0] & 0xfe00) == 0xfc00 || a.to_ipv4_mapped().is_some_and(|v| v.is_loopback() || v.is_private()),
    }
}

/// The hub at its address: TLS checked against the public roots (Caddy in front of it holds a public certificate),
/// or plain HTTP only to a private or loopback address written as one, checked again on the connection before
/// anything is sent. A name never goes over plain HTTP: what it resolves to is not the phone's to trust. Each call
/// on a connection of its own: messages are few.
pub(crate) struct HubHttp {
    host: String,
    port: u16,
    authority: String,
    prefix: String,
    token: String,
    tls: Option<Arc<rustls::ClientConfig>>,
}

impl HubHttp {
    pub(crate) fn new(url: &str, token: &str) -> Result<HubHttp> {
        let url = url.trim().trim_end_matches('/');
        let (https, rest) = match (url.strip_prefix("https://"), url.strip_prefix("http://")) {
            (Some(r), _) => (true, r),
            (_, Some(r)) => (false, r),
            _ => bail!("the hub address starts with https://"),
        };
        let (authority, prefix) = match rest.find('/') { Some(i) => (&rest[..i], rest[i..].to_string()), None => (rest, String::new()) };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if (!h.contains(':') || h.ends_with(']')) && p.parse::<u16>().is_ok() => (h, p.parse()?),
            _ => (authority, if https { 443 } else { 80 }),
        };
        let host = host.trim_matches(['[', ']']).to_string();
        if host.is_empty() { bail!("the hub address has no host"); }
        if !https && !host.parse::<IpAddr>().is_ok_and(private_ip) { bail!("the hub at {host} needs https://: plain http:// goes only to a private or loopback address"); }
        let tls = https.then(|| -> Result<Arc<rustls::ClientConfig>> {
            let mut roots = rustls::RootCertStore::empty();
            roots.add_parsable_certificates(webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().cloned());
            Ok(Arc::new(rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider())).with_safe_default_protocol_versions()?
                .with_root_certificates(roots).with_no_client_auth()))
        }).transpose()?;
        Ok(HubHttp { host, port, authority: authority.to_string(), prefix, token: token.to_string(), tls })
    }

    async fn call_once(&self, method: &str, path: &str, body: Option<Vec<u8>>) -> Result<(u16, Bytes)> {
        let tcp = net::tcp(&self.host, self.port).await?;
        let mut s = match &self.tls {
            Some(cfg) => {
                let name = ServerName::try_from(self.host.clone()).map_err(|_| anyhow!("{} is not a host name", self.host))?;
                let t = tokio::time::timeout(net::CONNECT, tokio_rustls::TlsConnector::from(cfg.clone()).connect(name, tcp)).await.map_err(|_| anyhow!("the hub did not finish the handshake"))??;
                net::handshake(t).await?
            }
            None => {
                // the address the connection reached, not the one asked for, before the code or the token goes out
                let peer = tcp.peer_addr()?.ip();
                if !private_ip(peer) { bail!("{peer} is not a private address; plain http:// is not sent there"); }
                net::handshake(tcp).await?
            }
        };
        let mut r = hyper::Request::builder().method(method).uri(format!("{}{path}", self.prefix)).header("host", &self.authority);
        if !self.token.is_empty() { r = r.header("authorization", format!("Bearer {}", self.token)); }
        // every body is JSON: the join's, and the sealed messages
        if body.is_some() { r = r.header("content-type", "application/json"); }
        net::send(&mut s, r.body(Full::new(Bytes::from(body.unwrap_or_default())))?).await
    }

    /// One call: `(status, body)`. Not reached is `ConnectError::Unreachable`; a phone token the hub no longer knows
    /// (the computer removed the phone) is `ConnectError::Revoked`; any other refusal is `Refused`.
    async fn call(&self, method: &str, path: &str, body: Option<Vec<u8>>) -> Result<Bytes> {
        let (status, b) = self.call_once(method, path, body).await.map_err(|e| ConnectError::Unreachable(format!("the hub: {e:#}")))?;
        if (200..300).contains(&status) { return Ok(b); }
        let error = serde_json::from_slice::<Value>(&b).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_default();
        if status == 401 && !self.token.is_empty() { return Err(ConnectError::Revoked(format!("the hub no longer knows this phone: {error}")).into()); }
        Err(Refused { status, error }.into())
    }

    async fn json(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        Ok(serde_json::from_slice(&self.call(method, path, body.map(|b| b.to_string().into_bytes())).await?)?)
    }

    /// A message's bytes; None when the slot is empty (not written yet, or expired).
    async fn get_mail(&self, mailbox: &str, dir: &str, seq: i64) -> Result<Option<Bytes>> {
        match self.call("GET", &format!("/v1/mail/{mailbox}/{dir}/{seq}"), None).await {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 404) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn put_mail(&self, mailbox: &str, dir: &str, seq: i64, body: Vec<u8>) -> Result<()> { self.call("PUT", &format!("/v1/mail/{mailbox}/{dir}/{seq}"), Some(body)).await.map(|_| ()) }

    /// `{max, mail: [{seq, bytes, at}], more}` after `after`, a page of at most 100.
    async fn list(&self, mailbox: &str, dir: &str, after: i64) -> Result<Value> { self.json("GET", &format!("/v1/mail/{mailbox}/{dir}?after={after}"), None).await }
}

// ---------- pairing through the hub ----------

/// What the computer's invite shows: `hm-hub:1?c=<code>&f=<fingerprint>&u=<hub url>` (the URL last: it may hold
/// anything but a newline).
pub fn hub_qr(hub_url: &str, code: &str, fingerprint: &str) -> String { format!("hm-hub:1?c={code}&f={fingerprint}&u={hub_url}") }

/// An invite read back: `(hub url, code, fingerprint)`.
pub fn parse_hub_qr(s: &str) -> Result<(String, String, String)> {
    let q = s.trim().strip_prefix("hm-hub:1?").context("not a Harness Manager hub invite")?;
    let (head, url) = q.split_once("&u=").context("the invite has no hub address")?;
    let get = |k: &str| head.split('&').find_map(|kv| kv.strip_prefix(k).and_then(|v| v.strip_prefix('='))).map(String::from).with_context(|| format!("the invite has no {k}"));
    let (c, f) = (get("c")?, get("f")?);
    if c.len() != 6 || !c.bytes().all(|b| b.is_ascii_digit()) { bail!("the code is six digits"); }
    Ok((url.to_string(), c, f))
}

/// How long the phone waits for the computer's half of the exchange: it answers within seconds while it shows the
/// invite.
const PAIR_ANSWER: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(300);
const GRACE: Duration = Duration::from_secs(5);
/// The longest an invite is waited on, whatever the hub says (it gives ten minutes).
const LONGEST_INVITE_MS: i64 = 15 * 60_000;

/// When an invite stops being waited on: the hub's word, kept between now and fifteen minutes from now.
pub fn invite_deadline(expires_at: i64, now: i64) -> i64 { expires_at.clamp(now, now.saturating_add(LONGEST_INVITE_MS)) }

/// The phone's payload is in the mailbox; the owner sees the three words and decides on the computer.
pub struct HubJoining {
    hub: HubHttp,
    me: PhoneIdentity,
    fingerprint: Option<String>,
    key: Keyed,
    expires_at: i64,
    /// The exchange as this phone saw it: its first message, the computer's answer, its sealed payload.
    transcript: [Vec<u8>; 3],
    pub hub_url: String,
    /// The phone's token on the hub and its mailbox there: what the app keeps, with the computer's payload.
    pub token: String,
    pub mailbox: String,
    /// The computer that invited this phone, as the hub names it.
    pub desktop_id: String,
    pub desktop_name: String,
}

/// Join through the hub with the invite the computer shows.
pub async fn join_hub_qr(qr: &str, me: &PhoneIdentity, name: &str, platform: &str) -> Result<HubJoining> {
    let (u, code, f) = parse_hub_qr(qr)?;
    join_hub(&u, &code, Some(&f), me, name, platform).await
}

/// The same with the hub's address and the code typed; without a fingerprint the computer's certificate is the one the
/// sealed answer names, which only the holder of the code could seal (and the owner compared the words).
pub async fn join_hub(hub_url: &str, code: &str, fingerprint: Option<&str>, me: &PhoneIdentity, name: &str, platform: &str) -> Result<HubJoining> {
    let r = match HubHttp::new(hub_url, "")?.json("POST", "/v1/phones/join", Some(json!({"code": code.trim()}))).await {
        Ok(r) => r,
        Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 401) => bail!("the hub did not take the code: it is wrong, used or expired"),
        Err(e) => return Err(e),
    };
    let (token, mailbox) = (r["token"].as_str().unwrap_or("").to_string(), r["phone_id"].as_str().unwrap_or("").to_string());
    let (desktop_id, desktop_name) = (r["computer"]["id"].as_str().unwrap_or("").to_string(), r["computer"]["name"].as_str().unwrap_or("").to_string());
    if token.is_empty() || mailbox.is_empty() || desktop_id.is_empty() { bail!("the hub answered without a token"); }
    let hub = HubHttp::new(hub_url, &token)?;
    let (p, msg) = Pairing::start(code, &me.id, &desktop_id)?;
    let up1 = json!({"kind": "pair-start", "id": me.id, "msg": msg}).to_string().into_bytes();
    hub.put_mail(&mailbox, "up", 1, up1.clone()).await?;
    let until = Instant::now() + PAIR_ANSWER;
    let down1 = loop {
        if let Some(b) = hub.get_mail(&mailbox, "down", 1).await? { break b.to_vec(); }
        if Instant::now() >= until { bail!("the computer did not answer through the hub; is it on and showing the invite?"); }
        tokio::time::sleep(POLL).await;
    };
    let back: Value = serde_json::from_slice(&down1)?;
    let key = p.finish(back["msg"].as_str().unwrap_or(""))?;
    let up2 = json!({"kind": "pair-finish", "payload": key.seal(&PhonePayload::of(me, name, platform)?)?}).to_string().into_bytes();
    hub.put_mail(&mailbox, "up", 2, up2.clone()).await?;
    Ok(HubJoining { hub, me: me.clone(), fingerprint: fingerprint.map(String::from), key, expires_at: invite_deadline(r["expires_at"].as_i64().unwrap_or(0), now_ms()),
        transcript: [up1, down1, up2], hub_url: hub_url.trim().trim_end_matches('/').to_string(), token, mailbox, desktop_id, desktop_name })
}

impl HubJoining {
    /// The three words to compare with the computer's screen.
    pub fn words(&self) -> [&'static str; 3] { self.key.words() }

    /// The owner's decision as it stands now. An approval counts only signed by the computer's own identity over
    /// what this phone sent and saw. Once it is read the phone says so in slot 3 (sealed, or signed for a refusal), so
    /// the computer can stop showing the invite.
    pub async fn result(&self) -> Result<Outcome> {
        let got = match self.hub.get_mail(&self.mailbox, "down", 2).await {
            // the computer stopped showing the invite (its time ran out, or it was cancelled): the mailbox is gone
            Err(e) if matches!(e.downcast_ref::<ConnectError>(), Some(ConnectError::Revoked(_))) => return Ok(Outcome::Expired),
            r => r?,
        };
        let Some(b) = got else { return Ok(Outcome::Pending((self.expires_at - now_ms()).max(0))) };
        let r: Value = serde_json::from_slice(&b).context("the computer's decision")?;
        if r["kind"] != "pair-result" { bail!("slot 2 holds no decision"); }
        Ok(match r["state"].as_str() {
            Some("approved") => {
                let p: DesktopPayload = self.key.open(r["payload"].as_str().context("the computer approved without its payload")?)?;
                let der = crate::identity::pem_to_der(&p.cert_pem)?;
                if self.fingerprint.as_deref().is_some_and(|f| f != pairing::fingerprint(&der)) { bail!("that is not the computer the invite named"); }
                if p.id != self.desktop_id { bail!("the computer changed during the pairing"); }
                let x = p.x25519.as_deref().context("the computer sent no key to seal to")?;
                let [a, b2, c] = &self.transcript;
                let binding = approval_binding(&self.mailbox, &self.me.id, &self.me.pubkey(), &self.me.x25519_public(), &p, [a, b2, c]);
                if !approval_ok(&p, &der, &binding, r["sig"].as_str().unwrap_or("")) { bail!("the approval is not signed by the computer for this exchange"); }
                // told once; a second look finds the slot taken, which is the same news
                if let Ok(m) = seal_mail(self.me.signing_key(), x, &self.mailbox, "up", 3, &json!({"kind": "fetched"})) { let _ = self.hub.put_mail(&self.mailbox, "up", 3, m).await; }
                Outcome::Approved(p)
            }
            Some("refused") => {
                let sig = b64(&self.me.signing_key().sign(&fetched_refusal(&self.mailbox)).to_bytes());
                let _ = self.hub.put_mail(&self.mailbox, "up", 3, json!({"kind": "fetched", "sig": sig}).to_string().into_bytes()).await;
                Outcome::Refused
            }
            _ => Outcome::Expired,
        })
    }

    /// Asked every `every` until the owner decides, or the invite's time (and a moment more) is up: `Expired`.
    pub async fn wait(&self, every: Duration) -> Result<Outcome> {
        let deadline = Instant::now() + Duration::from_millis((self.expires_at - now_ms()).max(0) as u64) + GRACE;
        loop {
            match self.result().await? {
                Outcome::Pending(_) if Instant::now() < deadline => tokio::time::sleep(every.min(deadline - Instant::now())).await,
                Outcome::Pending(_) => return Ok(Outcome::Expired),
                o => return Ok(o),
            }
        }
    }

    /// The mailbox client once approved, past the pairing's own messages.
    pub fn mailbox(&self, desk: &DesktopPayload) -> Result<Mailbox> {
        let m = Mailbox::new(&self.me, &self.hub_url, &self.token, &self.mailbox, &desk.pubkey, desk.x25519.as_deref().unwrap_or(""))?;
        m.st.try_lock().expect("new").down_seen = 2;
        Ok(m)
    }
}

// ---------- the mailbox, as the direct client ----------

/// The most messages one look reads: after a long absence the older ones are passed over (and counted as missed),
/// since a newer snapshot supersedes them and a receipt is asked for again. One look also asks for at most
/// `LOOK_PAGES` listings and lasts at most `LOOK_TIME`, whatever the hub answers.
const READ_AT_MOST: i64 = 50;
const LOOK_PAGES: usize = 5;
const LOOK_TIME: Duration = Duration::from_secs(10);
/// The hub's own range of message numbers.
const MAX_SEQ: i64 = 1 << 53;
/// How long an action or a receipt question waits for the computer's answer through the hub.
const ANSWER_WAIT: Duration = Duration::from_secs(20);
/// The computer sends a snapshot at least every five minutes: none received here for longer than this, one older
/// than this by the computer's own clock, or no word from the hub for as long, is stale.
pub const STALE_MS: i64 = 12 * 60_000;

/// How fresh what the mailbox shows is: when the newest snapshot was taken (the computer's clock) and how long ago
/// this phone received it (its own clock), when the hub last answered, how many of the computer's messages never
/// arrived or were passed over, and whether it is stale. A hub can always withhold mail; this is how the phone sees
/// it did.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Freshness {
    pub snapshot_at: Option<i64>,
    pub snapshot_age_ms: Option<i64>,
    pub heard_at: Option<i64>,
    pub missed: u64,
    pub stale: bool,
}

#[derive(Default)]
struct MailSt {
    /// The last message read that opened: the cursor the app keeps.
    down_seen: i64,
    /// The last slot looked at, whether it opened or not: where the next listing starts; never kept.
    inspected: i64,
    snapshot: Option<Value>,
    /// When the newest snapshot arrived, and when the hub last answered, by this phone's monotonic clock.
    snapshot_got: Option<Instant>,
    heard: Option<Instant>,
    heard_at: Option<i64>,
    dropped: u32,
    missed: u64,
}

impl MailSt {
    /// One message read from slot `seq`: when it opened, the cursor moves up to it (the slots it jumped counted as
    /// missed) and its body is kept or handed back as an answer; one that did not open moves nothing the app keeps.
    fn take(&mut self, seq: i64, opened: Result<Value>) -> Option<Value> {
        self.inspected = self.inspected.max(seq);
        if seq <= self.down_seen { return None; }
        let Ok(body) = opened else { self.dropped += 1; return None };
        self.missed += (seq - self.down_seen - 1) as u64;
        self.down_seen = seq;
        match body["kind"].as_str() {
            Some("snapshot") => { self.snapshot = Some(body); self.snapshot_got = Some(Instant::now()); None }
            Some("receipt" | "refused") => Some(body),
            _ => None,
        }
    }

    fn freshness(&self, now_ms: i64, now: Instant) -> Freshness {
        let snapshot_at = self.snapshot.as_ref().and_then(|s| s["hello"]["observed_at"].as_i64());
        let age = self.snapshot_got.map(|t| now.saturating_duration_since(t).as_millis() as i64);
        let quiet = self.heard.map(|t| now.saturating_duration_since(t).as_millis() as i64);
        let stale = age.is_none_or(|a| a > STALE_MS) || quiet.is_none_or(|q| q > STALE_MS) || snapshot_at.is_none_or(|t| now_ms - t > STALE_MS);
        Freshness { snapshot_at, snapshot_age_ms: age, heard_at: self.heard_at, missed: self.missed, stale }
    }
}

/// One call waiting for the computer's answer under `key` (`act:<action id>` or `q:<query id>`); each call has its
/// own, so two calls for one action each get it.
struct Waiter {
    id: u64,
    key: String,
    answer: Option<Value>,
}

/// An answer handed to every call waiting for it; one nobody waits for is dropped.
fn hand(waiters: &mut [Waiter], body: &Value) {
    let key = match body["query"].as_str() { Some(q) => format!("q:{q}"), None => format!("act:{}", body["action_id"].as_str().unwrap_or("")) };
    for w in waiters.iter_mut().filter(|w| w.key == key && w.answer.is_none()) { w.answer = Some(body.clone()); }
}

/// Takes its waiter away however the call ends, given up on included.
struct Leave<'a>(&'a std::sync::Mutex<Vec<Waiter>>, u64);

impl Drop for Leave<'_> {
    fn drop(&mut self) { self.0.lock().unwrap_or_else(|e| e.into_inner()).retain(|w| w.id != self.1); }
}

/// One paired computer through the hub.
pub struct Mailbox {
    hub: HubHttp,
    me: PhoneIdentity,
    mailbox: String,
    desk_pubkey: String,
    desk_x25519: String,
    st: Mutex<MailSt>,
    waiters: std::sync::Mutex<Vec<Waiter>>,
    next_waiter: std::sync::atomic::AtomicU64,
}

impl Mailbox {
    /// The mailbox `mailbox` on the hub at `hub_url` with the phone's `token`, to the computer whose Ed25519 and
    /// X25519 keys the pairing gave (`DesktopPayload::pubkey`, `x25519`).
    pub fn new(me: &PhoneIdentity, hub_url: &str, token: &str, mailbox: &str, desk_pubkey: &str, desk_x25519: &str) -> Result<Mailbox> {
        if desk_x25519.is_empty() { bail!("the computer gave no key to seal to; pair through the hub again"); }
        Ok(Mailbox { hub: HubHttp::new(hub_url, token)?, me: me.clone(), mailbox: mailbox.into(), desk_pubkey: desk_pubkey.into(), desk_x25519: desk_x25519.into(),
            st: Mutex::new(MailSt::default()), waiters: std::sync::Mutex::new(vec![]), next_waiter: Default::default() })
    }

    /// The last message read from the computer that opened, for the app to keep, and to start from again
    /// (`resume_from`, which never moves it back).
    pub async fn down_seen(&self) -> i64 { self.st.lock().await.down_seen }

    pub async fn resume_from(&self, down_seen: i64) { let mut st = self.st.lock().await; st.down_seen = st.down_seen.max(down_seen.clamp(0, MAX_SEQ)); }

    /// Messages read that did not open (changed on the way, sealed by someone else, replayed): never used.
    pub async fn dropped(&self) -> u32 { self.st.lock().await.dropped }

    pub async fn freshness(&self) -> Freshness { self.st.lock().await.freshness(now_ms(), Instant::now()) }

    /// Read what the computer left since the last look, oldest first: at most `READ_AT_MOST` messages, `LOOK_PAGES`
    /// listings and `LOOK_TIME`. A listing that brings nothing new ends the look, whatever it says of more.
    pub async fn sync(&self) -> Result<()> {
        let until = Instant::now() + LOOK_TIME;
        let mut st = self.st.lock().await;
        let mut after = st.down_seen.max(st.inspected);
        let mut list = self.hub.list(&self.mailbox, "down", after).await?;
        (st.heard, st.heard_at) = (Some(Instant::now()), Some(now_ms()));
        // far behind: start with the newest, the rest counted as missed once a newer one opens
        let max = list["max"].as_i64().unwrap_or(0).clamp(0, MAX_SEQ);
        if max.saturating_sub(after) > READ_AT_MOST {
            after = max - READ_AT_MOST;
            list = self.hub.list(&self.mailbox, "down", after).await?;
        }
        let mut read = 0;
        for _ in 0..LOOK_PAGES {
            let seqs = ahead(&list, after);
            if seqs.is_empty() { return Ok(()); }
            for seq in seqs {
                if read >= READ_AT_MOST || Instant::now() >= until { return Ok(()); }
                read += 1;
                after = seq;
                let Some(bytes) = self.hub.get_mail(&self.mailbox, "down", seq).await? else { st.inspected = st.inspected.max(seq); continue };
                let opened = open_mail(&self.me.x25519_secret(), &self.desk_pubkey, &self.mailbox, "down", seq, &bytes);
                if let Some(answer) = st.take(seq, opened) { hand(&mut self.waiters.lock().unwrap_or_else(|e| e.into_inner()), &answer); }
            }
            if list["more"] != true || Instant::now() >= until { return Ok(()); }
            list = self.hub.list(&self.mailbox, "down", after).await?;
        }
        Ok(())
    }

    /// One message to the computer, sealed for the next free slot from 4 on (slots 1 to 3 are the pairing's, 3 the
    /// phone's word that it read the decision); another of this phone's takes the slot first: nothing was stored, so
    /// the next one is tried.
    async fn send(&self, body: &Value) -> Result<()> {
        for _ in 0..3 {
            let seq = next_seq(self.hub.list(&self.mailbox, "up", i64::MAX).await?["max"].as_i64().unwrap_or(0))?.max(4);
            let m = seal_mail(self.me.signing_key(), &self.desk_x25519, &self.mailbox, "up", seq, body)?;
            match self.hub.put_mail(&self.mailbox, "up", seq, m).await {
                Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 409) => continue,
                r => return r,
            }
        }
        bail!("the mailbox is busy; try again")
    }

    /// `body` sent, then the computer's answer under `key`, read as it comes. The waiter goes when the call ends,
    /// however it ends.
    async fn ask(&self, key: String, body: Value) -> Result<Value> {
        let id = self.next_waiter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.waiters.lock().unwrap_or_else(|e| e.into_inner()).push(Waiter { id, key, answer: None });
        let _leave = Leave(&self.waiters, id);
        self.send(&body).await?;
        let until = Instant::now() + ANSWER_WAIT;
        loop {
            self.sync().await?;
            let got = self.waiters.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|w| w.id == id).and_then(|w| w.answer.clone());
            if let Some(v) = got { return Ok(v); }
            if Instant::now() >= until { return Err(ConnectError::Unreachable("the computer did not answer through the hub in time".into()).into()); }
            tokio::time::sleep(POLL).await;
        }
    }

    fn receipt_of(v: &Value) -> Result<Receipt> {
        if v["kind"] == "refused" { return Err(Refused { status: v["status"].as_u64().unwrap_or(403) as u16, error: v["error"].as_str().unwrap_or("").into() }.into()); }
        Ok(serde_json::from_value(v["receipt"].clone())?)
    }

    /// A part of the newest snapshot; 403 with the computer's reason when this phone is not granted it.
    async fn snap<T: DeserializeOwned>(&self, part: &str) -> Result<T> {
        self.sync().await?;
        let st = self.st.lock().await;
        let s = st.snapshot.as_ref().ok_or_else(|| ConnectError::Unreachable("the computer has not sent its state through the hub yet".into()))?;
        match &s[part] {
            Value::Null => Err(Refused { status: 403, error: s["refused"].as_str().unwrap_or("not granted to this phone").into() }.into()),
            v => Ok(serde_json::from_value(v.clone())?),
        }
    }

    pub async fn hello(&self) -> Result<Hello> { self.snap("hello").await }

    pub async fn overview(&self) -> Result<Overview> { self.snap("overview").await }

    pub async fn accounts(&self) -> Result<Accounts> { self.snap("accounts").await }

    pub async fn account(&self, profile: &str) -> Result<AccountOne> {
        let a: Accounts = self.accounts().await?;
        let Accounts { computer, observed_at, accounts } = a;
        let account = accounts.into_iter().find(|x| x.profile == profile).ok_or_else(|| Refused { status: 404, error: format!("no account is called {profile}") })?;
        Ok(AccountOne { computer, observed_at, account })
    }

    pub async fn requests(&self) -> Result<Requests> { self.snap("requests").await }

    /// The computer this mailbox reaches, from its newest snapshot.
    pub async fn computer(&self) -> Result<Computer> { Ok(self.hello().await?.computer) }

    /// An action, sent once: its receipt, as the computer's `POST /c1/actions` gives it. When no answer comes in time,
    /// ask `receipt` with the same id; if that fails too, the phone shows "delivery unknown".
    pub async fn act(&self, a: &Action) -> Result<Receipt> {
        Self::receipt_of(&self.ask(format!("act:{}", a.action_id), json!({"kind": "act", "action": a})).await?)
    }

    /// The stored receipt of an action, asked of the computer each time (it checks this phone's grants as it answers);
    /// None when the computer never got the action.
    pub async fn receipt(&self, action_id: &str) -> Result<Option<Receipt>> {
        let query = hex::encode(crate::random::<16>()?);
        let v = self.ask(format!("q:{query}"), json!({"kind": "receipt", "action_id": action_id, "query": query})).await?;
        match Self::receipt_of(&v) {
            Ok(r) => Ok(Some(r)),
            Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 404) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_envelope_opens_only_where_it_was_written() {
        let (phone, desk) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
        let m = seal_mail(phone.signing_key(), &desk.x25519_public(), "ph-1", "up", 4, &json!({"kind": "act"})).unwrap();
        assert_eq!(open_mail(&desk.x25519_secret(), &phone.pubkey(), "ph-1", "up", 4, &m).unwrap()["kind"], "act");
        for (mb, dir, seq) in [("ph-1", "up", 5), ("ph-1", "down", 4), ("ph-2", "up", 4)] {
            assert!(open_mail(&desk.x25519_secret(), &phone.pubkey(), mb, dir, seq, &m).is_err(), "{mb} {dir} {seq}");
        }
    }

    #[test]
    fn the_hub_invite_reads_back_and_the_address_splits() {
        let s = hub_qr("https://hub.example.net/hm", "012345", "ab12");
        assert_eq!(parse_hub_qr(&s).unwrap(), ("https://hub.example.net/hm".into(), "012345".into(), "ab12".into()));
        assert!(parse_hub_qr("hm-hub:1?c=1&f=x&u=http://h").is_err());
        let h = HubHttp::new("https://hub.example.net/hm/", "t").unwrap();
        assert_eq!((h.host.as_str(), h.port, h.prefix.as_str(), h.tls.is_some()), ("hub.example.net", 443, "/hm", true));
        let h = HubHttp::new("http://[::1]:8787", "").unwrap();
        assert_eq!((h.host.as_str(), h.port, h.authority.as_str(), h.tls.is_some()), ("::1", 8787, "[::1]:8787", false));
        assert!(HubHttp::new("ftp://x", "").is_err());
    }

    /// #2: plain HTTP only to a private or loopback address written as one; a name, or a public address, needs HTTPS.
    #[test]
    fn plain_http_goes_only_to_a_private_address() {
        for ok in ["http://127.0.0.1:8787", "http://192.168.1.5", "http://10.0.0.2:80", "http://172.20.0.1", "http://[fd00::1]:9"] { assert!(HubHttp::new(ok, "t").is_ok(), "{ok}"); }
        for no in ["http://hub.example.net", "http://localhost:8787", "http://8.8.8.8", "http://172.32.0.1", "http://[2001:db8::1]"] { assert!(HubHttp::new(no, "t").is_err(), "{no}"); }
        assert!(HubHttp::new("https://hub.example.net", "t").is_ok());
    }

    /// #2: the check the connection's peer goes through before anything is sent over plain HTTP.
    #[test]
    fn a_public_or_mapped_public_peer_is_not_private() {
        assert!(private_ip("127.0.0.1".parse().unwrap()) && !private_ip("93.184.216.34".parse().unwrap()) && !private_ip("::ffff:8.8.8.8".parse().unwrap()));
    }

    /// #8: a listing that goes back, or repeats, is not followed; the cursor only moves up and jumped slots are missed.
    #[test]
    fn the_cursor_only_moves_up() {
        assert_eq!(ahead(&json!({"mail": [{"seq": 5}, {"seq": 3}, {"seq": 5}, {"seq": 7}]}), 2), [5, 7]);
        let mut st = MailSt { down_seen: 4, ..Default::default() };
        st.take(6, Ok(json!({"kind": "snapshot", "hello": {"observed_at": 10}})));
        st.take(5, Ok(json!({"kind": "snapshot", "hello": {"observed_at": 1}})));
        assert_eq!((st.down_seen, st.missed, st.snapshot.as_ref().unwrap()["hello"]["observed_at"].as_i64()), (6, 1, Some(10)), "an older snapshot is not taken back");
        // one that does not open moves nothing the app keeps
        st.take(7, Err(anyhow!("changed")));
        assert_eq!((st.down_seen, st.inspected, st.dropped), (6, 7, 1));
    }

    /// #8: a snapshot that is not renewed, or a hub that stops answering, reads as stale.
    #[test]
    fn an_old_snapshot_or_a_silent_hub_is_stale() {
        let (now, t) = (100 * STALE_MS, Instant::now());
        let later = |ms: i64| t + Duration::from_millis(ms as u64);
        let mut st = MailSt::default();
        assert!(st.freshness(now, t).stale, "nothing yet");
        st.take(3, Ok(json!({"kind": "snapshot", "hello": {"observed_at": now - 1000}})));
        st.heard = Some(t);
        assert!(!st.freshness(now, t).stale);
        assert!(st.freshness(now, later(STALE_MS + 1000)).stale, "the hub went silent and no snapshot came");
        st.heard = Some(later(STALE_MS + 1000));
        assert!(st.freshness(now, later(STALE_MS + 1000)).stale, "the hub answers, but no snapshot came for twelve minutes");
        st.take(4, Ok(json!({"kind": "snapshot", "hello": {"observed_at": now - STALE_MS - 1}})));
        st.heard = Some(Instant::now());
        assert!(st.freshness(now, Instant::now()).stale, "the computer's own time says it is old");
    }

    /// #10: only answers waited for are kept; an answer nobody asks for is dropped.
    #[test]
    fn only_answers_waited_for_are_kept() {
        let mut ws = vec![Waiter { id: 1, key: "q:abc".into(), answer: None }];
        hand(&mut ws, &json!({"kind": "receipt", "action_id": "a", "query": "abc", "receipt": {}}));
        hand(&mut ws, &json!({"kind": "receipt", "action_id": "b", "receipt": {}}));
        assert_eq!(ws.len(), 1);
        assert!(ws[0].answer.is_some());
    }

    /// #13: numbers and deadlines from the hub are bounded.
    #[test]
    fn numbers_and_deadlines_from_the_hub_are_bounded() {
        assert_eq!(next_seq(4).unwrap(), 5);
        assert!(next_seq(i64::MAX).is_err() && next_seq(1 << 53).is_err() && next_seq(-5).is_err());
        let now = 1_000_000;
        assert_eq!(invite_deadline(i64::MAX, now), now + LONGEST_INVITE_MS);
        assert_eq!(invite_deadline(i64::MIN, now), now);
        assert_eq!(invite_deadline(now + 60_000, now), now + 60_000);
    }
}

#[cfg(test)]
mod hostile_hub {
    //! The phone against a hub that answers what it likes: a few lines of HTTP on loopback.

    use super::*;
    use std::sync::Mutex as StdMutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    type Answer = Arc<dyn Fn(&str, &str, &[u8]) -> (u16, Vec<u8>) + Send + Sync>;

    /// A hub answering each request with `f(method, path, body)`, and the paths it was asked for.
    async fn hub(f: Answer) -> (String, Arc<StdMutex<Vec<String>>>) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let asked = Arc::new(StdMutex::new(vec![]));
        let a2 = asked.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else { return };
                let (f, asked) = (f.clone(), a2.clone());
                tokio::spawn(async move {
                    let (mut buf, mut tmp) = (Vec::new(), [0u8; 8192]);
                    let end = loop {
                        let n = s.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 { return; }
                        buf.extend_from_slice(&tmp[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") { break i + 4; }
                    };
                    let head = String::from_utf8_lossy(&buf[..end]).to_string();
                    let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    while buf.len() < end + len { let n = s.read(&mut tmp).await.unwrap_or(0); if n == 0 { break; } buf.extend_from_slice(&tmp[..n]); }
                    let mut w = head.split_whitespace();
                    let (m, p) = (w.next().unwrap_or("").to_string(), w.next().unwrap_or("").to_string());
                    asked.lock().unwrap().push(format!("{m} {p}"));
                    let (st, body) = f(&m, &p, &buf[end..]);
                    let _ = s.write_all(format!("HTTP/1.1 {st} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len()).as_bytes()).await;
                    let _ = s.write_all(&body).await;
                });
            }
        });
        (format!("http://{addr}"), asked)
    }

    fn ok(v: Value) -> (u16, Vec<u8>) { (200, v.to_string().into_bytes()) }

    /// A phone, the computer it pairs with, and a mailbox past the pairing's slots on a hub answering `f`.
    async fn mailbox(f: Answer) -> (Mailbox, PhoneIdentity, PhoneIdentity, Arc<StdMutex<Vec<String>>>) {
        let (me, desk) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
        let (url, asked) = hub(f).await;
        let m = Mailbox::new(&me, &url, "tok", "ph-1", &desk.pubkey(), &desk.x25519_public()).unwrap();
        m.resume_from(2).await;
        (m, me, desk, asked)
    }

    /// P1: a page that says there is more and holds nothing new ends the look instead of asking for ever.
    #[tokio::test]
    async fn a_page_that_makes_no_progress_ends_the_look() {
        let (m, _, _, _) = mailbox(Arc::new(|_, _, _| ok(json!({"max": 5, "mail": [], "more": true})))).await;
        let r = tokio::time::timeout(Duration::from_secs(10), m.sync()).await;
        assert!(r.is_ok(), "sync never returned");
    }

    /// P1: a slot that does not open, or a number out of range, never moves the cursor the app keeps.
    #[tokio::test]
    async fn a_slot_that_does_not_open_does_not_move_the_cursor() {
        let (m, _, _, asked) = mailbox(Arc::new(|_, p, _| {
            if p.contains("?after=") { return ok(json!({"max": 9, "mail": [{"seq": 1i64 << 60}, {"seq": 9}], "more": false})); }
            (200, b"{\"v\":1}".to_vec())
        })).await;
        m.sync().await.unwrap();
        assert_eq!(m.down_seen().await, 2, "the app's cursor stayed below what did not authenticate");
        assert_eq!(m.dropped().await, 1);
        assert!(!asked.lock().unwrap().iter().any(|a| a.contains(&(1i64 << 60).to_string())), "a number out of range is not asked for");
    }

    /// P2: up slot 3 is the pairing's acknowledgement: the mailbox's own messages start at 4.
    #[tokio::test]
    async fn the_mailbox_leaves_up_slot_3_to_the_acknowledgement() {
        let (m, _, _, asked) = mailbox(Arc::new(|meth, p, _| if meth == "PUT" { (204, vec![]) } else if p.contains("/up?") { ok(json!({"max": 2, "mail": [], "more": false})) } else { ok(json!({})) })).await;
        m.send(&json!({"kind": "receipt", "action_id": "a"})).await.unwrap();
        let puts: Vec<String> = asked.lock().unwrap().iter().filter(|a| a.starts_with("PUT")).cloned().collect();
        assert_eq!(puts, ["PUT /v1/mail/ph-1/up/4"]);
    }

    /// P2: an `act` given up (the app moved on) leaves no waiter behind.
    #[tokio::test]
    async fn a_cancelled_call_leaves_no_waiter() {
        let (m, _, _, _) = mailbox(Arc::new(|meth, _, _| if meth == "PUT" { (204, vec![]) } else { ok(json!({"max": 3, "mail": [], "more": false})) })).await;
        let a: Action = serde_json::from_value(json!({"action_id": "a", "kind": "answer", "target": {"run": "r", "ask_id": "q"}, "state_version": 1, "expires_at": 9, "payload": {}})).unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(500), m.act(&a)).await.is_err());
        assert_eq!(m.waiters.lock().unwrap().len(), 0);
    }

    /// P2: two calls for the same action each get the computer's answer.
    #[tokio::test]
    async fn two_calls_for_one_action_both_get_the_answer() {
        let (me, desk) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
        let receipt = json!({"kind": "receipt", "action_id": "a", "receipt": {"action_id": "a", "status": "accepted", "detail": "", "at": 1}});
        let sealed = seal_mail(desk.signing_key(), &me.x25519_public(), "ph-1", "down", 3, &receipt).unwrap();
        let put = Arc::new(StdMutex::new(false));
        let (url, _) = hub(Arc::new(move |meth, p, _| {
            if meth == "PUT" { *put.lock().unwrap() = true; return (204, vec![]); }
            if p.contains("/up?") { return ok(json!({"max": 3, "mail": [], "more": false})); }
            if p.contains("/down?") { return ok(if *put.lock().unwrap() { json!({"max": 3, "mail": [{"seq": 3}], "more": false}) } else { json!({"max": 2, "mail": [], "more": false}) }); }
            (200, sealed.clone())
        })).await;
        let m = Mailbox::new(&me, &url, "tok", "ph-1", &desk.pubkey(), &desk.x25519_public()).unwrap();
        m.resume_from(2).await;
        let a: Action = serde_json::from_value(json!({"action_id": "a", "kind": "answer", "target": {"run": "r", "ask_id": "q"}, "state_version": 1, "expires_at": 9, "payload": {}})).unwrap();
        let (r1, r2) = tokio::time::timeout(Duration::from_secs(30), async { tokio::join!(m.act(&a), m.act(&a)) }).await.unwrap();
        assert_eq!((r1.unwrap().status, r2.unwrap().status), (crate::api::Status::Accepted, crate::api::Status::Accepted));
    }

    /// #5: the computer's key is the one in its certificate's public-key field, not bytes found anywhere in it.
    #[test]
    fn the_key_must_be_the_certificates_own() {
        let (x, k) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params.serial_number = Some(rcgen::SerialNumber::from(1u64));
        let kp = unb64(&k.pubkey()).unwrap();
        params.custom_extensions.push(rcgen::CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 55555, 1], [ED25519_SPKI.as_slice(), &kp].concat()));
        let cert = params.self_signed(&crate::identity::Ed(x.signing_key().clone())).unwrap();
        let desk = |pubkey: String, cert_pem: String| DesktopPayload { id: "d".into(), name: "D".into(), cert_pem, pubkey, address_hints: vec![], hub_url: None, hub_token: None, grants: crate::api::Grants::default(), x25519: None };
        let sig = b64(&k.signing_key().sign(b"binding").to_bytes());
        assert!(!approval_ok(&desk(k.pubkey(), cert.pem()), cert.der(), b"binding", &sig), "K is only in an extension of X's certificate");
        let (pem, der) = k.cert().unwrap();
        assert!(approval_ok(&desk(k.pubkey(), pem), &der, b"binding", &sig));
    }

    /// P2: the computer's clock an hour ahead, and a hub that answers every look but withholds the snapshots: stale
    /// once twelve minutes passed here.
    #[test]
    fn a_computer_clock_ahead_does_not_hide_a_withheld_snapshot() {
        let (now, t) = (100 * STALE_MS, Instant::now());
        let later = t + Duration::from_secs(13 * 60);
        let mut st = MailSt::default();
        st.take(3, Ok(json!({"kind": "snapshot", "hello": {"observed_at": now + 3_600_000}})));
        st.heard = Some(later);
        assert!(st.freshness(now + 13 * 60_000, later).stale);
    }

    /// P2: two waiters for the same action: one finishing does not take the other's answer.
    #[test]
    fn one_waiter_finishing_keeps_the_others_answer() {
        let ws = std::sync::Mutex::new(vec![Waiter { id: 1, key: "act:a".into(), answer: None }, Waiter { id: 2, key: "act:a".into(), answer: None }]);
        hand(&mut ws.lock().unwrap(), &json!({"kind": "receipt", "action_id": "a", "receipt": {}}));
        drop(Leave(&ws, 1));
        assert!(ws.lock().unwrap().iter().find(|w| w.id == 2).is_some_and(|w| w.answer.is_some()), "the second call lost its answer");
    }
}
