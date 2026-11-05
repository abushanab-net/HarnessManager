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

    fn record(&self) -> Value {
        let mut v = json!({"id": self.id, "name": format!("pc-{}", self.id), "os": "windows", "app_version": "1.5.0", "incarnation": "inc1", "roots": {}, "updated_at": 1});
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
