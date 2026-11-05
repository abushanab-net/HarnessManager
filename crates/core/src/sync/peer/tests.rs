//! Two or three computers in temp folders, listeners on loopback at free ports, credentials under a test prefix.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::{client::Client, identity, pull_all, store, Runtime};
use crate::journal;
use crate::model::UsageRow;
use crate::pricing::Prices;
use crate::Ctx;

struct Computer {
    ctx: Arc<Ctx>,
    rt: Runtime,
    events: Arc<Mutex<Vec<Value>>>,
}

impl Drop for Computer {
    fn drop(&mut self) {
        self.rt.stop();
        let _ = crate::secrets::delete(&identity::target(&self.ctx.settings.device_id));
    }
}

fn computer(dir: &std::path::Path, name: &str) -> Computer {
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
    let data = dir.join(name);
    let mut ctx = Ctx::open_at(&data, &dir.join("home")).unwrap();
    ctx.settings.device_name = name.into();
    ctx.settings.direct_connection = true;
    ctx.save_settings().unwrap();
    let ctx = Arc::new(Ctx::open_at(&data, &dir.join("home")).unwrap());
    let events = Arc::new(Mutex::new(vec![]));
    let ev = events.clone();
    let rt = Runtime::new(Ipv4Addr::LOCALHOST.into(), Arc::new(move |_, v| ev.lock().unwrap().push(v))).unwrap();
    // the listener up and its port saved
    let ctx = rt.ensure(&ctx).unwrap();
    Computer { ctx, rt, events }
}

fn row(id: &str, ts: i64) -> UsageRow {
    UsageRow { request_id: id.into(), profile: "work".into(), harness: "claude".into(), ts, model: "claude-opus-5-5".into(), project: "C:\\p".into(),
        session_id: "s1".into(), input: 100, output: 10, cache_read: 50, cache_write: 0, ttft_ms: Some(900), gen_ms: None }
}

fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

fn peer<'a>(st: &'a Value, id: &str) -> &'a Value { st["peers"].as_array().unwrap().iter().find(|p| p["id"] == id).expect("peer in status") }

fn port(c: &Computer) -> u16 { Ctx::open_at(&c.ctx.data, &c.ctx.home).unwrap().settings.listen_port }

/// `a` shows a code, `b` joins with it over loopback. `a` is then told `b`'s loopback address: the joiner's host
/// name does not resolve to the test's loopback listener.
fn pair(a: &Computer, b: &Computer) -> Value {
    let v = a.rt.pair_start(&a.ctx, Some("127.0.0.1")).unwrap();
    assert_eq!(v["code"].as_str().unwrap().len(), 6);
    let st = b.rt.pair_join(&b.ctx, v["code"].as_str().unwrap(), v["host_hint"].as_str()).unwrap();
    super::set_peer(&a.ctx, &b.ctx.settings.device_id, Some(&format!("127.0.0.1:{}", port(b))), None).unwrap();
    st
}

#[test]
fn pairing_with_the_right_code_stores_both_peers() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    let st = pair(&a, &b);
    let pa = peer(&st, &a.ctx.settings.device_id);
    assert_eq!((pa["name"].as_str(), pa["direct"].as_bool(), pa["connected"].as_bool(), pa["path"].as_str()), (Some("A"), Some(true), Some(true), Some("address")), "{st}");
    assert_eq!(pa["address"], format!("127.0.0.1:{}", port(&a)));
    assert!(pa["paired_at"].is_i64());
    let (cert, pk): (String, String) = b.ctx.db.with(|c| Ok(c.query_row("select cert_pem, pubkey from peers where device_id=?", [&a.ctx.settings.device_id], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert!(cert.starts_with("-----BEGIN CERTIFICATE-----"));
    assert_eq!(pk, identity::pubkey(&a.ctx));
    // the other side too, and it was told
    let sa = journal::status(&a.ctx).unwrap();
    let pb = peer(&sa, &b.ctx.settings.device_id);
    assert_eq!((pb["name"].as_str(), pb["direct"].as_bool()), (Some("B"), Some(true)));
    assert_eq!(sa["this"]["port"], port(&a));
    assert_eq!(sa["this"]["pairing"], Value::Null, "the code is gone once used");
    let ev = a.events.lock().unwrap().clone();
    assert_eq!((ev.len(), ev[0]["ok"].as_bool(), ev[0]["peer"]["name"].as_str()), (1, Some(true), Some("B")), "{ev:?}");
    assert_eq!(a.rt.pair_result().flatten().unwrap()["ok"], true);
}

#[test]
fn a_wrong_code_fails_and_stores_nothing() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    let v = a.rt.pair_start(&a.ctx, Some("127.0.0.1")).unwrap();
    let wrong = format!("{:06}", (v["code"].as_str().unwrap().parse::<u32>().unwrap() + 1) % 1_000_000);
    let e = b.rt.pair_join(&b.ctx, &wrong, v["host_hint"].as_str()).unwrap_err();
    assert!(format!("{e:#}").contains("did not match"), "{e:#}");
    assert_eq!((count(&a.ctx, "select count(*) from peers"), count(&b.ctx, "select count(*) from peers")), (0, 0));
    assert!(a.events.lock().unwrap().is_empty());
    assert!(journal::status(&a.ctx).unwrap()["this"]["pairing"].is_object(), "one wrong code does not end it");
    for _ in 0..2 { let _ = b.rt.pair_join(&b.ctx, &wrong, v["host_hint"].as_str()); }
    let ev = a.events.lock().unwrap().clone();
    assert_eq!((ev.len(), ev[0]["ok"].as_bool()), (1, Some(false)), "three wrong codes end it: {ev:?}");
    assert_eq!(journal::status(&a.ctx).unwrap()["this"]["pairing"], Value::Null);
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(b.rt.pair_join(&b.ctx, v["code"].as_str().unwrap(), v["host_hint"].as_str()).is_err(), "the route is gone");
}

#[test]
fn after_pairing_as_chunks_reach_b_over_tls_and_apply_once() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    pair(&a, &b);
    let prices: Prices = [("claude-opus-5-5".to_string(), crate::pricing::Price { input: 10.0, output: 10.0, cache_read: 1.0, cache_write: 1.0 })].into_iter().collect();
    assert_eq!(a.ctx.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &prices).unwrap(), 2, "direct on: the journal records");
    a.ctx.db.tag_set("project", "github.com/o/r", &["release".into()]).unwrap();
    a.rt.tick(&a.ctx);
    let man = store::manifest(&a.ctx, &a.ctx.settings.device_id).unwrap();
    assert_eq!(man["chunks"].as_array().unwrap().len(), 1);
    assert!(man["chunks"][0]["sig"].as_str().unwrap().len() > 60, "{man}");
    b.rt.tick(&b.ctx);
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 2);
    let (dev, cost): (String, f64) = b.ctx.db.with(|c| Ok(c.query_row("select device, cost_usd from usage where request_id='claude:1'", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!(dev, a.ctx.settings.device_id);
    assert!((cost - 0.00115).abs() < 1e-9);
    assert_eq!(b.ctx.db.tags_of("project").unwrap()["github.com/o/r"], vec!["release".to_string()]);
    let st = journal::status(&b.ctx).unwrap();
    let pa = peer(&st, &a.ctx.settings.device_id);
    assert_eq!((pa["applied_seq"].as_i64(), pa["last_seq"].as_i64(), pa["connected"].as_bool()), (Some(3), Some(3), Some(true)), "{st}");
    assert!(pa["last_pull"].is_i64());
    // B keeps A's chunk to relay it, and a second pull adds nothing
    assert_eq!(store::manifest(&b.ctx, &a.ctx.settings.device_id).unwrap()["chunks"].as_array().unwrap().len(), 1);
    pull_all(&b.ctx);
    assert_eq!((count(&b.ctx, "select count(*) from usage"), count(&b.ctx, "select count(*) from cursors2")), (2, 1));
    assert_eq!(count(&b.ctx, "select count(*) from outbox"), 0, "nothing merged bounces back");
}

#[test]
fn bs_presence_reaches_a_within_a_poll() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    pair(&a, &b);
    a.rt.tick(&a.ctx);
    let st = journal::status(&a.ctx).unwrap();
    let pb = peer(&st, &b.ctx.settings.device_id);
    assert!(pb["presence_at"].as_i64().unwrap() > 0, "{st}");
    assert_eq!((pb["connected"].as_bool(), pb["path"].as_str(), pb["running"].as_array().map(|r| r.len())), (Some(true), Some("address"), Some(0)));
    let p: String = a.ctx.db.with(|c| Ok(c.query_row("select presence from peers where device_id=?", [&b.ctx.settings.device_id], |r| r.get(0))?)).unwrap();
    let p: Value = serde_json::from_str(&p).unwrap();
    assert!(p["accounts"].is_object() && p["tps_5m"].is_number(), "{p}");
}

#[test]
fn a_connection_from_an_unpaired_certificate_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let (a, b, x) = (computer(t.path(), "A"), computer(t.path(), "B"), computer(t.path(), "X"));
    pair(&a, &b);
    a.rt.tick(&a.ctx);
    let a_cert: String = b.ctx.db.with(|c| Ok(c.query_row("select cert_pem from peers where device_id=?", [&a.ctx.settings.device_id], |r| r.get(0))?)).unwrap();
    let ok = Client::connect(&identity::load(&b.ctx).unwrap(), &a_cert, "127.0.0.1", port(&a)).unwrap().json("/v1/hello").unwrap().unwrap();
    assert_eq!(ok["id"], a.ctx.settings.device_id);
    assert_eq!(ok["origins"], json!([a.ctx.settings.device_id]));
    let e = Client::connect(&identity::load(&x.ctx).unwrap(), &a_cert, "127.0.0.1", port(&a)).unwrap().json("/v1/hello").unwrap_err();
    assert!(!format!("{e:#}").is_empty());
    assert_eq!(count(&a.ctx, "select count(*) from peers"), 1, "X is not known to A");
}

#[test]
fn forget_makes_the_next_hello_fail() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    pair(&a, &b);
    let a_cert: String = b.ctx.db.with(|c| Ok(c.query_row("select cert_pem from peers where device_id=?", [&a.ctx.settings.device_id], |r| r.get(0))?)).unwrap();
    let id_b = identity::load(&b.ctx).unwrap();
    assert!(Client::connect(&id_b, &a_cert, "127.0.0.1", port(&a)).unwrap().json("/v1/hello").is_ok());
    let st = super::forget(&a.ctx, "B").unwrap();
    assert!(st["peers"].as_array().unwrap().is_empty(), "{st}");
    assert!(!store::dir(&a.ctx, &b.ctx.settings.device_id).exists());
    assert!(Client::connect(&id_b, &a_cert, "127.0.0.1", port(&a)).unwrap().json("/v1/hello").is_err(), "a forgotten certificate is refused at the handshake");
    assert!(super::forget(&a.ctx, "B").is_err());
}

#[test]
fn c_unreachable_by_a_is_pulled_through_b_with_its_signature_verified() {
    let t = tempfile::tempdir().unwrap();
    let (a, b, c) = (computer(t.path(), "A"), computer(t.path(), "B"), computer(t.path(), "C"));
    pair(&a, &b);
    pair(&b, &c);
    c.ctx.db.insert_usage(&[row("claude:c1", 1_000)], &Prices::new()).unwrap();
    c.rt.tick(&c.ctx);
    b.rt.tick(&b.ctx);
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 1);
    // A knows nothing of C and cannot reach it; B lists it
    assert_eq!(count(&a.ctx, "select count(*) from peers"), 1);
    a.rt.tick(&a.ctx);
    assert_eq!(count(&a.ctx, "select count(*) from usage"), 1);
    let st = journal::status(&a.ctx).unwrap();
    let pc = peer(&st, &c.ctx.settings.device_id);
    assert_eq!((pc["name"].as_str(), pc["path"].as_str(), pc["relayed_by"].as_str(), pc["direct"].as_bool(), pc["connected"].as_bool()),
        (Some("C"), Some("relay"), Some(b.ctx.settings.device_id.as_str()), Some(false), Some(false)), "{st}");
    assert_eq!(journal::pubkey_of(&a.ctx, &c.ctx.settings.device_id).unwrap(), identity::pubkey(&c.ctx), "C's key came in B's copy of C's signed record");
    // B presents C's record under another key: ignored, and C's next chunk still verifies against the real key
    let mut fake = store::device(&b.ctx, &c.ctx.settings.device_id).unwrap();
    identity::load(&b.ctx).unwrap().sign_record(&mut fake);
    store::put_device(&b.ctx, &c.ctx.settings.device_id, &fake).unwrap();
    c.ctx.db.insert_usage(&[row("claude:c2", 2_000)], &Prices::new()).unwrap();
    c.rt.tick(&c.ctx);
    b.rt.tick(&b.ctx);
    a.rt.tick(&a.ctx);
    assert_eq!(count(&a.ctx, "select count(*) from usage"), 2);
    assert_eq!(journal::pubkey_of(&a.ctx, &c.ctx.settings.device_id).unwrap(), identity::pubkey(&c.ctx));
}

#[test]
fn a_tampered_chunk_is_skipped() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    pair(&a, &b);
    a.ctx.db.insert_usage(&[row("claude:1", 1_000)], &Prices::new()).unwrap();
    journal::publish(&a.ctx).unwrap();
    let me = a.ctx.settings.device_id.clone();
    let mut man = store::manifest(&a.ctx, &me).unwrap();
    let file = man["chunks"][0]["file"].as_str().unwrap().to_string();
    let path = store::dir(&a.ctx, &me).join("journal").join(&file);
    let good = std::fs::read(&path).unwrap();
    let bad = String::from_utf8(good.clone()).unwrap().replace("\"output\":10", "\"output\":99");
    std::fs::write(&path, &bad).unwrap();
    pull_all(&b.ctx);
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 0, "the bytes do not match the manifest");
    // a manifest made to match the changed bytes still carries A's signature of the real chunk
    man["chunks"][0]["sha256"] = json!(crate::util::sha256_hex(bad.as_bytes()));
    crate::util::write_atomic(&store::dir(&a.ctx, &me).join("manifest.json"), man.to_string().as_bytes()).unwrap();
    pull_all(&b.ctx);
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 0, "the signature does not verify");
    assert_eq!(count(&b.ctx, "select count(*) from cursors2"), 0);
    // put right, it applies once
    std::fs::write(&path, &good).unwrap();
    man["chunks"][0]["sha256"] = json!(crate::util::sha256_hex(&good));
    crate::util::write_atomic(&store::dir(&a.ctx, &me).join("manifest.json"), man.to_string().as_bytes()).unwrap();
    pull_all(&b.ctx);
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 1);
    let out: i64 = b.ctx.db.with(|c| Ok(c.query_row("select output from usage", [], |r| r.get(0))?)).unwrap();
    assert_eq!(out, 10);
}
