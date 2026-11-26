//! The phones paired with this computer: each with its certificate (pinned by the listener), its keys and what the
//! owner granted it (summaries, transcripts, Control). A phone is never a peer: it gets no ring key, no journal and
//! no `/v1` route, only the companion API (`companion`). Revoking keeps the row, marked, so its certificate is
//! refused from then on and the Phones list can say what was acknowledged. Each pairing is a new enrollment
//! (`generation`): a connection or a receipt of an earlier one is not the phone paired now, even under the same id.

use anyhow::{bail, Context, Result};
use companion_proto::api::Grants;
use companion_proto::pairing::PhonePayload;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::util::now_ms;
use crate::Ctx;

/// How long a question waits for an answer, seconds: a phone with Control needs the time to wake, unlock and answer.
pub const QUESTION_WITH_PHONE: u32 = 60;
pub const QUESTION_WITHOUT: u32 = 15;

/// One phone's row, as the companion API checks it on every request.
#[derive(Debug, Clone)]
pub struct Phone {
    pub id: String,
    pub name: String,
    pub grants: Grants,
    pub revoked_at: Option<i64>,
    pub x25519: String,
    /// This enrollment; a pairing again under the same id makes a new one.
    pub generation: String,
    pub push_needs_reregistering: bool,
}

fn grants_of(s: &str) -> Grants { serde_json::from_str(s).unwrap_or(Grants { summaries: false, transcripts: false, control: false }) }

fn row_json(ctx: &Ctx, r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let id: String = r.get(0)?;
    let route = crate::companion::conn::connected(ctx, &id).then_some("direct");
    let revoked: Option<i64> = r.get(6)?;
    let mut v = json!({"phone_id": id, "name": r.get::<_, Option<String>>(1)?, "platform": r.get::<_, Option<String>>(2)?, "grants": grants_of(&r.get::<_, Option<String>>(3)?.unwrap_or_default()),
        "paired_at": r.get::<_, Option<i64>>(4)?, "last_seen": r.get::<_, Option<i64>>(5)?, "revoked_at": revoked, "route": route});
    // what a revocation got acknowledged: the listener's cut may come later, from the process that holds it
    if revoked.is_some() { v["acks"] = json!({"connection": r.get::<_, Option<i64>>(7)?.is_some(), "hub": null, "push": null}); }
    Ok(v)
}

const COLS: &str = "phone_id, name, platform, grants, paired_at, last_seen, revoked_at, cut_at";

/// Every phone paired here, revoked ones too: `[{phone_id, name, platform, grants, paired_at, last_seen, revoked_at,
/// route, acks?}]`; `route` is "direct" while it is connected, "hub" (wave 2) or null; a revoked phone carries the
/// acknowledgements as `revoke` reports them, up to date.
pub fn list(ctx: &Ctx) -> Result<Value> {
    ctx.db.read(|c| {
        let v: Vec<Value> = c.prepare(&format!("select {COLS} from phones order by paired_at"))?.query_map([], |r| row_json(ctx, r))?.collect::<rusqlite::Result<_>>()?;
        Ok(json!(v))
    })
}

/// A phone's id from its id or name.
fn phone_id(ctx: &Ctx, id_or_name: &str) -> Result<String> {
    let hit: Vec<String> = ctx.db.read(|c| Ok(c.prepare("select phone_id from phones where phone_id = ?1 or lower(name) = lower(?1)")?.query_map([id_or_name], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?))?;
    match hit.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no phone is called {id_or_name}"),
        _ => bail!("{id_or_name} names more than one phone; give its id"),
    }
}

fn row(ctx: &Ctx, id: &str) -> Result<Value> {
    ctx.db.read(|c| Ok(c.query_row(&format!("select {COLS} from phones where phone_id = ?"), [id], |r| row_json(ctx, r))?))
}

/// What a phone may do: `{summaries?, transcripts?, control?}`, each left as it was when not given. Answers the row.
/// The row is read and written in one transaction, so two changes at once cannot undo each other.
pub fn set_grants(ctx: &Ctx, phone_id: &str, grants: &Value) -> Result<Value> {
    let id = self::phone_id(ctx, phone_id)?;
    let mut patch = vec![];
    for k in ["summaries", "transcripts", "control"] {
        match &grants[k] {
            Value::Null => {}
            Value::Bool(b) => patch.push((k, *b)),
            _ => bail!("{k} is on or off"),
        }
    }
    let (name, g) = ctx.db.with(|c| {
        let tx = c.transaction()?;
        let (name, now, revoked): (String, String, Option<i64>) = tx.query_row("select coalesce(name, ''), coalesce(grants, ''), revoked_at from phones where phone_id = ?", [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?.context("no such phone")?;
        if revoked.is_some() { bail!("that phone was revoked; pair it again"); }
        let mut g = serde_json::to_value(grants_of(&now))?;
        for (k, b) in &patch { g[*k] = json!(b); }
        tx.execute("update phones set grants = ?2 where phone_id = ?1", params![id, g.to_string()])?;
        tx.commit()?;
        Ok((name, g))
    })?;
    crate::log::info(&format!("phones: {name} may now {g}"));
    row(ctx, &id)
}

/// Revoke a phone: its open connections are cut, its certificate refused from now on, its relay handle dropped.
/// `{phone_id, revoked_at, acks: {connection, hub, push}}`; hub and push are null until the hub mailbox and the
/// relay exist (wave 2).
/// The connection ack is true once the listener has cut every connection of the phone: at once when it runs in this
/// process, else when the listener of the process that holds it says so (it looks every second; this waits 3 s).
pub fn revoke(ctx: &Ctx, phone_id: &str) -> Result<Value> {
    let id = self::phone_id(ctx, phone_id)?;
    let at = ctx.db.with(|c| {
        let tx = c.transaction()?;
        let old = push_key_id(&tx, &id)?;
        tx.execute("update phones set revoked_at = ?2, relay_handle = null, push_key_id = null, push_needs_reregistering=0 where phone_id = ?1 and revoked_at is null", params![id, now_ms()])?;
        tx.commit()?;
        forget_push_key(old);
        Ok(c.query_row("select revoked_at from phones where phone_id = ?", [&id], |r| r.get::<_, i64>(0))?)
    })?;
    // after the mark: a connection that comes back is refused at its handshake
    if crate::companion::conn::cut(ctx, &id) { cut_done(ctx, std::slice::from_ref(&id)); }
    let acked = || ctx.db.read(|c| Ok(c.query_row("select cut_at is not null from phones where phone_id = ?", [&id], |r| r.get::<_, bool>(0))?)).unwrap_or(false);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !acked() && std::time::Instant::now() < deadline { std::thread::sleep(std::time::Duration::from_millis(100)); }
    let connection = acked();
    crate::log::info(&format!("phones: {id} revoked{}", if connection { "" } else { "; no listener acknowledged the cut yet" }));
    Ok(json!({"phone_id": id, "revoked_at": at, "acks": {"connection": connection, "hub": null, "push": null}}))
}

/// The listener has no connection of these revoked phones any more.
pub(crate) fn cut_done(ctx: &Ctx, ids: &[String]) {
    let _ = ctx.db.with(|c| { for id in ids { c.execute("update phones set cut_at = ?2 where phone_id = ?1 and revoked_at is not null and cut_at is null", params![id, now_ms()])?; } Ok(()) });
}

/// One phone's row by id, revoked or not.
pub fn get(ctx: &Ctx, id: &str) -> Result<Option<Phone>> {
    ctx.db.read(|c| Ok(c.query_row("select phone_id, coalesce(name, ''), coalesce(grants, ''), revoked_at, coalesce(x25519, ''), generation,
        push_needs_reregistering or (relay_handle is not null and push_key_id is null) from phones where phone_id = ?", [id],
        |r| Ok(Phone { id: r.get(0)?, name: r.get(1)?, grants: grants_of(&r.get::<_, String>(2)?), revoked_at: r.get(3)?, x25519: r.get(4)?, generation: r.get(5)?, push_needs_reregistering: r.get(6)? })).optional()?))
}

/// Who a certificate belongs to, read from one snapshot of the phones and the paired computers.
#[derive(Debug, Clone, PartialEq)]
pub enum Holder {
    Nobody,
    /// One phone enrollment, not revoked: `(phone_id, generation)`.
    Phone(String, String),
    /// One paired computer.
    Peer,
    /// More than one holder (rows from before enrollment checked certificates): nobody is let in on it.
    Ambiguous,
}

pub fn holder(ctx: &Ctx, der: &[u8]) -> Result<Holder> {
    let same = |pem: &str| crate::sync::peer::identity::pem_to_der(pem).ok().as_deref() == Some(der);
    ctx.db.read(|c| {
        let tx = c.transaction()?;
        let phones: Vec<(String, String, String)> = tx.prepare("select phone_id, generation, cert_pem from phones where revoked_at is null")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let peers: Vec<String> = tx.prepare("select cert_pem from peers where coalesce(cert_pem, '') <> ''")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let mut phones: Vec<(String, String)> = phones.into_iter().filter(|(_, _, pem)| same(pem)).map(|(id, g, _)| (id, g)).collect();
        let peers = peers.iter().filter(|pem| same(pem)).count();
        Ok(match (phones.len(), peers) {
            (0, 0) => Holder::Nobody,
            (1, 0) => { let (id, g) = phones.remove(0); Holder::Phone(id, g) }
            (0, 1) => Holder::Peer,
            _ => Holder::Ambiguous,
        })
    })
}

/// The enrollment this certificate belongs to, when it is the certificate's only holder: `(phone_id, generation)`.
pub fn by_cert(ctx: &Ctx, der: &[u8]) -> Option<(String, String)> {
    match holder(ctx, der) { Ok(Holder::Phone(id, g)) => Some((id, g)), _ => None }
}

/// Ed25519's SubjectPublicKeyInfo (RFC 8410) for a public key: what the phone's certificate must carry.
fn ed25519_spki(pk: &[u8]) -> Vec<u8> { [[0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00].as_slice(), pk].concat() }

/// A certificate the phone sent: one that parses, made for the signing key it sent with it (the TLS handshake then
/// proves the phone holds that key).
fn check_cert(p: &PhonePayload) -> Result<Vec<u8>> {
    let b64 = |s: &str| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s.trim()).ok().filter(|b| b.len() == 32);
    let (Some(pk), Some(_)) = (b64(&p.pubkey), b64(&p.x25519)) else { bail!("the phone sent no usable keys") };
    let der = crate::sync::peer::identity::pem_to_der(&p.cert_pem).context("the phone sent no certificate")?;
    let cert_der = rustls::pki_types::CertificateDer::from(der.as_slice());
    let cert = webpki::EndEntityCert::try_from(&cert_der).map_err(|e| anyhow::anyhow!("the phone's certificate does not parse: {e:?}"))?;
    if cert.subject_public_key_info().as_ref() != ed25519_spki(&pk).as_slice() { bail!("the phone's certificate is not made for its key"); }
    Ok(der)
}

/// Another enrollment or a paired computer already has this certificate: the listener could not tell them apart.
pub(crate) fn cert_taken(c: &rusqlite::Connection, der: &[u8], except_phone: Option<&str>) -> Result<bool> {
    let same = |pem: String| crate::sync::peer::identity::pem_to_der(&pem).ok().as_deref() == Some(der);
    let peers: Vec<String> = c.prepare("select cert_pem from peers where coalesce(cert_pem, '') <> ''")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let phones: Vec<String> = c.prepare("select cert_pem from phones where revoked_at is null and phone_id <> ?1")?.query_map([except_phone.unwrap_or("")], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(peers.into_iter().chain(phones).any(same))
}

/// A phone the owner approved, stored with its grants as a new enrollment. A phone paired again (the same id) starts
/// over unrevoked, and the connections of its earlier enrollment are cut.
pub(crate) fn add(ctx: &Ctx, p: &PhonePayload, grants: Grants) -> Result<()> {
    let ok_id = !p.id.is_empty() && p.id.len() <= 80 && p.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok_id { bail!("the phone sent no usable id"); }
    let der = check_cert(p)?;
    let name: String = p.name.trim().chars().take(60).collect();
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        if cert_taken(&tx, &der, Some(&p.id))? { bail!("that certificate already belongs to another phone or computer"); }
        let old = push_key_id(&tx, &p.id)?;
        tx.execute("insert into phones(phone_id, name, platform, cert_pem, pubkey, x25519, grants, paired_at, last_seen, revoked_at, relay_handle, generation, cut_at) values(?1,?2,?3,?4,?5,?6,?7,?8,null,null,null,?9,null)
            on conflict(phone_id) do update set name=?2, platform=?3, cert_pem=?4, pubkey=?5, x25519=?6, grants=?7, paired_at=?8, last_seen=null, revoked_at=null, relay_handle=null, push_key_id=null, push_needs_reregistering=0, generation=?9, cut_at=null",
            params![p.id, name, p.platform, p.cert_pem, p.pubkey, p.x25519, serde_json::to_string(&grants)?, now_ms(), crate::util::uuid4()])?;
        tx.commit()?;
        forget_push_key(old);
        Ok(())
    })?;
    crate::companion::conn::cut(ctx, &p.id);
    crate::log::info(&format!("phones: paired with {name} ({})", p.id));
    Ok(())
}

/// The phone was heard from now.
pub(crate) fn seen(ctx: &Ctx, id: &str) {
    let _ = ctx.db.with(|c| { c.execute("update phones set last_seen = ?2 where phone_id = ?1", params![id, now_ms()])?; Ok(()) });
}

/// The phone's relay handle, or none (`PUT` / `DELETE /c1/push`).
/// The enrollment `p` checked at the request's start, not one paired again since under the same id.
pub(crate) fn set_push(ctx: &Ctx, p: &Phone, handle: Option<&str>, secret: Option<&str>) -> Result<()> {
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        let old = push_key_id(&tx, &p.id)?;
        let slot = secret.map(|_| crate::util::uuid4());
        let n = tx.execute("update phones set relay_handle=?3, push_key_id=?4, push_needs_reregistering=?5
            where phone_id=?1 and generation=?2 and revoked_at is null",
            params![p.id, p.generation, handle, slot, handle.is_some() && secret.is_none()])?;
        if n == 0 { bail!("this phone was revoked or paired again"); }
        if let (Some(slot), Some(secret)) = (&slot, secret) { crate::secrets::set_key(&push_target(slot), secret)?; }
        if let Err(e) = tx.commit() { forget_push_key(slot); return Err(e.into()); }
        forget_push_key(old);
        Ok(())
    })
}

pub(crate) fn push_target(slot: &str) -> String { format!("{}:phone-push:{slot}", crate::secrets::prefix()) }

fn push_key_id(c: &rusqlite::Connection, id: &str) -> rusqlite::Result<Option<String>> {
    Ok(c.query_row("select push_key_id from phones where phone_id=?", [id], |r| r.get(0)).optional()?.flatten())
}

fn forget_push_key(slot: Option<String>) {
    if let Some(slot) = slot {
        if crate::secrets::delete(&push_target(&slot)).is_err() { crate::log::warn("could not remove the retired phone push credential"); }
    }
}

/// The slot changes on every PUT, even when the phone reinstalls the same handle during a wake.
pub(crate) fn rejected_push(ctx: &Ctx, slot: &str) -> Result<()> {
    let n = ctx.db.with(|c| Ok(c.execute("update phones set relay_handle=null, push_key_id=null, push_needs_reregistering=1
        where push_key_id=? and revoked_at is null", [slot])?))?;
    if n != 0 { forget_push_key(Some(slot.into())); }
    Ok(())
}

pub(crate) fn migrate_push_keys(c: &mut rusqlite::Connection) -> Result<()> {
    let legacy = "select 1 from pragma_table_info('phones') where name='relay_secret'";
    if !c.prepare(legacy)?.exists([])? { return Ok(()); }
    let tx = c.transaction()?;
    if !tx.prepare(legacy)?.exists([])? { return Ok(()); }
    tx.pragma_update(None, "secure_delete", true)?;
    {
        let mut st = tx.prepare("select phone_id, generation, relay_handle, relay_secret from phones where revoked_at is null and relay_handle is not null")?;
        let rows = st.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,Option<String>>(3)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, generation, handle, secret) in rows {
            if let Some(secret) = secret.filter(|s| valid_handle(&handle) && hex::decode_to_slice(s, &mut [0;32]).is_ok()) {
                // A retry after an interrupted migration writes the same credential.
                crate::secrets::set_key(&push_target(&generation), &secret)?;
                tx.execute("update phones set push_key_id=?, relay_handle=lower(relay_handle) where phone_id=?", params![generation,id])?;
            } else {
                tx.execute("update phones set relay_handle=null, push_needs_reregistering=1 where phone_id=?", [id])?;
            }
        }
    }
    tx.execute_batch("update phones set relay_secret=null where relay_secret is not null; alter table phones drop column relay_secret")?;
    tx.commit()?;
    c.execute_batch("pragma wal_checkpoint(truncate)")?;
    Ok(())
}

pub(crate) fn valid_handle(handle: &str) -> bool { handle.len() == 48 && handle.bytes().all(|b| b.is_ascii_hexdigit()) }

/// A phone that is not revoked may answer requests.
pub fn control_paired(ctx: &Ctx) -> bool {
    ctx.db.read(|c| {
        let gs: Vec<String> = c.prepare("select coalesce(grants, '') from phones where revoked_at is null")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(gs.iter().any(|g| grants_of(g).control))
    }).unwrap_or(false)
}

/// The question window a launch gives its hooks: 60 s while a phone with Control is paired, else 15 s.
pub fn question_seconds(ctx: &Ctx) -> u32 { if control_paired(ctx) { QUESTION_WITH_PHONE } else { QUESTION_WITHOUT } }

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(id: &str) -> PhonePayload {
        let i = companion_proto::PhoneIdentity::from_secrets(id, &[7; 32], &[8; 32]).unwrap();
        PhonePayload::of(&i, "Pixel", "android").unwrap()
    }

    #[test]
    fn grants_change_one_at_a_time_and_revoke_marks_the_row() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        assert_eq!(question_seconds(&ctx), QUESTION_WITHOUT);
        add(&ctx, &payload("phone-1"), Grants::default()).unwrap();
        assert_eq!(question_seconds(&ctx), QUESTION_WITHOUT, "summaries and transcripts only");
        let r = set_grants(&ctx, "Pixel", &json!({"control": true})).unwrap();
        assert_eq!(r["grants"], json!({"summaries": true, "transcripts": true, "control": true}));
        assert_eq!(question_seconds(&ctx), QUESTION_WITH_PHONE);
        assert!(set_grants(&ctx, "phone-1", &json!({"control": "yes"})).is_err());
        let der = crate::sync::peer::identity::pem_to_der(&payload("phone-1").cert_pem).unwrap();
        assert_eq!(by_cert(&ctx, &der).map(|(id, _)| id).as_deref(), Some("phone-1"));
        let v = revoke(&ctx, "phone-1").unwrap();
        // no listener runs here, so nothing acknowledged the cut (one would, at its next look)
        assert_eq!((v["acks"]["connection"].as_bool(), v["acks"]["hub"].is_null(), v["acks"]["push"].is_null()), (Some(false), true, true));
        assert!(v["revoked_at"].is_i64());
        assert_eq!(by_cert(&ctx, &der), None, "a revoked phone's certificate is refused");
        assert_eq!(question_seconds(&ctx), QUESTION_WITHOUT);
        assert!(set_grants(&ctx, "phone-1", &json!({"control": true})).is_err());
        let l = list(&ctx).unwrap();
        assert_eq!((l[0]["phone_id"].as_str(), l[0]["revoked_at"].is_i64(), l[0]["route"].is_null()), (Some("phone-1"), true, true));
        assert!(revoke(&ctx, "nobody").is_err());
        // pairing again starts over
        add(&ctx, &payload("phone-1"), Grants::default()).unwrap();
        assert!(list(&ctx).unwrap()[0]["revoked_at"].is_null());
    }
}
