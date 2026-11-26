mod providers;
#[cfg(test)]
mod tests;

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use companion_proto::push::{self, Registration, Wake};
use hmac::{Hmac, Mac};
use providers::{ApnsConfig, Delivery, Provider, Providers};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const WINDOW_MS: u64 = 60_000;
const CLOCK_SKEW_MS: u64 = 5_000;
const WAKES_PER_MINUTE: i64 = 10;
const MAX_REGISTRATIONS: i64 = 10_000;
const AUTH_FAILURES_PER_MINUTE: u32 = 30;
type HttpResult<T> = Result<T, StatusCode>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bind: SocketAddr,
    database: PathBuf,
    signing_key_file: PathBuf,
    fcm_service_account_file: Option<PathBuf>,
    apns: Option<ApnsConfig>,
}

#[derive(Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Platform {
    Android,
    Ios,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Register {
    platform: Platform,
    token: String,
}

struct Relay<P> {
    db: Mutex<Connection>,
    key: [u8; 32],
    replay_floor: u64,
    // Fixed buckets prevent guessed handles from allocating unbounded failure counters.
    failures: Mutex<[(u64, u32); 256]>,
    provider: P,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

impl<P: Provider> Relay<P> {
    fn new(mut db: Connection, key: [u8; 32], provider: P) -> Result<Arc<Self>> {
        db.execute_batch("create table if not exists registrations(handle text primary key, platform text not null, token text not null);
            pragma temp_store=memory;
            create temp table accepted(device_hash blob not null, signature blob not null, received integer not null, primary key(device_hash,signature));
            create index accepted_time on accepted(received);")?;
        let tx = db.transaction()?;
        if !tx.prepare("select 1 from pragma_index_list('registrations') where name='registration_token'")?.exists([])?
        {
            tx.execute_batch(
                "update registrations set token=lower(token) where platform='ios';
                delete from registrations where rowid not in (select max(rowid) from registrations group by token);
                create unique index registration_token on registrations(token);",
            )?;
        }
        tx.commit()?;
        let has_handles: bool = db.query_row("select exists(select 1 from registrations)", [], |r| r.get(0))?;
        // Cover even future-dated signatures the previous instance could have accepted.
        let replay_floor = if has_handles { now_ms() + CLOCK_SKEW_MS } else { 0 };
        Ok(Arc::new(Self { db: Mutex::new(db), key, provider, replay_floor, failures: Mutex::new([(0, 0); 256]) }))
    }

    fn secret(&self, handle: &str) -> [u8; 32] {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("32-byte key");
        mac.update(handle.as_bytes());
        mac.finalize().into_bytes().into()
    }

    async fn access<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&Connection) -> HttpResult<T> + Send + 'static,
    ) -> HttpResult<T> {
        let relay = self.clone();
        tokio::task::spawn_blocking(move || {
            let db = relay.db.lock().map_err(internal)?;
            f(&db)
        })
        .await
        .map_err(internal)?
    }

    fn authenticate(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
        handle: &str,
        body: &[u8],
    ) -> HttpResult<(u64, [u8; 32])> {
        let key = self.secret(handle);
        let now = now_ms();
        let mut failures = self.failures.lock().map_err(internal)?;
        let bucket = &mut failures[key[0] as usize];
        if now.saturating_sub(bucket.0) >= WINDOW_MS {
            *bucket = (now, 0);
        }
        if bucket.1 >= AUTH_FAILURES_PER_MINUTE {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        let result = self.check_signature(headers, method, path, &key, body, now);
        if result.is_err() {
            bucket.1 += 1;
        }
        result
    }

    fn check_signature(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
        key: &[u8; 32],
        body: &[u8],
        now: u64,
    ) -> HttpResult<(u64, [u8; 32])> {
        let refused = StatusCode::UNAUTHORIZED;
        let header = |name| headers.get(name).and_then(|v| v.to_str().ok()).ok_or(refused);
        let timestamp: u64 = header(push::TIMESTAMP)?.parse().map_err(|_| refused)?;
        let mut signature = [0; 32];
        hex::decode_to_slice(header(push::SIGNATURE)?, &mut signature).map_err(|_| refused)?;
        if timestamp <= self.replay_floor || !fresh(timestamp, now) {
            return Err(refused);
        }
        push::request_mac(key, method, path, timestamp, body).verify_slice(&signature).map_err(|_| refused)?;
        Ok((timestamp, signature))
    }
}

fn internal<E>(_: E) -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}

fn fresh(timestamp: u64, now: u64) -> bool {
    timestamp <= now + CLOCK_SKEW_MS && now.saturating_sub(timestamp) < WINDOW_MS
}

async fn register<P: Provider>(
    State(relay): State<Arc<Relay<P>>>,
    Json(mut r): Json<Register>,
) -> HttpResult<(StatusCode, Json<Registration>)> {
    if r.token.is_empty()
        || r.token.len() > 4096
        || !r.token.bytes().all(|b| b.is_ascii_graphic())
        || matches!(r.platform, Platform::Ios)
            && (r.token.len() % 2 != 0 || !r.token.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut bytes = [0; 24];
    getrandom::fill(&mut bytes).map_err(internal)?;
    let handle = hex::encode(bytes);
    let result = Registration { secret: hex::encode(relay.secret(&handle)), relay_handle: handle.clone() };
    let platform = match r.platform {
        Platform::Android => "android",
        Platform::Ios => {
            r.token.make_ascii_lowercase();
            "ios"
        }
    };
    relay
        .access(move |db| {
            let n = db
                .execute(
                    "insert into registrations(handle,platform,token) select ?1,?2,?3
                where exists(select 1 from registrations where token=?3) or (select count(*) from registrations) < ?4
                on conflict(token) do update set handle=excluded.handle, platform=excluded.platform",
                    params![handle, platform, r.token, MAX_REGISTRATIONS],
                )
                .map_err(internal)?;
            if n == 0 {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Ok(())
        })
        .await?;
    Ok((StatusCode::CREATED, Json(result)))
}

async fn wake<P: Provider>(
    State(relay): State<Arc<Relay<P>>>,
    headers: HeaderMap,
    body: Bytes,
) -> HttpResult<StatusCode> {
    let w: Wake = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    if w.relay_handle.len() != 48
        || w.collapse_id.is_empty()
        || w.collapse_id.len() > 64
        || !w.collapse_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (ts, sig) = relay.authenticate(&headers, "POST", push::WAKE_PATH, &w.relay_handle, &body)?;
    let handle = w.relay_handle.clone();
    let (platform, token) = relay
        .access(move |db| {
            let registration = db
                .query_row("select platform,token from registrations where handle=?", [&handle], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .optional()
                .map_err(internal)?
                .ok_or(StatusCode::NOT_FOUND)?;
            let device_hash: [u8; 32] = Sha256::digest(registration.1.as_bytes()).into();
            let now = now_ms();
            if !fresh(ts, now) {
                return Err(StatusCode::UNAUTHORIZED);
            }
            db.execute(
                "delete from accepted where received <= ?",
                [now.saturating_sub(WINDOW_MS + CLOCK_SKEW_MS) as i64],
            )
            .map_err(internal)?;
            if db
                .query_row(
                    "select exists(select 1 from accepted where device_hash=? and signature=?)",
                    params![device_hash, sig],
                    |r| r.get(0),
                )
                .map_err(internal)?
            {
                return Err(StatusCode::CONFLICT);
            }
            let count: i64 = db
                .query_row(
                    "select count(*) from accepted where device_hash=? and received>?",
                    params![device_hash, now.saturating_sub(WINDOW_MS) as i64],
                    |r| r.get(0),
                )
                .map_err(internal)?;
            if count >= WAKES_PER_MINUTE {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            db.execute("insert into accepted values (?,?,?)", params![device_hash, sig, now as i64])
                .map_err(internal)?;
            Ok(registration)
        })
        .await?;
    let platform = match platform.as_str() {
        "android" => Platform::Android,
        "ios" => Platform::Ios,
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    match relay.provider.send(platform, &token, w.kind, &w.collapse_id).await {
        Ok(Delivery::Sent) => Ok(StatusCode::NO_CONTENT),
        Ok(Delivery::InvalidToken) => {
            remove(&relay, w.relay_handle).await?;
            Err(StatusCode::GONE)
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn remove<P: Provider>(relay: &Arc<Relay<P>>, handle: String) -> HttpResult<usize> {
    relay.access(move |db| db.execute("delete from registrations where handle=?", [handle]).map_err(internal)).await
}

async fn unregister<P: Provider>(
    State(relay): State<Arc<Relay<P>>>,
    Path(handle): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> HttpResult<StatusCode> {
    if !body.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    relay.authenticate(&headers, "DELETE", &format!("/v1/register/{handle}"), &handle, &[])?;
    if remove(&relay, handle).await? == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

fn router<P: Provider>(relay: Arc<Relay<P>>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/register", post(register::<P>))
        .route("/v1/register/{handle}", delete(unregister::<P>))
        .route(push::WAKE_PATH, post(wake::<P>))
        .layer(DefaultBodyLimit::max(8192))
        .with_state(relay)
}

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::env::var_os("PUSH_CONFIG").unwrap_or_else(|| "/etc/hrm-push/config.json".into());
    let config: Config = serde_json::from_slice(&std::fs::read(path).context("read push relay config")?)?;
    let mut key = [0; 32];
    hex::decode_to_slice(std::fs::read_to_string(&config.signing_key_file)?.trim(), &mut key)
        .context("relay signing key must be 64 hex digits")?;
    let provider = Providers::new(config.fcm_service_account_file, config.apns)?;
    let relay = Relay::new(Connection::open(config.database)?, key, provider)?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    eprintln!("Push relay listening on {}", listener.local_addr()?);
    axum::serve(listener, router(relay)).await?;
    Ok(())
}
