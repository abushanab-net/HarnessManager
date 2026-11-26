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
    let mut sealed: Value = serde_json::from_slice(&mailbox::seal_mail(me.signing_key(), x, &j.mailbox, desk.enrollment.as_deref().unwrap(), "up", next, &body).unwrap()).unwrap();
    let mut ct = base64::engine::general_purpose::STANDARD.decode(sealed["ct"].as_str().unwrap()).unwrap();
    ct[0] ^= 1;
    sealed["ct"] = json!(base64::engine::general_purpose::STANDARD.encode(&ct));
    assert_eq!(raw(&w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{next}", j.mailbox), Some(sealed.to_string().into_bytes())), 204);
    let moved = mailbox::seal_mail(me.signing_key(), x, &j.mailbox, desk.enrollment.as_deref().unwrap(), "up", next + 1, &body).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{}", j.mailbox, next + 2), Some(moved)), 204);
    // both looked at, neither moving the cursor the computer keeps
    eventually("both are read", || (super::mailbox::was_skipped(&j.mailbox, next) && super::mailbox::was_skipped(&j.mailbox, next + 2)).then_some(()));
    let k = format!("mail:{}:up", j.mailbox);
    assert!(w.ctx.db.kv_get(&k).unwrap().and_then(|s| s.parse::<i64>().ok()) < Some(next));
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
    // #10: a receipt the phone got before is asked for again, and the computer answers under the grants of now
    assert_eq!(status_of(w.rt.block_on(m.receipt("h-1"))), 403);
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

// ---------- the review's findings, one test each ----------

/// An action sealed by the phone's key for its next slot up, put straight on the hub (the app's `act` waits for the
/// answer; these tests run the computer's pass by hand).
fn put_act(w: &World, j: &HubJoining, me: &PhoneIdentity, desk: &DesktopPayload, a: companion_proto::api::Action) {
    let seq = up_max(w, j) + 1;
    let m = mailbox::seal_mail(me.signing_key(), desk.x25519.as_deref().unwrap(), &j.mailbox, desk.enrollment.as_deref().unwrap(), "up", seq, &json!({"kind": "act", "action": a})).unwrap();
    assert_eq!(raw(w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{seq}", j.mailbox), Some(m)), 204);
}

fn actions_named(ctx: &Ctx, id: &str) -> i64 { ctx.db.read(|c| Ok(c.query_row("select count(*) from phone_actions where action_id = ?", [id], |r| r.get(0))?)).unwrap() }

/// #1: a message is handled for the enrollment whose key opened it. One replaced between the pass's read and the
/// message (a new generation, granted Control) does not lend the message its Control.
#[test]
fn a_message_is_acted_on_only_for_the_enrollment_that_sealed_it() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: false });
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_act(&w, &j, &me, &desk, answer(&run, &ask, v, "e-1"));
    let (ctx, id) = (w.ctx.clone(), me.id.clone());
    super::mailbox::AFTER_READ.with(|s| *s.borrow_mut() = Some(Box::new(move || {
        ctx.db.with(|c| Ok(c.execute("update phones set generation = 'replaced', grants = ? where phone_id = ?", rusqlite::params![r#"{"summaries":true,"transcripts":true,"control":true}"#, id])?)).unwrap();
    })));
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "open", "not answered on the replacement's Control");
    assert_eq!(actions_named(&w.ctx, "e-1"), 0);
}

/// #3: the approval is kept with the enrollment and delivered by the mailbox pass when the pairing itself does not
/// (a computer stopped after approving), with the pass running all through the pairing, and slot 2 stays the
/// decision's: the snapshots come after it.
#[test]
fn the_approval_reaches_the_phone_from_the_pass_with_the_worker_running_throughout() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let _desk = desk_loop(&w.ctx);
    let (p, j) = joined(&w, &me);
    p.hold_delivery();
    p.approve(Grants { summaries: true, transcripts: true, control: true }).unwrap();
    let Outcome::Approved(desk) = w.rt.block_on(j.wait(Duration::from_millis(100))).unwrap() else { panic!("not approved") };
    let m = j.mailbox(&desk).unwrap();
    eventually("a snapshot follows the decision", || w.rt.block_on(m.hello()).ok());
    let left: Option<String> = w.ctx.db.read(|c| Ok(c.query_row("select hub_pairing from phones where phone_id = ?", [&me.id], |r| r.get(0))?)).unwrap();
    assert!(left.is_none(), "delivered, so no longer kept");
}

/// #4: a snapshot built while a grant is withdrawn is not sent: what Summaries showed is not disclosed after it went.
#[test]
fn a_snapshot_built_before_a_grant_was_withdrawn_is_not_sent() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (_j, _desk, m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: false });
    pass(&w.ctx).unwrap();
    assert!(w.rt.block_on(m.requests()).unwrap().requests.is_empty());
    let (_, ask, _) = waiting(&w.ctx, 60_000);
    let (ctx, id) = (w.ctx.clone(), me.id.clone());
    super::mailbox::BEFORE_PUBLISH.with(|s| *s.borrow_mut() = Some(Box::new(move || { crate::phones::set_grants(&ctx, &id, &json!({"summaries": false})).unwrap(); })));
    pass(&w.ctx).unwrap();
    if let Ok(r) = w.rt.block_on(m.requests()) { assert!(!r.requests.iter().any(|q| q.ask_id == ask), "the request built under Summaries reached the phone"); }
    pass(&w.ctx).unwrap();
    assert_eq!(status_of(w.rt.block_on(m.requests())), 403);
}

/// #5: a hub that knows the code runs the exchange itself and presents the computer's real certificate with keys of
/// its own: the phone refuses an approval the computer's identity did not sign for this exchange.
#[test]
fn an_approval_not_signed_by_the_computer_is_refused() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let inv = crate::sync::hub::phone_invite(&w.ctx).unwrap();
    let (code, mb) = (inv["code"].as_str().unwrap().to_string(), inv["phone_id"].as_str().unwrap().to_string());
    let id = identity::load(&w.ctx).unwrap();
    let fp = companion_proto::pairing::fingerprint(&id.cert_der);
    let (url, me2, c2, f2) = (w.url.clone(), me.clone(), code.clone(), fp.clone());
    let joining = w.rt.spawn(async move { mailbox::join_hub(&url, &c2, Some(&f2), &me2, "Pixel 9", "android").await });
    // the hub's side of the exchange, with the computer's token as only the hub could also use
    let tok = crate::sync::hub::token(&w.ctx).unwrap();
    let get = |seq: i64| reqwest::blocking::Client::new().get(format!("{}/v1/mail/{mb}/up/{seq}", w.url)).bearer_auth(&tok).send().unwrap();
    let up1: Value = eventually("the phone's first message", || { let r = get(1); (r.status() == 200).then(|| r.json().unwrap()) });
    let (key, back) = companion_proto::pairing::answer(&code, &me.id, &w.ctx.settings.device_id, up1["msg"].as_str().unwrap()).unwrap();
    assert_eq!(raw(&w, &tok, reqwest::Method::PUT, &format!("/v1/mail/{mb}/down/1"), Some(json!({"kind": "pair-start", "msg": back}).to_string().into_bytes())), 204);
    let j = w.rt.block_on(joining).unwrap().unwrap();
    let forger = PhoneIdentity::generate().unwrap();
    let p = DesktopPayload { id: w.ctx.settings.device_id.clone(), name: "Desk".into(), cert_pem: id.cert_pem.clone(), pubkey: id.pubkey(), address_hints: vec![], hub_url: Some(w.url.clone()),
        hub_token: None, grants: Grants { summaries: true, transcripts: true, control: true }, x25519: Some(forger.x25519_public()), enrollment: Some("e".into()) };
    let sig = base64::engine::general_purpose::STANDARD.encode(ed25519_dalek::Signer::sign(forger.signing_key(), b"anything").to_bytes());
    let decision = json!({"kind": "pair-result", "state": "approved", "payload": key.seal(&p).unwrap(), "sig": sig});
    assert_eq!(raw(&w, &tok, reqwest::Method::PUT, &format!("/v1/mail/{mb}/down/2"), Some(decision.to_string().into_bytes())), 204);
    let e = w.rt.block_on(j.result()).unwrap_err();
    assert!(format!("{e:#}").contains("not signed by the computer"), "{e:#}");
}

/// #6: one pass reads a bounded number of one phone's messages; the rest wait for the next.
#[test]
fn one_pass_reads_a_bounded_number_of_a_phones_messages() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants::default());
    pass(&w.ctx).unwrap();
    let k = format!("mail:{}:up", j.mailbox);
    let cursor = || w.ctx.db.kv_get(&k).unwrap().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    let before = cursor();
    for i in 0..25 { put_sealed(&w, &j, &me, &desk, json!({"kind": "receipt", "action_id": format!("b-{i}"), "query": format!("q{i}")})); }
    pass(&w.ctx).unwrap();
    assert_eq!(cursor(), before + 20);
    pass(&w.ctx).unwrap();
    assert_eq!(cursor(), before + 25);
}

/// #7: a deletion the hub could not do is kept and done later, after the phone was paired again too, and its
/// acknowledgement lands on the enrollment it was owed for, not on the one after it.
#[test]
fn a_deletion_the_hub_missed_is_kept_and_done_later_for_that_enrollment_only() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, _desk, _m) = paired(&w, &me, Grants::default());
    let hub_db = rusqlite::Connection::open(w.hub_dir.path().join("hub.db")).unwrap();
    hub_db.execute_batch("begin exclusive").unwrap();
    assert_eq!(crate::phones::revoke(&w.ctx, &me.id).unwrap()["acks"]["hub"], false, "the hub could not answer");
    // paired again (directly), and revoked again, while the hub still cannot answer
    crate::phones::add(&w.ctx, &companion_proto::pairing::PhonePayload::of(&me, "Pixel 9", "android").unwrap(), Grants::default()).unwrap();
    crate::phones::revoke(&w.ctx, &me.id).unwrap();
    hub_db.execute_batch("commit").unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 200, "the first mailbox is still there");
    pass(&w.ctx).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 401, "deleted once the hub answered");
    let gone: Option<i64> = w.ctx.db.read(|c| Ok(c.query_row("select hub_gone from phones where phone_id = ?", [&me.id], |r| r.get(0))?)).unwrap();
    assert!(gone.is_none(), "the later enrollment had no mailbox there: nothing of it was acknowledged");
    assert_eq!(w.ctx.db.read(|c| Ok(c.query_row("select count(*) from phone_hub_deletes", [], |r| r.get::<_, i64>(0))?)).unwrap(), 0);
}

/// #9: a message claimed just before the computer stopped is answered by the next pass, once.
#[test]
fn a_message_claimed_when_the_computer_stopped_is_answered_by_the_next_pass() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    // the phone's word in slot 3 read first, so the stop lands on the action's own claim
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_act(&w, &j, &me, &desk, answer(&run, &ask, v, "s-1"));
    let k = format!("mail:{}:up", j.mailbox);
    let before = w.ctx.db.kv_get(&k).unwrap();
    super::mailbox::STOP_AFTER_CLAIM.with(|c| c.set(true));
    assert!(pass(&w.ctx).is_err());
    let _ = before;
    assert_eq!(kept_count(&w.ctx), 1, "stopped after the action's slot was claimed");
    assert_eq!((ask_state(&w.ctx, &ask).0.as_str(), actions_named(&w.ctx, "s-1")), ("open", 0), "and before its reservation");
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask), ("answered".into(), "allow".into()));
    let _desk = desk_loop(&w.ctx);
    assert_eq!(w.rt.block_on(m.receipt("s-1")).unwrap().map(|r| r.status), Some(Status::Accepted));
    assert_eq!(actions_named(&w.ctx, "s-1"), 1);
}

/// #11: only the phone can say it read the approval: something the hub put in slot 3 is not that word.
#[test]
fn only_the_phone_can_say_it_read_the_approval() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (p, j) = joined(&w, &me);
    p.approve(Grants::default()).unwrap();
    let hub_db = rusqlite::Connection::open(w.hub_dir.path().join("hub.db")).unwrap();
    hub_db.execute("insert into mail(phone, dir, seq, at, bytes, body) values(?, 'up', 3, ?, 2, x'7b7d')", rusqlite::params![j.mailbox, crate::util::now_ms()]).unwrap();
    hub_db.execute("update phones set up_max = 3 where id = ?", [&j.mailbox]).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(p.status()["fetched"], false);
    assert!(matches!(w.rt.block_on(j.result()).unwrap(), Outcome::Approved(_)));
}

// ---------- the second review's findings ----------

/// A message sealed by the phone's key for its next slot up (4 at the least: 1 to 3 are the pairing's).
fn put_sealed(w: &World, j: &HubJoining, me: &PhoneIdentity, desk: &DesktopPayload, body: Value) {
    let seq = (up_max(w, j) + 1).max(4);
    let m = mailbox::seal_mail(me.signing_key(), desk.x25519.as_deref().unwrap(), &j.mailbox, desk.enrollment.as_deref().unwrap(), "up", seq, &body).unwrap();
    assert_eq!(raw(w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{seq}", j.mailbox), Some(m)), 204);
}

fn kept_count(ctx: &Ctx) -> i64 { ctx.db.read(|c| Ok(c.query_row("select count(*) from phone_mail_in", [], |r| r.get(0))?)).unwrap() }

/// Another hub on loopback.
fn second_hub(w: &World) -> (String, axum_server::Handle<SocketAddr>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = hrm_hub::Config { data_dir: dir.path().to_path_buf(), admin_token: ADMIN.into(), bind: "127.0.0.1:0".parse().unwrap(), max_blob_mb: 64, retain_mb: 2048 };
    let (addr, handle) = w.rt.block_on(hrm_hub::serve(cfg)).unwrap();
    (format!("http://{addr}"), handle, dir)
}

/// P1: a phone paired through hub A, revoked once this computer moved to hub B: B saying "no such phone" settles
/// nothing; the deletion stays owed to A.
#[test]
fn a_revocation_is_settled_only_by_the_hub_the_phone_was_paired_through() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, _desk, _m) = paired(&w, &me, Grants::default());
    let (url_b, _hb, _db) = second_hub(&w);
    let ctx_b = Arc::new(crate::sync::hub::join(&w.ctx, &url_b, None, Some(ADMIN)).unwrap());
    let r = crate::phones::revoke(&ctx_b, &me.id).unwrap();
    assert_eq!(r["acks"]["hub"], false, "hub B cannot settle a mailbox on hub A");
    pass(&ctx_b).unwrap();
    assert_eq!(crate::phones::list(&ctx_b).unwrap()[0]["acks"]["hub"], false);
    let owed: Vec<String> = ctx_b.db.read(|c| Ok(c.prepare("select hub_url from phone_hub_deletes")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).unwrap();
    assert_eq!(owed, std::slice::from_ref(&w.url));
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 200, "the token on A is untouched and still owed");
}

/// P1: a revocation from before the deletions were kept (revoked, not acknowledged, nothing owed) is taken up.
#[test]
fn a_revocation_from_before_the_queue_is_taken_up() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, _desk, _m) = paired(&w, &me, Grants::default());
    w.ctx.db.with(|c| Ok(c.execute("update phones set revoked_at = 1 where phone_id = ?", [&me.id])?)).unwrap();
    pass(&w.ctx).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 401);
    assert_eq!(crate::phones::list(&w.ctx).unwrap()[0]["acks"]["hub"], true);
}

/// P1: a refusal that could not be delivered is delivered as decided: Control granted meanwhile does not make the
/// same message an answer.
#[test]
fn a_decided_reply_is_kept_and_sent_as_decided() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: false });
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "d-1")}));
    super::mailbox::FAIL_SEND.with(|c| c.set(1));
    assert!(pass(&w.ctx).is_err());
    crate::phones::set_grants(&w.ctx, &me.id, &json!({"control": true})).unwrap();
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "open", "the message was decided once, refused");
    assert_eq!(actions_named(&w.ctx, "d-1"), 0);
    let _ = m;
}

/// P1: an enrollment read that fails keeps the claimed message for the next pass instead of losing it.
#[test]
fn a_failed_enrollment_read_keeps_the_claimed_message() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "r-1")}));
    super::mailbox::FAIL_RELOAD.with(|c| c.set(true));
    let _ = pass(&w.ctx);
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask), ("answered".into(), "allow".into()), "the claimed message was answered on the next pass");
}

/// P1: a slot the hub fills with something that does not open, numbered high, does not move the cursor past the
/// phone's real messages below it.
#[test]
fn a_slot_that_does_not_open_does_not_move_the_cursor() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    pass(&w.ctx).unwrap();
    let hub_db = rusqlite::Connection::open(w.hub_dir.path().join("hub.db")).unwrap();
    hub_db.execute("insert into mail(phone, dir, seq, at, bytes, body) values(?, 'up', 500, ?, 2, x'7b7d')", rusqlite::params![j.mailbox, crate::util::now_ms()]).unwrap();
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "c-1")}));
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "answered");
}

/// P2: replies that keep failing do not let the claimed backlog grow past one pass's worth; a revoked phone's
/// claims go.
#[test]
fn a_backlog_stays_bounded_and_goes_with_its_enrollment() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants::default());
    pass(&w.ctx).unwrap();
    for i in 0..25 { put_sealed(&w, &j, &me, &desk, json!({"kind": "receipt", "action_id": format!("x-{i}"), "query": format!("q{i}")})); }
    super::mailbox::FAIL_SEND.with(|c| c.set(1000));
    for _ in 0..3 { let _ = pass(&w.ctx); }
    assert!(kept_count(&w.ctx) <= 20, "{} claimed and kept", kept_count(&w.ctx));
    super::mailbox::FAIL_SEND.with(|c| c.set(0));
    crate::phones::revoke(&w.ctx, &me.id).unwrap();
    let _ = pass(&w.ctx);
    assert_eq!(kept_count(&w.ctx), 0, "a revoked enrollment's claims go");
}

/// P2: a computer that stops dead after the phone redeemed its invite, before any decision: the invite is kept, and
/// once its time is up its token is deleted on the hub.
#[test]
fn an_interrupted_pairing_does_not_leave_a_live_token() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (p, j) = joined(&w, &me);
    p.crash();
    eventually("the pairing stopped", || p.ended().then_some(()));
    let _ = w.ctx.db.with(|c| Ok(c.execute("update phone_hub_invites set expires_at = 0", [])?));
    pass(&w.ctx).unwrap();
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 401);
}

/// #4: Summaries withdrawn just before the PUT (after the slot was found) stops the snapshot.
#[test]
fn a_grant_withdrawn_just_before_the_put_stops_the_snapshot() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (_j, _desk, m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: false });
    pass(&w.ctx).unwrap();
    assert!(w.rt.block_on(m.requests()).unwrap().requests.is_empty());
    let (_, ask, _) = waiting(&w.ctx, 60_000);
    let (ctx, id) = (w.ctx.clone(), me.id.clone());
    super::mailbox::BEFORE_PUT.with(|s| *s.borrow_mut() = Some(Box::new(move || { crate::phones::set_grants(&ctx, &id, &json!({"summaries": false})).unwrap(); })));
    let _ = pass(&w.ctx);
    if let Ok(r) = w.rt.block_on(m.requests()) { assert!(!r.requests.iter().any(|q| q.ask_id == ask), "the request built under Summaries reached the phone"); }
}

/// #5: the computer's own key signing an approval for an exchange other than the one this phone saw is refused.
#[test]
fn a_correctly_signed_approval_of_another_exchange_is_refused() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let inv = crate::sync::hub::phone_invite(&w.ctx).unwrap();
    let (code, mb) = (inv["code"].as_str().unwrap().to_string(), inv["phone_id"].as_str().unwrap().to_string());
    let id = identity::load(&w.ctx).unwrap();
    let fp = companion_proto::pairing::fingerprint(&id.cert_der);
    let (url, me2, c2) = (w.url.clone(), me.clone(), code.clone());
    let joining = w.rt.spawn(async move { mailbox::join_hub(&url, &c2, Some(&fp), &me2, "Pixel 9", "android").await });
    let tok = crate::sync::hub::token(&w.ctx).unwrap();
    let get = |seq: i64| reqwest::blocking::Client::new().get(format!("{}/v1/mail/{mb}/up/{seq}", w.url)).bearer_auth(&tok).send().unwrap();
    let up1: Value = eventually("the phone's first message", || { let r = get(1); (r.status() == 200).then(|| r.json().unwrap()) });
    let (key, back) = companion_proto::pairing::answer(&code, &me.id, &w.ctx.settings.device_id, up1["msg"].as_str().unwrap()).unwrap();
    let down1 = json!({"kind": "pair-start", "msg": back}).to_string().into_bytes();
    assert_eq!(raw(&w, &tok, reqwest::Method::PUT, &format!("/v1/mail/{mb}/down/1"), Some(down1.clone())), 204);
    let j = w.rt.block_on(joining).unwrap().unwrap();
    let up2 = get(2).bytes().unwrap().to_vec();
    let p = DesktopPayload { id: w.ctx.settings.device_id.clone(), name: "Desk".into(), cert_pem: id.cert_pem.clone(), pubkey: id.pubkey(), address_hints: vec![], hub_url: Some(w.url.clone()),
        hub_token: None, grants: Grants { summaries: true, transcripts: true, control: true }, x25519: Some(super::mailbox::keys(&w.ctx).unwrap().x_public), enrollment: Some("e".into()) };
    // the computer's real key, over an exchange whose first message is not the phone's
    let other = br#"{"kind":"pair-start","id":"someone-else","msg":"x"}"#.to_vec();
    let sig = id.sign(&mailbox::approval_binding(&mb, &me.id, &me.pubkey(), &me.x25519_public(), &p, [&other, &down1, &up2]));
    let decision = json!({"kind": "pair-result", "state": "approved", "payload": key.seal(&p).unwrap(), "sig": sig});
    assert_eq!(raw(&w, &tok, reqwest::Method::PUT, &format!("/v1/mail/{mb}/down/2"), Some(decision.to_string().into_bytes())), 204);
    assert!(w.rt.block_on(j.result()).is_err());
}

// ---------- the third review's findings ----------

/// A proxy on loopback in front of `to`, keeping every byte computers send through it.
fn proxy(w: &World, to: &str) -> (String, Arc<std::sync::Mutex<Vec<u8>>>) {
    let to = to.trim_start_matches("http://").to_string();
    let seen = Arc::new(std::sync::Mutex::new(vec![]));
    let s2 = seen.clone();
    let addr = w.rt.block_on(async move {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut client, _)) = l.accept().await else { return };
                let (to, seen) = (to.clone(), s2.clone());
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let Ok(up) = tokio::net::TcpStream::connect(&to).await else { return };
                    let (mut cr, mut cw) = client.split();
                    let (mut ur, mut uw) = tokio::io::split(up);
                    let read_client = async {
                        let mut buf = [0u8; 8192];
                        loop {
                            let n = cr.read(&mut buf).await.unwrap_or(0);
                            if n == 0 { let _ = uw.shutdown().await; return; }
                            seen.lock().unwrap().extend_from_slice(&buf[..n]);
                            if uw.write_all(&buf[..n]).await.is_err() { return; }
                        }
                    };
                    let back = async { let _ = tokio::io::copy(&mut ur, &mut cw).await; };
                    tokio::join!(read_client, back);
                });
            }
        });
        addr
    });
    (format!("http://{addr}"), seen)
}

/// P1: a pairing on hub A still open when this computer joins hub B: its cleanup never shows A the token B gave,
/// and the deletion stays owed to A.
#[test]
fn a_hub_never_receives_another_hubs_token() {
    let w = world();
    let (url_a, seen) = proxy(&w, &w.url);
    let ctx_a = Arc::new(crate::sync::hub::join(&w.ctx, &url_a, None, Some(ADMIN)).unwrap());
    let p = pair_start(ctx_a.clone(), Arc::new(|_: &str, _| {})).unwrap();
    let (url_b, _hb, _db) = second_hub(&w);
    let ctx_b = crate::sync::hub::join(&ctx_a, &url_b, None, Some(ADMIN)).unwrap();
    let tok_b = crate::sync::hub::token(&ctx_b).unwrap();
    p.cancel();
    eventually("the pairing ends", || p.ended().then_some(()));
    let sent = String::from_utf8_lossy(&seen.lock().unwrap()).to_string();
    assert!(!sent.contains(&tok_b), "hub A was sent hub B's token");
    let owed: Vec<String> = ctx_b.db.read(|c| Ok(c.prepare("select hub_url from phone_hub_deletes")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).unwrap();
    assert_eq!(owed, std::slice::from_ref(&url_a), "still owed to A");
}

/// P1: an enrollment whose hub is not known (paired by this branch before it kept the hub) is never settled by the
/// hub this computer happens to be on.
#[test]
fn an_enrollment_of_unknown_hub_is_never_settled_by_another() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, _desk, _m) = paired(&w, &me, Grants::default());
    w.ctx.db.with(|c| Ok(c.execute("update phones set hub_url = null where phone_id = ?", [&me.id])?)).unwrap();
    let (url_b, _hb, _db) = second_hub(&w);
    let ctx_b = Arc::new(crate::sync::hub::join(&w.ctx, &url_b, None, Some(ADMIN)).unwrap());
    assert_eq!(crate::phones::revoke(&ctx_b, &me.id).unwrap()["acks"]["hub"], false);
    pass(&ctx_b).unwrap();
    assert_eq!(crate::phones::list(&ctx_b).unwrap()[0]["acks"]["hub"], false, "hub B's 'no such phone' settles nothing");
    assert_eq!(raw(&w, &j.token, reqwest::Method::GET, &format!("/v1/mail/{}/down", j.mailbox), None), 200);
}

/// P1: the same phone paired again on the same mailbox with the same keys: what it sent under the earlier
/// enrollment is not this one's.
#[test]
fn a_message_of_an_earlier_enrollment_on_the_same_mailbox_is_refused() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: false });
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "old-1")}));
    let again = companion_proto::pairing::PhonePayload::of(&me, "Pixel 9", "android").unwrap();
    crate::phones::add_with(&w.ctx, &again, Grants { summaries: true, transcripts: true, control: true }, Some(&j.mailbox), None).unwrap();
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "open", "an answer of the earlier pairing acted under the new one");
    assert_eq!(actions_named(&w.ctx, "old-1"), 0);
}

/// P1: an action whose own slot the hub served wrong for a while is read once it serves it right, though a later
/// message was read meanwhile; the cursor kept waits below it.
#[test]
fn an_action_survives_its_slot_being_unavailable_for_a_while() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    pass(&w.ctx).unwrap();
    let (run, ask, v) = waiting(&w.ctx, 120_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "late-1")}));
    let slot = up_max(&w, &j);
    let hub_db = rusqlite::Connection::open(w.hub_dir.path().join("hub.db")).unwrap();
    let real: Vec<u8> = hub_db.query_row("select body from mail where phone = ? and dir = 'up' and seq = ?", rusqlite::params![j.mailbox, slot], |r| r.get(0)).unwrap();
    hub_db.execute("update mail set body = x'7b7d' where phone = ? and dir = 'up' and seq = ?", rusqlite::params![j.mailbox, slot]).unwrap();
    pass(&w.ctx).unwrap();
    put_sealed(&w, &j, &me, &desk, json!({"kind": "receipt", "action_id": "x", "query": "q1"}));
    pass(&w.ctx).unwrap();
    let k = format!("mail:{}:up", j.mailbox);
    assert!(w.ctx.db.kv_get(&k).unwrap().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0) < slot, "the kept cursor stays below the unread slot");
    hub_db.execute("update mail set body = ? where phone = ? and dir = 'up' and seq = ?", rusqlite::params![real, j.mailbox, slot]).unwrap();
    std::thread::sleep(Duration::from_millis(2200));
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "answered");
}

/// Partly (#6): a pass fetches at most 20 slots of one phone, those that do not open included; a real message behind
/// them is read by the next pass.
#[test]
fn a_pass_fetches_at_most_twenty_slots_bad_ones_included() {
    let w = world();
    let me = PhoneIdentity::generate().unwrap();
    let (j, desk, _m) = paired(&w, &me, Grants { summaries: true, transcripts: true, control: true });
    pass(&w.ctx).unwrap();
    let start = up_max(&w, &j).max(3);
    for seq in start + 1..=start + 20 { assert_eq!(raw(&w, &j.token, reqwest::Method::PUT, &format!("/v1/mail/{}/up/{seq}", j.mailbox), Some(b"{}".to_vec())), 204); }
    let (run, ask, v) = waiting(&w.ctx, 60_000);
    put_sealed(&w, &j, &me, &desk, json!({"kind": "act", "action": answer(&run, &ask, v, "behind-1")}));
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "open", "twenty fetches were spent on the bad slots");
    pass(&w.ctx).unwrap();
    assert_eq!(ask_state(&w.ctx, &ask).0, "answered", "the next pass reads what is new before trying the bad slots again");
}
