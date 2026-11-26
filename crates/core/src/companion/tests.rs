//! A fake phone (companion-proto) paired with a real Runtime on loopback: the pairing, every `/c1` route, the
//! grant boundaries, revocation and the action contract's cases that need no terminal.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use companion_proto::api::{Grants, Receipt, Status};
use companion_proto::pairing::{self, DesktopPayload, PairResult, PhonePayload};
use companion_proto::{Pairing, PhoneIdentity};
use rusqlite::params;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
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

/// Accepts exactly the computer's certificate, as the phone pins it.
#[derive(Debug)]
struct Pinned {
    der: Vec<u8>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, Error> {
        if end_entity.as_ref() == self.der.as_slice() { Ok(ServerCertVerified::assertion()) } else { Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

/// A phone as the app will be: its identity, and the computer it paired with.
struct Phone {
    me: PhoneIdentity,
    desk: DesktopPayload,
    port: u16,
}

impl Phone {
    fn tls(&self) -> Arc<rustls::ClientConfig> {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let (_, der) = self.me.cert().unwrap();
        Arc::new(rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions().unwrap().dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned { der: identity::pem_to_der(&self.desk.cert_pem).unwrap(), provider }))
            .with_client_auth_cert(vec![CertificateDer::from(der)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.me.key_der()))).unwrap())
    }

    fn http(&self) -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder().no_proxy().use_preconfigured_tls((*self.tls()).clone()).timeout(Duration::from_secs(20)).build().unwrap()
    }

    fn url(&self, path: &str) -> String { format!("https://127.0.0.1:{}{path}", self.port) }

    /// Status and JSON of a request; an error when the connection is refused.
    fn send(&self, method: &str, path: &str, body: Option<Value>) -> reqwest::Result<(u16, Value)> {
        let c = self.http();
        let r = match method { "GET" => c.get(self.url(path)), "POST" => c.post(self.url(path)), "PUT" => c.put(self.url(path)), _ => c.delete(self.url(path)) };
        let r = match body { Some(b) => r.json(&b), None => r }.send()?;
        let status = r.status().as_u16();
        Ok((status, r.json().unwrap_or(Value::Null)))
    }

    fn get(&self, path: &str) -> (u16, Value) { self.send("GET", path, None).unwrap() }

    fn act(&self, body: Value) -> Receipt {
        let (s, v) = self.send("POST", "/c1/actions", Some(body)).unwrap();
        assert_eq!(s, 200, "{v}");
        serde_json::from_value(v).unwrap()
    }
}

fn plain() -> reqwest::blocking::Client { reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap() }

/// The phone's half of the pairing up to the owner's approval: the key, from the code in the QR code.
fn start_and_finish(d: &Desk, me: &PhoneIdentity, code: &str) -> (pairing::Keyed, reqwest::StatusCode, String) {
    let st = d.rt.phone_pair_status().unwrap();
    let base = format!("http://127.0.0.1:{}", st["port"]);
    let hello: Value = plain().get(format!("{base}/pair/hello")).send().unwrap().json().unwrap();
    let (p, msg) = Pairing::start(code, &me.id, hello["id"].as_str().unwrap()).unwrap();
    let r: Value = plain().post(format!("{base}/pair/phone/start")).json(&json!({"id": me.id, "msg": msg})).send().unwrap().json().unwrap();
    let key = p.finish(r["msg"].as_str().unwrap()).unwrap();
    let f = plain().post(format!("{base}/pair/phone/finish")).json(&json!({"id": me.id, "payload": key.seal(&PhonePayload::of(me, "Pixel 9", "android").unwrap()).unwrap()})).send().unwrap();
    (key, f.status(), base)
}

fn result(base: &str, id: &str) -> PairResult { plain().get(format!("{base}/pair/phone/result?id={id}")).send().unwrap().json().unwrap() }

/// A phone paired with `d` and approved with `grants`.
fn pair(d: &Desk, grants: Value) -> Phone {
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let (host, code, fp) = pairing::parse_qr(v["qr"].as_str().unwrap()).unwrap();
    assert_eq!((host, code.as_str()), (format!("127.0.0.1:{}", v["port"]), v["code"].as_str().unwrap()));
    let me = PhoneIdentity::generate().unwrap();
    let (key, status, base) = start_and_finish(d, &me, &code);
    assert!(status.is_success(), "{status}");
    let st = d.rt.phone_pair_status().unwrap();
    assert_eq!((st["state"].as_str(), st["phone"]["name"].as_str()), (Some("confirm"), Some("Pixel 9")));
    assert_eq!(st["words"], json!(key.words()), "both sides show the same three words");
    let wait = result(&base, &me.id);
    assert_eq!((wait.state.as_str(), wait.payload.as_deref()), ("pending", None), "nothing sealed before the owner approves");
    assert!(wait.expires_in > 0);
    d.rt.phone_pair_approve(&d.ctx, &grants).unwrap();
    let r = result(&base, &me.id);
    assert_eq!(r.state, "approved");
    let raw: Value = key.open(r.payload.as_deref().unwrap()).unwrap();
    assert!(raw.get("ring").is_none(), "a phone never gets the ring key: {raw}");
    let desk: DesktopPayload = serde_json::from_value(raw).unwrap();
    assert_eq!(pairing::fingerprint(&identity::pem_to_der(&desk.cert_pem).unwrap()), fp, "the certificate the QR code named");
    let port = desk.address_hints[0].rsplit_once(':').unwrap().1.parse().unwrap();
    assert_eq!(port, d.ctx.settings.listen_port);
    Phone { me, desk, port }
}

fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

#[test]
fn a_phone_pairs_with_the_owners_approval_and_is_never_a_peer() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let p = pair(&d, json!({"control": true}));
    assert_eq!(p.desk.grants, Grants { summaries: true, transcripts: true, control: true });
    assert_eq!(p.desk.hub_token, None);
    assert_eq!((count(&d.ctx, "select count(*) from peers"), count(&d.ctx, "select count(*) from phones")), (0, 1));
    let list = crate::phones::list(&d.ctx).unwrap();
    assert_eq!((list[0]["phone_id"].as_str(), list[0]["platform"].as_str()), (Some(p.me.id.as_str()), Some("android")));
    assert_eq!(crate::phones::question_seconds(&d.ctx), 60, "a phone with Control lengthens the question window");
    let states: Vec<String> = d.events.lock().unwrap().iter().filter(|(k, _)| k == "phones-pairing").map(|(_, v)| v["state"].as_str().unwrap().to_string()).collect();
    assert_eq!(states, ["confirm", "paired"]);
    assert_eq!(d.rt.phone_pair_status().unwrap()["state"], "paired");
    // the pairing port closes once the phone read its result
    std::thread::sleep(Duration::from_millis(2500));
    assert!(plain().get(format!("http://127.0.0.1:{}/pair/hello", d.rt.phone_pair_status().unwrap()["port"])).send().is_err());
    // and the phone reaches the companion API with its certificate
    let (s, h) = p.get("/c1/hello");
    assert_eq!((s, h["api"].as_u64(), h["computer"]["name"].as_str()), (200, Some(1), Some("A")), "{h}");
    assert!(crate::phones::list(&d.ctx).unwrap()[0]["last_seen"].is_i64(), "heard from");
}

#[test]
fn three_wrong_codes_end_the_phone_pairing_and_the_owner_can_refuse() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let wrong = format!("{:06}", (v["code"].as_str().unwrap().parse::<u32>().unwrap() + 1) % 1_000_000);
    let me = PhoneIdentity::generate().unwrap();
    for i in 0..3 {
        let (_, status, _) = start_and_finish(&d, &me, &wrong);
        assert_eq!(status, reqwest::StatusCode::FORBIDDEN, "try {i}");
    }
    let st = d.rt.phone_pair_status().unwrap();
    assert_eq!((st["state"].as_str(), st["error"].as_str()), (Some("failed"), Some("three wrong codes; show a new one")));
    assert!(d.rt.phone_pair_approve(&d.ctx, &json!({})).is_err());
    // a new code; the owner refuses the phone shown
    let v = d.rt.phone_pair_start(&d.ctx, Some("127.0.0.1")).unwrap();
    let (_, status, base) = start_and_finish(&d, &me, v["code"].as_str().unwrap());
    assert!(status.is_success());
    assert_eq!(d.rt.phone_pair_refuse().unwrap()["state"], "refused");
    assert_eq!(result(&base, &me.id).state, "refused");
    assert_eq!(count(&d.ctx, "select count(*) from phones"), 0);
    assert_eq!(d.rt.phone_pair_cancel().unwrap(), Value::Null);
    assert!(d.rt.phone_pair_status().is_none());
}

#[test]
fn every_route_answers_a_phone_with_its_grants_and_nothing_outside_them() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
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

    let full = pair(&d, json!({"control": true}));
    let (s, o) = full.get("/c1/overview");
    assert_eq!(s, 200, "{o}");
    assert!(o["use_next"].is_array() && o["today"]["tokens"].is_i64() && o["needs_you"] == 0 && o["observed_at"].is_i64(), "{o}");
    assert_eq!(o["computer"]["id"], d.ctx.settings.device_id.as_str());
    let (s, a) = full.get("/c1/accounts");
    assert_eq!(s, 200, "{a}");
    assert!(a["accounts"].is_array());
    let (s, _) = full.get("/c1/accounts/nobody");
    assert_eq!(s, 404);
    let (s, l) = full.get("/c1/sessions");
    assert_eq!(s, 200, "{l}");
    let one = l["sessions"].as_array().unwrap().iter().find(|x| x["id"] == "s1").unwrap_or_else(|| panic!("{l}"));
    assert_eq!((one["title"].as_str().is_some(), one["computer"]["name"].as_str()), (true, Some("A")));
    let (s, tr) = full.get("/c1/sessions/claude/claude-main/s1/transcript");
    assert_eq!(s, 200, "{tr}");
    assert_eq!((tr["turns"][0]["prompt"].as_str(), tr["next"].is_null(), tr["total"].as_u64()), (Some("fix the build"), true, Some(1)));
    let (s, _) = full.get("/c1/sessions/codex/claude-main/s1/transcript");
    assert_eq!(s, 404, "another tool's session of that id");
    let (s, r) = full.get("/c1/requests");
    assert_eq!((s, r["requests"].as_array().map(Vec::len), r["actions"]["answer"]["ok"].as_bool()), (200, Some(0), Some(true)), "{r}");
    let (s, _) = full.send("PUT", "/c1/push", Some(json!({"relay_handle": "h-1"}))).unwrap();
    assert_eq!(s, 200);
    assert_eq!(d.ctx.db.read(|c| Ok(c.query_row("select relay_handle from phones", [], |r| r.get::<_, String>(0))?)).unwrap(), "h-1");
    let (s, _) = full.send("DELETE", "/c1/push", None).unwrap();
    assert_eq!(s, 200);
    assert_eq!(count(&d.ctx, "select count(*) from phones where relay_handle is not null"), 0);
    let (s, _) = full.get("/c1/actions/never-sent");
    assert_eq!(s, 404);
    // a phone is refused on every computer route
    let me = d.ctx.settings.device_id.clone();
    for path in ["/v1/hello".to_string(), format!("/v1/chunks/{me}"), format!("/v1/chunk/{me}/x.jsonl"), "/v1/presence".into(), format!("/v1/transcript/{me}/claude/claude-main/s1")] {
        assert_eq!(full.get(&path).0, 403, "{path}");
    }

    // summaries only: no transcripts, no actions
    let small = pair(&d, json!({"summaries": true, "transcripts": false, "control": false}));
    assert_eq!(small.get("/c1/overview").0, 200);
    assert_eq!(small.get("/c1/requests").1["actions"]["answer"]["ok"], false);
    assert_eq!(small.get("/c1/sessions/claude/claude-main/s1/transcript").0, 403);
    assert_eq!(small.send("POST", "/c1/actions", Some(json!({}))).unwrap().0, 403);
    assert_eq!(small.get("/c1/actions/x").0, 403);
    for path in ["/v1/hello".to_string(), format!("/v1/chunks/{me}"), "/v1/presence".into()] { assert_eq!(small.get(&path).0, 403, "{path}"); }
    // nothing granted: hello and the push handle only
    let none = pair(&d, json!({"summaries": false, "transcripts": false, "control": false}));
    assert_eq!(none.get("/c1/hello").0, 200);
    for path in ["/c1/overview", "/c1/accounts", "/c1/sessions", "/c1/requests"] { assert_eq!(none.get(path).0, 403, "{path}"); }
    // a grant switched off holds at the next request
    crate::phones::set_grants(&d.ctx, &full.me.id, &json!({"transcripts": false})).unwrap();
    assert_eq!(full.get("/c1/sessions/claude/claude-main/s1/transcript").0, 403);
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
fn revoking_a_phone_cuts_its_events_and_refuses_it_from_then_on() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let p = pair(&d, json!({}));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        use futures_util::StreamExt;
        let url = format!("wss://127.0.0.1:{}/c1/events", p.port);
        let (mut ws, _) = tokio_tungstenite::connect_async_tls_with_config(url, None, false, Some(tokio_tungstenite::Connector::Rustls(p.tls()))).await.expect("the socket opens");
        let first = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
        let ev: Value = serde_json::from_str(first.to_text().unwrap()).unwrap();
        assert_eq!((ev["kind"].as_str(), ev["data"]["count"].as_u64()), (Some("needs"), Some(0)), "{ev}");
        assert!(crate::phones::list(&d.ctx).unwrap()[0]["route"] == "direct");
        let (c2, id) = (d.ctx.clone(), p.me.id.clone());
        let v = tokio::task::spawn_blocking(move || crate::phones::revoke(&c2, &id).unwrap()).await.unwrap();
        assert_eq!(v["acks"], json!({"connection": true, "hub": null, "push": null}));
        // the socket ends at once: cut under its TLS session, or told `revoked` first
        let end = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match ws.next().await {
                    Some(Ok(m)) if m.is_text() => { let e: Value = serde_json::from_str(m.to_text().unwrap()).unwrap(); if e["kind"] == "revoked" { continue; } }
                    Some(Ok(m)) if m.is_close() => return,
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => return,
                }
            }
        }).await;
        assert!(end.is_ok(), "the socket stayed open after revoke");
    });
    assert!(p.send("GET", "/c1/hello", None).is_err(), "a revoked certificate fails its handshake");
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

fn answer(run: &str, ask: &str, version: i64, id: &str) -> Value {
    json!({"action_id": id, "kind": "answer", "target": {"run": run, "ask_id": ask}, "state_version": version, "expires_at": now_ms() + 30_000, "payload": {"behavior": "allow"}})
}

fn ask_state(ctx: &Ctx, ask: &str) -> (String, String) { ctx.db.read(|c| Ok(c.query_row("select state, answer from run_ask where id = ?", [ask], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap() }

#[test]
fn the_action_contract_holds_without_a_terminal() {
    let t = tempfile::tempdir().unwrap();
    let d = desk(t.path(), "A");
    let p = pair(&d, json!({"control": true}));

    // the request is listed with the version an answer must name
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    let reqs = p.get("/c1/requests").1;
    assert_eq!((reqs["requests"][0]["ask_id"].as_str(), reqs["requests"][0]["state_version"].as_i64()), (Some(ask.as_str()), Some(v)), "{reqs}");
    let o = p.get("/c1/overview").1;
    assert_eq!(o["needs_you"], 1);

    // accepted, once: the same action_id again is the stored receipt and changes nothing
    let first = p.act(answer(&run, &ask, v, "a-1"));
    assert_eq!((first.status, first.detail.as_str()), (Status::Accepted, "answered"));
    assert_eq!(ask_state(&d.ctx, &ask), ("answered".into(), "allow".into()));
    d.ctx.db.with(|c| Ok(c.execute("update run_ask set answer = 'deny-marker' where id = ?", [&ask])?)).unwrap();
    assert_eq!(p.act(answer(&run, &ask, v, "a-1")), first);
    assert_eq!(ask_state(&d.ctx, &ask).1, "deny-marker", "nothing done again");
    let (s, stored) = p.get("/c1/actions/a-1");
    assert_eq!((s, serde_json::from_value::<Receipt>(stored).unwrap()), (200, first.clone()));

    // the terminal answered first: the request closed and the version moved on
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    d.ctx.db.with(|c| { c.execute("update run_ask set state = 'closed' where id = ?", [&ask])?; c.execute("update run_state set state = 'working', version = version + 1 where run = ?", [&run])?; Ok(()) }).unwrap();
    assert_eq!(p.act(answer(&run, &ask, v, "a-2")).status, Status::AlreadyHandled);

    // expiry: the request's, and the action's own while the phone unlocked
    let (run, ask, v) = waiting(&d.ctx, -1);
    assert_eq!(p.act(answer(&run, &ask, v, "a-3")).status, Status::Expired);
    let (run, ask, v) = waiting(&d.ctx, 60_000);
    let mut late = answer(&run, &ask, v, "a-4");
    late["expires_at"] = json!(now_ms() - 1);
    assert_eq!(p.act(late).status, Status::Expired);
    assert_eq!(ask_state(&d.ctx, &ask).0, "open", "an expired action does nothing");

    // a state the phone did not see
    assert_eq!(p.act(answer(&run, &ask, v + 1, "a-5")).status, Status::Expired);

    // two answers at once, from two phones: one is accepted
    let other = pair(&d, json!({"control": true}));
    let (body1, body2) = (answer(&run, &ask, v, "b-1"), answer(&run, &ask, v, "b-2"));
    let (r1, r2) = std::thread::scope(|s| {
        let h1 = s.spawn(|| p.act(body1));
        let h2 = s.spawn(|| other.act(body2));
        (h1.join().unwrap(), h2.join().unwrap())
    });
    let mut both = [r1.status, r2.status];
    both.sort_by_key(|s| s.as_str());
    assert_eq!(both, [Status::Accepted, Status::AlreadyHandled], "{r1:?} {r2:?}");
    // another phone's action id is not this phone's receipt
    assert_eq!(other.get("/c1/actions/a-1").0, 404);

    // reply and Stop keep the desktop's rules: no owned terminal here
    let (run, _, v) = waiting(&d.ctx, 60_000);
    for (kind, payload) in [("reply", json!({"text": "go on"})), ("stop", json!({}))] {
        let r = p.act(json!({"action_id": format!("c-{kind}"), "kind": kind, "target": {"run": run}, "state_version": v, "expires_at": now_ms() + 30_000, "payload": payload}));
        assert_eq!((r.status, r.detail.as_str()), (Status::Unavailable, "Needs Harness Manager's own terminal"), "{kind}");
    }
    let r = p.act(json!({"action_id": "c-gone", "kind": "stop", "target": {"run": "1:2"}, "state_version": 0, "expires_at": now_ms() + 30_000, "payload": {}}));
    assert_eq!(r.status, Status::Unavailable);

    // Control taken away between the screen and the tap
    crate::phones::set_grants(&d.ctx, &p.me.id, &json!({"control": false})).unwrap();
    assert_eq!(p.send("POST", "/c1/actions", Some(answer(&run, "x", v, "d-1"))).unwrap().0, 403);
    assert!(count(&d.ctx, "select count(*) from phone_actions") >= 10, "each receipt kept");
}
