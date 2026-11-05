//! The hub: a small plain-HTTP service every computer pushes its journal to and pulls the others' from (phase 5
//! of `docs/plans/across-computers/PLAN.md`). Caddy terminates TLS in front of it. SQLite at `<data>/hub.db`
//! holds the signed device records, the chunk index, presence, transcripts and rendezvous addresses; the bytes
//! live under `<data>/blobs/<origin>/`. Every request carries `Authorization: Bearer <token>`: the admin token
//! from the environment, or a per-computer token minted at join and stored hashed. Chunks, presence and
//! transcripts arrive encrypted by the ring key; the hub never sees a key.

// a handler's Err is the HTTP answer itself, the same size as its Ok
#![allow(clippy::result_large_err)]

use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

const MB: u64 = 1024 * 1024;
const INVITE_MS: i64 = 10 * 60 * 1000;

pub struct Config {
    pub data_dir: PathBuf,
    pub admin_token: String,
    pub bind: SocketAddr,
    pub max_blob_mb: u64,
    pub retain_mb: u64,
}

/// What the hub keeps. One lock around the database and the blob writes: a few computers, never a crowd.
struct Hub {
    cfg: Config,
    admin_hash: String,
    db: Mutex<Connection>,
    events: broadcast::Sender<String>,
}

type St = State<Arc<Hub>>;
type Reply = Result<Response, Response>;

const SCHEMA: &str = "
create table if not exists devices(id text primary key, record text not null, token_hash text not null, joined_at integer not null, last_seen integer not null default 0);
create table if not exists invites(code text primary key, expires_at integer not null, used integer not null default 0);
create table if not exists chunks(origin text not null, file text not null, incarnation text not null, from_seq integer not null, to_seq integer not null,
  sha256 text not null, sig text not null, bytes integer not null, at integer not null, primary key(origin, file));
create table if not exists presence(id text primary key, at integer not null, blob blob not null);
create table if not exists transcripts(origin text not null, key text not null, sha256 text not null, bytes integer not null, at integer not null, primary key(origin, key));
create table if not exists rendezvous(id text primary key, addresses text not null, at integer not null);
";

/// Bind `cfg.bind` (port 0: any free one; the one taken is in the result) and serve from the current tokio
/// runtime. `handle.shutdown()` stops it.
pub async fn serve(cfg: Config) -> Result<(SocketAddr, axum_server::Handle<SocketAddr>)> {
    if cfg.admin_token.is_empty() { return Err(anyhow!("HUB_ADMIN_TOKEN is empty")); }
    std::fs::create_dir_all(cfg.data_dir.join("blobs"))?;
    let db = Connection::open(cfg.data_dir.join("hub.db"))?;
    db.execute_batch(SCHEMA)?;
    let l = std::net::TcpListener::bind(cfg.bind)?;
    l.set_nonblocking(true)?;
    let addr = l.local_addr()?;
    let handle = axum_server::Handle::new();
    let server = axum_server::from_tcp(l)?.handle(handle.clone());
    let limit = (cfg.max_blob_mb * MB) as usize;
    let hub = Arc::new(Hub { admin_hash: sha_hex(cfg.admin_token.as_bytes()), cfg, db: Mutex::new(db), events: broadcast::channel(64).0 });
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/join", post(join))
        .route("/v1/invite", post(invite))
        .route("/v1/devices", get(devices))
        .route("/v1/devices/{id}", put(put_device).delete(delete_device))
        .route("/v1/chunks/{origin}", get(chunks))
        .route("/v1/chunks/{origin}/{file}", put(put_chunk))
        .route("/v1/chunk/{origin}/{file}", get(chunk))
        .route("/v1/presence", get(presence).put(put_presence))
        .route("/v1/transcripts/{origin}/{*key}", put(put_transcript).get(transcript).delete(delete_transcript))
        .route("/v1/rendezvous", get(rendezvous).put(put_rendezvous))
        .route("/v1/events", get(events))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(hub);
    tokio::spawn(async move {
        if let Err(e) = server.serve(app.into_make_service()).await { eprintln!("hrm-hub: the server stopped: {e}"); }
    });
    Ok((addr, handle))
}

fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }
fn sha_hex(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }
fn b64(b: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(b) }
fn unb64(s: &str) -> Option<Vec<u8>> { base64::engine::general_purpose::STANDARD.decode(s.trim()).ok() }
fn fail(status: StatusCode, msg: impl Into<String>) -> Response { (status, Json(json!({"error": msg.into()}))).into_response() }
fn internal(e: impl std::fmt::Display) -> Response { fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }

/// A device record that carries its own `pubkey` and a `sig` by it over its canonical JSON without `sig`.
fn record_ok(v: &Value) -> bool {
    let (Some(pk), Some(sig)) = (v["pubkey"].as_str(), v["sig"].as_str()) else { return false };
    let Some(pk) = unb64(pk).and_then(|b| <[u8; 32]>::try_from(b).ok()).and_then(|b| VerifyingKey::from_bytes(&b).ok()) else { return false };
    let Some(sig) = unb64(sig).and_then(|b| Signature::from_slice(&b).ok()) else { return false };
    let mut c = v.clone();
    c.as_object_mut().map(|o| o.remove("sig"));
    pk.verify(c.to_string().as_bytes(), &sig).is_ok()
}

/// Ids and chunk file names are path segments on disk: letters, digits, `.`, `-`, `_`, not starting with a dot.
fn safe_name(s: &str) -> bool { !s.is_empty() && !s.starts_with('.') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_') }

/// A valid signed record whose id is a safe name: `(id, record)`.
fn checked_record(v: &Value) -> Result<(String, Value), Response> {
    let id = v["id"].as_str().unwrap_or("").to_string();
    if !safe_name(&id) { return Err(fail(StatusCode::BAD_REQUEST, "the device record needs an id")); }
    if !record_ok(v) { return Err(fail(StatusCode::BAD_REQUEST, "the device record is not signed by its pubkey")); }
    Ok((id, v.clone()))
}

enum Who { Admin, Device(String) }

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> { self.db.lock().unwrap_or_else(|e| e.into_inner()) }

    fn who(&self, headers: &HeaderMap) -> Result<Who, Response> {
        let token = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim).unwrap_or("");
        if token.is_empty() { return Err(fail(StatusCode::UNAUTHORIZED, "a bearer token is needed")); }
        let h = sha_hex(token.as_bytes());
        if h == self.admin_hash { return Ok(Who::Admin); }
        let db = self.lock();
        let id: Option<String> = db.query_row("select id from devices where token_hash = ?", [&h], |r| r.get(0)).optional().map_err(internal)?;
        let Some(id) = id else { return Err(fail(StatusCode::UNAUTHORIZED, "unknown token")) };
        db.execute("update devices set last_seen = ? where id = ?", params![now_ms(), id]).map_err(internal)?;
        Ok(Who::Device(id))
    }

    /// The caller's own id, or 403: the admin owns nothing.
    fn own(&self, headers: &HeaderMap, id: &str) -> Result<String, Response> {
        match self.who(headers)? {
            Who::Device(me) if me == id => Ok(me),
            _ => Err(fail(StatusCode::FORBIDDEN, "own only")),
        }
    }

    fn emit(&self, kind: &str, id: &str) { let _ = self.events.send(json!({"kind": kind, "id": id}).to_string()); }

    fn device_list(&self, db: &Connection) -> rusqlite::Result<Vec<Value>> {
        let mut st = db.prepare("select d.record, d.last_seen, coalesce((select max(to_seq) from chunks c where c.origin = d.id), 0) from devices d order by d.joined_at")?;
        let rows = st.query_map([], |r| {
            let (rec, seen, seq): (String, i64, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
            let mut v: Value = serde_json::from_str(&rec).unwrap_or(Value::Null);
            v["last_seen"] = json!(seen);
            v["last_seq"] = json!(seq);
            Ok(v)
        })?;
        rows.collect()
    }

    fn blobs(&self, origin: &str) -> PathBuf { self.cfg.data_dir.join("blobs").join(origin) }

    fn write_blob(&self, path: &FsPath, bytes: &[u8]) -> Result<(), Response> {
        (|| -> std::io::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let tmp = path.with_extension("hub-tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, path)
        })().map_err(internal)
    }

    /// Drop an origin's oldest chunks until it holds at most `retain_mb`; the newest one always stays.
    fn retain(&self, db: &Connection, origin: &str) -> rusqlite::Result<()> {
        let cap = (self.cfg.retain_mb * MB) as i64;
        loop {
            let (n, total): (i64, i64) = db.query_row("select count(*), coalesce(sum(bytes), 0) from chunks where origin = ?", [origin], |r| Ok((r.get(0)?, r.get(1)?)))?;
            if n <= 1 || total <= cap { return Ok(()); }
            let file: String = db.query_row("select file from chunks where origin = ? order by at, to_seq limit 1", [origin], |r| r.get(0))?;
            db.execute("delete from chunks where origin = ? and file = ?", params![origin, file])?;
            let _ = std::fs::remove_file(self.blobs(origin).join(&file));
        }
    }
}

#[derive(Deserialize)]
struct JoinBody {
    code: Option<String>,
    admin_token: Option<String>,
    device: Value,
}

/// With the admin token or a live invite code: mint a token for the record's id (a second join by the same
/// computer replaces its token) and answer with every device known.
async fn join(State(hub): St, Json(body): Json<JoinBody>) -> Reply {
    let (id, record) = checked_record(&body.device)?;
    let db = hub.lock();
    match (body.admin_token.as_deref(), body.code.as_deref()) {
        (Some(t), _) if !t.is_empty() => {
            if sha_hex(t.as_bytes()) != hub.admin_hash { return Err(fail(StatusCode::UNAUTHORIZED, "wrong admin token")); }
        }
        (_, Some(code)) if !code.is_empty() => {
            let n = db.execute("update invites set used = 1 where code = ? and used = 0 and expires_at > ?", params![code, now_ms()]).map_err(internal)?;
            if n == 0 { return Err(fail(StatusCode::UNAUTHORIZED, "the code is wrong, used or expired")); }
        }
        _ => return Err(fail(StatusCode::BAD_REQUEST, "send a code or the admin token")),
    }
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).map_err(internal)?;
    let token = hex::encode(raw);
    db.execute("insert into devices(id, record, token_hash, joined_at, last_seen) values(?, ?, ?, ?, ?) on conflict(id) do update set record = excluded.record, token_hash = excluded.token_hash, last_seen = excluded.last_seen",
        params![id, record.to_string(), sha_hex(token.as_bytes()), now_ms(), now_ms()]).map_err(internal)?;
    let devices = hub.device_list(&db).map_err(internal)?;
    drop(db);
    hub.emit("device", &id);
    Ok(Json(json!({"token": token, "devices": devices})).into_response())
}

async fn invite(State(hub): St, headers: HeaderMap) -> Reply {
    hub.who(&headers)?;
    let code = format!("{:06}", getrandom::u32().map_err(internal)? % 1_000_000);
    let expires_at = now_ms() + INVITE_MS;
    let db = hub.lock();
    db.execute("delete from invites where expires_at < ?", [now_ms()]).map_err(internal)?;
    db.execute("insert or replace into invites(code, expires_at, used) values(?, ?, 0)", params![code, expires_at]).map_err(internal)?;
    Ok(Json(json!({"code": code, "expires_at": expires_at})).into_response())
}

async fn devices(State(hub): St, headers: HeaderMap) -> Reply {
    hub.who(&headers)?;
    let list = hub.device_list(&hub.lock()).map_err(internal)?;
    Ok(Json(Value::Array(list)).into_response())
}

async fn put_device(State(hub): St, headers: HeaderMap, Path(id): Path<String>, Json(v): Json<Value>) -> Reply {
    hub.own(&headers, &id)?;
    let (rid, record) = checked_record(&v)?;
    if rid != id { return Err(fail(StatusCode::BAD_REQUEST, "the record's id is not the path's")); }
    hub.lock().execute("update devices set record = ? where id = ?", params![record.to_string(), id]).map_err(internal)?;
    hub.emit("device", &id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Forget a computer: its token, record, chunks, presence, transcripts and addresses. Own, or the admin.
async fn delete_device(State(hub): St, headers: HeaderMap, Path(id): Path<String>) -> Reply {
    match hub.who(&headers)? {
        Who::Admin => {}
        Who::Device(me) if me == id => {}
        _ => return Err(fail(StatusCode::FORBIDDEN, "own only, or the admin")),
    }
    let db = hub.lock();
    for t in ["devices", "presence", "rendezvous"] { db.execute(&format!("delete from {t} where id = ?"), [&id]).map_err(internal)?; }
    for t in ["chunks", "transcripts"] { db.execute(&format!("delete from {t} where origin = ?"), [&id]).map_err(internal)?; }
    let _ = std::fs::remove_dir_all(hub.blobs(&id));
    drop(db);
    hub.emit("device", &id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
struct Since { since: Option<i64> }

/// An origin's chunks after `since`, of its record's incarnation (an older incarnation's chunks stay until
/// retention drops them, but are not listed).
async fn chunks(State(hub): St, headers: HeaderMap, Path(origin): Path<String>, Query(q): Query<Since>) -> Reply {
    hub.who(&headers)?;
    let db = hub.lock();
    let rec: Option<String> = db.query_row("select record from devices where id = ?", [&origin], |r| r.get(0)).optional().map_err(internal)?;
    let Some(rec) = rec else { return Err(fail(StatusCode::NOT_FOUND, "unknown origin")) };
    let inc = serde_json::from_str::<Value>(&rec).ok().and_then(|v| v["incarnation"].as_str().map(String::from)).unwrap_or_default();
    let mut st = db.prepare("select file, from_seq, to_seq, sha256, sig, bytes from chunks where origin = ? and incarnation = ? and to_seq > ? order by from_seq").map_err(internal)?;
    let list: Vec<Value> = st.query_map(params![origin, inc, q.since.unwrap_or(0)], |r| {
        Ok(json!({"file": r.get::<_, String>(0)?, "from": r.get::<_, i64>(1)?, "to": r.get::<_, i64>(2)?, "sha256": r.get::<_, String>(3)?, "sig": r.get::<_, String>(4)?, "bytes": r.get::<_, i64>(5)?, "enc": true}))
    }).map_err(internal)?.collect::<rusqlite::Result<_>>().map_err(internal)?;
    Ok(Json(json!({"origin": origin, "incarnation": inc, "chunks": list})).into_response())
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, Response> {
    headers.get(name).and_then(|v| v.to_str().ok()).filter(|s| !s.is_empty()).ok_or_else(|| fail(StatusCode::BAD_REQUEST, format!("header {name} is needed")))
}

/// `<incarnation>-<from>-<to>.<ext>`: the incarnation in the name, else the origin's current one.
fn incarnation_of(file: &str, db: &Connection, origin: &str) -> String {
    let stem = file.split('.').next().unwrap_or("");
    let mut parts = stem.rsplitn(3, '-');
    if let (Some(to), Some(from), Some(inc)) = (parts.next(), parts.next(), parts.next()) {
        if to.parse::<i64>().is_ok() && from.parse::<i64>().is_ok() && !inc.is_empty() { return inc.to_string(); }
    }
    db.query_row("select record from devices where id = ?", [origin], |r| r.get::<_, String>(0)).ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v["incarnation"].as_str().map(String::from)).unwrap_or_default()
}

async fn put_chunk(State(hub): St, headers: HeaderMap, Path((origin, file)): Path<(String, String)>, body: Bytes) -> Reply {
    hub.own(&headers, &origin)?;
    if !safe_name(&file) { return Err(fail(StatusCode::BAD_REQUEST, "bad chunk name")); }
    let from: i64 = header(&headers, "x-from")?.parse().map_err(|_| fail(StatusCode::BAD_REQUEST, "X-From is not a number"))?;
    let to: i64 = header(&headers, "x-to")?.parse().map_err(|_| fail(StatusCode::BAD_REQUEST, "X-To is not a number"))?;
    let sha = header(&headers, "x-sha256")?.to_string();
    let sig = header(&headers, "x-sig")?.to_string();
    let db = hub.lock();
    let inc = incarnation_of(&file, &db, &origin);
    hub.write_blob(&hub.blobs(&origin).join(&file), &body)?;
    db.execute("insert or replace into chunks(origin, file, incarnation, from_seq, to_seq, sha256, sig, bytes, at) values(?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![origin, file, inc, from, to, sha, sig, body.len() as i64, now_ms()]).map_err(internal)?;
    hub.retain(&db, &origin).map_err(internal)?;
    drop(db);
    hub.emit("chunks", &origin);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn chunk(State(hub): St, headers: HeaderMap, Path((origin, file)): Path<(String, String)>) -> Reply {
    hub.who(&headers)?;
    if !safe_name(&origin) || !safe_name(&file) { return Err(fail(StatusCode::BAD_REQUEST, "bad name")); }
    match std::fs::read(hub.blobs(&origin).join(&file)) {
        Ok(b) => Ok(b.into_response()),
        Err(_) => Err(fail(StatusCode::NOT_FOUND, "no such chunk")),
    }
}

async fn put_presence(State(hub): St, headers: HeaderMap, body: Bytes) -> Reply {
    let Who::Device(id) = hub.who(&headers)? else { return Err(fail(StatusCode::FORBIDDEN, "a computer's token is needed")) };
    hub.lock().execute("insert or replace into presence(id, at, blob) values(?, ?, ?)", params![id, now_ms(), body.as_ref()]).map_err(internal)?;
    hub.emit("presence", &id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `{<id>: {at, blob}}`, the blob as base64 of the encrypted bytes put.
async fn presence(State(hub): St, headers: HeaderMap) -> Reply {
    hub.who(&headers)?;
    let db = hub.lock();
    let mut st = db.prepare("select id, at, blob from presence").map_err(internal)?;
    let mut out = serde_json::Map::new();
    for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Vec<u8>>(2)?))).map_err(internal)? {
        let (id, at, blob) = r.map_err(internal)?;
        out.insert(id, json!({"at": at, "blob": b64(&blob)}));
    }
    Ok(Json(Value::Object(out)).into_response())
}

fn transcript_path(hub: &Hub, origin: &str, key: &str) -> PathBuf { hub.blobs(origin).join("transcripts").join(sha_hex(key.as_bytes())) }

async fn put_transcript(State(hub): St, headers: HeaderMap, Path((origin, key)): Path<(String, String)>, body: Bytes) -> Reply {
    hub.own(&headers, &origin)?;
    let sha = header(&headers, "x-sha256")?.to_string();
    let bytes: i64 = header(&headers, "x-bytes")?.parse().map_err(|_| fail(StatusCode::BAD_REQUEST, "X-Bytes is not a number"))?;
    let db = hub.lock();
    hub.write_blob(&transcript_path(&hub, &origin, &key), &body)?;
    db.execute("insert or replace into transcripts(origin, key, sha256, bytes, at) values(?, ?, ?, ?, ?)", params![origin, key, sha, bytes, now_ms()]).map_err(internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn transcript(State(hub): St, headers: HeaderMap, Path((origin, key)): Path<(String, String)>) -> Reply {
    hub.who(&headers)?;
    let db = hub.lock();
    let meta: Option<(String, i64)> = db.query_row("select sha256, bytes from transcripts where origin = ? and key = ?", params![origin, key], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(internal)?;
    let Some((sha, bytes)) = meta else { return Err(fail(StatusCode::NOT_FOUND, "no such transcript")) };
    let body = std::fs::read(transcript_path(&hub, &origin, &key)).map_err(|_| fail(StatusCode::NOT_FOUND, "no such transcript"))?;
    Ok(([("x-sha256", sha), ("x-bytes", bytes.to_string())], body).into_response())
}

async fn delete_transcript(State(hub): St, headers: HeaderMap, Path((origin, key)): Path<(String, String)>) -> Reply {
    hub.own(&headers, &origin)?;
    hub.lock().execute("delete from transcripts where origin = ? and key = ?", params![origin, key]).map_err(internal)?;
    let _ = std::fs::remove_file(transcript_path(&hub, &origin, &key));
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
struct Addresses { addresses: Vec<String> }

async fn put_rendezvous(State(hub): St, headers: HeaderMap, Json(a): Json<Addresses>) -> Reply {
    let Who::Device(id) = hub.who(&headers)? else { return Err(fail(StatusCode::FORBIDDEN, "a computer's token is needed")) };
    hub.lock().execute("insert or replace into rendezvous(id, addresses, at) values(?, ?, ?)", params![id, json!(a.addresses).to_string(), now_ms()]).map_err(internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn rendezvous(State(hub): St, headers: HeaderMap) -> Reply {
    hub.who(&headers)?;
    let db = hub.lock();
    let mut st = db.prepare("select id, addresses, at from rendezvous").map_err(internal)?;
    let mut out = serde_json::Map::new();
    for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))).map_err(internal)? {
        let (id, addrs, at) = r.map_err(internal)?;
        out.insert(id, json!({"addresses": serde_json::from_str::<Value>(&addrs).unwrap_or(json!([])), "at": at}));
    }
    Ok(Json(Value::Object(out)).into_response())
}

/// One text message per change: `{kind: "presence" | "chunks" | "device", id}`.
async fn events(State(hub): St, headers: HeaderMap, ws: WebSocketUpgrade) -> Reply {
    hub.who(&headers)?;
    let rx = hub.events.subscribe();
    Ok(ws.on_upgrade(move |socket| pump(socket, rx)))
}

async fn pump(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(msg) => { if socket.send(Message::Text(msg.into())).await.is_err() { return; } }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return,
            },
            in_ = socket.recv() => match in_ {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            },
        }
    }
}

#[cfg(test)]
mod tests;
