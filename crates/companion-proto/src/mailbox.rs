//! The phone away from home: the owner's hub carries numbered messages between the phone and one computer
//! (`/v1/mail/{phone}/up|down/{seq}`, docs/plans/mobile/PLAN.md, "the hub, for phones"). A computer on the hub
//! invites the phone with a one-use code; the phone redeems it for a token that reaches only its own mailbox and
//! pairs through it as it would on the LAN: SPAKE2 with the code, the three words, and nothing sealed back before the
//! owner approves on the computer. From then on every message is sealed to its recipient and signed by its sender
//! (`seal`), and names its mailbox, direction and number, so the hub sees only sizes and times and cannot replay or
//! move one. The computer posts snapshots (hello, overview, accounts, requests) after each change; the phone posts
//! the same action bodies as `POST /c1/actions` and the receipts come back the same way. `Mailbox` answers the same
//! typed calls as the direct `Client`, so the app can switch routes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use ed25519_dalek::SigningKey;
use http_body_util::Full;
use rustls::pki_types::ServerName;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::api::{AccountOne, Accounts, Action, Computer, ConnectError, Hello, Overview, Receipt, Refused, Requests};
use crate::join::Outcome;
use crate::pairing::{self, DesktopPayload, PhonePayload};
use crate::seal::{open_from, seal_to, Sealed};
use crate::{net, Keyed, Pairing, PhoneIdentity};

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

// ---------- the hub over HTTP ----------

/// The hub at its address: plain HTTP on a private network, else TLS checked against the public roots (Caddy in
/// front of it holds a public certificate). Each call on a connection of its own: messages are few.
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
            _ => bail!("the hub address starts with https:// (or http:// on a private network)"),
        };
        let (authority, prefix) = match rest.find('/') { Some(i) => (&rest[..i], rest[i..].to_string()), None => (rest, String::new()) };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if (!h.contains(':') || h.ends_with(']')) && p.parse::<u16>().is_ok() => (h, p.parse()?),
            _ => (authority, if https { 443 } else { 80 }),
        };
        if host.is_empty() { bail!("the hub address has no host"); }
        let tls = https.then(|| -> Result<Arc<rustls::ClientConfig>> {
            let mut roots = rustls::RootCertStore::empty();
            roots.add_parsable_certificates(webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().cloned());
            Ok(Arc::new(rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider())).with_safe_default_protocol_versions()?
                .with_root_certificates(roots).with_no_client_auth()))
        }).transpose()?;
        Ok(HubHttp { host: host.trim_matches(['[', ']']).to_string(), port, authority: authority.to_string(), prefix, token: token.to_string(), tls })
    }

    async fn call_once(&self, method: &str, path: &str, body: Option<Vec<u8>>) -> Result<(u16, Bytes)> {
        let tcp = net::tcp(&self.host, self.port).await?;
        let mut s = match &self.tls {
            Some(cfg) => {
                let name = ServerName::try_from(self.host.clone()).map_err(|_| anyhow!("{} is not a host name", self.host))?;
                let t = tokio::time::timeout(net::CONNECT, tokio_rustls::TlsConnector::from(cfg.clone()).connect(name, tcp)).await.map_err(|_| anyhow!("the hub did not finish the handshake"))??;
                net::handshake(t).await?
            }
            None => net::handshake(tcp).await?,
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

    /// `{max, mail: [{seq, bytes, at}]}` after `after`.
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

/// The phone's payload is in the mailbox; the owner sees the three words and decides on the computer.
pub struct HubJoining {
    hub: HubHttp,
    me: PhoneIdentity,
    fingerprint: Option<String>,
    key: Keyed,
    expires_at: i64,
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
    hub.put_mail(&mailbox, "up", 1, json!({"kind": "pair-start", "id": me.id, "msg": msg}).to_string().into_bytes()).await?;
    let until = Instant::now() + PAIR_ANSWER;
    let back = loop {
        if let Some(b) = hub.get_mail(&mailbox, "down", 1).await? { break serde_json::from_slice::<Value>(&b)?; }
        if Instant::now() >= until { bail!("the computer did not answer through the hub; is it on and showing the invite?"); }
        tokio::time::sleep(POLL).await;
    };
    let key = p.finish(back["msg"].as_str().unwrap_or(""))?;
    let mine = key.seal(&PhonePayload::of(me, name, platform)?)?;
    hub.put_mail(&mailbox, "up", 2, json!({"kind": "pair-finish", "payload": mine}).to_string().into_bytes()).await?;
    Ok(HubJoining { hub, me: me.clone(), fingerprint: fingerprint.map(String::from), key, expires_at: r["expires_at"].as_i64().unwrap_or(0), hub_url: hub_url.trim().trim_end_matches('/').to_string(),
        token, mailbox, desktop_id, desktop_name })
}

impl HubJoining {
    /// The three words to compare with the computer's screen.
    pub fn words(&self) -> [&'static str; 3] { self.key.words() }

    /// The owner's decision as it stands now. Once it is read the phone says so (slot 3), so the computer can stop
    /// showing the invite.
    pub async fn result(&self) -> Result<Outcome> {
        let got = match self.hub.get_mail(&self.mailbox, "down", 2).await {
            // the computer stopped showing the invite (its time ran out, or it was cancelled): the mailbox is gone
            Err(e) if matches!(e.downcast_ref::<ConnectError>(), Some(ConnectError::Revoked(_))) => return Ok(Outcome::Expired),
            r => r?,
        };
        let Some(b) = got else { return Ok(Outcome::Pending((self.expires_at - now_ms()).max(0))) };
        let r: Value = serde_json::from_slice(&b)?;
        Ok(match r["state"].as_str() {
            Some("approved") => {
                let p: DesktopPayload = self.key.open(r["payload"].as_str().context("the computer approved without its payload")?)?;
                let der = crate::identity::pem_to_der(&p.cert_pem)?;
                if self.fingerprint.as_deref().is_some_and(|f| f != pairing::fingerprint(&der)) { bail!("that is not the computer the invite named"); }
                if p.id != self.desktop_id { bail!("the computer changed during the pairing"); }
                let x = p.x25519.as_deref().context("the computer sent no key to seal to")?;
                // told once; a second look finds the slot taken, which is the same news
                if let Ok(m) = seal_mail(self.me.signing_key(), x, &self.mailbox, "up", 3, &json!({"kind": "fetched"})) { let _ = self.hub.put_mail(&self.mailbox, "up", 3, m).await; }
                Outcome::Approved(p)
            }
            Some("refused") => {
                let _ = self.hub.put_mail(&self.mailbox, "up", 3, json!({"kind": "fetched"}).to_string().into_bytes()).await;
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

/// The newest messages one look reads at most: older snapshots are superseded, and a receipt is asked for again.
const READ_AT_MOST: usize = 50;
/// How long an action or a receipt question waits for the computer's answer through the hub.
const ANSWER_WAIT: Duration = Duration::from_secs(20);

#[derive(Default)]
struct MailSt {
    down_seen: i64,
    snapshot: Option<Value>,
    /// The computer's answers by action id: `{kind: "receipt", receipt}` or `{kind: "refused", status, error}`.
    answers: HashMap<String, Value>,
    dropped: u32,
}

/// One paired computer through the hub.
pub struct Mailbox {
    hub: HubHttp,
    me: PhoneIdentity,
    mailbox: String,
    desk_pubkey: String,
    desk_x25519: String,
    st: Mutex<MailSt>,
}

impl Mailbox {
    /// The mailbox `mailbox` on the hub at `hub_url` with the phone's `token`, to the computer whose Ed25519 and
    /// X25519 keys the pairing gave (`DesktopPayload::pubkey`, `x25519`).
    pub fn new(me: &PhoneIdentity, hub_url: &str, token: &str, mailbox: &str, desk_pubkey: &str, desk_x25519: &str) -> Result<Mailbox> {
        if desk_x25519.is_empty() { bail!("the computer gave no key to seal to; pair through the hub again"); }
        Ok(Mailbox { hub: HubHttp::new(hub_url, token)?, me: me.clone(), mailbox: mailbox.into(), desk_pubkey: desk_pubkey.into(), desk_x25519: desk_x25519.into(), st: Mutex::new(MailSt::default()) })
    }

    /// The last message read from the computer, for the app to keep, and to start from again (`resume_from`).
    pub async fn down_seen(&self) -> i64 { self.st.lock().await.down_seen }

    pub async fn resume_from(&self, down_seen: i64) { self.st.lock().await.down_seen = down_seen; }

    /// Messages read that did not open (changed on the way, sealed by someone else, replayed): never used.
    pub async fn dropped(&self) -> u32 { self.st.lock().await.dropped }

    /// Read what the computer left since the last look.
    pub async fn sync(&self) -> Result<()> {
        let mut st = self.st.lock().await;
        let list = self.hub.list(&self.mailbox, "down", st.down_seen).await?;
        let seqs: Vec<i64> = list["mail"].as_array().into_iter().flatten().filter_map(|m| m["seq"].as_i64()).collect();
        for &seq in seqs.iter().skip(seqs.len().saturating_sub(READ_AT_MOST)) {
            let Some(bytes) = self.hub.get_mail(&self.mailbox, "down", seq).await? else { continue };
            st.down_seen = seq;
            match open_mail(&self.me.x25519_secret(), &self.desk_pubkey, &self.mailbox, "down", seq, &bytes) {
                Ok(body) => match body["kind"].as_str() {
                    Some("snapshot") => st.snapshot = Some(body),
                    Some("receipt" | "refused") => { if let Some(id) = body["action_id"].as_str() { st.answers.insert(id.to_string(), body.clone()); } }
                    _ => {}
                },
                Err(_) => st.dropped += 1,
            }
        }
        st.down_seen = st.down_seen.max(seqs.last().copied().unwrap_or(0));
        Ok(())
    }

    /// One message to the computer, sealed for the next free slot (another of this phone's takes the slot first:
    /// nothing was stored, so the next one is tried).
    async fn send(&self, body: &Value) -> Result<()> {
        for _ in 0..3 {
            let seq = self.hub.list(&self.mailbox, "up", i64::MAX).await?["max"].as_i64().unwrap_or(0) + 1;
            let m = seal_mail(self.me.signing_key(), &self.desk_x25519, &self.mailbox, "up", seq, body)?;
            match self.hub.put_mail(&self.mailbox, "up", seq, m).await {
                Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 409) => continue,
                r => return r,
            }
        }
        bail!("the mailbox is busy; try again")
    }

    /// The computer's answer about `action_id`, read as it comes.
    async fn answer_for(&self, action_id: &str) -> Result<Value> {
        let until = Instant::now() + ANSWER_WAIT;
        loop {
            self.sync().await?;
            if let Some(v) = self.st.lock().await.answers.get(action_id) { return Ok(v.clone()); }
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
        self.send(&json!({"kind": "act", "action": a})).await?;
        Self::receipt_of(&self.answer_for(&a.action_id).await?)
    }

    /// The stored receipt of an action; None when the computer never got it.
    pub async fn receipt(&self, action_id: &str) -> Result<Option<Receipt>> {
        let known = self.st.lock().await.answers.get(action_id).cloned();
        let v = match known { Some(v) => v, None => { self.send(&json!({"kind": "receipt", "action_id": action_id})).await?; self.answer_for(action_id).await? } };
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
}
