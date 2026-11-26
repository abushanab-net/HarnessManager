//! The phones paired through the owner's hub, from this computer's side (docs/plans/mobile/PLAN.md, "the hub, for
//! phones"). Pairing: this computer invites a phone on the hub (a one-use code and the phone's mailbox there), the
//! phone runs the same SPAKE2 exchange as on the LAN through its mailbox, the owner sees its name and the three words
//! and approves or refuses here; nothing is sealed back before the approval, and the approval is the one direct
//! pairing makes (the phone stored with its grants as a new enrollment). Then the mailbox pass: every message the
//! phone left is opened (sealed to this computer by the phone's key, for that very slot, and never read twice) and
//! goes through the companion API's own entry point and guards (`actions::act` behind the enrollment and Control
//! checks `/c1` makes); the answers and a snapshot of what `/c1` shows (hello, overview, accounts, requests, as the
//! grants allow) are sealed to the phone, the snapshot after each change. The hub sees sizes and times only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use companion_proto::api::{Action, Grants};
use companion_proto::mailbox::{hub_qr, open_mail, seal_mail};
use companion_proto::pairing::{self, DesktopPayload, PhonePayload};
use companion_proto::Keyed;
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::phones::{self, Phone};
use crate::sync::hub::{self, CtxOf, Hub};
use crate::sync::peer::{identity, OnEvent};
use crate::util::now_ms;
use crate::Ctx;

/// The most of one mailbox message read: the hub keeps none bigger.
const MAIL_MAX: u64 = 1024 * 1024;
/// A snapshot is sent again when nothing changed for this long: the phone sees the computer is still there.
const REPUBLISH_MS: i64 = 5 * 60_000;
/// How long a decided pairing waits for the phone to read the decision after the invite's time is up.
const RESULT_KEPT_MS: i64 = 30_000;

/// This computer's keys for the mailbox: its Ed25519 identity signs, and an X25519 key derived from the same seed
/// is what phones seal to (so it needs no secret of its own).
pub(crate) struct Keys {
    pub key: SigningKey,
    pub x_secret: [u8; 32],
    pub x_public: String,
}

pub(crate) fn keys(ctx: &Ctx) -> Result<Keys> {
    let id = identity::load(ctx)?;
    let x_secret: [u8; 32] = Sha256::new().chain_update(b"hm desktop x25519 v1").chain_update(id.key.to_bytes()).finalize().into();
    Ok(Keys { x_public: companion_proto::seal::x25519_public(&x_secret), key: id.key, x_secret })
}

// ---------- the mailbox pass ----------

static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Mail came: the pass looks now rather than at its next tick.
pub fn wake() {
    *WAKE.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
    WAKE.1.notify_all();
}

/// Every two seconds (at once when the hub says mail came), while the hub is this computer's transport: the pass.
pub fn watch(ctx_of: CtxOf) {
    std::thread::spawn(move || {
        let mut last_err = String::new();
        loop {
            {
                let g = WAKE.0.lock().unwrap_or_else(|e| e.into_inner());
                let (mut g, _) = WAKE.1.wait_timeout_while(g, Duration::from_secs(2), |woken| !*woken).unwrap_or_else(|e| e.into_inner());
                *g = false;
            }
            let ctx = ctx_of();
            if ctx.settings.sync_mode != "hub" || hub::token(&ctx).is_none() { continue; }
            // a hub that is down says so once, not every two seconds
            match pass(&ctx) {
                Ok(()) => last_err.clear(),
                Err(e) => { let m = format!("{e:#}"); if m != last_err { crate::log::warn(&format!("phones: the hub mailbox: {m}")); last_err = m; } }
            }
        }
    });
}

/// A phone paired through the hub, as the pass reads it.
struct Paired {
    phone: Phone,
    mailbox: String,
    pubkey: String,
}

/// What each mailbox was last sent: the change stamp and when.
static SENT: LazyLock<Mutex<HashMap<String, (String, i64)>>> = LazyLock::new(Default::default);

/// One look at every phone paired through the hub: its new messages, then its snapshot when something changed; and a
/// revoked phone's mailbox deleted on the hub where that was not acknowledged yet.
pub fn pass(ctx: &Ctx) -> Result<()> {
    let (rows, gone) = ctx.db.read(|c| {
        let rows: Vec<(String, String, String)> = c.prepare("select phone_id, hub_mailbox, coalesce(pubkey, '') from phones where hub_mailbox is not null and revoked_at is null")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let gone: Vec<(String, String)> = c.prepare("select phone_id, hub_mailbox from phones where hub_mailbox is not null and revoked_at is not null and hub_gone is null")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok((rows, gone))
    })?;
    if rows.is_empty() && gone.is_empty() { return Ok(()); }
    let hub = Hub::open(ctx)?;
    for (id, mb) in gone { if hub.delete(&format!("/v1/phones/{mb}")).is_ok() { phones::hub_gone(ctx, &id); } }
    if rows.is_empty() { return Ok(()); }
    let me = keys(ctx)?;
    let mut first_err = None;
    for (id, mailbox, pubkey) in rows {
        let Some(phone) = phones::get(ctx, &id)? else { continue };
        let p = Paired { phone, mailbox, pubkey };
        if let Err(e) = consume(ctx, &hub, &me, &p).and_then(|_| publish(ctx, &hub, &me, &p)) { first_err.get_or_insert(e); }
    }
    first_err.map_or(Ok(()), Err)
}

/// The phone's new messages, each opened and answered. A message is marked read before it is acted on: whatever
/// happens next, it is never acted on twice (the phone asks for the receipt instead).
fn consume(ctx: &Ctx, hub: &Hub, me: &Keys, p: &Paired) -> Result<()> {
    let k = format!("mail:{}:up", p.mailbox);
    let last: i64 = ctx.db.kv_get(&k)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let list = hub.get_json(&format!("/v1/mail/{}/up?after={last}", p.mailbox))?.unwrap_or(Value::Null);
    for seq in list["mail"].as_array().into_iter().flatten().filter_map(|m| m["seq"].as_i64()).filter(|s| *s > last) {
        let Some(bytes) = hub.get_opt_bytes(&format!("/v1/mail/{}/up/{seq}", p.mailbox), MAIL_MAX)? else { continue };
        ctx.db.kv_set(&k, &seq.to_string())?;
        let body = match open_mail(&me.x_secret, &p.pubkey, &p.mailbox, "up", seq, &bytes) {
            Ok(b) => b,
            Err(e) => { crate::log::warn(&format!("phones: a message for {} that does not open was dropped ({e:#})", p.phone.name)); continue; }
        };
        if let Some(reply) = handle(ctx, &p.phone, &body) { send(hub, me, p, &reply)?; }
    }
    Ok(())
}

fn refused(action_id: &str, status: u16, error: &str) -> Value { json!({"kind": "refused", "action_id": action_id, "status": status, "error": error}) }

/// One message from the phone: what `/c1` would answer it. The enrollment is read again here, as `/c1` reads it on
/// every request: a phone revoked or paired again since gets nothing.
fn handle(ctx: &Ctx, at_pass: &Phone, body: &Value) -> Option<Value> {
    let p = phones::get(ctx, &at_pass.id).ok().flatten().filter(|p| p.revoked_at.is_none() && p.generation == at_pass.generation)?;
    let control = (!p.grants.control).then_some("Control is not granted to this phone");
    match body["kind"].as_str()? {
        "act" => {
            let id = body["action"]["action_id"].as_str().unwrap_or("").to_string();
            if let Some(no) = control { return Some(refused(&id, 403, no)); }
            let a: Action = match serde_json::from_value(body["action"].clone()) {
                Ok(a) if !id.is_empty() && id.len() <= 100 => a,
                _ => return Some(refused(&id, 400, "an action needs its action_id")),
            };
            Some(match super::actions::act(ctx, &p, &a) {
                Ok(r) => json!({"kind": "receipt", "action_id": id, "receipt": super::stamped(ctx, json!(r))}),
                Err(e) => refused(&id, 500, &format!("{e:#}")),
            })
        }
        "receipt" => {
            let id = body["action_id"].as_str().unwrap_or("").to_string();
            if let Some(no) = control { return Some(refused(&id, 403, no)); }
            Some(match super::actions::stored(ctx, &id) {
                Ok(Some((phone, generation, r))) if phone == p.id && generation == p.generation => json!({"kind": "receipt", "action_id": id, "receipt": super::stamped(ctx, json!(r))}),
                Ok(_) => refused(&id, 404, "no such action"),
                Err(e) => refused(&id, 500, &format!("{e:#}")),
            })
        }
        _ => None,
    }
}

/// `body` sealed to the phone in the next free slot down.
fn send(hub: &Hub, me: &Keys, p: &Paired, body: &Value) -> Result<()> {
    for _ in 0..3 {
        let max = hub.get_json(&format!("/v1/mail/{}/down?after={}", p.mailbox, i64::MAX))?.and_then(|v| v["max"].as_i64()).unwrap_or(0);
        let m = seal_mail(&me.key, &p.phone.x25519, &p.mailbox, "down", max + 1, body)?;
        match hub.put(&format!("/v1/mail/{}/down/{}", p.mailbox, max + 1), m, &[]) {
            // another process of this computer took the slot first
            Err(e) if format!("{e:#}").contains("answered 409") => continue,
            r => return r,
        }
    }
    bail!("the mailbox of {} stayed busy", p.phone.name)
}

/// What `/c1` shows this phone now, as one message: hello always; the overview, accounts and requests with Summaries
/// (Claude's last line before a request with Transcripts too), else why not.
fn snapshot(ctx: &Ctx, p: &Phone) -> Result<Value> {
    let mut v = json!({"kind": "snapshot", "hello": super::hello_of(ctx, p), "overview": null, "accounts": null, "requests": null});
    if p.grants.summaries {
        v["overview"] = serde_json::to_value(super::overview_of(ctx)?)?;
        v["accounts"] = super::stamped(ctx, json!({"accounts": super::accounts_of(ctx, None)?}));
        v["requests"] = super::stamped(ctx, json!({"requests": super::requests_of(ctx, p.grants.transcripts)?, "actions": {"answer": super::may_answer(p)}}));
    } else {
        v["refused"] = json!("Summaries is not granted to this phone");
    }
    Ok(v)
}

/// The snapshot, when what it shows changed since the last one sent (the requests, the runs, usage and limits, the
/// grants), or that one is five minutes old.
fn publish(ctx: &Ctx, hub: &Hub, me: &Keys, p: &Paired) -> Result<()> {
    let stamp = format!("{:?}|{:?}|{}", super::events::look(ctx, &p.phone), p.phone.grants, p.phone.generation);
    let now = now_ms();
    if SENT.lock().unwrap_or_else(|e| e.into_inner()).get(&p.mailbox).is_some_and(|(s, at)| *s == stamp && now - at < REPUBLISH_MS) { return Ok(()); }
    send(hub, me, p, &snapshot(ctx, &p.phone)?)?;
    SENT.lock().unwrap_or_else(|e| e.into_inner()).insert(p.mailbox.clone(), (stamp, now));
    Ok(())
}

// ---------- pairing through the hub ----------

#[derive(Default)]
struct PairSt {
    /// "waiting", "confirm", "paired", "refused", "expired", "failed", or "cancelled" (shown to nobody).
    state: &'static str,
    /// The phone's id and the key from its first message.
    phone_id: Option<String>,
    key: Option<Keyed>,
    phone: Option<PhonePayload>,
    error: Option<String>,
    /// The decision for the phone, and whether it is on the hub yet; whether the phone read it.
    result: Option<Value>,
    delivered: bool,
    fetched: bool,
}

struct Shared {
    ctx: Arc<Ctx>,
    code: String,
    mailbox: String,
    expires_at: i64,
    /// `{attempt, route: "hub", code, expires_at, hub_url, qr}`.
    shown: Value,
    st: Mutex<PairSt>,
    on_event: OnEvent,
    ended: AtomicBool,
}

/// A phone pairing through the hub: the same states, words, grants and `attempt` as a direct one
/// (`Runtime::phone_pair_*`), the invite shown instead of the LAN code.
pub struct HubPairing {
    shared: Arc<Shared>,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, PairSt> { self.st.lock().unwrap_or_else(|e| e.into_inner()) }

    fn status_of(&self, st: &PairSt) -> Value {
        let mut v = self.shown.clone();
        v["state"] = json!(st.state);
        if let Some(p) = &st.phone { v["phone"] = json!({"name": p.name, "platform": p.platform}); }
        if let (Some(k), Some(_)) = (&st.key, &st.phone) { v["words"] = json!(k.words()); }
        if let Some(e) = &st.error { v["error"] = json!(e); }
        if matches!(st.state, "paired" | "refused") { v["fetched"] = json!(st.fetched); }
        v
    }

    /// A change of state, told as `phones-pairing`; false when the pairing is not at one of `from` any more.
    fn set(&self, from: &[&str], f: impl FnOnce(&mut PairSt)) -> bool {
        let v = {
            let mut st = self.lock();
            if !from.contains(&st.state) { return false; }
            f(&mut st);
            self.status_of(&st)
        };
        (self.on_event)("phones-pairing", v);
        true
    }

    fn mail(&self, dir: &str, seq: i64) -> String { format!("/v1/mail/{}/{dir}/{seq}", self.mailbox) }

    /// Waiting: the phone's first message answered with this computer's half of the exchange, then its sealed
    /// payload opened, which puts the pairing before the owner. A payload that does not open (another code) fails it.
    fn step(&self, hub: &Hub) -> Result<()> {
        if self.lock().key.is_none() {
            let Some(b) = hub.get_opt_bytes(&self.mail("up", 1), MAIL_MAX)? else { return Ok(()) };
            let v: Value = serde_json::from_slice(&b)?;
            let phone_id = v["id"].as_str().filter(|s| !s.is_empty()).context("the phone's first message names no phone")?.to_string();
            let (key, back) = pairing::answer(&self.code, &phone_id, &self.ctx.settings.device_id, v["msg"].as_str().unwrap_or(""))?;
            hub.put(&self.mail("down", 1), json!({"kind": "pair-start", "msg": back}).to_string().into_bytes(), &[])?;
            let mut st = self.lock();
            (st.phone_id, st.key) = (Some(phone_id), Some(key));
        }
        let Some(b) = hub.get_opt_bytes(&self.mail("up", 2), MAIL_MAX)? else { return Ok(()) };
        let v: Value = serde_json::from_slice(&b)?;
        let opened = {
            let st = self.lock();
            st.key.as_ref().map(|k| k.open::<PhonePayload>(v["payload"].as_str().unwrap_or(""))).transpose().map(|p| (p, st.phone_id.clone()))
        };
        match opened {
            Ok((Some(p), Some(id))) if p.id == id && identity::pem_to_der(&p.cert_pem).is_ok() => {
                let (name, platform) = (p.name.clone(), p.platform.clone());
                if self.set(&["waiting"], |st| { st.state = "confirm"; st.phone = Some(p); }) { crate::log::info(&format!("phones: {name} ({platform}) waits for approval through the hub")); }
            }
            _ => { self.set(&["waiting"], |st| { st.state = "failed"; st.error = Some("the phone's payload did not open; show a new invite".into()); }); }
        }
        Ok(())
    }

    /// The decision put on the hub once, then the phone's word that it read it.
    fn deliver(&self, hub: &Hub) -> Result<()> {
        let (result, delivered, fetched) = { let st = self.lock(); (st.result.clone(), st.delivered, st.fetched) };
        if let (Some(r), false) = (&result, delivered) {
            match hub.put(&self.mail("down", 2), r.to_string().into_bytes(), &[]) {
                Err(e) if !format!("{e:#}").contains("answered 409") => return Err(e),
                _ => self.lock().delivered = true,
            }
        }
        if (result.is_some() && !fetched) && hub.get_opt_bytes(&self.mail("up", 3), MAIL_MAX)?.is_some() {
            self.set(&["paired", "refused"], |st| st.fetched = true);
        }
        Ok(())
    }

    /// Until the phone read the decision, or the invite's time (and a moment more for a decision) is up. An invite that
    /// ends unpaired leaves nothing on the hub.
    fn run(self: Arc<Shared>) {
        let hub = match Hub::open(&self.ctx) { Ok(h) => h, Err(e) => { self.set(&["waiting"], |st| { st.state = "failed"; st.error = Some(format!("{e:#}")); }); return; } };
        let mut last_err = String::new();
        loop {
            let state = self.lock().state;
            let r = match state {
                "waiting" => self.step(&hub),
                "paired" | "refused" => self.deliver(&hub),
                _ => Ok(()),
            };
            if let Err(e) = r { let m = format!("{e:#}"); if m != last_err { crate::log::warn(&format!("phones: pairing through the hub: {m}")); last_err = m; } }
            let now = now_ms();
            if now >= self.expires_at { self.set(&["waiting", "confirm"], |st| { st.state = "expired"; st.error = Some("the invite expired".into()); }); }
            let (state, fetched) = { let st = self.lock(); (st.state, st.fetched) };
            let decided = matches!(state, "paired" | "refused");
            if (decided && fetched) || (!decided && now >= self.expires_at) || matches!(state, "failed" | "cancelled") || now >= self.expires_at + RESULT_KEPT_MS { break; }
            std::thread::sleep(Duration::from_millis(500));
        }
        if self.lock().state != "paired" { let _ = hub.delete(&format!("/v1/phones/{}", self.mailbox)); }
        self.ended.store(true, Ordering::SeqCst);
    }
}

/// Invite a phone through the hub for ten minutes: `{attempt, route: "hub", code, expires_at, hub_url, qr, state}`; the
/// QR code says `hm-hub:1?c=<code>&f=<this computer's certificate fingerprint>&u=<hub url>`. Every change of state is
/// told as `phones-pairing`, as a direct pairing's.
pub fn pair_start(ctx: Arc<Ctx>, on_event: OnEvent) -> Result<HubPairing> {
    let id = identity::load(&ctx)?;
    let inv = hub::phone_invite(&ctx)?;
    let (code, mailbox, expires_at) = (inv["code"].as_str().unwrap_or("").to_string(), inv["phone_id"].as_str().unwrap_or("").to_string(), inv["expires_at"].as_i64().unwrap_or(0));
    if code.is_empty() || mailbox.is_empty() { bail!("the hub answered without an invite"); }
    let qr = hub_qr(&ctx.settings.hub_url, &code, &pairing::fingerprint(&id.cert_der));
    let shown = json!({"attempt": crate::util::uuid4(), "route": "hub", "code": code, "expires_at": expires_at, "hub_url": ctx.settings.hub_url, "qr": qr});
    let shared = Arc::new(Shared { ctx, code, mailbox, expires_at, shown, st: Mutex::new(PairSt { state: "waiting", ..Default::default() }), on_event, ended: AtomicBool::new(false) });
    let s = shared.clone();
    std::thread::spawn(move || s.run());
    Ok(HubPairing { shared })
}

impl HubPairing {
    pub fn status(&self) -> Value { let st = self.shared.lock(); self.shared.status_of(&st) }

    pub fn attempt(&self) -> &str { self.shared.shown["attempt"].as_str().unwrap_or("") }

    /// The pairing is over here: the phone read the decision, or the invite's time ran out.
    pub fn ended(&self) -> bool { self.shared.ended.load(Ordering::SeqCst) }

    /// The owner approved the phone shown, with these grants: it is stored as a new enrollment with its mailbox, and
    /// this computer's payload (its certificate, its keys, its direct address when it has one) is sealed for it.
    /// Decided under the pairing's lock, with the deadline, as a direct approval.
    pub fn approve(&self, grants: Grants) -> Result<Value> {
        let s = &self.shared;
        let ctx = &s.ctx;
        let v = {
            let mut st = s.lock();
            if st.state != "confirm" { bail!("no phone waits for approval"); }
            if now_ms() >= s.expires_at {
                st.state = "expired";
                st.error = Some("the invite expired".into());
                let v = s.status_of(&st);
                drop(st);
                (s.on_event)("phones-pairing", v);
                bail!("the invite expired before the approval; show a new one");
            }
            let (Some(phone), Some(key)) = (st.phone.clone(), st.key.as_ref()) else { bail!("no phone waits for approval") };
            let id = identity::load(ctx)?;
            // the port the listener holds, as saved (another process may hold it); none without the direct connection
            let port = Ctx::open_at(&ctx.data, &ctx.home).map(|c| if c.settings.direct_connection { c.settings.listen_port } else { 0 }).unwrap_or(0);
            let address_hints = if port == 0 { vec![] } else { vec![format!("{}:{port}", crate::util::hostname())] };
            let p = DesktopPayload { id: ctx.settings.device_id.clone(), name: ctx.settings.device_name.clone(), cert_pem: id.cert_pem.clone(), pubkey: id.pubkey(), address_hints,
                hub_url: Some(ctx.settings.hub_url.clone()), hub_token: None, grants, x25519: Some(keys(ctx)?.x_public) };
            let sealed = key.seal(&p)?;
            phones::add_with(ctx, &phone, grants, Some(&s.mailbox))?;
            // the pairing's own two messages are not the mailbox pass's
            ctx.db.kv_set(&format!("mail:{}:up", s.mailbox), "2")?;
            st.state = "paired";
            st.result = Some(json!({"kind": "pair-result", "state": "approved", "payload": sealed}));
            s.status_of(&st)
        };
        (s.on_event)("phones-pairing", v.clone());
        Ok(v)
    }

    /// The owner refused the phone, or stopped waiting for one.
    pub fn refuse(&self) -> Result<Value> {
        if !self.shared.set(&["waiting", "confirm"], |st| { st.state = "refused"; st.result = Some(json!({"kind": "pair-result", "state": "refused"})); }) { bail!("the pairing already ended"); }
        Ok(self.status())
    }

    /// Stop showing the invite; the attempt ends without an event, and its mailbox goes from the hub unless paired.
    pub fn cancel(&self) { let mut st = self.shared.lock(); if st.state != "paired" { st.state = "cancelled"; } }
}
