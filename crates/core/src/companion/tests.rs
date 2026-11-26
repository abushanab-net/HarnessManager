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
fn joined(rt: &tokio::runtime::Runtime, d: &Desk) -> (PhoneIdentity, Joining) { joined_as(rt, d, PhoneIdentity::generate().unwrap()) }

fn joined_as(rt: &tokio::runtime::Runtime, d: &Desk, me: PhoneIdentity) -> (PhoneIdentity, Joining) {
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let qr = v["qr"].as_str().unwrap().to_string();
    let (host, code, _) = pairing::parse_qr(&qr).unwrap();
    assert_eq!((host, code.as_str()), (format!("127.0.0.1:{}", v["port"]), v["code"].as_str().unwrap()));
    let j = rt.block_on(join::join_qr(&qr, &me, "Pixel 9", "android")).unwrap();
    let st = d.rt.phone_pair_status().unwrap();
    assert_eq!((st["state"].as_str(), st["phone"]["name"].as_str()), (Some("confirm"), Some("Pixel 9")));
    assert_eq!(st["words"], json!(j.words()), "both sides show the same three words");
    assert_eq!((j.desktop_id.as_str(), j.desktop_name.as_str()), (d.ctx.settings.device_id.as_str(), d.ctx.settings.device_name.as_str()));
    (me, j)
}

/// A phone paired with `d` and approved with `grants`.
fn pair(rt: &tokio::runtime::Runtime, d: &Desk, grants: Value) -> Phone { pair_as(rt, d, PhoneIdentity::generate().unwrap(), grants) }

fn pair_as(rt: &tokio::runtime::Runtime, d: &Desk, me: PhoneIdentity, grants: Value) -> Phone {
    let (me, j) = joined_as(rt, d, me);
    assert!(matches!(rt.block_on(j.result()).unwrap(), Outcome::Pending(ms) if ms > 0), "nothing sealed before the owner approves");
    d.rt.phone_pair_approve(&d.ctx, &attempt(d), &grants).unwrap();
    let Outcome::Approved(desk) = rt.block_on(j.wait(Duration::from_millis(100))).unwrap() else { panic!("not approved") };
    assert_eq!(desk.address_hints[0], format!("127.0.0.1:{}", d.ctx.settings.listen_port));
    let c = Client::new(&me, &desk.cert_pem, &desk.address_hints[0]).unwrap();
    Phone { me, desk, c }
}

/// The attempt the computer shows, as the owner's window passes it back.
fn attempt(d: &Desk) -> String { d.rt.phone_pair_status().unwrap()["attempt"].as_str().unwrap().to_string() }

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
    assert!(d.rt.phone_pair_approve(&d.ctx, &attempt(&d), &json!({})).is_err());
    // a new code; the owner refuses the phone shown
    let (_, j) = joined(&rt, &d);
    assert_eq!(d.rt.phone_pair_refuse(&attempt(&d)).unwrap()["state"], "refused");
    assert_eq!(rt.block_on(j.wait(Duration::from_millis(100))).unwrap(), Outcome::Refused);
    assert_eq!(count(&d.ctx, "select count(*) from phones"), 0);
    assert_eq!(d.rt.phone_pair_cancel(&attempt(&d)).unwrap(), Value::Null);
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
    a.rt.phone_pair_approve(&a.ctx, &attempt(&a), &json!({})).unwrap();
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

// ---------- the security review's findings (Codex review of 7a8cc09), one test each where a test can show it ----------

/// A phone enrolled straight into the table (no network), with `grants`: for the tests that call `act` directly.
fn enrolled(ctx: &Ctx, control: bool) -> crate::phones::Phone {
    let me = PhoneIdentity::generate().unwrap();
    crate::phones::add(ctx, &pairing::PhonePayload::of(&me, "Pixel", "android").unwrap(), Grants { summaries: true, transcripts: true, control }).unwrap();
    crate::phones::get(ctx, &me.id).unwrap().unwrap()
}

/// A live run (this process) working in an owned terminal that is not there: an action is claimed and dispatched,
/// and the dispatch fails without reaching anything.
fn working(ctx: &Ctx) -> (String, i64) {
    let pid = std::process::id();
    let run = format!("{pid}:{}", crate::util::uuid4());
    ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness, session_id, state, version, terminal_port, terminal_token) values(?, ?, ?, 'work', 'claude', 's-2', 'working', 5, 9, x'00')",
        params![run, pid, crate::procs::start_ms(pid).unwrap()])?)).unwrap();
    (run, 5)
}

fn act_now(kind: ActionKind, run: &str, version: i64, id: &str, payload: Value) -> Action {
    Action { action_id: id.into(), kind, target: Target { run: run.into(), ask_id: None }, state_version: version, expires_at: now_ms() + 30_000, payload }
}

fn dispatched() -> usize { super::actions::DISPATCHED.with(|d| d.get()) }

/// #1: a phone paired again under the same id is a new enrollment: the earlier certificate's open connection does
/// not get the new grants, and the earlier enrollment's receipts are not the new one's.
#[test]
fn a_phone_paired_again_cuts_the_earlier_enrollment_and_keeps_its_receipts_apart() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let old = pair(&rt, &d, json!({"control": true}));
    let r = rt.block_on(old.c.act(&act_now(ActionKind::Stop, "1:2", 0, "old-1", json!({})))).unwrap();
    assert_eq!(rt.block_on(old.c.receipt("old-1")).unwrap(), Some(r));
    crate::phones::set_grants(&d.ctx, &old.me.id, &json!({"control": false})).unwrap();
    assert!(rt.block_on(old.c.hello()).is_ok(), "the old connection is open and kept");
    // the same phone id, another key, paired again with Control
    let new = pair_as(&rt, &d, PhoneIdentity::from_secrets(&old.me.id, &[3; 32], &[4; 32]).unwrap(), json!({"control": true}));
    assert!(rt.block_on(old.c.act(&act_now(ActionKind::Stop, "1:2", 0, "old-2", json!({})))).is_err(), "the earlier enrollment gained Control");
    assert_eq!(rt.block_on(new.c.receipt("old-1")).unwrap(), None, "the earlier enrollment's receipt");
    assert_eq!(count(&d.ctx, "select count(*) from phone_actions where action_id = 'old-2'"), 0);
}

/// #2: the receipt is reserved before dispatch; one whose outcome was never written reads `unknown`, and the same
/// id again is not dispatched again.
#[test]
fn an_action_whose_outcome_was_never_written_reads_unknown_and_is_not_sent_again() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    let p = enrolled(&ctx, true);
    let (run, ask, v) = waiting(&ctx, 60_000);
    let before = dispatched();
    super::actions::CRASH_AFTER_DISPATCH.with(|c| c.set(true));
    assert!(super::actions::act(&ctx, &p, &answer(&run, &ask, v, "crash-1")).is_err(), "the process ended after the dispatch");
    assert_eq!(dispatched(), before + 1);
    let again = super::actions::act(&ctx, &p, &answer(&run, &ask, v, "crash-1")).unwrap();
    assert_eq!(again.status, Status::Unknown, "{again:?}");
    assert_eq!(dispatched(), before + 1, "never dispatched twice");
}

/// #3: an action consumes the version it was shown: a second action on the same screen never reaches the terminal.
#[test]
fn two_replies_on_one_screen_reach_the_terminal_once() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    let (a, b) = (enrolled(&ctx, true), enrolled(&ctx, true));
    let (run, v) = working(&ctx);
    let before = dispatched();
    let first = super::actions::act(&ctx, &a, &act_now(ActionKind::Reply, &run, v, "r-1", json!({"text": "go on"}))).unwrap();
    assert_eq!(dispatched(), before + 1, "{first:?}");
    let second = super::actions::act(&ctx, &b, &act_now(ActionKind::Reply, &run, v, "r-2", json!({"text": "go on"}))).unwrap();
    assert_eq!((second.status, dispatched()), (Status::Expired, before + 1), "{second:?}");
    // a reply the terminal would refuse is refused before it costs the version
    let (run, v) = working(&ctx);
    let bad = super::actions::act(&ctx, &a, &act_now(ActionKind::Reply, &run, v, "r-3", json!({"text": "two\nlines"}))).unwrap();
    assert_eq!((bad.status, dispatched()), (Status::Unavailable, before + 1));
}

/// #3: what changes between the claim and the dispatch holds: Control taken away, the deadline passed, or the run
/// moved (a prompt opening under a reply).
#[test]
fn control_withdrawn_the_deadline_passed_or_a_prompt_opened_after_the_claim_stops_the_dispatch() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Arc::new(Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap());
    let p = enrolled(&ctx, true);
    let before = dispatched();
    let (run, v) = working(&ctx);
    let (c2, id) = (ctx.clone(), p.id.clone());
    super::actions::BEFORE_DISPATCH.with(|b| *b.borrow_mut() = Some(Box::new(move || { crate::phones::set_grants(&c2, &id, &json!({"control": false})).unwrap(); })));
    let r = super::actions::act(&ctx, &p, &act_now(ActionKind::Stop, &run, v, "g-1", json!({}))).unwrap();
    assert_eq!((r.status, r.detail.as_str(), dispatched()), (Status::Unavailable, "Control is not granted to this phone", before));
    crate::phones::set_grants(&ctx, &p.id, &json!({"control": true})).unwrap();
    let (run, v) = working(&ctx);
    let mut late = act_now(ActionKind::Reply, &run, v, "g-2", json!({"text": "go on"}));
    late.expires_at = now_ms() + 50;
    super::actions::BEFORE_DISPATCH.with(|b| *b.borrow_mut() = Some(Box::new(|| std::thread::sleep(Duration::from_millis(80)))));
    assert_eq!((super::actions::act(&ctx, &p, &late).unwrap().status, dispatched()), (Status::Expired, before));
    let (run, v) = working(&ctx);
    let (c2, r2) = (ctx.clone(), run.clone());
    super::actions::BEFORE_DISPATCH.with(|b| *b.borrow_mut() = Some(Box::new(move || {
        c2.db.with(|c| Ok(c.execute("update run_state set state = 'waiting', version = version + 1 where run = ?", [&r2])?)).unwrap();
    })));
    assert_eq!((super::actions::act(&ctx, &p, &act_now(ActionKind::Reply, &run, v, "g-3", json!({"text": "go on"}))).unwrap().status, dispatched()), (Status::Expired, before));
}

/// #3: one Stop per turn, from the desktop and from a phone alike.
#[test]
fn one_stop_per_turn_whoever_presses_it() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    let p = enrolled(&ctx, true);
    let (run, v) = working(&ctx);
    let c = rusqlite::Connection::open(ctx.data.join("hm.db")).unwrap();
    crate::hooks::claim_stop(&c, &run).unwrap();
    assert_eq!(format!("{:#}", crate::hooks::claim_stop(&c, &run).unwrap_err()), "Nothing to stop: the session is at its prompt", "a second Stop would press Ctrl+C again");
    let before = dispatched();
    let r = super::actions::act(&ctx, &p, &act_now(ActionKind::Stop, &run, v, "s-1", json!({}))).unwrap();
    assert_eq!((r.status, dispatched()), (Status::Expired, before), "the phone's screen showed the turn the desktop stopped");
}

/// #4: an approval that comes once the code expired stores nothing, and says so.
#[test]
fn an_approval_after_the_code_expired_stores_nothing() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let (_, j) = joined(&rt, &d);
    d.rt.with_phone_host(|h| h.run_out()).unwrap();
    let e = d.rt.phone_pair_approve(&d.ctx, &attempt(&d), &json!({"control": true})).unwrap_err();
    assert!(format!("{e:#}").contains("expired"), "{e:#}");
    assert_eq!(count(&d.ctx, "select count(*) from phones"), 0);
    assert_eq!(d.rt.phone_pair_status().unwrap()["state"], "expired");
    assert_eq!(rt.block_on(j.result()).unwrap(), Outcome::Expired);
}

/// #5: the owner's decision names the attempt it was taken on; a cancelled attempt's expiry says nothing.
#[test]
fn a_decision_names_the_pairing_it_was_taken_on() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let (_, _a) = joined(&rt, &d);
    let first = attempt(&d);
    let (_, b) = joined(&rt, &d);
    let second = attempt(&d);
    assert_ne!(first, second);
    assert!(d.events.lock().unwrap().iter().filter(|(k, _)| k == "phones-pairing").all(|(_, v)| v["attempt"].is_string()), "events carry the attempt");
    for e in [d.rt.phone_pair_approve(&d.ctx, &first, &json!({"control": true})).unwrap_err(), d.rt.phone_pair_refuse(&first).unwrap_err(), d.rt.phone_pair_cancel(&first).unwrap_err()] {
        assert!(format!("{e:#}").contains("no longer shown"), "{e:#}");
    }
    assert_eq!(count(&d.ctx, "select count(*) from phones"), 0, "the first attempt's grants did not land on the second phone");
    d.rt.phone_pair_approve(&d.ctx, &second, &json!({})).unwrap();
    assert!(matches!(rt.block_on(b.result()).unwrap(), Outcome::Approved(_)));
    // a cancelled attempt's own expiry publishes nothing
    let events = Arc::new(Mutex::new(0));
    let e2 = events.clone();
    let h = crate::sync::peer::pairing::start_phone(d.ctx.clone(), rt.handle(), Ipv4Addr::LOCALHOST.into(), None, Arc::new(move |_, _| *e2.lock().unwrap() += 1)).unwrap();
    h.cancel();
    rt.block_on(h.expire_now());
    assert_eq!(*events.lock().unwrap(), 0);
}

/// #6: a certificate is enrolled only when it is made for the key sent with it, and only once across phones and
/// computers; a computer is not stored with a phone's certificate either.
#[test]
fn a_certificate_not_made_for_its_key_or_already_in_use_is_not_enrolled() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (desk(t.path(), "A"), desk(t.path(), "B"));
    let g = Grants::default();
    let (x, y) = (PhoneIdentity::generate().unwrap(), PhoneIdentity::generate().unwrap());
    let mut p = pairing::PhonePayload::of(&x, "X", "ios").unwrap();
    p.pubkey = y.pubkey();
    assert!(format!("{:#}", crate::phones::add(&a.ctx, &p, g).unwrap_err()).contains("not made for its key"));
    // a paired computer's certificate, with its own key, offered as a phone
    let v = a.rt.pair_start(&a.ctx, Some("127.0.0.1")).unwrap();
    b.rt.pair_join(&b.ctx, v["code"].as_str().unwrap(), v["host_hint"].as_str()).unwrap();
    let bid = identity::load(&b.ctx).unwrap();
    let as_phone = pairing::PhonePayload { id: "phone-b".into(), name: "B".into(), platform: "ios".into(), cert_pem: bid.cert_pem.clone(), pubkey: bid.pubkey(), x25519: x.x25519_public() };
    assert!(format!("{:#}", crate::phones::add(&a.ctx, &as_phone, g).unwrap_err()).contains("already belongs"));
    // one certificate under two phone ids
    crate::phones::add(&a.ctx, &pairing::PhonePayload::of(&x, "X", "ios").unwrap(), g).unwrap();
    let mut twin = pairing::PhonePayload::of(&x, "X2", "ios").unwrap();
    twin.id = "another-id".into();
    assert!(crate::phones::add(&a.ctx, &twin, g).is_err());
    // and a computer offering a phone's certificate
    let xp = pairing::PhonePayload::of(&x, "X", "ios").unwrap();
    let as_peer = json!({"id": "dev-x", "name": "X", "cert_pem": xp.cert_pem, "pubkey": xp.pubkey, "address_hint": "x:1"});
    assert!(crate::sync::peer::pairing::store_peer(&a.ctx, &as_peer, "").is_err());
    assert_eq!(count(&a.ctx, "select count(*) from phones"), 1);
}

/// #7: Summaries show a request's command or file and a question's text and options, never the tool's input.
#[test]
fn summaries_show_a_requests_question_and_options_but_never_its_tool_input() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({"summaries": true, "transcripts": false, "control": false}));
    let pid = std::process::id();
    let started = crate::procs::start_ms(pid).unwrap();
    let questions = json!([{"question": "Which?", "header": "H", "multiSelect": false, "options": [{"label": "A", "description": "a", "secret": "S1"}]}]);
    d.ctx.db.with(|c| {
        c.execute("insert into run_state(run, pid, started, profile, harness, state) values('r1', ?1, ?2, 'w', 'claude', 'waiting'), ('r2', ?1, ?2, 'w', 'claude', 'waiting')", params![pid, started])?;
        c.execute("insert into run_ask(id, run, kind, tool, what, input, created, expires, hook_pid, hook_started) values('k1', 'r1', 'permission', 'Write', 'notes.txt', ?1, ?3, ?4, ?5, ?6),
            ('k2', 'r2', 'question', 'AskUserQuestion', 'Which?', ?2, ?3, ?4, ?5, ?6)",
            params![json!({"file_path": "notes.txt", "content": "THE-FILE-BODY"}).to_string(), json!({"questions": questions}).to_string(), now_ms(), now_ms() + 60_000, pid, started])?;
        Ok(())
    }).unwrap();
    let r = rt.block_on(p.c.requests()).unwrap();
    let text = serde_json::to_string(&r).unwrap();
    assert!(!text.contains("THE-FILE-BODY") && !text.contains("S1") && !text.contains("\"header\""), "{text}");
    let perm = r.requests.iter().find(|x| x.ask_id == "k1").unwrap();
    assert_eq!((perm.what.as_str(), &perm.input), ("notes.txt", &json!({})));
    let q = r.requests.iter().find(|x| x.ask_id == "k2").unwrap();
    assert_eq!(q.input, json!({"questions": [{"question": "Which?", "multiSelect": false, "options": [{"label": "A", "description": "a"}]}]}));
}

/// #8: grant changes are read and written in one transaction: taking Control away is not undone by another change.
#[test]
fn two_grant_changes_at_once_do_not_undo_each_other() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    let p = enrolled(&ctx, true);
    for round in 0..40 {
        crate::phones::set_grants(&ctx, &p.id, &json!({"control": true})).unwrap();
        let (c1, c2) = (Ctx::open_at(&ctx.data, &ctx.home).unwrap(), Ctx::open_at(&ctx.data, &ctx.home).unwrap());
        std::thread::scope(|s| {
            s.spawn(|| crate::phones::set_grants(&c1, &p.id, &json!({"control": false})).unwrap());
            s.spawn(|| crate::phones::set_grants(&c2, &p.id, &json!({"transcripts": round % 2 == 0})).unwrap());
        });
        assert!(!crate::phones::get(&ctx, &p.id).unwrap().unwrap().grants.control, "round {round}: Control came back");
    }
}

/// #9: a revocation made in another process is cut and acknowledged by the listener that holds the connection.
#[test]
fn a_revocation_from_another_process_is_cut_by_the_listener_and_acknowledged() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({}));
    assert!(rt.block_on(p.c.hello()).is_ok());
    assert!(crate::companion::conn::connected(&d.ctx, &p.me.id));
    // what `hrm phones revoke` in another process leaves: the mark, and no cut of its own
    let other = Ctx::open_at(&d.ctx.data, &d.ctx.home).unwrap();
    other.db.with(|c| Ok(c.execute("update phones set revoked_at = ?1 where phone_id = ?2", params![now_ms(), p.me.id])?)).unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(5);
    while crate::companion::conn::connected(&d.ctx, &p.me.id) && std::time::Instant::now() < end { std::thread::sleep(Duration::from_millis(50)); }
    assert!(!crate::companion::conn::connected(&d.ctx, &p.me.id), "the kept connection survived");
    while crate::phones::list(&d.ctx).unwrap()[0]["acks"]["connection"] != true && std::time::Instant::now() < end { std::thread::sleep(Duration::from_millis(50)); }
    assert_eq!(crate::phones::list(&d.ctx).unwrap()[0]["acks"], json!({"connection": true, "hub": null, "push": null}));
}

/// #11: a turn too big for a page comes cut to fit, and says so.
#[test]
fn a_turn_too_large_for_a_page_comes_cut_to_fit() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let mut reg = d.ctx.registry().unwrap();
    reg.add_profile(crate::registry::Profile { name: "claude-main".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true,
        base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 }).unwrap();
    d.ctx.save_registry(&reg).unwrap();
    let cwd = t.path().join("big");
    std::fs::create_dir_all(&cwd).unwrap();
    let f = d.ctx.home.join(".claude").join("projects").join(crate::harness::claude::project_dir_name(&cwd)).join("s9.jsonl");
    let stamp = chrono::Utc::now().to_rfc3339();
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, [json!({"type": "user", "sessionId": "s9", "cwd": cwd, "timestamp": stamp, "message": {"role": "user", "content": "x".repeat(2_000_000)}}),
        json!({"type": "assistant", "sessionId": "s9", "cwd": cwd, "timestamp": stamp, "message": {"id": "m9", "model": "claude-opus-5-5", "usage": {"input_tokens": 1, "output_tokens": 1}, "content": [{"type": "text", "text": "ok"}]}})]
        .iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    let p = pair(&rt, &d, json!({}));
    let page = rt.block_on(p.c.transcript("claude", "claude-main", "s9", None)).unwrap();
    assert_eq!(page.turns.len(), 1);
    assert!(serde_json::to_string(&page).unwrap().len() < 600_000, "a page of {} bytes", serde_json::to_string(&page).unwrap().len());
    assert_eq!(page.turns[0]["truncated"], true);
}

/// #12: a cursor past any page is refused, not multiplied into an overflow.
#[test]
fn a_cursor_past_any_page_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let p = pair(&rt, &d, json!({}));
    assert_eq!(refused(rt.block_on(p.c.sessions(Some(i64::MAX)))), 400);
    assert_eq!(refused(rt.block_on(p.c.sessions(Some(-1)))), 400);
    assert!(rt.block_on(p.c.sessions(Some(3))).is_ok());
}

/// #13: the answer can be fetched again, and wrong codes from exchanges begun earlier do not close the port under a
/// phone at confirm.
#[test]
fn the_pairing_answer_can_be_fetched_again_and_late_wrong_codes_do_not_close_it() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let rt = phone_rt();
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let base = format!("http://127.0.0.1:{}", v["port"]);
    let http = reqwest::blocking::Client::builder().no_proxy().build().unwrap();
    // three exchanges begun with a wrong code, not finished yet
    let keys: Vec<(String, companion_proto::Keyed)> = (0..3).map(|i| {
        let id = format!("guess-{i}");
        let (st, msg) = companion_proto::Pairing::start("000001", &id, &d.ctx.settings.device_id).unwrap();
        let r: Value = http.post(format!("{base}/pair/phone/start")).json(&json!({"id": id, "msg": msg})).send().unwrap().json().unwrap();
        (id, st.finish(r["msg"].as_str().unwrap()).unwrap())
    }).collect();
    let me = PhoneIdentity::generate().unwrap();
    let j = rt.block_on(join::join_qr(v["qr"].as_str().unwrap(), &me, "Pixel 9", "android")).unwrap();
    for (id, key) in &keys {
        let payload = key.seal(&json!({"id": id})).unwrap();
        let _ = http.post(format!("{base}/pair/phone/finish")).json(&json!({"id": id, "payload": payload})).send().unwrap();
    }
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(d.rt.phone_pair_status().unwrap()["state"], "confirm");
    d.rt.phone_pair_approve(&d.ctx, &attempt(&d), &json!({})).unwrap();
    for _ in 0..2 { assert!(matches!(rt.block_on(j.result()).unwrap(), Outcome::Approved(_)), "the answer, fetched again"); }
}

/// #15: "5h first" is kept in core, where the phone's Use next reads it.
#[test]
fn five_hour_first_is_kept_where_the_phone_reads_it() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    assert!(!crate::recommend::five_hour_first(&ctx));
    crate::recommend::set_five_hour_first(&ctx, true).unwrap();
    assert!(crate::recommend::five_hour_first(&Ctx::open_at(&ctx.data, &ctx.home).unwrap()));
}

/// #16: a request the hook let go at its deadline is reported expired, not answered; a receipt older than a day is gone.
#[test]
fn a_request_that_timed_out_reads_expired_and_an_old_receipt_is_gone() {
    let t = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
    let p = enrolled(&ctx, true);
    let pid = std::process::id();
    let run = format!("{pid}:9");
    ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness) values(?, ?, ?, 'w', 'claude')", params![run, pid, crate::procs::start_ms(pid).unwrap()])?)).unwrap();
    crate::hooks::heartbeat(&ctx).unwrap();
    let c = rusqlite::Connection::open(ctx.data.join("hm.db")).unwrap();
    c.busy_timeout(Duration::from_secs(5)).unwrap();
    let ev = json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": {"command": "ls"}});
    assert_eq!(crate::hooks::handle(&c, &run, &ev, Duration::from_millis(300)).unwrap(), None, "no answer: the hook let it go");
    let (ask, version): (String, i64) = ctx.db.read(|c| Ok(c.query_row("select a.id, s.version from run_ask a join run_state s on s.run = a.run", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    let r = super::actions::act(&ctx, &p, &answer(&run, &ask, version, "late-1")).unwrap();
    assert_eq!(r.status, Status::Expired, "{r:?}");
    let d = desk(t.path(), "B");
    let rt = phone_rt();
    let ph = pair(&rt, &d, json!({"control": true}));
    let r = rt.block_on(ph.c.act(&act_now(ActionKind::Stop, "1:2", 0, "aged-1", json!({})))).unwrap();
    d.ctx.db.with(|c| Ok(c.execute("update phone_actions set at = ?1 where action_id = 'aged-1'", [now_ms() - 25 * 3_600_000])?)).unwrap();
    assert_eq!(rt.block_on(ph.c.receipt("aged-1")).unwrap(), None, "kept 24 h: {r:?}");
}
