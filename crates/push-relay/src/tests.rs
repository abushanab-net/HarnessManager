use super::*;
use companion_proto::push::Kind;
use reqwest::{Client, Method};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeProvider(Arc<AtomicUsize>);
impl Provider for FakeProvider {
    async fn send(&self, _: Platform, token: &str, _: Kind, _: &str) -> Result<Delivery> {
        self.0.fetch_add(1, Ordering::SeqCst);
        match token {
            "gone" => Ok(Delivery::InvalidToken),
            "unavailable" => anyhow::bail!("unavailable"),
            _ => Ok(Delivery::Sent),
        }
    }
}

struct Server {
    http: Client,
    url: String,
    sends: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let sends = Arc::new(AtomicUsize::new(0));
        let relay = Relay::new(Connection::open_in_memory().unwrap(), [17; 32], FakeProvider(sends.clone())).unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, router(relay)).await.unwrap() });
        Self { url, http: Client::new(), sends, task }
    }

    async fn register(&self, token: &str) -> Registration {
        let response = self
            .http
            .post(format!("{}/v1/register", self.url))
            .json(&json!({"platform":"android","token":token}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        response.json().await.unwrap()
    }

    fn wake(&self, r: &Registration, collapse: &str, ts: u64) -> reqwest::RequestBuilder {
        let body = json!({"relay_handle":r.relay_handle,"kind":"needs","collapse_id":collapse}).to_string();
        self.signed(r, Method::POST, push::WAKE_PATH, ts, &body)
    }

    fn signed(&self, r: &Registration, method: Method, path: &str, ts: u64, body: &str) -> reqwest::RequestBuilder {
        let mut key = [0; 32];
        hex::decode_to_slice(&r.secret, &mut key).unwrap();
        self.http
            .request(method.clone(), format!("{}{path}", self.url))
            .header(push::TIMESTAMP, ts)
            .header(push::SIGNATURE, push::signature(&key, method.as_str(), path, ts, body.as_bytes()))
            .body(body.to_owned())
    }
}

async fn status(req: reqwest::RequestBuilder) -> StatusCode {
    req.send().await.unwrap().status()
}

#[tokio::test]
async fn authentication_binds_the_body_method_and_time_and_rejects_concurrent_replay() {
    let s = Server::start().await;
    let r = s.register("phone").await;
    let ts = now_ms();
    assert_eq!(
        status(
            s.wake(&r, "a", ts)
                .body(json!({"relay_handle":r.relay_handle,"kind":"live","collapse_id":"a"}).to_string())
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(status(s.wake(&r, "a", ts - WINDOW_MS)).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(s.wake(&r, "a", ts + WINDOW_MS)).await, StatusCode::UNAUTHORIZED);
    let (a, b) = tokio::join!(status(s.wake(&r, "a", ts)), status(s.wake(&r, "a", ts)));
    assert!([a, b].contains(&StatusCode::NO_CONTENT) && [a, b].contains(&StatusCode::CONFLICT));
    let path = format!("/v1/register/{}", r.relay_handle);
    assert_eq!(status(s.http.delete(format!("{}{path}", s.url))).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(s.signed(&r, Method::DELETE, &path, now_ms(), "")).await, StatusCode::NO_CONTENT);
    assert_eq!(status(s.wake(&r, "b", now_ms())).await, StatusCode::NOT_FOUND);
    assert_eq!(s.sends.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rate_limit_is_per_handle_and_leaves_unregistration_available() {
    let s = Server::start().await;
    let r = s.register("one").await;
    for n in 0..WAKES_PER_MINUTE {
        assert_eq!(status(s.wake(&r, &n.to_string(), now_ms())).await, StatusCode::NO_CONTENT);
    }
    assert_eq!(status(s.wake(&r, "over", now_ms())).await, StatusCode::TOO_MANY_REQUESTS);
    let other = s.register("two").await;
    assert_eq!(status(s.wake(&other, "ok", now_ms())).await, StatusCode::NO_CONTENT);
    let path = format!("/v1/register/{}", r.relay_handle);
    assert_eq!(status(s.signed(&r, Method::DELETE, &path, now_ms(), "")).await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn only_permanent_token_rejection_removes_the_registration() {
    let s = Server::start().await;
    let gone = s.register("gone").await;
    assert_eq!(status(s.wake(&gone, "first", now_ms())).await, StatusCode::GONE);
    assert_eq!(status(s.wake(&gone, "second", now_ms())).await, StatusCode::NOT_FOUND);
    let temporary = s.register("unavailable").await;
    for id in ["first", "second"] {
        assert_eq!(status(s.wake(&temporary, id, now_ms())).await, StatusCode::SERVICE_UNAVAILABLE);
    }
    assert_eq!(s.sends.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_fast_sender_cannot_replay_when_its_rate_limit_slot_expires() {
    let relay = Relay::new(Connection::open_in_memory().unwrap(), [0; 32], FakeProvider(Arc::new(AtomicUsize::new(0))))
        .unwrap();
    let (_, Json(r)) =
        register(State(relay.clone()), Json(Register { platform: Platform::Ios, token: "a1b2".into() })).await.unwrap();
    let mut key = [0; 32];
    hex::decode_to_slice(&r.secret, &mut key).unwrap();
    // It arrived 61 seconds ago from a sender three seconds ahead of the relay.
    let ts = now_ms() - WINDOW_MS + 2000;
    let body = Bytes::from(json!({"relay_handle":r.relay_handle,"kind":"live","collapse_id":"opaque"}).to_string());
    let mut h = HeaderMap::new();
    h.insert(push::TIMESTAMP, ts.to_string().parse().unwrap());
    h.insert(push::SIGNATURE, push::signature(&key, "POST", push::WAKE_PATH, ts, &body).parse().unwrap());
    assert_eq!(wake(State(relay.clone()), h.clone(), body.clone()).await, Ok(StatusCode::NO_CONTENT));
    relay.db.lock().unwrap().execute("update accepted set received = ?", [(now_ms() - WINDOW_MS - 1) as i64]).unwrap();
    assert_eq!(wake(State(relay), h, body).await, Err(StatusCode::CONFLICT));
}
