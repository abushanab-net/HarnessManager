//! The service in-process on a loopback port, driven with reqwest; two signing computers A and B.

use ed25519_dalek::{Signer, SigningKey};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::{b64, Config};

const ADMIN: &str = "admin-secret";

struct Hub {
    url: String,
    _dir: tempfile::TempDir,
    _handle: axum_server::Handle<std::net::SocketAddr>,
}

async fn start(retain_mb: u64) -> Hub {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config { data_dir: dir.path().to_path_buf(), admin_token: ADMIN.into(), bind: "127.0.0.1:0".parse().unwrap(), max_blob_mb: 4, retain_mb };
    let (addr, handle) = super::serve(cfg).await.unwrap();
    Hub { url: format!("http://{addr}"), _dir: dir, _handle: handle }
}

struct Dev {
    id: String,
    key: SigningKey,
    token: String,
}

impl Dev {
    fn new(id: &str) -> Dev {
        let mut raw = [0u8; 32];
        getrandom::fill(&mut raw).unwrap();
        Dev { id: id.into(), key: SigningKey::from_bytes(&raw), token: String::new() }
    }

    fn record(&self) -> Value { self.record_at(1) }

    fn record_at(&self, updated_at: i64) -> Value {
        let mut v = json!({"id": self.id, "name": format!("pc-{}", self.id), "os": "windows", "app_version": "1.5.0", "incarnation": "inc1", "roots": {}, "updated_at": updated_at});
        v["pubkey"] = json!(b64(&self.key.verifying_key().to_bytes()));
        let sig = self.key.sign(v.to_string().as_bytes());
        v["sig"] = json!(b64(&sig.to_bytes()));
        v
    }

    async fn join(&mut self, hub: &Hub, how: Value) -> StatusCode {
        let mut body = how;
        body["device"] = self.record();
        let r = Client::new().post(format!("{}/v1/join", hub.url)).json(&body).send().await.unwrap();
        let status = r.status();
        if status == StatusCode::OK {
            let v: Value = r.json().await.unwrap();
            self.token = v["token"].as_str().unwrap().into();
        }
        status
    }

    fn req(&self, method: reqwest::Method, hub: &Hub, path: &str) -> reqwest::RequestBuilder { Client::new().request(method, format!("{}{path}", hub.url)).bearer_auth(&self.token) }

    async fn put_chunk(&self, hub: &Hub, origin: &str, file: &str, from: i64, to: i64, body: Vec<u8>) -> StatusCode {
        self.req(reqwest::Method::PUT, hub, &format!("/v1/chunks/{origin}/{file}")).header("X-From", from).header("X-To", to).header("X-Sha256", format!("sha-{file}")).header("X-Sig", format!("sig-{file}")).body(body).send().await.unwrap().status()
    }

    async fn chunks(&self, hub: &Hub, origin: &str, since: i64) -> Value {
        self.req(reqwest::Method::GET, hub, &format!("/v1/chunks/{origin}?since={since}")).send().await.unwrap().json().await.unwrap()
    }
}

async fn joined(hub: &Hub) -> (Dev, Dev) {
    let (mut a, mut b) = (Dev::new("aaaa"), Dev::new("bbbb"));
    assert_eq!(a.join(hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    assert_eq!(b.join(hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    (a, b)
}

#[tokio::test]
async fn join_with_the_admin_token_then_an_invite_code() {
    let hub = start(2048).await;
    let mut a = Dev::new("aaaa");
    assert_eq!(a.join(&hub, json!({"admin_token": "nope"})).await, StatusCode::UNAUTHORIZED);
    assert_eq!(a.join(&hub, json!({})).await, StatusCode::BAD_REQUEST);
    assert_eq!(a.join(&hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    // an unsigned record is refused
    let mut rec = a.record();
    rec["name"] = json!("tampered");
    let r = Client::new().post(format!("{}/v1/join", hub.url)).json(&json!({"admin_token": ADMIN, "device": rec})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    let inv: Value = a.req(reqwest::Method::POST, &hub, "/v1/invite").send().await.unwrap().json().await.unwrap();
    let code = inv["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 6);
    assert!(inv["expires_at"].as_i64().unwrap() > 0);
    let mut b = Dev::new("bbbb");
    assert_eq!(b.join(&hub, json!({"code": "000000"})).await, StatusCode::UNAUTHORIZED, "a wrong code");
    assert_eq!(b.join(&hub, json!({"code": code})).await, StatusCode::OK);
    let mut c = Dev::new("cccc");
    assert_eq!(c.join(&hub, json!({"code": code})).await, StatusCode::UNAUTHORIZED, "a used code");

    let list: Value = b.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().json().await.unwrap();
    let ids: Vec<&str> = list.as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["aaaa", "bbbb"]);
    assert_eq!(list[0]["last_seq"], 0);
    assert!(list[0]["last_seen"].as_i64().unwrap() > 0);
    // no token, or a made-up one
    assert_eq!(Client::new().get(format!("{}/v1/devices", hub.url)).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(Client::new().get(format!("{}/v1/devices", hub.url)).bearer_auth("made-up").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_code_does_not_take_over_a_computer_already_here() {
    let hub = start(2048).await;
    let mut a = Dev::new("aaaa");
    assert_eq!(a.join(&hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    let code = |a: &Dev| { let r = a.req(reqwest::Method::POST, &hub, "/v1/invite").send(); async move { r.await.unwrap().json::<Value>().await.unwrap()["code"].as_str().unwrap().to_string() } };
    let post = |code: String, device: Value| { let url = format!("{}/v1/join", hub.url); async move { Client::new().post(url).json(&json!({"code": code, "device": device})).send().await.unwrap().status() } };
    // another key under its id
    let thief = Dev::new("aaaa");
    assert_eq!(post(code(&a).await, thief.record_at(99)).await, StatusCode::FORBIDDEN);
    // its own record sent again: every computer here can read that one
    assert_eq!(post(code(&a).await, a.record()).await, StatusCode::FORBIDDEN);
    let alive = a.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().status();
    assert_eq!(alive, StatusCode::OK, "its token still works");
    // the computer itself, with a record only its key can have signed just now
    assert_eq!(post(code(&a).await, a.record_at(2)).await, StatusCode::OK);
    // and the admin can always put a computer back
    let mut again = Dev::new("aaaa");
    assert_eq!(again.join(&hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
}

#[tokio::test]
async fn wrong_join_attempts_are_held_up() {
    let hub = start(2048).await;
    let mut a = Dev::new("aaaa");
    assert_eq!(a.join(&hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    let inv: Value = a.req(reqwest::Method::POST, &hub, "/v1/invite").send().await.unwrap().json().await.unwrap();
    let mut b = Dev::new("bbbb");
    for i in 0..super::JOIN_FAILS_A_MINUTE {
        let wrong = if inv["code"] == "999999" { format!("99999{}", i % 8) } else { "999999".to_string() };
        assert_eq!(b.join(&hub, json!({"code": wrong})).await, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(b.join(&hub, json!({"code": inv["code"]})).await, StatusCode::TOO_MANY_REQUESTS, "even the right code waits: a guess must not be told apart");
    assert_eq!(b.join(&hub, json!({"admin_token": "nope"})).await, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_token_is_tied_to_its_computer() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    assert_eq!(a.put_chunk(&hub, "bbbb", "inc1-1-2.jsonl", 1, 2, b"x".to_vec()).await, StatusCode::FORBIDDEN);
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-1-2.jsonl", 1, 2, b"x".to_vec()).await, StatusCode::NO_CONTENT);
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/devices/bbbb").json(&b.record()).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/devices/aaaa").json(&a.record()).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/transcripts/bbbb/claude/p/s1").header("X-Sha256", "s").header("X-Bytes", "1").body("t").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    // the admin token is nobody's: it cannot put
    let r = Client::new().put(format!("{}/v1/chunks/aaaa/inc1-3-4.jsonl", hub.url)).bearer_auth(ADMIN).header("X-From", 3).header("X-To", 4).header("X-Sha256", "s").header("X-Sig", "g").body("x").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn chunks_round_trip_with_their_headers() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-1-5.jsonl", 1, 5, b"encrypted-one".to_vec()).await, StatusCode::NO_CONTENT);
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-6-9.jsonl", 6, 9, b"encrypted-two".to_vec()).await, StatusCode::NO_CONTENT);
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/chunks/aaaa/inc1-10-11.jsonl").header("X-From", 10).body("x").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST, "missing headers");
    // a name that decodes to a path (`sub/x`) never reaches the disk
    assert_eq!(a.put_chunk(&hub, "aaaa", "sub%2Fx", 1, 2, b"x".to_vec()).await, StatusCode::BAD_REQUEST);

    let list = b.chunks(&hub, "aaaa", 0).await;
    assert_eq!(list["origin"], "aaaa");
    assert_eq!(list["incarnation"], "inc1");
    assert_eq!(list["chunks"], json!([
        {"file": "inc1-1-5.jsonl", "from": 1, "to": 5, "sha256": "sha-inc1-1-5.jsonl", "sig": "sig-inc1-1-5.jsonl", "bytes": 13, "enc": true},
        {"file": "inc1-6-9.jsonl", "from": 6, "to": 9, "sha256": "sha-inc1-6-9.jsonl", "sig": "sig-inc1-6-9.jsonl", "bytes": 13, "enc": true}]));
    assert_eq!(b.chunks(&hub, "aaaa", 5).await["chunks"].as_array().unwrap().len(), 1);
    assert_eq!(b.chunks(&hub, "aaaa", 9).await["chunks"].as_array().unwrap().len(), 0);
    assert_eq!(b.req(reqwest::Method::GET, &hub, "/v1/chunks/zzzz").send().await.unwrap().status(), StatusCode::NOT_FOUND);
    let bytes = b.req(reqwest::Method::GET, &hub, "/v1/chunk/aaaa/inc1-6-9.jsonl").send().await.unwrap().bytes().await.unwrap();
    assert_eq!(&bytes[..], b"encrypted-two");
    assert_eq!(b.req(reqwest::Method::GET, &hub, "/v1/chunk/aaaa/inc1-0-0.jsonl").send().await.unwrap().status(), StatusCode::NOT_FOUND);
    let list: Value = b.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().json().await.unwrap();
    assert_eq!(list[0]["last_seq"], 9);

    // a transcript too: bytes back with its headers, then gone
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/transcripts/aaaa/claude/p/s1").header("X-Sha256", "tsha").header("X-Bytes", "4096").body("enc-transcript").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let r = b.req(reqwest::Method::GET, &hub, "/v1/transcripts/aaaa/claude/p/s1").send().await.unwrap();
    assert_eq!(r.headers()["x-sha256"], "tsha");
    assert_eq!(r.headers()["x-bytes"], "4096");
    assert_eq!(&r.bytes().await.unwrap()[..], b"enc-transcript");
    assert_eq!(b.req(reqwest::Method::DELETE, &hub, "/v1/transcripts/aaaa/claude/p/s1").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(a.req(reqwest::Method::DELETE, &hub, "/v1/transcripts/aaaa/claude/p/s1").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(b.req(reqwest::Method::GET, &hub, "/v1/transcripts/aaaa/claude/p/s1").send().await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn presence_and_rendezvous_round_trip() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    assert_eq!(a.req(reqwest::Method::PUT, &hub, "/v1/presence").body(b"enc-presence".to_vec()).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let p: Value = b.req(reqwest::Method::GET, &hub, "/v1/presence").send().await.unwrap().json().await.unwrap();
    assert_eq!(p["aaaa"]["blob"], b64(b"enc-presence"));
    assert!(p["aaaa"]["at"].as_i64().unwrap() > 0);
    assert!(p.get("bbbb").is_none());

    assert_eq!(a.req(reqwest::Method::PUT, &hub, "/v1/rendezvous").json(&json!({"addresses": ["10.0.0.5:4711", "pc-a.vpn:4711"]})).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let r: Value = b.req(reqwest::Method::GET, &hub, "/v1/rendezvous").send().await.unwrap().json().await.unwrap();
    assert_eq!(r["aaaa"]["addresses"], json!(["10.0.0.5:4711", "pc-a.vpn:4711"]));
    assert!(r["aaaa"]["at"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn retention_drops_the_oldest_chunk_but_never_the_newest() {
    let hub = start(1).await;
    let (a, _b) = joined(&hub).await;
    let big = vec![7u8; 700 * 1024];
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-1-1.jsonl", 1, 1, big.clone()).await, StatusCode::NO_CONTENT);
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-2-2.jsonl", 2, 2, big.clone()).await, StatusCode::NO_CONTENT);
    let files = |v: &Value| v["chunks"].as_array().unwrap().iter().map(|c| c["file"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(files(&a.chunks(&hub, "aaaa", 0).await), ["inc1-2-2.jsonl"]);
    assert_eq!(a.req(reqwest::Method::GET, &hub, "/v1/chunk/aaaa/inc1-1-1.jsonl").send().await.unwrap().status(), StatusCode::NOT_FOUND);
    // one chunk over the cap by itself still stays
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-3-3.jsonl", 3, 3, vec![1u8; 1100 * 1024]).await, StatusCode::NO_CONTENT);
    assert_eq!(files(&a.chunks(&hub, "aaaa", 0).await), ["inc1-3-3.jsonl"]);
    // over max_blob_mb (4) is refused: 413, or the connection cut while the body is still being sent
    let r = a.req(reqwest::Method::PUT, &hub, "/v1/chunks/aaaa/inc1-4-4.jsonl").header("X-From", 4).header("X-To", 4).header("X-Sha256", "s").header("X-Sig", "g").body(vec![1u8; 5 * 1024 * 1024]).send().await;
    assert!(r.map(|r| r.status() == StatusCode::PAYLOAD_TOO_LARGE).unwrap_or(true));
    assert_eq!(files(&a.chunks(&hub, "aaaa", 0).await), ["inc1-3-3.jsonl"]);
}

#[tokio::test]
async fn the_websocket_tells_of_a_chunk_put() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    let mut req = format!("{}/v1/events", hub.url.replace("http://", "ws://")).into_client_request().unwrap();
    req.headers_mut().insert("Authorization", format!("Bearer {}", b.token).parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-1-2.jsonl", 1, 2, b"x".to_vec()).await, StatusCode::NO_CONTENT);
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(msg.to_text().unwrap()).unwrap(), json!({"kind": "chunks", "id": "aaaa"}));
    assert_eq!(a.req(reqwest::Method::PUT, &hub, "/v1/presence").body("p").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(msg.to_text().unwrap()).unwrap(), json!({"kind": "presence", "id": "aaaa"}));
    // without a token the upgrade is refused
    let req = format!("{}/v1/events", hub.url.replace("http://", "ws://")).into_client_request().unwrap();
    assert!(tokio_tungstenite::connect_async(req).await.is_err());
}

#[tokio::test]
async fn delete_a_device_by_its_owner_and_by_the_admin() {
    let hub = start(2048).await;
    let (mut a, b) = joined(&hub).await;
    assert_eq!(a.put_chunk(&hub, "aaaa", "inc1-1-2.jsonl", 1, 2, b"x".to_vec()).await, StatusCode::NO_CONTENT);
    assert_eq!(b.req(reqwest::Method::DELETE, &hub, "/v1/devices/aaaa").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(a.req(reqwest::Method::DELETE, &hub, "/v1/devices/aaaa").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(a.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().status(), StatusCode::UNAUTHORIZED, "the token is gone");
    assert_eq!(b.req(reqwest::Method::GET, &hub, "/v1/chunks/aaaa").send().await.unwrap().status(), StatusCode::NOT_FOUND);
    assert_eq!(b.req(reqwest::Method::GET, &hub, "/v1/chunk/aaaa/inc1-1-2.jsonl").send().await.unwrap().status(), StatusCode::NOT_FOUND);
    let list: Value = b.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    assert_eq!(a.join(&hub, json!({"admin_token": ADMIN})).await, StatusCode::OK);
    let r = Client::new().delete(format!("{}/v1/devices/aaaa", hub.url)).bearer_auth(ADMIN).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let list: Value = b.req(reqwest::Method::GET, &hub, "/v1/devices").send().await.unwrap().json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
}

// ---------- phones and their mailboxes ----------

/// A phone redeemed with an invite from `by`: (its id on the hub, its token).
async fn phone(hub: &Hub, by: &Dev) -> (String, String) {
    let inv: Value = by.req(reqwest::Method::POST, hub, "/v1/phones/invite").send().await.unwrap().json().await.unwrap();
    let r = Client::new().post(format!("{}/v1/phones/join", hub.url)).json(&json!({"code": inv["code"]})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v: Value = r.json().await.unwrap();
    assert_eq!((v["phone_id"].as_str(), v["computer"]["id"].as_str(), v["computer"]["name"].as_str()), (inv["phone_id"].as_str(), Some(by.id.as_str()), Some(format!("pc-{}", by.id).as_str())));
    (v["phone_id"].as_str().unwrap().into(), v["token"].as_str().unwrap().into())
}

fn as_token(method: reqwest::Method, hub: &Hub, token: &str, path: &str) -> reqwest::RequestBuilder { Client::new().request(method, format!("{}{path}", hub.url)).bearer_auth(token) }

#[tokio::test]
async fn a_phone_invite_is_a_computers_and_is_redeemed_once() {
    let hub = start(2048).await;
    let (a, _b) = joined(&hub).await;
    // the admin owns nothing, and nobody without a token invites
    assert_eq!(Client::new().post(format!("{}/v1/phones/invite", hub.url)).bearer_auth(ADMIN).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(Client::new().post(format!("{}/v1/phones/invite", hub.url)).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    let inv: Value = a.req(reqwest::Method::POST, &hub, "/v1/phones/invite").send().await.unwrap().json().await.unwrap();
    assert_eq!(inv["code"].as_str().unwrap().len(), 6);
    assert!(inv["phone_id"].as_str().unwrap().starts_with("ph-") && inv["expires_at"].as_i64().unwrap() > 0);
    let join = |code: Value| { let url = format!("{}/v1/phones/join", hub.url); async move { Client::new().post(url).json(&json!({"code": code})).send().await.unwrap().status() } };
    let wrong = if inv["code"] == "000000" { "000001" } else { "000000" };
    assert_eq!(join(json!(wrong)).await, StatusCode::UNAUTHORIZED, "a wrong code");
    assert_eq!(join(inv["code"].clone()).await, StatusCode::OK);
    assert_eq!(join(inv["code"].clone()).await, StatusCode::UNAUTHORIZED, "a used code");
    // a computer's invite code is not a phone's
    let cinv: Value = a.req(reqwest::Method::POST, &hub, "/v1/invite").send().await.unwrap().json().await.unwrap();
    assert_eq!(join(cinv["code"].clone()).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_phone_token_reaches_only_its_own_mailbox() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    let (p1, t1) = phone(&hub, &a).await;
    let (_p2, t2) = phone(&hub, &a).await;
    // nothing of the computers' routes
    for path in ["/v1/devices", "/v1/presence", "/v1/rendezvous", "/v1/chunks/aaaa", "/v1/chunk/aaaa/x", "/v1/transcripts/aaaa/k"] {
        assert_eq!(as_token(reqwest::Method::GET, &hub, &t1, path).send().await.unwrap().status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    assert_eq!(as_token(reqwest::Method::POST, &hub, &t1, "/v1/invite").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(as_token(reqwest::Method::POST, &hub, &t1, "/v1/phones/invite").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t1, "/v1/presence").body("x").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    let mut req = format!("{}/v1/events", hub.url.replace("http://", "ws://")).into_client_request().unwrap();
    req.headers_mut().insert("Authorization", format!("Bearer {t1}").parse().unwrap());
    assert!(tokio_tungstenite::connect_async(req).await.is_err(), "no events for a phone");
    // its own up, read back by the computer that invited it
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t1, &format!("/v1/mail/{p1}/up/1")).body("sealed-1").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let r = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p1}/up/1")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers()["x-at"].to_str().unwrap().parse::<i64>().unwrap() > 0);
    assert_eq!(&r.bytes().await.unwrap()[..], b"sealed-1");
    // never another phone's mailbox, and never the other side's direction
    assert_eq!(as_token(reqwest::Method::GET, &hub, &t2, &format!("/v1/mail/{p1}/up/1")).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t2, &format!("/v1/mail/{p1}/up/2")).body("x").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t1, &format!("/v1/mail/{p1}/down/1")).body("x").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(a.req(reqwest::Method::PUT, &hub, &format!("/v1/mail/{p1}/up/2")).body("x").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(as_token(reqwest::Method::DELETE, &hub, &t1, &format!("/v1/phones/{p1}")).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    // another computer on the hub is not the one that invited it
    assert_eq!(b.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p1}/up/1")).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(b.req(reqwest::Method::PUT, &hub, &format!("/v1/mail/{p1}/down/1")).body("x").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(b.req(reqwest::Method::DELETE, &hub, &format!("/v1/phones/{p1}")).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    // the computer's down, read by the phone; an unknown direction or phone is nothing
    assert_eq!(a.req(reqwest::Method::PUT, &hub, &format!("/v1/mail/{p1}/down/1")).body("sealed-d1").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(&as_token(reqwest::Method::GET, &hub, &t1, &format!("/v1/mail/{p1}/down/1")).send().await.unwrap().bytes().await.unwrap()[..], b"sealed-d1");
    assert_eq!(as_token(reqwest::Method::GET, &hub, &t1, &format!("/v1/mail/{p1}/sideways/1")).send().await.unwrap().status(), StatusCode::NOT_FOUND);
    assert_eq!(a.req(reqwest::Method::GET, &hub, "/v1/mail/ph-nobody/up/1").send().await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mail_is_numbered_once_capped_and_kept_a_week() {
    let hub = start(2048).await;
    let (a, _b) = joined(&hub).await;
    let (p, t) = phone(&hub, &a).await;
    let put = |seq: i64, body: Vec<u8>| as_token(reqwest::Method::PUT, &hub, &t, &format!("/v1/mail/{p}/up/{seq}")).body(body).send();
    assert_eq!(put(1, b"one".to_vec()).await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(put(3, b"three".to_vec()).await.unwrap().status(), StatusCode::NO_CONTENT);
    // a number already used, or below the highest, is refused: nothing is overwritten or slipped in before
    let r = put(3, b"again".to_vec()).await.unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    assert_eq!(r.json::<Value>().await.unwrap()["max"], 3);
    assert_eq!(put(2, b"two".to_vec()).await.unwrap().status(), StatusCode::CONFLICT);
    let list: Value = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list["max"], 3);
    assert_eq!(list["mail"].as_array().unwrap().iter().map(|m| (m["seq"].as_i64().unwrap(), m["bytes"].as_i64().unwrap())).collect::<Vec<_>>(), [(1, 3), (3, 5)]);
    let after: Value = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up?after=1")).send().await.unwrap().json().await.unwrap();
    assert_eq!(after["mail"].as_array().unwrap().len(), 1);
    // one message is at most 1 MiB
    let r = put(4, vec![0u8; super::MAIL_MAX + 1]).await;
    assert!(r.map(|r| r.status() == StatusCode::PAYLOAD_TOO_LARGE).unwrap_or(true));
    // one direction holds at most 16 MiB: the oldest go, the newest stays
    for seq in 10..28 { assert_eq!(put(seq, vec![1u8; super::MAIL_MAX]).await.unwrap().status(), StatusCode::NO_CONTENT); }
    let list: Value = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up")).send().await.unwrap().json().await.unwrap();
    let seqs: Vec<i64> = list["mail"].as_array().unwrap().iter().map(|m| m["seq"].as_i64().unwrap()).collect();
    assert_eq!((seqs.first().copied(), seqs.last().copied(), seqs.len()), (Some(12), Some(27), 16));
    // a message a week old is gone, read or not; its number stays used
    let db = rusqlite::Connection::open(hub._dir.path().join("hub.db")).unwrap();
    db.execute("update mail set at = at - ?1 where seq = 27", [super::MAIL_KEPT_MS + 1]).unwrap();
    assert_eq!(a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up/27")).send().await.unwrap().status(), StatusCode::NOT_FOUND);
    assert_eq!(put(27, b"x".to_vec()).await.unwrap().status(), StatusCode::CONFLICT);
    assert_eq!(put(28, b"x".to_vec()).await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(db.query_row("select count(*) from mail where seq = 27", [], |r| r.get::<_, i64>(0)).unwrap(), 0, "swept at the next put");
}

#[tokio::test]
async fn the_computer_is_told_of_its_phones_mail_and_a_phone_goes_with_its_revocation() {
    let hub = start(2048).await;
    let (a, b) = joined(&hub).await;
    let (p, t) = phone(&hub, &a).await;
    let mut req = format!("{}/v1/events", hub.url.replace("http://", "ws://")).into_client_request().unwrap();
    req.headers_mut().insert("Authorization", format!("Bearer {}", a.token).parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t, &format!("/v1/mail/{p}/up/1")).body("x").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(msg.to_text().unwrap()).unwrap(), json!({"kind": "mail", "id": p, "to": "aaaa"}));
    // revoked: the token and the mailbox go; the phone reads as unknown from then on
    assert_eq!(a.req(reqwest::Method::DELETE, &hub, &format!("/v1/phones/{p}")).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(as_token(reqwest::Method::GET, &hub, &t, &format!("/v1/mail/{p}/up/1")).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    assert_eq!(a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up/1")).send().await.unwrap().status(), StatusCode::NOT_FOUND);
    assert_eq!(a.req(reqwest::Method::DELETE, &hub, &format!("/v1/phones/{p}")).send().await.unwrap().status(), StatusCode::NOT_FOUND);
    // the admin can remove any phone; a computer leaving takes its phones with it
    let (p2, _) = phone(&hub, &b).await;
    assert_eq!(Client::new().delete(format!("{}/v1/phones/{p2}", hub.url)).bearer_auth(ADMIN).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let (p3, t3) = phone(&hub, &b).await;
    assert_eq!(b.req(reqwest::Method::DELETE, &hub, "/v1/devices/bbbb").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(as_token(reqwest::Method::GET, &hub, &t3, &format!("/v1/mail/{p3}/down")).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

// ---------- the review's findings on the hub ----------

/// #6: a message's limit applies while its body is read: a body over 1 MiB is refused before anything else looks at
/// the request (before, it was buffered up to the server-wide limit and the token was checked first).
#[tokio::test]
async fn a_mail_body_over_its_limit_is_refused_while_it_is_read() {
    let hub = start(2048).await;
    let r = Client::new().put(format!("{}/v1/mail/ph-x/up/1", hub.url)).bearer_auth("nobody").body(vec![0u8; 2 * super::MAIL_MAX]).send().await;
    assert!(r.map(|r| r.status() == StatusCode::PAYLOAD_TOO_LARGE).unwrap_or(true));
    let r = Client::new().put(format!("{}/v1/mail/ph-x/up/1", hub.url)).bearer_auth("nobody").body(vec![0u8; super::MAIL_MAX + 1]).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE, "refused at extraction, not 401 after buffering");
}

/// #6 and #13: an empty message, or a number outside 1..=2^53, is refused; a direction keeps at most 1000 messages
/// whatever their size, and a listing gives at most 100 at a time.
#[tokio::test]
async fn mail_rows_are_capped_numbers_bounded_and_listings_paged() {
    let hub = start(2048).await;
    let (a, _b) = joined(&hub).await;
    let (p, t) = phone(&hub, &a).await;
    let put = |seq: i64, body: &'static [u8]| as_token(reqwest::Method::PUT, &hub, &t, &format!("/v1/mail/{p}/up/{seq}")).body(body).send();
    assert_eq!(put(1, b"").await.unwrap().status(), StatusCode::BAD_REQUEST, "empty");
    assert_eq!(put(0, b"x").await.unwrap().status(), StatusCode::BAD_REQUEST);
    assert_eq!(put(i64::MAX, b"x").await.unwrap().status(), StatusCode::BAD_REQUEST, "no number left after it");
    assert_eq!(put(super::MAX_SEQ + 1, b"x").await.unwrap().status(), StatusCode::BAD_REQUEST);
    for seq in 1..=1005 { assert_eq!(put(seq, b"x").await.unwrap().status(), StatusCode::NO_CONTENT); }
    let db = rusqlite::Connection::open(hub._dir.path().join("hub.db")).unwrap();
    let (n, first): (i64, i64) = db.query_row("select count(*), min(seq) from mail where phone = ?", [&p], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((n, first), (super::MAILBOX_ROWS, 6));
    let page: Value = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up")).send().await.unwrap().json().await.unwrap();
    assert_eq!((page["mail"].as_array().unwrap().len() as i64, page["more"].as_bool(), page["max"].as_i64()), (super::LIST_PAGE, Some(true), Some(1005)));
    let last: Value = a.req(reqwest::Method::GET, &hub, &format!("/v1/mail/{p}/up?after=1000&limit=3")).send().await.unwrap().json().await.unwrap();
    assert_eq!(last["mail"].as_array().unwrap().iter().map(|m| m["seq"].as_i64().unwrap()).collect::<Vec<_>>(), [1001, 1002, 1003]);
    assert_eq!(last["more"], true);
}

/// #14: a phone whose computer is no longer on the hub is gone too, even if its own row was left behind.
#[tokio::test]
async fn a_phone_whose_computer_is_gone_is_refused() {
    let hub = start(2048).await;
    let (a, _b) = joined(&hub).await;
    let (p, t) = phone(&hub, &a).await;
    let db = rusqlite::Connection::open(hub._dir.path().join("hub.db")).unwrap();
    db.execute("delete from devices where id = 'aaaa'", []).unwrap();
    assert_eq!(as_token(reqwest::Method::GET, &hub, &t, &format!("/v1/mail/{p}/down")).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

/// #12: six digits a live computer invite holds are never a phone invite's, nor the other way round.
#[test]
fn an_invite_code_is_unique_across_computers_and_phones() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(super::SCHEMA).unwrap();
    let later = super::now_ms() + 60_000;
    db.execute("insert into invites(code, expires_at, used) values('123456', ?, 0)", [later]).unwrap();
    db.execute("insert into phones(id, device, code, expires_at) values('ph-1', 'aaaa', '222222', ?)", [later]).unwrap();
    let mut draws = vec![123456u32, 222222, 333333].into_iter();
    assert_eq!(super::fresh_code(&db, || Ok(draws.next().unwrap())).unwrap(), "333333");
}

/// #12: wrong joins at once, all released together by a barrier on several threads, are counted as they are let in:
/// no more than the minute's budget gets through.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_joins_at_once_do_not_pass_the_budget_together() {
    let hub = start(2048).await;
    let gate = std::sync::Arc::new(tokio::sync::Barrier::new(30));
    let tries: Vec<_> = (0..30).map(|_| {
        let (url, gate) = (format!("{}/v1/phones/join", hub.url), gate.clone());
        tokio::spawn(async move { gate.wait().await; Client::new().post(url).json(&json!({"code": "999999"})).send().await.unwrap().status() })
    }).collect();
    let mut wrong = 0;
    for t in tries { if t.await.unwrap() == StatusCode::UNAUTHORIZED { wrong += 1; } }
    assert!(wrong <= super::JOIN_FAILS_A_MINUTE, "{wrong} wrong attempts got through");
}

/// P2: a join that succeeds gives its place back to the minute it was let in, not to a later one.
#[test]
fn a_success_gives_back_only_its_own_minutes_place() {
    let hub = super::Hub { cfg: Config { data_dir: std::env::temp_dir(), admin_token: ADMIN.into(), bind: "127.0.0.1:0".parse().unwrap(), max_blob_mb: 4, retain_mb: 4 },
        admin_hash: String::new(), db: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()), events: tokio::sync::broadcast::channel(4).0, join_fails: std::sync::Mutex::new((0, 0, 0)) };
    let first = hub.join_admit().unwrap();
    // that minute ends; a wrong attempt is the first of the next one
    hub.join_fails.lock().unwrap().0 -= 61_000;
    let _wrong = hub.join_admit().unwrap();
    hub.join_ok(first);
    assert_eq!(hub.join_fails.lock().unwrap().1, 1, "the late success took a place of the new minute's");
}

/// P1: a hub whose database the first mailbox version made (its mail table has no `bytes`) still lists, takes and
/// trims mail, the messages it held included.
#[tokio::test]
async fn a_hub_upgraded_from_the_first_mailbox_version_keeps_its_mail() {
    let hub = start(2048).await;
    let (a, _b) = joined(&hub).await;
    let (p, t) = phone(&hub, &a).await;
    assert_eq!(as_token(reqwest::Method::PUT, &hub, &t, &format!("/v1/mail/{p}/up/1")).body("aaaa").send().await.unwrap().status(), StatusCode::NO_CONTENT);
    hub._handle.shutdown();
    let db = rusqlite::Connection::open(hub._dir.path().join("hub.db")).unwrap();
    // the mail table as b7f7c50 made it, holding what it held
    db.execute_batch("create table mail_old(phone text not null, dir text not null, seq integer not null, at integer not null, body blob not null, primary key(phone, dir, seq));
        insert into mail_old select phone, dir, seq, at, body from mail; drop table mail; alter table mail_old rename to mail; drop index if exists mail_at;").unwrap();
    db.execute("insert into mail(phone, dir, seq, at, body) values(?1, 'up', 2, ?2, x'0102'), (?1, 'up', 3, ?2, x'010203')", rusqlite::params![p, super::now_ms()]).unwrap();
    db.execute("update phones set up_max = 3 where id = ?", [&p]).unwrap();
    drop(db);
    let cfg = Config { data_dir: hub._dir.path().to_path_buf(), admin_token: ADMIN.into(), bind: "127.0.0.1:0".parse().unwrap(), max_blob_mb: 4, retain_mb: 2048 };
    let (addr, handle) = super::serve(cfg).await.unwrap();
    let again = Hub { url: format!("http://{addr}"), _dir: hub._dir, _handle: handle };
    let list: Value = a.req(reqwest::Method::GET, &again, &format!("/v1/mail/{p}/up")).send().await.unwrap().json().await.unwrap();
    assert_eq!(list["mail"].as_array().unwrap().iter().map(|m| (m["seq"].as_i64().unwrap(), m["bytes"].as_i64().unwrap())).collect::<Vec<_>>(), [(1, 4), (2, 2), (3, 3)]);
    assert_eq!(&as_token(reqwest::Method::GET, &again, &t, &format!("/v1/mail/{p}/up/1")).send().await.unwrap().bytes().await.unwrap()[..], b"aaaa");
    for seq in 4..20 { assert_eq!(as_token(reqwest::Method::PUT, &again, &t, &format!("/v1/mail/{p}/up/{seq}")).body(vec![1u8; super::MAIL_MAX]).send().await.unwrap().status(), StatusCode::NO_CONTENT); }
    let list: Value = a.req(reqwest::Method::GET, &again, &format!("/v1/mail/{p}/up")).send().await.unwrap().json().await.unwrap();
    let total: i64 = list["mail"].as_array().unwrap().iter().map(|m| m["bytes"].as_i64().unwrap()).sum();
    assert!(total <= super::MAILBOX_MAX, "trimmed to the cap, old messages counted: {total}");
}
