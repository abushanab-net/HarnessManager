//! A fake phone paired with a real Runtime on loopback, end to end over real TLS: the phone is companion-proto's own
//! clients (`join` for pairing, `Client` and `Events` for the companion API), as the app will be. The pairing, every
//! `/c1` route, the grant boundaries, revocation and the action contract's cases that need no terminal.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use companion_proto::api::{Action, ActionKind, Grants, Refused, Status, Target, Update};
use companion_proto::join::{self, Joining, Outcome};
use companion_proto::pairing::DesktopPayload;
use companion_proto::{pairing, Client, PhoneIdentity};
use rusqlite::params;
use serde_json::{json, Value};

use crate::sync::peer::{identity, Runtime};
use crate::util::now_ms;
use crate::Ctx;

struct Desk {
    ctx: Arc<Ctx>,
    rt: Runtime,
    events: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Drop for Desk {
    fn drop(&mut self) {
        self.rt.stop();
        let _ = crate::secrets::delete(&identity::target(&self.ctx.settings.device_id));
    }
}

fn desk(dir: &std::path::Path, name: &str) -> Desk {
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
    let data = dir.join(name);
    let mut ctx = Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap();
    ctx.settings.device_name = name.into();
    ctx.settings.direct_connection = true;
    ctx.save_settings().unwrap();
    let ctx = Arc::new(Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap());
    let events = Arc::new(Mutex::new(vec![]));
    let ev = events.clone();
    let rt = Runtime::new(Ipv4Addr::LOCALHOST.into(), Arc::new(move |k: &str, v| ev.lock().unwrap().push((k.to_string(), v)))).unwrap();
    let ctx = rt.ensure(&ctx).unwrap();
    Desk { ctx, rt, events }
}

/// The phone's own runtime, as the app's wrapper gives it one.
fn phone_rt() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap() }

/// A phone as the app will be: its identity, the computer it paired with and its client.
struct Phone {
    me: PhoneIdentity,
    desk: DesktopPayload,
    c: Client,
}

impl Phone {
    /// A raw request with the phone's own TLS client, for routes the phone's client does not call (`/v1`).
    fn status_of(&self, path: &str) -> u16 {
        let cfg = companion_proto::net::tls_config(&self.me, &self.desk.cert_pem).unwrap();
        let http = reqwest::blocking::Client::builder().no_proxy().use_preconfigured_tls((*cfg).clone()).timeout(Duration::from_secs(20)).build().unwrap();
        http.get(format!("https://{}{path}", self.desk.address_hints[0])).send().unwrap().status().as_u16()
    }
}

/// The status a refused call answered with.
fn refused<T: std::fmt::Debug>(r: anyhow::Result<T>) -> u16 { r.unwrap_err().downcast::<Refused>().expect("an answer from the computer").status }

/// The phone joined with the QR code, up to the owner's decision; the computer shows the same words.
fn joined(rt: &tokio::runtime::Runtime, d: &Desk) -> (PhoneIdentity, Joining) {
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let qr = v["qr"].as_str().unwrap().to_string();
    let (host, code, _) = pairing::parse_qr(&qr).unwrap();
    assert_eq!((host, code.as_str()), (format!("127.0.0.1:{}", v["port"]), v["code"].as_str().unwrap()));
    let me = PhoneIdentity::generate().unwrap();
    let j = rt.block_on(join::join_qr(&qr, &me, "Pixel 9", "android")).unwrap();
    let st = d.rt.phone_pair_status().unwrap();
    assert_eq!((st["state"].as_str(), st["phone"]["name"].as_str()), (Some("confirm"), Some("Pixel 9")));
    assert_eq!(st["words"], json!(j.words()), "both sides show the same three words");
    assert_eq!((j.desktop_id.as_str(), j.desktop_name.as_str()), (d.ctx.settings.device_id.as_str(), d.ctx.settings.device_name.as_str()));
    (me, j)
}

/// A phone paired with `d` and approved with `grants`.
fn pair(rt: &tokio::runtime::Runtime, d: &Desk, grants: Value) -> Phone {
    let (me, j) = joined(rt, d);
    assert!(matches!(rt.block_on(j.result()).unwrap(), Outcome::Pending(ms) if ms > 0), "nothing sealed before the owner approves");
    d.rt.phone_pair_approve(&d.ctx, &grants).unwrap();
    let Outcome::Approved(desk) = rt.block_on(j.wait(Duration::from_millis(100))).unwrap() else { panic!("not approved") };
    assert_eq!(desk.address_hints[0], format!("127.0.0.1:{}", d.ctx.settings.listen_port));
    let c = Client::new(&me, &desk.cert_pem, &desk.address_hints[0]).unwrap();
    Phone { me, desk, c }
}

fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

#[test]
fn a_phone_pairs_with_the_owners_approval_and_is_never_a_peer() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({"control": true}));
    assert_eq!(p.desk.grants, Grants { summaries: true, transcripts: true, control: true });
    assert_eq!((p.desk.hub_url.as_deref(), p.desk.hub_token.as_deref()), (None, None));
    // the payload is typed: a ring key has no field to travel in, and the phone is no peer
    assert_eq!((count(&d.ctx, "select count(*) from peers"), count(&d.ctx, "select count(*) from phones")), (0, 1));
    let list = crate::phones::list(&d.ctx).unwrap();
    assert_eq!((list[0]["phone_id"].as_str(), list[0]["platform"].as_str()), (Some(p.me.id.as_str()), Some("android")));
    assert_eq!(crate::phones::question_seconds(&d.ctx), 60, "a phone with Control lengthens the question window");
    let states: Vec<String> = d.events.lock().unwrap().iter().filter(|(k, _)| k == "phones-pairing").map(|(_, v)| v["state"].as_str().unwrap().to_string()).collect();
    assert_eq!(states, ["confirm", "paired"]);
    // the phone reaches the companion API over mutual TLS, keeps its connection, and opens a fresh one on resume
    let h = rt.block_on(p.c.hello()).unwrap();
    assert_eq!((h.api, h.computer.name.as_str(), h.grants), (1, "A", p.desk.grants));
    assert_eq!(rt.block_on(p.c.hello()).unwrap().computer.id, d.ctx.settings.device_id);
    assert_eq!(crate::phones::list(&d.ctx).unwrap()[0]["route"], "direct", "the kept connection is open");
    assert_eq!(rt.block_on(p.c.resume()).unwrap().api, 1);
    // the computer's listener restarts under the kept connection: the next call opens a new one by itself
    d.rt.stop();
    d.rt.ensure(&d.ctx).unwrap();
    assert_eq!(rt.block_on(p.c.overview()).unwrap().computer.name, "A");
    assert!(crate::phones::list(&d.ctx).unwrap()[0]["last_seen"].is_i64(), "heard from");
    // a stranger's certificate, or a computer not the one pinned, gets no further than the handshake
    let stranger = Client::new(&PhoneIdentity::generate().unwrap(), &p.desk.cert_pem, &p.desk.address_hints[0]).unwrap();
    assert!(rt.block_on(stranger.hello()).unwrap_err().downcast_ref::<Refused>().is_none());
    let other = PhoneIdentity::generate().unwrap();
    let wrong_pin = Client::new(&p.me, &other.cert().unwrap().0, &p.desk.address_hints[0]).unwrap();
    assert!(rt.block_on(wrong_pin.hello()).is_err());
}

#[test]
fn three_wrong_codes_end_the_phone_pairing_and_the_owner_can_refuse() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let wrong = format!("{:06}", (v["code"].as_str().unwrap().parse::<u32>().unwrap() + 1) % 1_000_000);
    let me = PhoneIdentity::generate().unwrap();
    for i in 0..3 {
        let e = rt.block_on(join::join(v["host_hint"].as_str().unwrap(), &wrong, None, &me, "Pixel 9", "android")).err().unwrap_or_else(|| panic!("try {i} paired"));
        assert_eq!(format!("{e:#}"), "the code did not match; check it and try again", "try {i}");
    }
    let st = d.rt.phone_pair_status().unwrap();
    assert_eq!((st["state"].as_str(), st["error"].as_str()), (Some("failed"), Some("three wrong codes; show a new one")));
    assert!(d.rt.phone_pair_approve(&d.ctx, &json!({})).is_err());
    // a new code; the owner refuses the phone shown
    let (_, j) = joined(&rt, &d);
    assert_eq!(d.rt.phone_pair_refuse().unwrap()["state"], "refused");
    assert_eq!(rt.block_on(j.wait(Duration::from_millis(100))).unwrap(), Outcome::Refused);
    assert_eq!(count(&d.ctx, "select count(*) from phones"), 0);
    assert_eq!(d.rt.phone_pair_cancel().unwrap(), Value::Null);
    assert!(d.rt.phone_pair_status().is_none());
}

#[test]
fn a_phone_that_scanned_another_computers_code_does_not_trust_this_one() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (desk(t.path(), "A"), desk(t.path(), "B"));
    let rt = phone_rt();
    let va = a.rt.phone_pair_start(&a.ctx, Some("127.0.0.1")).unwrap();
    let vb = b.rt.phone_pair_start(&b.ctx, Some("127.0.0.1")).unwrap();
    // A's address and code, with B's fingerprint: the payload opens, but names a certificate the QR code did not
    let (_, _, fb) = pairing::parse_qr(vb["qr"].as_str().unwrap()).unwrap();
    let me = PhoneIdentity::generate().unwrap();
    let j = rt.block_on(join::join(va["host_hint"].as_str().unwrap(), va["code"].as_str().unwrap(), Some(&fb), &me, "Pixel 9", "android")).unwrap();
    a.rt.phone_pair_approve(&a.ctx, &json!({})).unwrap();
    let e = rt.block_on(j.result()).unwrap_err();
    assert_eq!(format!("{e:#}"), "that is not the computer the QR code named");
}

#[test]
fn every_route_answers_a_phone_with_its_grants_and_nothing_outside_them() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    // one account with one session on this computer, for the lists and a transcript
    let mut reg = d.ctx.registry().unwrap();
    reg.add_profile(crate::registry::Profile { name: "claude-main".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true,
        base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 }).unwrap();
    d.ctx.save_registry(&reg).unwrap();
    let cwd = t.path().join("app");
    std::fs::create_dir_all(&cwd).unwrap();
    let f = d.ctx.home.join(".claude").join("projects").join(crate::harness::claude::project_dir_name(&cwd)).join("s1.jsonl");
    let stamp = chrono::Utc::now().to_rfc3339();
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, [json!({"type": "user", "sessionId": "s1", "cwd": cwd, "timestamp": stamp, "message": {"role": "user", "content": "fix the build"}}),
        json!({"type": "assistant", "sessionId": "s1", "cwd": cwd, "timestamp": stamp, "message": {"id": "m1", "model": "claude-opus-5-5", "usage": {"input_tokens": 10, "output_tokens": 5}, "content": [{"type": "text", "text": "done"}]}})]
        .iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();

    let full = pair(&rt, &d, json!({"control": true}));
    let c = &full.c;
    let o = rt.block_on(c.overview()).unwrap();
    assert_eq!((o.needs_you, o.computer.id.as_str()), (0, d.ctx.settings.device_id.as_str()));
    assert!(o.today["tokens"].is_i64() && o.observed_at > 0, "{o:?}");
    let a = rt.block_on(c.accounts()).unwrap();
    assert_eq!(a.computer.name, "A");
    for acct in &a.accounts { assert_eq!(rt.block_on(c.account(&acct.profile)).unwrap().account, *acct); }
    assert_eq!(refused(rt.block_on(c.account("nobody"))), 404);
    let page = rt.block_on(c.sessions(None)).unwrap();
    let one = page.sessions.iter().find(|x| x.session.id == "s1").unwrap_or_else(|| panic!("{page:?}"));
    assert_eq!((one.session.computer.name.as_str(), one.session.profile.as_str(), page.next_cursor), ("A", "claude-main", None));
    assert!(rt.block_on(c.sessions(Some(1))).unwrap().sessions.is_empty(), "past the last page");
    let tr = rt.block_on(c.transcript("claude", "claude-main", "s1", None)).unwrap();
    assert_eq!((tr.turns[0]["prompt"].as_str(), tr.next, tr.total), (Some("fix the build"), None, 1));
    assert!(rt.block_on(c.transcript("claude", "claude-main", "s1", Some(1))).unwrap().turns.is_empty());
    assert_eq!(refused(rt.block_on(c.transcript("codex", "claude-main", "s1", None))), 404, "another tool's session of that id");
    assert_eq!(refused(rt.block_on(c.transcript("claude", "claude-main", "../s1", None))), 404, "a name with a slash is one segment");
    let r = rt.block_on(c.requests()).unwrap();
    assert_eq!((r.requests.len(), r.actions.answer.ok), (0, true));
    rt.block_on(c.push_register("h-1")).unwrap();
    assert_eq!(d.ctx.db.read(|c| Ok(c.query_row("select relay_handle from phones", [], |r| r.get::<_, String>(0))?)).unwrap(), "h-1");
    assert_eq!(refused(rt.block_on(c.push_register(" "))), 400);
    rt.block_on(c.push_remove()).unwrap();
    assert_eq!(count(&d.ctx, "select count(*) from phones where relay_handle is not null"), 0);
    assert_eq!(rt.block_on(c.receipt("never-sent")).unwrap(), None);
    // a phone is refused on every computer route
    let me = d.ctx.settings.device_id.clone();
    for path in ["/v1/hello".to_string(), format!("/v1/chunks/{me}"), format!("/v1/chunk/{me}/x.jsonl"), "/v1/presence".into(), format!("/v1/transcript/{me}/claude/claude-main/s1")] {
        assert_eq!(full.status_of(&path), 403, "{path}");
    }

    // summaries only: no transcripts, no actions, no computer routes
    let small = pair(&rt, &d, json!({"summaries": true, "transcripts": false, "control": false}));
    assert!(rt.block_on(small.c.overview()).is_ok());
    assert!(!rt.block_on(small.c.requests()).unwrap().actions.answer.ok);
    assert_eq!(refused(rt.block_on(small.c.transcript("claude", "claude-main", "s1", None))), 403);
    let act = Action { action_id: "x".into(), kind: ActionKind::Stop, target: Target { run: "1:2".into(), ask_id: None }, state_version: 0, expires_at: now_ms() + 30_000, payload: json!({}) };
    assert_eq!(refused(rt.block_on(small.c.act(&act))), 403);
    assert_eq!(refused(rt.block_on(small.c.receipt("x"))), 403);
    for path in ["/v1/hello".to_string(), format!("/v1/chunks/{me}"), "/v1/presence".into()] { assert_eq!(small.status_of(&path), 403, "{path}"); }
    // nothing granted: hello and the push handle only
    let none = pair(&rt, &d, json!({"summaries": false, "transcripts": false, "control": false}));
    assert!(rt.block_on(none.c.hello()).is_ok());
    rt.block_on(none.c.push_register("h-2")).unwrap();
    assert_eq!(refused(rt.block_on(none.c.overview())), 403);
    assert_eq!(refused(rt.block_on(none.c.accounts())), 403);
    assert_eq!(refused(rt.block_on(none.c.sessions(None))), 403);
    assert_eq!(refused(rt.block_on(none.c.requests())), 403);
    assert!(rt.block_on(none.c.events()).is_err(), "no events without Summaries");
    // a grant switched off holds at the next request, on the same kept connection
    crate::phones::set_grants(&d.ctx, &full.me.id, &json!({"transcripts": false})).unwrap();
    assert_eq!(refused(rt.block_on(c.transcript("claude", "claude-main", "s1", None))), 403);
}

#[test]
fn a_paired_computer_is_refused_on_the_companion_api() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (desk(t.path(), "A"), desk(t.path(), "B"));
    let v = a.rt.pair_start(&a.ctx, Some("127.0.0.1")).unwrap();
    b.rt.pair_join(&b.ctx, v["code"].as_str().unwrap(), v["host_hint"].as_str()).unwrap();
    let peer = crate::journal::peers(&b.ctx).unwrap().remove(0);
    let c = crate::sync::peer::client::Client::connect(&identity::load(&b.ctx).unwrap(), &peer.cert_pem, "127.0.0.1", a.ctx.settings.listen_port).unwrap();
    assert!(c.json("/v1/hello").unwrap().is_some(), "the computer routes answer it");
    for path in ["/c1/hello", "/c1/overview", "/c1/requests"] {
        let e = c.json(path).unwrap_err();
        assert!(format!("{e:#}").contains("403"), "{path}: {e:#}");
    }
}

#[test]
fn revoking_a_phone_closes_its_open_events_and_refuses_it_from_then_on() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({}));
    rt.block_on(async {
        let mut ev = p.c.events().await.expect("the socket opens");
        let (first, update) = tokio::time::timeout(Duration::from_secs(5), ev.next()).await.unwrap().unwrap().unwrap();
        assert_eq!((first.kind.as_str(), update), ("needs", Update::Needs(vec![])));
        assert_eq!(crate::phones::list(&d.ctx).unwrap()[0]["route"], "direct");
        let (ctx, id) = (d.ctx.clone(), p.me.id.clone());
        let v = tokio::task::spawn_blocking(move || crate::phones::revoke(&ctx, &id).unwrap()).await.unwrap();
        assert_eq!(v["acks"], json!({"connection": true, "hub": null, "push": null}));
        // the socket ends at once: cut under its TLS session, or told `revoked` first
        let end = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match ev.next().await {
                    Some(Ok((_, Update::Revoked))) => continue,
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => return,
                }
            }
        }).await;
        assert!(end.is_ok(), "the socket stayed open after revoke");
        let e = p.c.resume().await.unwrap_err();
        assert!(e.downcast_ref::<Refused>().is_none(), "refused at the handshake, not by a route: {e:#}");
    });
    assert_eq!(crate::phones::list(&d.ctx).unwrap()[0]["route"], Value::Null);
}

// ---------- actions: a run and its open request as `hrm run` and its hook leave them, the hook being this process ----------

/// A live run waiting on a permission request; answers (run, ask id, state version).
fn waiting(ctx: &Ctx, expires_in: i64) -> (String, String, i64) {
    let pid = std::process::id();
    let started = crate::procs::start_ms(pid).unwrap();
    let (run, ask) = (format!("{pid}:{}", crate::util::uuid4()), crate::util::uuid4());
    ctx.db.with(|c| {
        c.execute("insert into run_state(run, pid, started, profile, harness, session_id, state, version) values(?, ?, ?, 'work', 'claude', 's-1', 'waiting', 3)", params![run, pid, started])?;
        c.execute("insert into run_ask(id, run, kind, tool, what, created, expires, hook_pid, hook_started) values(?, ?, 'permission', 'Bash', 'npm test', ?, ?, ?, ?)",
            params![ask, run, now_ms(), now_ms() + expires_in, pid, started])?;
        Ok(())
    }).unwrap();
    (run, ask, 3)
}

fn answer(run: &str, ask: &str, version: i64, id: &str) -> Action {
    Action { action_id: id.into(), kind: ActionKind::Answer, target: Target { run: run.into(), ask_id: Some(ask.into()) }, state_version: version, expires_at: now_ms() + 30_000, payload: json!({"behavior": "allow"}) }
}

fn ask_state(ctx: &Ctx, ask: &str) -> (String, String) { ctx.db.read(|c| Ok(c.query_row("select state, answer from run_ask where id = ?", [ask], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap() }

#[test]
fn the_action_contract_holds_without_a_terminal() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({"control": true}));
    let act = |a: Action| rt.block_on(p.c.act(&a)).unwrap();

    // the request is listed with the version an answer must name
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    let reqs = rt.block_on(p.c.requests()).unwrap();
    assert_eq!((reqs.requests[0].ask_id.as_str(), reqs.requests[0].state_version, reqs.requests[0].what.as_str()), (ask.as_str(), v, "npm test"));
    assert_eq!(rt.block_on(p.c.overview()).unwrap().needs_you, 1);
    let mine = rt.block_on(p.c.sessions(None)).unwrap();
    assert!(mine.sessions.iter().all(|s| s.run.is_none()), "no session file for this run: {mine:?}");

    // accepted, once: the same action_id again is the stored receipt and changes nothing
    let first = act(answer(&run, &ask, v, "a-1"));
    assert_eq!((first.status, first.detail.as_str()), (Status::Accepted, "answered"));
    assert_eq!(ask_state(&d.ctx, &ask), ("answered".into(), "allow".into()));
    d.ctx.db.with(|c| Ok(c.execute("update run_ask set answer = 'deny-marker' where id = ?", [&ask])?)).unwrap();
    assert_eq!(act(answer(&run, &ask, v, "a-1")), first);
    assert_eq!(ask_state(&d.ctx, &ask).1, "deny-marker", "nothing done again");
    assert_eq!(rt.block_on(p.c.receipt("a-1")).unwrap(), Some(first.clone()), "the receipt for delivery unknown");

    // the terminal answered first: the request closed and the version moved on
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    d.ctx.db.with(|c| { c.execute("update run_ask set state = 'closed' where id = ?", [&ask])?; c.execute("update run_state set state = 'working', version = version + 1 where run = ?", [&run])?; Ok(()) }).unwrap();
    assert_eq!(act(answer(&run, &ask, v, "a-2")).status, Status::AlreadyHandled);

    // expiry: the request's, and the action's own while the phone unlocked
    let (run, ask, v) = waiting(&d.ctx, -1);
    assert_eq!(act(answer(&run, &ask, v, "a-3")).status, Status::Expired);
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    let mut late = answer(&run, &ask, v, "a-4");
    late.expires_at = now_ms() - 1;
    assert_eq!(act(late).status, Status::Expired);
    assert_eq!(ask_state(&d.ctx, &ask).0, "open", "an expired action does nothing");

    // a state the phone did not see
    assert_eq!(act(answer(&run, &ask, v + 1, "a-5")).status, Status::Expired);

    // two answers at once, from two phones: one is accepted
    let other = pair(&rt, &d, json!({"control": true}));
    let (a1, a2) = (answer(&run, &ask, v, "b-1"), answer(&run, &ask, v, "b-2"));
    let (r1, r2) = rt.block_on(async { tokio::join!(p.c.act(&a1), other.c.act(&a2)) });
    let (r1, r2) = (r1.unwrap(), r2.unwrap());
    let mut both = [r1.status, r2.status];
    both.sort_by_key(|s| s.as_str());
    assert_eq!(both, [Status::Accepted, Status::AlreadyHandled], "{r1:?} {r2:?}");
    // another phone's action id is not this phone's receipt
    assert_eq!(rt.block_on(other.c.receipt("a-1")).unwrap(), None);

    // reply and Stop keep the desktop's rules: no owned terminal here
    let (run, _, v) = waiting(&d.ctx, 60_000);
    for (kind, payload) in [(ActionKind::Reply, json!({"text": "go on"})), (ActionKind::Stop, json!({}))] {
        let r = act(Action { action_id: format!("c-{kind:?}"), kind, target: Target { run: run.clone(), ask_id: None }, state_version: v, expires_at: now_ms() + 30_000, payload });
        assert_eq!((r.status, r.detail.as_str()), (Status::Unavailable, "Needs Harness Manager's own terminal"), "{kind:?}");
    }
    let gone = act(Action { action_id: "c-gone".into(), kind: ActionKind::Stop, target: Target { run: "1:2".into(), ask_id: None }, state_version: 0, expires_at: now_ms() + 30_000, payload: json!({}) });
    assert_eq!(gone.status, Status::Unavailable);

    // Control taken away between the screen and the tap
    crate::phones::set_grants(&d.ctx, &p.me.id, &json!({"control": false})).unwrap();
    assert_eq!(refused(rt.block_on(p.c.act(&answer(&run, "x", v, "d-1")))), 403);
    assert!(count(&d.ctx, "select count(*) from phone_actions") >= 10, "each receipt kept");
}
