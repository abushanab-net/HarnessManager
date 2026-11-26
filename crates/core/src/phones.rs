//! The phones paired with this computer: each with its certificate (pinned by the listener), its keys and what the
//! owner granted it (summaries, transcripts, Control). A phone is never a peer: it gets no ring key, no journal and
//! no `/v1` route, only the companion API (`companion`). Revoking keeps the row, marked, so its certificate is
//! refused from then on and the Phones list can say what was acknowledged.

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
}

fn grants_of(s: &str) -> Grants { serde_json::from_str(s).unwrap_or(Grants { summaries: false, transcripts: false, control: false }) }

fn row_json(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let id: String = r.get(0)?;
    let route = crate::companion::conn::connected(&id).then_some("direct");
    Ok(json!({"phone_id": id, "name": r.get::<_, Option<String>>(1)?, "platform": r.get::<_, Option<String>>(2)?, "grants": grants_of(&r.get::<_, Option<String>>(3)?.unwrap_or_default()),
        "paired_at": r.get::<_, Option<i64>>(4)?, "last_seen": r.get::<_, Option<i64>>(5)?, "revoked_at": r.get::<_, Option<i64>>(6)?, "route": route}))
}

const COLS: &str = "phone_id, name, platform, grants, paired_at, last_seen, revoked_at";

/// Every phone paired here, revoked ones too: `[{phone_id, name, platform, grants, paired_at, last_seen, revoked_at,
/// route}]`; `route` is "direct" while it is connected, "hub" (wave 2) or null.
pub fn list(ctx: &Ctx) -> Result<Value> {
    ctx.db.read(|c| {
        let v: Vec<Value> = c.prepare(&format!("select {COLS} from phones order by paired_at"))?.query_map([], row_json)?.collect::<rusqlite::Result<_>>()?;
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
    ctx.db.read(|c| Ok(c.query_row(&format!("select {COLS} from phones where phone_id = ?"), [id], row_json)?))
}

/// What a phone may do: `{summaries?, transcripts?, control?}`, each left as it was when not given. Answers the row.
pub fn set_grants(ctx: &Ctx, phone_id: &str, grants: &Value) -> Result<Value> {
    let id = self::phone_id(ctx, phone_id)?;
    let p = get(ctx, &id)?.context("no such phone")?;
    if p.revoked_at.is_some() { bail!("that phone was revoked; pair it again"); }
    let mut g = serde_json::to_value(p.grants)?;
    for k in ["summaries", "transcripts", "control"] {
        match &grants[k] {
            Value::Null => {}
            Value::Bool(b) => g[k] = json!(b),
            _ => bail!("{k} is on or off"),
        }
    }
    ctx.db.with(|c| { c.execute("update phones set grants = ?2 where phone_id = ?1", params![id, g.to_string()])?; Ok(()) })?;
    crate::log::info(&format!("phones: {} may now {}", p.name, g));
    row(ctx, &id)
}

/// Revoke a phone: its open connections are cut, its certificate refused from now on, its relay handle dropped.
/// `{phone_id, revoked_at, acks: {connection, hub, push}}`; hub and push are null until the hub mailbox and the
/// relay exist (wave 2).
pub fn revoke(ctx: &Ctx, phone_id: &str) -> Result<Value> {
    let id = self::phone_id(ctx, phone_id)?;
    let at = ctx.db.with(|c| {
        c.execute("update phones set revoked_at = ?2, relay_handle = null where phone_id = ?1 and revoked_at is null", params![id, now_ms()])?;
        Ok(c.query_row("select revoked_at from phones where phone_id = ?", [&id], |r| r.get::<_, i64>(0))?)
    })?;
    // after the mark: a connection that comes back is refused at its handshake
    let connection = crate::companion::conn::cut(&id);
    crate::log::info(&format!("phones: {id} revoked"));
    Ok(json!({"phone_id": id, "revoked_at": at, "acks": {"connection": connection, "hub": null, "push": null}}))
}

/// One phone's row by id, revoked or not.
pub fn get(ctx: &Ctx, id: &str) -> Result<Option<Phone>> {
    ctx.db.read(|c| Ok(c.query_row("select phone_id, coalesce(name, ''), coalesce(grants, ''), revoked_at, coalesce(x25519, '') from phones where phone_id = ?", [id],
        |r| Ok(Phone { id: r.get(0)?, name: r.get(1)?, grants: grants_of(&r.get::<_, String>(2)?), revoked_at: r.get(3)?, x25519: r.get(4)? })).optional()?))
}

/// The phone whose certificate this is, when it is not revoked.
pub fn by_cert(ctx: &Ctx, der: &[u8]) -> Option<String> {
    let rows: Vec<(String, String)> = ctx.db.read(|c| Ok(c.prepare("select phone_id, cert_pem from phones where revoked_at is null")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?)).ok()?;
    rows.into_iter().find(|(_, pem)| crate::sync::peer::identity::pem_to_der(pem).ok().as_deref() == Some(der)).map(|(id, _)| id)
}

/// A phone the owner approved, stored with its grants. A phone paired again (the same id) starts over unrevoked.
pub(crate) fn add(ctx: &Ctx, p: &PhonePayload, grants: Grants) -> Result<()> {
    let ok_id = !p.id.is_empty() && p.id.len() <= 80 && p.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok_id { bail!("the phone sent no usable id"); }
    if crate::sync::peer::identity::pem_to_der(&p.cert_pem).is_err() { bail!("the phone sent no certificate"); }
    let name: String = p.name.trim().chars().take(60).collect();
    ctx.db.with(|c| {
        c.execute("insert into phones(phone_id, name, platform, cert_pem, pubkey, x25519, grants, paired_at, last_seen, revoked_at, relay_handle) values(?1,?2,?3,?4,?5,?6,?7,?8,null,null,null)
            on conflict(phone_id) do update set name=?2, platform=?3, cert_pem=?4, pubkey=?5, x25519=?6, grants=?7, paired_at=?8, last_seen=null, revoked_at=null, relay_handle=null",
            params![p.id, name, p.platform, p.cert_pem, p.pubkey, p.x25519, serde_json::to_string(&grants)?, now_ms()])?;
        Ok(())
    })?;
    crate::log::info(&format!("phones: paired with {name} ({})", p.id));
    Ok(())
}

/// The phone was heard from now.
pub(crate) fn seen(ctx: &Ctx, id: &str) {
    let _ = ctx.db.with(|c| { c.execute("update phones set last_seen = ?2 where phone_id = ?1", params![id, now_ms()])?; Ok(()) });
}

/// The phone's relay handle, or none (`PUT` / `DELETE /c1/push`).
pub(crate) fn set_push(ctx: &Ctx, id: &str, handle: Option<&str>) -> Result<()> {
    ctx.db.with(|c| { c.execute("update phones set relay_handle = ?2 where phone_id = ?1 and revoked_at is null", params![id, handle])?; Ok(()) })
}

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
        assert_eq!(by_cert(&ctx, &der).as_deref(), Some("phone-1"));
        let v = revoke(&ctx, "phone-1").unwrap();
        assert_eq!((v["acks"]["connection"].as_bool(), v["acks"]["hub"].is_null(), v["acks"]["push"].is_null()), (Some(true), true, true));
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
