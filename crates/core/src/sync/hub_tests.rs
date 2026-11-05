//! Two or three computers in temp folders against a hub in this process. `fake::start` stands in for the hub
//! crate's `hub::serve(Config) -> (SocketAddr, Handle)` with the contract's routes and shapes, in memory; when
//! `crates/hub` is in the workspace, `start_hub` below is the one function to point at it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::*;
use crate::model::{SessionMeta, UsageRow};
use crate::pricing::Prices;
use crate::sync::peer::identity;

mod fake {
    use super::*;
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use axum::extract::{Path, Query, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::{IntoResponse, Response};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use base64::Engine;

    pub struct Hub {
        admin: String,
        tokens: Mutex<HashMap<String, String>>,
        codes: Mutex<HashSet<String>>,
        devices: Mutex<BTreeMap<String, Value>>,
        chunks: Mutex<HashMap<String, Vec<Value>>>,
        pub blobs: Mutex<HashMap<String, Vec<u8>>>,
        presence: Mutex<HashMap<String, (i64, String)>>,
        pub transcripts: Mutex<HashMap<String, Vec<u8>>>,
        rendezvous: Mutex<HashMap<String, Value>>,
        events: tokio::sync::broadcast::Sender<String>,
        next_code: AtomicU32,
        pub handle: axum_server::Handle<SocketAddr>,
    }

    type S = State<Arc<Hub>>;

    fn who(s: &Hub, h: &HeaderMap) -> Result<String, StatusCode> {
        let t = h.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
        s.tokens.lock().unwrap().get(t).cloned().ok_or(StatusCode::UNAUTHORIZED)
    }

    fn emit(s: &Hub, kind: &str, id: &str) { let _ = s.events.send(json!({"kind": kind, "id": id}).to_string()); }

    async fn join(State(s): S, Json(a): Json<Value>) -> Response {
        let ok = a["admin_token"].as_str() == Some(&s.admin) || a["code"].as_str().map(|c| s.codes.lock().unwrap().remove(c)).unwrap_or(false);
        if !ok { return (StatusCode::FORBIDDEN, Json(json!({"error": "the code or token is not known"}))).into_response(); }
        let Some(id) = a["device"]["id"].as_str().map(String::from) else { return StatusCode::BAD_REQUEST.into_response() };
        let token = crate::util::uuid4();
        s.tokens.lock().unwrap().insert(token.clone(), id.clone());
        s.devices.lock().unwrap().insert(id.clone(), a["device"].clone());
        emit(&s, "device", &id);
        Json(json!({"token": token, "devices": s.devices.lock().unwrap().values().cloned().collect::<Vec<_>>()})).into_response()
    }

    async fn invite(State(s): S, h: HeaderMap) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        let code = format!("{:06}", 100_000 + s.next_code.fetch_add(1, Ordering::SeqCst));
        s.codes.lock().unwrap().insert(code.clone());
        Json(json!({"code": code, "expires_at": crate::util::now_ms() + 600_000})).into_response()
    }

    async fn devices(State(s): S, h: HeaderMap) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        let chunks = s.chunks.lock().unwrap();
        // the contract's "records with their last_seen and last_seq": the record under `device`, so its signature stands
        let v: Vec<Value> = s.devices.lock().unwrap().iter().map(|(id, d)| json!({"device": d, "last_seen": crate::util::now_ms(),
            "last_seq": chunks.get(id).and_then(|c| c.last()).and_then(|c| c["to"].as_i64()).unwrap_or(0)})).collect();
        Json(v).into_response()
    }

    async fn put_device(State(s): S, h: HeaderMap, Path(id): Path<String>, body: String) -> Response {
        if who(&s, &h).ok().as_deref() != Some(&id) { return StatusCode::FORBIDDEN.into_response(); }
        let Ok(v) = serde_json::from_str::<Value>(&body) else { return StatusCode::BAD_REQUEST.into_response() };
        s.devices.lock().unwrap().insert(id.clone(), v);
        emit(&s, "device", &id);
        StatusCode::OK.into_response()
    }

    async fn delete_device(State(s): S, h: HeaderMap, Path(id): Path<String>) -> Response {
        if who(&s, &h).ok().as_deref() != Some(&id) { return StatusCode::FORBIDDEN.into_response(); }
        s.devices.lock().unwrap().remove(&id);
        s.tokens.lock().unwrap().retain(|_, d| d != &id);
        StatusCode::OK.into_response()
    }

    #[derive(serde::Deserialize)]
    pub struct Since { since: Option<i64> }

    fn inc_of(file: &str) -> String { file.trim_end_matches(".jsonl").rsplitn(3, '-').last().unwrap_or("").to_string() }

    async fn chunks(State(s): S, h: HeaderMap, Path(origin): Path<String>, Query(q): Query<Since>) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        let all = s.chunks.lock().unwrap();
        let Some(list) = all.get(&origin) else { return StatusCode::NOT_FOUND.into_response() };
        let inc = list.last().map(|c| inc_of(c["file"].as_str().unwrap_or(""))).unwrap_or_default();
        let since = q.since.unwrap_or(0);
        let chunks: Vec<Value> = list.iter().filter(|c| inc_of(c["file"].as_str().unwrap_or("")) == inc && c["to"].as_i64().unwrap_or(0) > since).cloned().collect();
        Json(json!({"origin": origin, "incarnation": inc, "last_seq": list.last().and_then(|c| c["to"].as_i64()).unwrap_or(0), "chunks": chunks})).into_response()
    }

    async fn put_chunk(State(s): S, h: HeaderMap, Path((origin, file)): Path<(String, String)>, body: axum::body::Bytes) -> Response {
        if who(&s, &h).ok().as_deref() != Some(&origin) { return StatusCode::FORBIDDEN.into_response(); }
        let hv = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let entry = json!({"file": file, "from": hv("x-from").parse::<i64>().unwrap_or(0), "to": hv("x-to").parse::<i64>().unwrap_or(0), "sha256": hv("x-sha256"), "sig": hv("x-sig"), "bytes": body.len(), "enc": true});
        let mut all = s.chunks.lock().unwrap();
        let list = all.entry(origin.clone()).or_default();
        list.retain(|c| c["file"] != entry["file"]);
        list.push(entry);
        list.sort_by_key(|c| c["to"].as_i64().unwrap_or(0));
        s.blobs.lock().unwrap().insert(format!("{origin}/{file}"), body.to_vec());
        drop(all);
        emit(&s, "chunks", &origin);
        StatusCode::OK.into_response()
    }

    async fn chunk(State(s): S, h: HeaderMap, Path((origin, file)): Path<(String, String)>) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        match s.blobs.lock().unwrap().get(&format!("{origin}/{file}")) { Some(b) => (StatusCode::OK, b.clone()).into_response(), None => StatusCode::NOT_FOUND.into_response() }
    }

    async fn put_presence(State(s): S, h: HeaderMap, body: axum::body::Bytes) -> Response {
        let Ok(id) = who(&s, &h) else { return StatusCode::UNAUTHORIZED.into_response() };
        s.presence.lock().unwrap().insert(id.clone(), (crate::util::now_ms(), base64::engine::general_purpose::STANDARD.encode(&body)));
        emit(&s, "presence", &id);
        StatusCode::OK.into_response()
    }

    async fn presence(State(s): S, h: HeaderMap) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        let m: serde_json::Map<String, Value> = s.presence.lock().unwrap().iter().map(|(id, (at, blob))| (id.clone(), json!({"at": at, "blob": blob}))).collect();
        Json(Value::Object(m)).into_response()
    }

    async fn put_transcript(State(s): S, h: HeaderMap, Path((origin, key)): Path<(String, String)>, body: axum::body::Bytes) -> Response {
        if who(&s, &h).ok().as_deref() != Some(&origin) { return StatusCode::FORBIDDEN.into_response(); }
        s.transcripts.lock().unwrap().insert(format!("{origin}/{key}"), body.to_vec());
        StatusCode::OK.into_response()
    }

    async fn get_transcript(State(s): S, h: HeaderMap, Path((origin, key)): Path<(String, String)>) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        match s.transcripts.lock().unwrap().get(&format!("{origin}/{key}")) { Some(b) => (StatusCode::OK, b.clone()).into_response(), None => StatusCode::NOT_FOUND.into_response() }
    }

    async fn delete_transcript(State(s): S, h: HeaderMap, Path((origin, key)): Path<(String, String)>) -> Response {
        if who(&s, &h).ok().as_deref() != Some(&origin) { return StatusCode::FORBIDDEN.into_response(); }
        s.transcripts.lock().unwrap().remove(&format!("{origin}/{key}"));
        StatusCode::OK.into_response()
    }

    async fn rendezvous(State(s): S, h: HeaderMap) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        Json(Value::Object(s.rendezvous.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect())).into_response()
    }

    async fn put_rendezvous(State(s): S, h: HeaderMap, Json(a): Json<Value>) -> Response {
        let Ok(id) = who(&s, &h) else { return StatusCode::UNAUTHORIZED.into_response() };
        s.rendezvous.lock().unwrap().insert(id, json!({"addresses": a["addresses"], "at": crate::util::now_ms()}));
        StatusCode::OK.into_response()
    }

    async fn events(State(s): S, h: HeaderMap, ws: WebSocketUpgrade) -> Response {
        if who(&s, &h).is_err() { return StatusCode::UNAUTHORIZED.into_response(); }
        let mut rx = s.events.subscribe();
        ws.on_upgrade(move |mut sock| async move {
            while let Ok(m) = rx.recv().await { if sock.send(Message::Text(m.into())).await.is_err() { break; } }
        }).into_response()
    }

    /// The hub on loopback at a free port, with its admin token.
    pub fn start(rt: &tokio::runtime::Handle, admin: &str) -> (SocketAddr, Arc<Hub>) {
        let (tx, _) = tokio::sync::broadcast::channel(64);
        let hub = Arc::new(Hub { admin: admin.into(), tokens: Mutex::new(HashMap::new()), codes: Mutex::new(HashSet::new()), devices: Mutex::new(BTreeMap::new()), chunks: Mutex::new(HashMap::new()),
            blobs: Mutex::new(HashMap::new()), presence: Mutex::new(HashMap::new()), transcripts: Mutex::new(HashMap::new()), rendezvous: Mutex::new(HashMap::new()), events: tx, next_code: AtomicU32::new(0), handle: axum_server::Handle::new() });
        let app = Router::new()
            .route("/v1/join", post(join)).route("/v1/invite", post(invite))
            .route("/v1/devices", get(devices)).route("/v1/devices/{id}", axum::routing::put(put_device).delete(delete_device))
            .route("/v1/chunks/{origin}", get(chunks)).route("/v1/chunks/{origin}/{file}", axum::routing::put(put_chunk)).route("/v1/chunk/{origin}/{file}", get(chunk))
            .route("/v1/presence", get(presence).put(put_presence))
            .route("/v1/transcripts/{origin}/{*key}", get(get_transcript).put(put_transcript).delete(delete_transcript))
            .route("/v1/rendezvous", get(rendezvous).put(put_rendezvous))
            .route("/v1/events", get(events))
            .with_state(hub.clone());
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        let addr = l.local_addr().unwrap();
        let _in = rt.enter();
        let server = axum_server::from_tcp(l).unwrap().handle(hub.handle.clone());
        rt.spawn(async move { let _ = server.serve(app.into_make_service()).await; });
        (addr, hub)
    }
}

/// The hub for a test: the fake today; `hub::serve(hub::Config { data_dir, admin_token, bind, max_blob_mb, retain_mb })` once the crate is merged.
fn start_hub(rt: &tokio::runtime::Handle, admin: &str) -> (String, Arc<fake::Hub>) {
    let (addr, h) = fake::start(rt, admin);
    (format!("http://{addr}"), h)
}

struct Computer { ctx: Ctx }

impl Drop for Computer {
    fn drop(&mut self) {
        let _ = crate::secrets::delete(&identity::target(&self.ctx.settings.device_id));
        for t in keys_of(&self.ctx) { let _ = crate::secrets::delete(&t); }
    }
}

fn computer(dir: &Path, name: &str) -> Computer {
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id()));
    let data = dir.join(name);
    let mut ctx = Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap();
    ctx.settings.device_name = name.into();
    ctx.save_settings().unwrap();
    Computer { ctx: Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap() }
}

fn row(id: &str, ts: i64) -> UsageRow {
    UsageRow { request_id: id.into(), profile: "work".into(), harness: "claude".into(), ts, model: "claude-opus-5-5".into(), project: "C:\\p".into(),
        session_id: "s1".into(), input: 100, output: 10, cache_read: 50, cache_write: 0, ttft_ms: Some(900), gen_ms: None }
}

fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

fn peer<'a>(st: &'a Value, id: &str) -> &'a Value { st["peers"].as_array().unwrap().iter().find(|p| p["id"] == id).expect("peer in status") }

/// A joins with the admin token (and makes the ring), B with a code from A's invite and A's recovery key typed in.
fn joined(dir: &Path, url: &str) -> (Computer, Computer) {
    let (mut a, mut b) = (computer(dir, "A"), computer(dir, "B"));
    a.ctx = join(&a.ctx, url, None, Some("admin-secret")).unwrap();
    let inv = invite(&a.ctx).unwrap();
    b.ctx = join(&b.ctx, url, inv["code"].as_str(), None).unwrap();
    ring_enter(&b.ctx, &ring_show(&a.ctx).unwrap()).unwrap();
    (a, b)
}

fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap() }

#[test]
fn the_ring_key_has_a_recovery_form_that_round_trips_and_seals_blobs() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    assert!(!has_ring(&a.ctx) && ring_key(&a.ctx).unwrap_err().to_string().contains("enter the recovery key"));
    let shown = ring_make(&a.ctx).unwrap();
    let groups: Vec<&str> = shown.split(' ').collect();
    assert_eq!(groups.len(), 8, "{shown}");
    assert!(groups.iter().all(|g| g.len() == 4 && g.chars().all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))), "{shown}");
    assert_eq!(status(&a.ctx)["ring_shown"], false, "made, not yet shown");
    assert_eq!(ring_show(&a.ctx).unwrap(), shown);
    assert_eq!(status(&a.ctx)["ring_shown"], true);
    // typed on B with dashes, lower case and the usual mistakes: the same key
    ring_enter(&b.ctx, &shown.to_lowercase().replace(' ', "-").replace('o', "0")).unwrap();
    assert_eq!(ring_key(&b.ctx).unwrap(), ring_key(&a.ctx).unwrap());
    assert!(ring_enter(&b.ctx, "ABCD").unwrap_err().to_string().contains("8 groups of 4"));
    let sealed = encrypt(&ring_key(&a.ctx).unwrap(), b"hello").unwrap();
    assert_eq!(decrypt(&ring_key(&b.ctx).unwrap(), &sealed).unwrap(), b"hello");
    let other = ring_key(&computer(t.path(), "C").ctx).err();
    assert!(other.is_some());
    let mut bad = sealed.clone();
    bad[20] ^= 1;
    assert!(decrypt(&ring_key(&a.ctx).unwrap(), &bad).is_err());
    // the pairing payload carries it, and a computer without one takes it
    let c = computer(t.path(), "C");
    ring_adopt(&c.ctx, &ring_for_pairing(&a.ctx));
    assert_eq!(ring_key(&c.ctx).unwrap(), ring_key(&a.ctx).unwrap());
    assert_eq!(ring_for_pairing(&computer(t.path(), "D").ctx), Value::Null);
}

#[test]
fn two_computers_join_a_pushes_and_b_pulls_and_applies_once() {
    let rt = runtime();
    let (url, _hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (mut a, mut b) = (computer(t.path(), "A"), computer(t.path(), "B"));
    // a wrong code or token is refused; the right ones join and switch the settings to the hub
    let e = join(&b.ctx, &url, Some("000000"), None).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(e.contains("did not accept the code"), "{e}");
    assert!(join(&a.ctx, &url, None, Some("wrong")).is_err());
    assert!(join(&a.ctx, "hub.example.com", None, Some("admin-secret")).err().map(|e| e.to_string()).unwrap_or_default().contains("https://"));
    a.ctx = join(&a.ctx, &url, None, Some("admin-secret")).unwrap();
    assert_eq!((a.ctx.settings.sync_mode.as_str(), a.ctx.settings.hub_url.as_str()), ("hub", url.as_str()));
    assert!(has_ring(&a.ctx), "the first computer makes the ring");
    let st = status(&a.ctx);
    assert_eq!((st["joined"].as_bool(), st["ring"].as_bool(), st["ring_shown"].as_bool(), st["devices"].as_i64()), (Some(true), Some(true), Some(false), Some(1)), "{st}");
    let inv = invite(&a.ctx).unwrap();
    assert_eq!(inv["code"].as_str().unwrap().len(), 6);
    b.ctx = join(&b.ctx, &url, inv["code"].as_str(), None).unwrap();
    assert!(!has_ring(&b.ctx), "a code does not carry the ring");
    assert!(join(&computer(t.path(), "X").ctx, &url, inv["code"].as_str(), None).is_err(), "a code is one-time");
    // A's rows go up sealed; B cannot read without the ring and is told so
    let prices: Prices = [("claude-opus-5-5".to_string(), crate::pricing::Price { input: 10.0, output: 10.0, cache_read: 1.0, cache_write: 1.0 })].into_iter().collect();
    assert_eq!(a.ctx.db.insert_usage(&[row("claude:1", 1_000), row("claude:2", 2_000)], &prices).unwrap(), 2, "hub mode: the journal records");
    a.ctx.db.tag_set("project", "github.com/o/r", &["release".into()]).unwrap();
    let st = journal::round(&a.ctx).unwrap();
    assert_eq!((st["error"].as_str(), st["this"]["published_seq"].as_i64()), (None, Some(3)), "{st}");
    assert!(status(&a.ctx)["last_push"].is_i64());
    let blobs = _hub.blobs.lock().unwrap().clone();
    assert_eq!(blobs.len(), 1);
    assert!(!String::from_utf8_lossy(blobs.values().next().unwrap()).contains("claude:1"), "the hub holds ciphertext");
    let st = journal::round(&b.ctx).unwrap();
    assert!(st["error"].as_str().unwrap_or("").contains("enter the recovery key"), "{st}");
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 0);
    assert_eq!(st["hub"]["ring"], false);
    // with A's recovery key typed in, B reads A: rows, cost, tag, the device learned with its key, once
    ring_enter(&b.ctx, &ring_show(&a.ctx).unwrap()).unwrap();
    let st = journal::round(&b.ctx).unwrap();
    assert_eq!(st["error"], Value::Null, "{st}");
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 2);
    let (dev, cost): (String, f64) = b.ctx.db.with(|c| Ok(c.query_row("select device, cost_usd from usage where request_id='claude:1'", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!(dev, a.ctx.settings.device_id);
    assert!((cost - 0.00115).abs() < 1e-9);
    assert_eq!(b.ctx.db.tags_of("project").unwrap()["github.com/o/r"], vec!["release".to_string()]);
    let pa = peer(&st, &a.ctx.settings.device_id);
    assert_eq!((pa["name"].as_str(), pa["transport"].as_str(), pa["applied_seq"].as_i64(), pa["last_seq"].as_i64(), pa["direct"].as_bool()), (Some("A"), Some("hub"), Some(3), Some(3), Some(false)), "{st}");
    assert!(pa["last_pull"].is_i64() && pa["last_seen"].is_i64());
    assert_eq!(journal::pubkey_of(&b.ctx, &a.ctx.settings.device_id).unwrap(), identity::pubkey(&a.ctx), "A's key came in its signed record");
    assert_eq!((st["hub"]["joined"].as_bool(), st["hub"]["devices"].as_i64(), st["hub"]["ws"].as_bool()), (Some(true), Some(2), Some(false)));
    // B keeps A's chunk for relay; another round adds nothing and nothing bounces back
    assert_eq!(store::manifest(&b.ctx, &a.ctx.settings.device_id).unwrap()["chunks"].as_array().unwrap().len(), 1);
    journal::round(&b.ctx).unwrap();
    assert_eq!((count(&b.ctx, "select count(*) from usage"), count(&b.ctx, "select count(*) from cursors2"), count(&b.ctx, "select count(*) from outbox")), (2, 1, 0));
    // the other way: B's row reaches A
    b.ctx.db.insert_usage(&[row("claude:b1", 3_000)], &prices).unwrap();
    journal::round(&b.ctx).unwrap();
    journal::round(&a.ctx).unwrap();
    assert_eq!(count(&a.ctx, "select count(*) from usage"), 3);
    assert_eq!(crate::service::overview(&a.ctx, None, None, Some("here")).unwrap().requests, 2);
    // leaving: the token goes and the mode falls back to off
    a.ctx = leave(&a.ctx).unwrap();
    assert_eq!((a.ctx.settings.sync_mode.as_str(), token(&a.ctx).is_none(), status(&a.ctx)["joined"].as_bool()), ("off", true, Some(false)));
}

#[test]
fn presence_round_trips_through_the_hub() {
    let rt = runtime();
    let (url, _hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (a, b) = joined(t.path(), &url);
    journal::round(&a.ctx).unwrap();
    journal::round(&b.ctx).unwrap();
    let st = journal::round(&a.ctx).unwrap();
    let pb = peer(&st, &b.ctx.settings.device_id);
    assert!(pb["presence_at"].as_i64().unwrap() > 0, "{st}");
    let p: String = a.ctx.db.with(|c| Ok(c.query_row("select presence from peers where device_id=?", [&b.ctx.settings.device_id], |r| r.get(0))?)).unwrap();
    let p: Value = serde_json::from_str(&p).unwrap();
    assert!(p["accounts"].is_object() && p["tps_5m"].is_number() && p["running"].is_array(), "{p}");
    // the minute tick pushes presence alone
    journal::presence_tick(&b.ctx).unwrap();
    let st = journal::round(&a.ctx).unwrap();
    assert!(peer(&st, &b.ctx.settings.device_id)["presence_at"].as_i64().unwrap() >= p["at"].as_i64().unwrap());
}

#[test]
fn a_tampered_chunk_on_the_hub_is_skipped() {
    let rt = runtime();
    let (url, hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (a, b) = joined(t.path(), &url);
    a.ctx.db.insert_usage(&[row("claude:1", 1_000)], &Prices::new()).unwrap();
    journal::round(&a.ctx).unwrap();
    let (name, good) = hub.blobs.lock().unwrap().iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap();
    let mut bad = good.clone();
    bad[30] ^= 0x55;
    hub.blobs.lock().unwrap().insert(name.clone(), bad);
    let st = journal::round(&b.ctx).unwrap();
    assert_eq!(st["error"], Value::Null, "a bad chunk is not an error, it is retried: {st}");
    assert_eq!((count(&b.ctx, "select count(*) from usage"), count(&b.ctx, "select count(*) from cursors2")), (0, 0));
    hub.blobs.lock().unwrap().insert(name, good);
    journal::round(&b.ctx).unwrap();
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 1);
    journal::round(&b.ctx).unwrap();
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 1);
}

#[test]
fn rendezvous_addresses_flow_and_come_first_for_the_direct_connection() {
    let rt = runtime();
    let (url, _hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (mut a, mut b) = joined(t.path(), &url);
    for c in [&mut a, &mut b] {
        let mut x = Ctx::open_at(&c.ctx.data, &c.ctx.home).unwrap();
        x.settings.direct_connection = true;
        x.settings.listen_port = 45_000 + (c.ctx.settings.device_name.as_bytes()[0] as u16);
        x.save_settings().unwrap();
        c.ctx = Ctx::open_at(&c.ctx.data, &c.ctx.home).unwrap();
    }
    journal::round(&a.ctx).unwrap();
    journal::round(&b.ctx).unwrap();
    let addrs = rendezvous_of(&b.ctx, &a.ctx.settings.device_id);
    assert!(addrs.iter().any(|x| x == &format!("{}:45065", crate::util::hostname())), "{addrs:?}");
    assert!(addrs.iter().all(|x| x.ends_with(":45065")), "{addrs:?}");
    let pairs = rendezvous_pairs(&b.ctx, &a.ctx.settings.device_id);
    assert_eq!(pairs.len(), addrs.len());
    assert!(pairs.iter().all(|(_, p)| *p == 45065));
    assert!(rendezvous_of(&b.ctx, "nobody").is_empty());
}

#[test]
fn transcripts_go_to_the_hub_sealed_and_are_fetched_on_demand() {
    let rt = runtime();
    let (url, hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (a, b) = joined(t.path(), &url);
    let mut reg = a.ctx.registry().unwrap();
    reg.add_profile(crate::registry::Profile { name: "claude-main".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }).unwrap();
    a.ctx.save_registry(&reg).unwrap();
    let cwd = t.path().join("app");
    std::fs::create_dir_all(&cwd).unwrap();
    let f = a.ctx.home.join(".claude").join("projects").join(crate::harness::claude::project_dir_name(&cwd)).join("s1.jsonl");
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, format!("{}\n", json!({"type": "user", "sessionId": "s1", "cwd": cwd, "timestamp": "2026-11-05T10:00:00.000Z", "message": {"role": "user", "content": "fix the build"}}))).unwrap();
    let ts = crate::harness::parse::ts_ms("2026-11-05T10:00:00.000Z");
    a.ctx.db.journal_session(&SessionMeta { harness: "claude".into(), profile: "claude-main".into(), id: "s1".into(), title: "fix the build".into(), cwd: cwd.display().to_string(), started_at: ts, last_active_at: ts,
        project_key: "k".into(), project_name: "r".into(), source_path: f.display().to_string(), ..Default::default() }).unwrap();
    journal::round(&a.ctx).unwrap();
    assert_eq!(crate::transcripts::status(&a.ctx).unwrap()["carried"], 1);
    let stored = hub.transcripts.lock().unwrap().clone();
    assert_eq!(stored.len(), 1);
    assert!(stored.keys().next().unwrap().ends_with("/claude/claude-main/s1"), "{:?}", stored.keys());
    assert!(zstd::decode_all(&stored.values().next().unwrap()[..]).is_err(), "sealed, not plain zstd");
    journal::round(&b.ctx).unwrap();
    assert_eq!(count(&b.ctx, "select count(*) from transcripts"), 1);
    let fetched = crate::transcripts::fetch(&b.ctx, &a.ctx.settings.device_id, "claude", "claude-main", "s1").unwrap();
    assert_eq!(std::fs::read(&fetched.path).unwrap(), std::fs::read(&f).unwrap());
    assert_eq!(crate::transcripts::status(&b.ctx).unwrap()["fetched"], 1);
}

#[test]
fn an_event_from_the_hub_runs_a_round_on_the_other_computer() {
    let rt = runtime();
    let (url, _hub) = start_hub(rt.handle(), "admin-secret");
    let t = tempfile::tempdir().unwrap();
    let (a, b) = joined(t.path(), &url);
    journal::round(&b.ctx).unwrap();
    let (data, home) = (b.ctx.data.clone(), b.ctx.home.clone());
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    watch(Arc::new(move || Arc::new(Ctx::open_at(&data, &home).unwrap())), Arc::new(move |v| s2.lock().unwrap().push(v)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < until && status(&b.ctx)["ws"] != true { std::thread::sleep(std::time::Duration::from_millis(100)); }
    assert_eq!(status(&b.ctx)["ws"], true, "the events socket is open");
    a.ctx.db.insert_usage(&[row("claude:1", 1_000)], &Prices::new()).unwrap();
    journal::round(&a.ctx).unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < until && count(&b.ctx, "select count(*) from usage") == 0 { std::thread::sleep(std::time::Duration::from_millis(100)); }
    assert_eq!(count(&b.ctx, "select count(*) from usage"), 1, "the chunk event made B pull");
    assert!(!seen.lock().unwrap().is_empty(), "the status was told");
}
