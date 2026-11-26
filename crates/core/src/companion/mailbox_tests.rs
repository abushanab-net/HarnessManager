//! A phone away from home, end to end through a real hub (crates/hub, served in-process): the phone is
//! companion-proto's own `HubJoining` and `Mailbox`, as the app will be, and this computer runs its mailbox pass. The
//! pairing with the owner's approval and nothing sealed before it, the snapshots, an answer and its receipt through
//! the same entry point as `/c1`, the grant boundaries, a message changed or moved by the hub refused on both sides,
//! and revocation deleting the phone's token and mailbox.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use companion_proto::api::{ConnectError, Grants, Refused, Status};
use companion_proto::join::Outcome;
use companion_proto::mailbox::{self, HubJoining, Mailbox};
use companion_proto::pairing::DesktopPayload;
use companion_proto::PhoneIdentity;
use serde_json::{json, Value};

use super::mailbox::{pair_start, pass, HubPairing};
use super::tests::{answer, ask_state, waiting};
use crate::sync::peer::identity;
use crate::Ctx;

const ADMIN: &str = "admin-secret";

struct World {
    rt: tokio::runtime::Runtime,
    _hub: axum_server::Handle<SocketAddr>,
    hub_dir: tempfile::TempDir,
    url: String,
    ctx: Arc<Ctx>,
    _dir: tempfile::TempDir,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = crate::secrets::delete(&identity::target(&self.ctx.settings.device_id));
        for t in crate::sync::hub::keys_of(&self.ctx) { let _ = crate::secrets::delete(&t); }
    }
}

/// A hub on loopback and this computer joined to it with the admin token.
fn world() -> World {
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let hub_dir = tempfile::tempdir().unwrap();
    let cfg = hrm_hub::Config { data_dir: hub_dir.path().to_path_buf(), admin_token: ADMIN.into(), bind: "127.0.0.1:0".parse().unwrap(), max_blob_mb: 64, retain_mb: 2048 };
    let (addr, handle) = rt.block_on(hrm_hub::serve(cfg)).unwrap();
    let url = format!("http://{addr}");
    let dir = tempfile::tempdir().unwrap();
    let (data, home) = (dir.path().join("desk"), dir.path().join("home"));
    let mut ctx = Ctx::open_at(&data, &home).unwrap();
    ctx.settings.device_name = "Desk".into();
    ctx.save_settings().unwrap();
    let ctx = crate::sync::hub::join(&Ctx::open_at(&data, &home).unwrap(), &url, None, Some(ADMIN)).unwrap();
    World { rt, _hub: handle, hub_dir, url, ctx: Arc::new(ctx), _dir: dir }
}

/// The computer's mailbox pass, every 150 ms while it lives, as `mailbox::watch` runs it in the app.
struct Desk(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);

fn desk_loop(ctx: &Arc<Ctx>) -> Desk {
    let (stop, c) = (Arc::new(AtomicBool::new(false)), ctx.clone());
    let s = stop.clone();
    Desk(stop, Some(std::thread::spawn(move || while !s.load(Ordering::SeqCst) { let _ = pass(&c); std::thread::sleep(Duration::from_millis(150)); })))
}

impl Drop for Desk {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
        if let Some(h) = self.1.take() { let _ = h.join(); }
    }
}

/// `f` until it gives something, at most 15 s.
fn eventually<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(v) = f() { return v; }
        assert!(Instant::now() < until, "never: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The phone joined through the hub with the invite the computer shows, up to the owner's decision.
fn joined(w: &World, me: &PhoneIdentity) -> (HubPairing, HubJoining) {
    let p = pair_start(w.ctx.clone(), Arc::new(|_: &str, _| {})).unwrap();
    let st = p.status();
    assert_eq!((st["state"].as_str(), st["route"].as_str()), (Some("waiting"), Some("hub")));
    let (u, code, _) = mailbox::parse_hub_qr(st["qr"].as_str().unwrap()).unwrap();
    assert_eq!((u.as_str(), Some(code.as_str())), (w.url.as_str(), st["code"].as_str()));
    let j = w.rt.block_on(mailbox::join_hub_qr(st["qr"].as_str().unwrap(), me, "Pixel 9", "android")).unwrap();
    eventually(&format!("the owner is shown the phone ({})", p.status()), || (p.status()["state"] == "confirm").then_some(()));
    let st = p.status();
    assert_eq!(st["phone"]["name"], "Pixel 9");
    assert_eq!(st["words"], json!(j.words()), "both sides show the same three words");
    assert_eq!((j.desktop_id.as_str(), j.desktop_name.as_str()), (w.ctx.settings.device_id.as_str(), "Desk"));
    (p, j)
}

/// A phone paired through the hub with `grants`, and its mailbox client.
fn paired(w: &World, me: &PhoneIdentity, grants: Grants) -> (HubJoining, DesktopPayload, Mailbox) {
    let (p, j) = joined(w, me);
    assert!(matches!(w.rt.block_on(j.result()).unwrap(), Outcome::Pending(ms) if ms > 0), "nothing sealed before the owner approves");
    assert!(crate::phones::get(&w.ctx, &me.id).unwrap().is_none(), "nor stored");
    p.approve(grants).unwrap();
    let Outcome::Approved(desk) = w.rt.block_on(j.wait(Duration::from_millis(100))).unwrap() else { panic!("not approved") };
    assert_eq!((desk.grants, desk.hub_url.as_deref(), desk.address_hints.len()), (grants, Some(w.url.as_str()), 0), "no direct address without the direct connection");
    assert_eq!(desk.x25519.as_deref(), Some(super::mailbox::keys(&w.ctx).unwrap().x_public.as_str()));
    eventually("the computer sees the phone read its decision", || (p.status()["fetched"] == true).then_some(()));
    eventually("the pairing ends", || p.ended().then_some(()));
    let m = j.mailbox(&desk).unwrap();
    (j, desk, m)
}

/// Raw bytes on the hub, with a token, as the phone or the hub itself could put them.
fn raw(w: &World, token: &str, method: reqwest::Method, path: &str, body: Option<Vec<u8>>) -> u16 {
    let r = reqwest::blocking::Client::new().request(method, format!("{}{path}", w.url)).bearer_auth(token);
    r.body(body.unwrap_or_default()).send().unwrap().status().as_u16()
}

fn up_max(w: &World, j: &HubJoining) -> i64 {
    let v: Value = reqwest::blocking::Client::new().get(format!("{}/v1/mail/{}/up?after={}", w.url, j.mailbox, i64::MAX)).bearer_auth(&j.token).send().unwrap().json().unwrap();
    v["max"].as_i64().unwrap()
}

fn status_of<T: std::fmt::Debug>(r: anyhow::Result<T>) -> u16 { r.unwrap_err().downcast::<Refused>().expect("a refusal").status }

#[test]
fn a_phone_answers_a_request_through_the_hub_within_its_grants_and_is_cut_off_when_revoked() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    let row = &crate::phones::list(&w.ctx).unwrap()[0];
    assert_eq!((row["phone_id"].as_str(), row["route"].as_str()), (Some(me.id.as_str()), Some("hub")));
    let _desk = desk_loop(&w.ctx);

    // the snapshot shows what /c1 would: the request with the version an answer names
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    let reqs = eventually("the request reaches the phone", || w.rt.block_on(m.requests()).ok().filter(|r| r.requests.len() == 1));
    assert_eq!((reqs.requests[0].ask_id.as_str(), reqs.requests[0].state_version, reqs.requests[0].what.as_str()), (ask.as_str(), v, "npm test"));
    assert!(reqs.actions.answer.ok);
    assert_eq!(w.rt.block_on(m.overview()).unwrap().needs_you, 1);
    let hello = w.rt.block_on(m.hello()).unwrap();
    assert_eq!((hello.computer.id.as_str(), hello.grants.control), (w.ctx.settings.device_id.as_str(), true));
    assert!(w.rt.block_on(m.accounts()).is_ok());

    // answered through the same entry point as /c1: accepted once, the same action id again is the stored receipt
    let first = w.rt.block_on(m.act(&answer(&run, &ask, v, "h-1"))).unwrap();
    assert_eq!((first.status, first.detail.as_str()), (Status::Accepted, super::actions::ACCEPTED));
    assert_eq!(ask_state(&w.ctx, &ask), ("answered".into(), "allow".into()));
    w.ctx.db.with(|c| Ok(c.execute("update run_ask set answer = 'deny-marker' where id = ?", [&ask])?)).unwrap();
    assert_eq!(w.rt.block_on(m.act(&answer(&run, &ask, v, "h-1"))).unwrap(), first);
    assert_eq!(ask_state(&w.ctx, &ask).1, "deny-marker", "nothing done again");
    assert_eq!(w.rt.block_on(m.receipt("h-1")).unwrap(), Some(first.clone()));
    assert_eq!(w.rt.block_on(m.receipt("never-sent")).unwrap(), None);

    // a message the hub changed, or moved to another slot (a replay), is never acted on
    let (run2, ask2, v2) = waiting(&w.ctx, 60_000);
    let body = json!({"kind": "act", "action": answer(&run2, &ask2, v2, "t-1")});
    let x = desk.x25519.as_deref().unwrap();
    let next = up_max(&w, &j) + 1;
    let mut sealed: Value = serde_json::from_slice(&mailbox::seal_mail(me.signing_key(), x, &j.mailbox, "up", next, &body).unwrap()).unwrap();
    let mut ct = base64::engine::general_purpose::STANDARD.decode(sealed["ct"].as_str().unwrap()).unwrap();
    ct[0] ^= 1;
    sealed["ct"] = json!(base64::engine::general_purpose::STANDARD.encode(&ct));
    assert_eq!(raw(&w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{next}", j.mailbox), Some(sealed.to_string().into_bytes())), 204);
    let moved = mailbox::seal_mail(me.signing_key(), x, &j.mailbox, "up", next + 1, &body).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{}", j.mailbox, next + 2), Some(moved)), 204);
    let k = format!("mail:{}:up", j.mailbox);
    eventually("both are read", || (w.ctx.db.kv_get(&k).unwrap().and_then(|s| s.parse::<i64>().ok()) >= Some(next + 2)).then_some(()));
    assert_eq!(ask_state(&w.ctx, &ask2).0, "open", "neither reached the session");
    assert_eq!(w.rt.block_on(m.receipt("t-1")).unwrap(), None, "the computer never took it");

    // and a snapshot the hub changed is never shown on the phone
    drop(_desk);
    w.rt.block_on(m.sync()).unwrap();
    let (_, ask3, _) = waiting(&w.ctx, 60_000);
    pass(&w.ctx).unwrap();
    let hub_db = rusqlite::Connection::open(w.hub_dir.path().join("hub.db")).unwrap();
    hub_db.execute("update mail set body = cast(replace(cast(body as text), '\"ct\":\"', '\"ct\":\"AA') as blob) where phone = ?1 and dir = 'down' and seq = (select max(seq) from mail where phone = ?1 and dir = 'down')", [&j.mailbox]).unwrap();
    let dropped = w.rt.block_on(m.dropped());
    let shown = w.rt.block_on(m.requests()).unwrap();
    assert_eq!(w.rt.block_on(m.dropped()), dropped + 1);
    assert!(!shown.requests.iter().any(|r| r.ask_id == ask3), "the changed snapshot is not read");
    let _desk = desk_loop(&w.ctx);

    // Control withdrawn: an answer is refused as /c1 refuses it; Summaries withdrawn: no overview, hello still
    crate::phones::set_grants(&w.ctx, &me.id, &json!({"control": false})).unwrap();
    assert_eq!(status_of(w.rt.block_on(m.act(&answer(&run2, &ask2, v2, "g-1")))), 403);
    assert_eq!(ask_state(&w.ctx, &ask2).0, "open");
    crate::phones::set_grants(&w.ctx, &me.id, &json!({"summaries": false})).unwrap();
    eventually("the phone loses the overview", || (w.rt.block_on(m.overview()).err()?.downcast::<Refused>().ok()?.status == 403).then_some(()));
    assert_eq!(status_of(w.rt.block_on(m.requests())), 403);
    assert!(!w.rt.block_on(m.hello()).unwrap().grants.summaries);

    // revoked: the hub's token and mailbox go at once, acknowledged; the phone reads as removed, not unreachable
    let r = crate::phones::revoke(&w.ctx, &me.id).unwrap();
    assert_eq!(r["acks"]["hub"], true);
    assert_eq!(crate::phones::list(&w.ctx).unwrap()[0]["acks"]["hub"], true);
    let e = w.rt.block_on(m.hello()).unwrap_err();
    assert!(matches!(e.downcast_ref::<ConnectError>(), Some(ConnectError::Revoked(_))), "{e:#}");
    let desk_token = crate::sync::hub::token(&w.ctx).unwrap();
    assert_eq!(raw(&w, &desk_token, reqwest::Method::GET, &format!("/v1/mail/{}/up", j.mailbox), None), 404);
}

#[test]
fn a_refused_phone_gets_nothing_and_its_mailbox_goes() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (p, j) = joined(&w, &me);
    p.refuse().unwrap();
    assert_eq!(w.rt.block_on(j.wait(Duration::from_millis(100))).unwrap(), Outcome::Refused);
    eventually("the pairing ends", || p.ended().then_some(()));
    assert_eq!(p.status()["fetched"], true);
    assert!(crate::phones::get(&w.ctx, &me.id).unwrap().is_none(), "nothing stored");
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 401, "the phone's token is gone");
    assert!(p.approve(Grants::default()).is_err(), "a refusal stands");
}

#[test]
fn a_phone_paired_again_directly_loses_its_hub_mailbox() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, _, _) = paired(&w, &me, Grants::default());
    crate::phones::add(&w.ctx, &companion_proto::pairing::PhonePayload::of(&me, "Pixel 9", "android").unwrap(), Grants::default()).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 401);
    assert!(crate::phones::list(&w.ctx).unwrap()[0]["route"].is_null());
}
