//! The phones paired through the owner's hub, from this computer's side (docs/plans/mobile/PLAN.md, "the hub, for
//! phones"). Pairing: this computer invites a phone on the hub (a one-use code and the phone's mailbox there), the
//! phone runs the same SPAKE2 exchange as on the LAN through its mailbox, the owner sees its name and the three words
//! and approves or refuses here; nothing is sealed back before the approval, and the approval is the one direct
//! pairing makes (the phone stored with its grants as a new enrollment), signed by this computer's identity over the
//! exchange, the mailbox, both ends' keys and the grants, and kept with the enrollment until it is on the hub. Slots
//! 1 and 2 down are the pairing's; nothing else goes down before the decision is there.
//!
//! Then the mailbox pass: each enrollment is read in one row (its mailbox, keys, grants and generation), and every
//! message is opened under that row's key and handled for that very enrollment. A message is claimed with the read
//! cursor in one transaction (numbers only upwards) and kept until it is answered, so a computer stopping on the way
//! hands it to the next pass, where the action contract's own rule (a reserved action reads `unknown`, never sent
//! again) holds. Messages go through the companion API's own entry point and guards (`actions::act` behind the
//! enrollment and Control checks `/c1` makes); the answers and a snapshot of what `/c1` shows (hello, overview,
//! accounts, requests, as the grants allow) are sealed to the phone. A snapshot is built under the grants read just
//! before it and not sent if they changed while it was built. Deletions owed on the hub are kept until it confirms
//! them. The hub sees sizes and times only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use companion_proto::api::{Action, Grants};
use companion_proto::mailbox::{ahead, approval_binding, fetched_refusal, hub_qr, next_seq, open_mail, seal_mail};
use companion_proto::pairing::{self, DesktopPayload, PhonePayload};
use companion_proto::Keyed;
use ed25519_dalek::SigningKey;
use rusqlite::{params, Connection, OptionalExtension};
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
/// The longest an invite is shown, whatever the hub says (it gives ten minutes).
const LONGEST_INVITE_MS: i64 = 15 * 60_000;
/// The messages one pass reads from one phone: a phone that sends many waits for the next passes, the others do not.
const PER_PASS: i64 = 20;

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

#[cfg(test)]
type Seam = std::cell::RefCell<Option<Box<dyn FnOnce()>>>;

#[cfg(test)]
thread_local! {
    /// Test seams: run once the pass read its enrollments; run once a snapshot is built, before it is sent; stop the
    /// pass just after a message is claimed.
    pub(crate) static AFTER_READ: Seam = std::cell::RefCell::new(None);
    pub(crate) static BEFORE_PUBLISH: Seam = std::cell::RefCell::new(None);
    pub(crate) static STOP_AFTER_CLAIM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn seam(s: &'static std::thread::LocalKey<Seam>) { if let Some(f) = s.with(|b| b.borrow_mut().take()) { f(); } }

// ---------- deletions owed on the hub ----------

/// A mailbox the hub at `hub_url` is to delete (`phone_id` and `generation` name the enrollment it belonged to, empty
/// for an invite never paired). Written in the caller's transaction; kept until that hub confirms.
pub(crate) fn delete_later(c: &Connection, hub_url: &str, mailbox: &str, phone_id: &str, generation: &str) -> rusqlite::Result<()> {
    c.execute("insert or ignore into phone_hub_deletes(hub_url, mailbox, phone_id, generation, at) values(?, ?, ?, ?, ?)", params![hub_url, mailbox, phone_id, generation, now_ms()]).map(|_| ())
}

/// The deletion owed for `mailbox` tried now: true once the hub confirmed it (gone, or never there). The enrollment
/// it was owed for is marked acknowledged, and only that one.
pub(crate) fn delete_now(ctx: &Ctx, mailbox: &str) -> bool {
    let row: Option<(String, String)> = ctx.db.read(|c| Ok(c.query_row("select phone_id, generation from phone_hub_deletes where hub_url = ? and mailbox = ?", params![ctx.settings.hub_url, mailbox],
        |r| Ok((r.get(0)?, r.get(1)?))).optional()?)).ok().flatten();
    let Some((phone_id, generation)) = row else { return false };
    if let Err(e) = Hub::open(ctx).and_then(|h| h.delete(&format!("/v1/phones/{mailbox}"))) {
        crate::log::warn(&format!("phones: the hub did not delete mailbox {mailbox} yet: {e:#}"));
        return false;
    }
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        tx.execute("delete from phone_hub_deletes where hub_url = ? and mailbox = ?", params![ctx.settings.hub_url, mailbox])?;
        tx.execute("update phones set hub_gone = ?4 where phone_id = ?1 and generation = ?2 and hub_mailbox = ?3 and revoked_at is not null and hub_gone is null", params![phone_id, generation, mailbox, now_ms()])?;
        tx.commit()?;
        Ok(())
    }).is_ok()
}

/// Every deletion owed on this computer's hub, tried again. One owed on a hub this computer left stays: no other hub
/// may be told.
fn retry_deletes(ctx: &Ctx) {
    let owed: Vec<String> = ctx.db.read(|c| Ok(c.prepare("select mailbox from phone_hub_deletes where hub_url = ?")?.query_map([&ctx.settings.hub_url], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).unwrap_or_default();
    for mb in owed { delete_now(ctx, &mb); }
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

/// One enrollment paired through the hub, as one row gave it: the phone, its mailbox, its Ed25519 key, and the signed
/// approval still owed there.
#[derive(Clone)]
struct Paired {
    phone: Phone,
    mailbox: String,
    pubkey: String,
    decision: Option<String>,
}

impl Paired {
    /// The same enrollment: its generation, mailbox and key.
    fn same(&self, o: &Paired) -> bool { self.phone.generation == o.phone.generation && self.mailbox == o.mailbox && self.pubkey == o.pubkey }
}

/// The enrollments paired through the hub (or the one of `only`), not revoked, each from one row.
fn enrolled(c: &Connection, only: Option<&str>) -> rusqlite::Result<Vec<Paired>> {
    c.prepare("select phone_id, coalesce(name, ''), coalesce(grants, ''), revoked_at, coalesce(x25519, ''), generation, hub_mailbox, coalesce(pubkey, ''), hub_pairing from phones
        where hub_mailbox is not null and revoked_at is null and (?1 is null or phone_id = ?1)")?
        .query_map([only], |r| Ok(Paired {
            phone: Phone { id: r.get(0)?, name: r.get(1)?, grants: phones::grants_of(&r.get::<_, String>(2)?), revoked_at: r.get(3)?, x25519: r.get(4)?, generation: r.get(5)? },
            mailbox: r.get(6)?, pubkey: r.get(7)?, decision: r.get(8)?,
        }))?.collect()
}

/// `p`'s enrollment as it stands now, when it is still that one.
fn reload(ctx: &Ctx, p: &Paired) -> Option<Paired> { ctx.db.read(|c| Ok(enrolled(c, Some(&p.phone.id))?)).ok()?.into_iter().next().filter(|n| n.same(p)) }

/// What each mailbox was last sent: the change stamp and when.
static SENT: LazyLock<Mutex<HashMap<String, (String, i64)>>> = LazyLock::new(Default::default);

/// One look at every phone paired through the hub: a decision still owed delivered first, then its new messages,
/// then its snapshot when something changed; and the deletions owed on the hub tried again.
pub fn pass(ctx: &Ctx) -> Result<()> {
    retry_deletes(ctx);
    let rows = ctx.db.read(|c| Ok(enrolled(c, None)?))?;
    if rows.is_empty() { return Ok(()); }
    let hub = Hub::open(ctx)?;
    let me = keys(ctx)?;
    #[cfg(test)]
    seam(&AFTER_READ);
    let mut first_err = None;
    for p in rows {
        let r = deliver_decision(ctx, &hub, &p).and_then(|_| consume(ctx, &hub, &me, &p)).and_then(|_| publish(ctx, &hub, &me, &p));
        if let Err(e) = r { first_err.get_or_insert(e); }
    }
    first_err.map_or(Ok(()), Err)
}

/// Down slot 2 holds the decision: put now, or found there already, byte for byte.
fn put_decision(hub: &Hub, mailbox: &str, decision: &str) -> Result<()> {
    let path = format!("/v1/mail/{mailbox}/down/2");
    match hub.put(&path, decision.as_bytes().to_vec(), &[]) {
        Err(e) if format!("{e:#}").contains("answered 409") => {
            if hub.get_opt_bytes(&path, MAIL_MAX)?.as_deref() != Some(decision.as_bytes()) { bail!("slot 2 of {mailbox} holds something other than the decision"); }
            Ok(())
        }
        r => r,
    }
}

/// The signed approval kept with the enrollment, delivered (after a restart too); until it is, nothing else goes down.
fn deliver_decision(ctx: &Ctx, hub: &Hub, p: &Paired) -> Result<()> {
    let Some(d) = &p.decision else { return Ok(()) };
    put_decision(hub, &p.mailbox, d)?;
    ctx.db.with(|c| Ok(c.execute("update phones set hub_pairing = null where hub_mailbox = ? and hub_pairing = ?", params![p.mailbox, d])?)).map(|_| ())
}

fn cursor_key(mailbox: &str) -> String { format!("mail:{mailbox}:up") }

/// The phone's new messages (at most `PER_PASS`), each claimed with the cursor and kept until answered, then every
/// one kept for this enrollment answered.
fn consume(ctx: &Ctx, hub: &Hub, me: &Keys, p: &Paired) -> Result<()> {
    let k = cursor_key(&p.mailbox);
    let cursor: i64 = ctx.db.kv_get(&k)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let list = hub.get_json(&format!("/v1/mail/{}/up?after={cursor}&limit={PER_PASS}", p.mailbox))?.unwrap_or(Value::Null);
    for seq in ahead(&list, cursor) {
        let bytes = hub.get_opt_bytes(&format!("/v1/mail/{}/up/{seq}", p.mailbox), MAIL_MAX)?;
        let opened = bytes.as_deref().map(|b| open_mail(&me.x_secret, &p.pubkey, &p.mailbox, "up", seq, b));
        if let Some(Err(e)) = &opened { crate::log::warn(&format!("phones: a message for {} that does not open was dropped ({e:#})", p.phone.name)); }
        // the cursor only moves up, and what the slot held is kept in the same transaction: claimed once, never lost
        ctx.db.with(|c| {
            let tx = c.transaction()?;
            let claimed = tx.execute("insert into kv(k, v) values(?1, ?2) on conflict(k) do update set v = excluded.v where cast(kv.v as integer) < cast(excluded.v as integer)", params![k, seq.to_string()])? > 0;
            if let (true, Some(Ok(body))) = (claimed, &opened) {
                tx.execute("insert or ignore into phone_mail_in(mailbox, seq, generation, body) values(?, ?, ?, ?)", params![p.mailbox, seq, p.phone.generation, body.to_string()])?;
            }
            tx.commit()?;
            Ok(())
        })?;
        #[cfg(test)]
        if STOP_AFTER_CLAIM.with(|c| c.replace(false)) { bail!("the computer stopped after claiming a message"); }
    }
    let kept: Vec<(i64, String)> = ctx.db.with(|c| {
        c.execute("delete from phone_mail_in where mailbox = ? and generation <> ?", params![p.mailbox, p.phone.generation])?;
        Ok(c.prepare("select seq, body from phone_mail_in where mailbox = ? and generation = ? order by seq")?.query_map(params![p.mailbox, p.phone.generation], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?)
    })?;
    for (seq, body) in kept {
        let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        if let Some(reply) = handle(ctx, p, &body) { send(hub, me, p, &reply)?; }
        ctx.db.with(|c| Ok(c.execute("delete from phone_mail_in where mailbox = ? and seq = ?", params![p.mailbox, seq])?))?;
    }
    Ok(())
}

fn refused(action_id: &str, query: &Value, status: u16, error: &str) -> Value { json!({"kind": "refused", "action_id": action_id, "query": query, "status": status, "error": error}) }

/// One message from the phone: what `/c1` would answer it, for the enrollment whose key opened it and only while it
/// is still that enrollment (read again here, as `/c1` reads it on every request).
fn handle(ctx: &Ctx, sealed_by: &Paired, body: &Value) -> Option<Value> {
    let now = reload(ctx, sealed_by)?;
    let p = &now.phone;
    let control = (!p.grants.control).then_some("Control is not granted to this phone");
    match body["kind"].as_str()? {
        "act" => {
            let id = body["action"]["action_id"].as_str().unwrap_or("").to_string();
            if let Some(no) = control { return Some(refused(&id, &Value::Null, 403, no)); }
            let a: Action = match serde_json::from_value(body["action"].clone()) {
                Ok(a) if !id.is_empty() && id.len() <= 100 => a,
                _ => return Some(refused(&id, &Value::Null, 400, "an action needs its action_id")),
            };
            Some(match super::actions::act(ctx, p, &a) {
                Ok(r) => json!({"kind": "receipt", "action_id": id, "receipt": super::stamped(ctx, json!(r))}),
                Err(e) => refused(&id, &Value::Null, 500, &format!("{e:#}")),
            })
        }
        "receipt" => {
            let id = body["action_id"].as_str().unwrap_or("").to_string();
            // the phone's own question id, so it matches the answer to the question it asked
            let query = body["query"].as_str().filter(|q| q.len() <= 64).map(|q| json!(q)).unwrap_or(Value::Null);
            if let Some(no) = control { return Some(refused(&id, &query, 403, no)); }
            Some(match super::actions::stored(ctx, &id) {
                Ok(Some((phone, generation, r))) if phone == p.id && generation == p.generation => json!({"kind": "receipt", "action_id": id, "query": query, "receipt": super::stamped(ctx, json!(r))}),
                Ok(_) => refused(&id, &query, 404, "no such action"),
                Err(e) => refused(&id, &query, 500, &format!("{e:#}")),
            })
        }
        _ => None,
    }
}

/// `body` sealed to the phone in the next free slot down, above the pairing's two.
fn send(hub: &Hub, me: &Keys, p: &Paired, body: &Value) -> Result<()> {
    for _ in 0..3 {
        let max = hub.get_json(&format!("/v1/mail/{}/down?after={}&limit=1", p.mailbox, i64::MAX))?.and_then(|v| v["max"].as_i64()).unwrap_or(0);
        let seq = next_seq(max)?.max(3);
        let m = seal_mail(&me.key, &p.phone.x25519, &p.mailbox, "down", seq, body)?;
        match hub.put(&format!("/v1/mail/{}/down/{seq}", p.mailbox), m, &[]) {
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
/// grants), or that one is five minutes old. Built under the enrollment as it stands after the messages were
/// answered, and dropped when it changed (grants, revocation, a new pairing) while it was built.
fn publish(ctx: &Ctx, hub: &Hub, me: &Keys, at_pass: &Paired) -> Result<()> {
    let Some(p) = reload(ctx, at_pass) else { return Ok(()) };
    let stamp = format!("{:?}|{:?}|{}", super::events::look(ctx, &p.phone), p.phone.grants, p.phone.generation);
    let now = now_ms();
    if SENT.lock().unwrap_or_else(|e| e.into_inner()).get(&p.mailbox).is_some_and(|(s, at)| *s == stamp && now - at < REPUBLISH_MS) { return Ok(()); }
    let snap = snapshot(ctx, &p.phone)?;
    #[cfg(test)]
    seam(&BEFORE_PUBLISH);
    if reload(ctx, &p).is_none_or(|again| again.phone.grants != p.phone.grants) { return Ok(()); }
    send(hub, me, &p, &snap)?;
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
    /// The exchange as this computer saw it: the phone's first message, the answer, the phone's sealed payload.
    transcript: [Vec<u8>; 3],
    error: Option<String>,
    /// The decision for the phone, and whether it is on the hub yet; whether the phone said it read it.
    result: Option<String>,
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
    /// Tests: the pairing does not deliver its decision itself (as if the computer stopped once it approved).
    #[cfg(test)]
    hold: AtomicBool,
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
            let Some(up1) = hub.get_opt_bytes(&self.mail("up", 1), MAIL_MAX)? else { return Ok(()) };
            let v: Value = serde_json::from_slice(&up1)?;
            let phone_id = v["id"].as_str().filter(|s| !s.is_empty()).context("the phone's first message names no phone")?.to_string();
            let (key, back) = pairing::answer(&self.code, &phone_id, &self.ctx.settings.device_id, v["msg"].as_str().unwrap_or(""))?;
            let down1 = json!({"kind": "pair-start", "msg": back}).to_string().into_bytes();
            hub.put(&self.mail("down", 1), down1.clone(), &[])?;
            let mut st = self.lock();
            (st.phone_id, st.key) = (Some(phone_id), Some(key));
            st.transcript[0] = up1;
            st.transcript[1] = down1;
        }
        let Some(up2) = hub.get_opt_bytes(&self.mail("up", 2), MAIL_MAX)? else { return Ok(()) };
        let v: Value = serde_json::from_slice(&up2)?;
        let opened = {
            let st = self.lock();
            st.key.as_ref().map(|k| k.open::<PhonePayload>(v["payload"].as_str().unwrap_or(""))).transpose().map(|p| (p, st.phone_id.clone()))
        };
        match opened {
            Ok((Some(p), Some(id))) if p.id == id && identity::pem_to_der(&p.cert_pem).is_ok() => {
                let (name, platform) = (p.name.clone(), p.platform.clone());
                if self.set(&["waiting"], |st| { st.state = "confirm"; st.phone = Some(p); st.transcript[2] = up2; }) { crate::log::info(&format!("phones: {name} ({platform}) waits for approval through the hub")); }
            }
            _ => { self.set(&["waiting"], |st| { st.state = "failed"; st.error = Some("the phone's payload did not open; show a new invite".into()); }); }
        }
        Ok(())
    }

    /// The decision put on the hub once, then the phone's word that it read it: sealed by the approved phone for slot
    /// 3, or signed by the refused one. Anything else in slot 3 is not that word.
    fn deliver(&self, hub: &Hub) -> Result<()> {
        let (result, delivered, fetched, state, phone) = { let st = self.lock(); (st.result.clone(), st.delivered, st.fetched, st.state, st.phone.clone()) };
        #[cfg(test)]
        let delivered = delivered || self.hold.load(Ordering::SeqCst);
        if let (Some(r), false) = (&result, delivered) {
            put_decision(hub, &self.mailbox, r)?;
            if state == "paired" { self.ctx.db.with(|c| Ok(c.execute("update phones set hub_pairing = null where hub_mailbox = ? and hub_pairing = ?", params![self.mailbox, r])?))?; }
            self.lock().delivered = true;
        }
        let (Some(_), false, Some(phone)) = (&result, fetched, phone) else { return Ok(()) };
        let Some(b) = hub.get_opt_bytes(&self.mail("up", 3), MAIL_MAX)? else { return Ok(()) };
        let said = match state {
            "paired" => open_mail(&keys(&self.ctx)?.x_secret, &phone.pubkey, &self.mailbox, "up", 3, &b).is_ok_and(|v| v["kind"] == "fetched"),
            _ => serde_json::from_slice::<Value>(&b).ok().and_then(|v| v["sig"].as_str().map(String::from))
                .is_some_and(|sig| identity::verify(&phone.pubkey, &fetched_refusal(&self.mailbox), &sig)),
        };
        if said { self.set(&["paired", "refused"], |st| st.fetched = true); }
        Ok(())
    }

    /// Until the phone read the decision, or the invite's time (and a moment more for a decision) is up. An invite that
    /// ends unpaired is owed a deletion on the hub.
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
        if self.lock().state != "paired" {
            let owed = self.ctx.db.with(|c| Ok(delete_later(c, &self.ctx.settings.hub_url, &self.mailbox, "", "")?));
            if owed.is_ok() { delete_now(&self.ctx, &self.mailbox); }
        }
        self.ended.store(true, Ordering::SeqCst);
    }
}

/// Invite a phone through the hub for ten minutes: `{attempt, route: "hub", code, expires_at, hub_url, qr, state}`; the
/// QR code says `hm-hub:1?c=<code>&f=<this computer's certificate fingerprint>&u=<hub url>`. Every change of state is
/// told as `phones-pairing`, as a direct pairing's.
pub fn pair_start(ctx: Arc<Ctx>, on_event: OnEvent) -> Result<HubPairing> {
    let id = identity::load(&ctx)?;
    let inv = hub::phone_invite(&ctx)?;
    let now = now_ms();
    let (code, mailbox) = (inv["code"].as_str().unwrap_or("").to_string(), inv["phone_id"].as_str().unwrap_or("").to_string());
    let expires_at = inv["expires_at"].as_i64().unwrap_or(0).clamp(now, now + LONGEST_INVITE_MS);
    if code.len() != 6 || mailbox.is_empty() { bail!("the hub answered without an invite"); }
    let qr = hub_qr(&ctx.settings.hub_url, &code, &pairing::fingerprint(&id.cert_der));
    let shown = json!({"attempt": crate::util::uuid4(), "route": "hub", "code": code, "expires_at": expires_at, "hub_url": ctx.settings.hub_url, "qr": qr});
    let shared = Arc::new(Shared { ctx, code, mailbox, expires_at, shown, st: Mutex::new(PairSt { state: "waiting", ..Default::default() }), on_event, ended: AtomicBool::new(false),
        #[cfg(test)]
        hold: AtomicBool::new(false) });
    let s = shared.clone();
    std::thread::spawn(move || s.run());
    Ok(HubPairing { shared })
}

impl HubPairing {
    pub fn status(&self) -> Value { let st = self.shared.lock(); self.shared.status_of(&st) }

    pub fn attempt(&self) -> &str { self.shared.shown["attempt"].as_str().unwrap_or("") }

    /// The pairing is over here: the phone read the decision, or the invite's time ran out.
    pub fn ended(&self) -> bool { self.shared.ended.load(Ordering::SeqCst) }

    /// Tests: this pairing leaves its decision to the mailbox pass, as after a restart.
    #[cfg(test)]
    pub(crate) fn hold_delivery(&self) { self.shared.hold.store(true, Ordering::SeqCst); }

    /// The owner approved the phone shown, with these grants: it is stored as a new enrollment with its mailbox and
    /// this computer's payload (its certificate, its keys, its direct address when it has one), sealed for the phone
    /// and signed by this computer's identity over the exchange, kept with the enrollment until it is on the hub.
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
            let [a, b, c] = &st.transcript;
            let sig = id.sign(&approval_binding(&s.mailbox, &phone.id, &phone.pubkey, &phone.x25519, &p, [a, b, c]));
            let decision = json!({"kind": "pair-result", "state": "approved", "payload": key.seal(&p)?, "sig": sig}).to_string();
            phones::add_with(ctx, &phone, grants, Some(&s.mailbox), Some(&decision))?;
            // the pairing's own two messages are not the mailbox pass's
            ctx.db.kv_set(&cursor_key(&s.mailbox), "2")?;
            st.state = "paired";
            st.result = Some(decision);
            s.status_of(&st)
        };
        (s.on_event)("phones-pairing", v.clone());
        Ok(v)
    }

    /// The owner refused the phone, or stopped waiting for one.
    pub fn refuse(&self) -> Result<Value> {
        if !self.shared.set(&["waiting", "confirm"], |st| { st.state = "refused"; st.result = Some(json!({"kind": "pair-result", "state": "refused"}).to_string()); }) { bail!("the pairing already ended"); }
        Ok(self.status())
    }

    /// Stop showing the invite; the attempt ends without an event, and its mailbox is owed a deletion unless paired.
    pub fn cancel(&self) { let mut st = self.shared.lock(); if st.state != "paired" { st.state = "cancelled"; } }
}
