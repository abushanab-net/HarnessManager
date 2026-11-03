//! The router end to end: real HTTP into the router, mock upstream providers behind it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use hrm_core::registry::{Provider, Route};
use hrm_core::Ctx;
use serde_json::{json, Value};

#[derive(Default)]
struct Hits {
    fail: AtomicUsize,
    anth: AtomicUsize,
    oai: AtomicUsize,
}

async fn fail(State(h): State<Arc<Hits>>) -> Response {
    h.fail.fetch_add(1, Ordering::SeqCst);
    (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response()
}

async fn bad() -> Response { (StatusCode::BAD_REQUEST, "prompt is too long").into_response() }

async fn anth(State(h): State<Arc<Hits>>, headers: HeaderMap, Json(req): Json<Value>) -> Response {
    h.anth.fetch_add(1, Ordering::SeqCst);
    assert_eq!(headers.get("x-api-key").unwrap(), "key-anth", "the provider key is injected");
    Json(json!({"id": "msg_a", "type": "message", "role": "assistant", "model": req["model"], "content": [{"type": "text", "text": "from anthropic"}],
        "stop_reason": "end_turn", "usage": {"input_tokens": 11, "output_tokens": 3}})).into_response()
}

async fn oai(State(h): State<Arc<Hits>>, headers: HeaderMap, Json(req): Json<Value>) -> Response {
    h.oai.fetch_add(1, Ordering::SeqCst);
    assert_eq!(headers.get("authorization").unwrap(), "Bearer key-oai");
    assert_eq!(req["messages"][0]["role"], "system", "converted to Chat Completions");
    if req["stream"] == true {
        let body = [
            json!({"id": "c", "choices": [{"delta": {"content": "Hel"}}]}),
            json!({"id": "c", "choices": [{"delta": {"content": "lo"}, "finish_reason": "stop"}]}),
            json!({"id": "c", "choices": [], "usage": {"prompt_tokens": 9, "completion_tokens": 2}}),
        ].iter().map(|c| format!("data: {c}\n\n")).collect::<String>() + "data: [DONE]\n\n";
        return ([("content-type", "text/event-stream")], body).into_response();
    }
    Json(json!({"id": "c1", "model": req["model"], "choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": format!("from openai {}", req["model"].as_str().unwrap())}}],
        "usage": {"prompt_tokens": 5, "completion_tokens": 4}})).into_response()
}

/// The tests share one credential namespace (an environment variable) and clear it when done: one at a time.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Setup {
    _serial: tokio::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
    base: String,
    token: String,
    hits: Arc<Hits>,
    ctx: Arc<Ctx>,
}

/// Test keys go even when a test fails, while this test still holds the turn.
impl Drop for Setup {
    fn drop(&mut self) { let _ = hrm_core::secrets::delete_prefix(&hrm_core::secrets::prefix()); }
}

async fn setup() -> Setup {
    let serial = SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    // one credential namespace for the whole test binary; cleaned at the end of each test
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-router-{}", std::process::id()));
    let ctx = Arc::new(Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap());

    let hits = Arc::new(Hits::default());
    let mock = Router::new()
        .route("/fail/v1/messages", post(fail))
        .route("/bad/v1/messages", post(bad))
        .route("/anth/v1/messages", post(anth))
        .route("/oai/chat/completions", post(oai))
        .with_state(hits.clone());
    let ml = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up = format!("http://{}", ml.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(ml, mock).await.unwrap() });

    let mut reg = ctx.registry().unwrap();
    let prov = |id: &str, path: &str, format: &str, models: &[&str]| Provider { id: id.into(), name: id.into(), base_url: format!("{up}/{path}"), format: format.into(), models: models.iter().map(|s| s.to_string()).collect() };
    reg.providers = vec![
        prov("p-fail", "fail", "anthropic", &[]),
        prov("p-bad", "bad", "anthropic", &[]),
        prov("p-anth", "anth", "anthropic", &["claude-sonnet-5-5"]),
        prov("p-oai", "oai", "openai", &["gpt-6", "gpt-6-mini"]),
    ];
    reg.routes = vec![
        Route { id: "fo".into(), mode: "failover".into(), providers: vec!["p-fail".into(), "p-anth".into()], default_provider: None },
        Route { id: "bad".into(), mode: "failover".into(), providers: vec!["p-bad".into(), "p-anth".into()], default_provider: None },
        Route { id: "oai".into(), mode: "failover".into(), providers: vec!["p-oai".into()], default_provider: None },
        Route { id: "agg".into(), mode: "aggregate".into(), providers: vec!["p-anth".into(), "p-oai".into()], default_provider: Some("p-anth".into()) },
    ];
    ctx.save_registry(&reg).unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-anth"), "key-anth").unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-oai"), "key-oai").unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-fail"), "x").unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-bad"), "x").unwrap();
    let token = hrm_core::service::router_token(&ctx).unwrap();

    let rl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", rl.local_addr().unwrap());
    let c2 = ctx.clone();
    tokio::spawn(async move { hrm_router::serve_on(c2, rl).await.unwrap() });
    Setup { _serial: serial, _dir: dir, base, token, hits, ctx }
}

fn body(model: &str, stream: bool) -> Value {
    json!({"model": model, "max_tokens": 50, "stream": stream, "system": "be brief", "messages": [{"role": "user", "content": "hi"}]})
}

async fn post_msg(s: &Setup, route: &str, b: &Value, token: Option<&str>) -> reqwest::Response {
    let mut r = reqwest::Client::new().post(format!("{}/r/{route}/acct/v1/messages", s.base)).json(b);
    if let Some(t) = token { r = r.bearer_auth(t); }
    r.send().await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn router_end_to_end() {
    let s = setup().await;

    // 1. the launch token is required
    assert_eq!(post_msg(&s, "fo", &body("claude-sonnet-5-5", false), None).await.status(), 401);
    assert_eq!(post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some("wrong")).await.status(), 401);

    // 2. failover: p-fail answers 500, p-anth serves the request
    let r = post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some(&s.token)).await;
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["content"][0]["text"], "from anthropic");
    assert_eq!(s.hits.fail.load(Ordering::SeqCst), 1);

    // 3. after three failures the breaker pauses p-fail: no more requests reach it
    for _ in 0..4 {
        assert_eq!(post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some(&s.token)).await.status(), 200);
    }
    assert_eq!(s.hits.fail.load(Ordering::SeqCst), 3, "paused after three failures");
    let breaker: Value = serde_json::from_str(&s.ctx.db.kv_get("breaker:p-fail").unwrap().unwrap()).unwrap();
    assert_eq!(breaker["state"], "open");

    // 4. a request the provider rejects as invalid is not failed over
    let r = post_msg(&s, "bad", &body("claude-sonnet-5-5", false), Some(&s.token)).await;
    assert_eq!(r.status(), 400);
    let anth_before = s.hits.anth.load(Ordering::SeqCst);
    assert_eq!(anth_before, 5);

    // 5. OpenAI-format provider: converted both ways; the provider's own model is used
    let v: Value = post_msg(&s, "oai", &body("claude-sonnet-5-5", false), Some(&s.token)).await.json().await.unwrap();
    assert_eq!(v["type"], "message");
    assert_eq!(v["content"][0]["text"], "from openai gpt-6");
    assert_eq!(v["usage"]["output_tokens"], 4);

    // 6. streaming conversion produces Anthropic events
    let text = post_msg(&s, "oai", &body("claude-sonnet-5-5", true), Some(&s.token)).await.text().await.unwrap();
    let events: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("event: ")).collect();
    assert_eq!(events, vec!["message_start", "content_block_start", "content_block_delta", "content_block_delta", "content_block_stop", "message_delta", "message_stop"]);
    assert!(text.contains("\"text\":\"Hel\"") && text.contains("\"output_tokens\":2"));

    // 7. aggregate: the id names the provider; unknown providers fail instead of falling back
    let v: Value = post_msg(&s, "agg", &body("hm-p-oai--gpt-6-mini", false), Some(&s.token)).await.json().await.unwrap();
    assert_eq!(v["content"][0]["text"], "from openai gpt-6-mini");
    let v: Value = post_msg(&s, "agg", &body("claude-sonnet-5-5", false), Some(&s.token)).await.json().await.unwrap();
    assert_eq!(v["content"][0]["text"], "from anthropic", "no prefix goes to the default provider");
    assert_eq!(post_msg(&s, "agg", &body("hm-p-fail--x", false), Some(&s.token)).await.status(), 404);
    assert_eq!(post_msg(&s, "agg", &body("hm-nope--x", false), Some(&s.token)).await.status(), 404);

    // 8. model list for the picker
    let m: Value = reqwest::Client::new().get(format!("{}/r/agg/acct/v1/models", s.base)).bearer_auth(&s.token).send().await.unwrap().json().await.unwrap();
    let ids: Vec<&str> = m["data"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["hm-p-anth--claude-sonnet-5-5", "hm-p-oai--gpt-6", "hm-p-oai--gpt-6-mini"]);

    // 9. every request is logged with its profile and provider
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let log = s.ctx.db.route_log(100).unwrap();
    assert!(log.iter().any(|r| r["provider"] == "p-fail" && r["status"] == 500));
    assert!(log.iter().any(|r| r["provider"] == "p-oai" && r["output"] == 2), "stream usage recorded");
    assert!(log.iter().all(|r| r["profile"] == "acct"));

}

/// A provider that answers with a chunked stream cut exactly where the test says, by request path:
/// `/split/...` and `/usage/...` split a line (and a character) across two chunks, `/drop/...` closes mid-answer.
async fn raw_provider() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = l.accept().await else { return };
            tokio::spawn(async move {
                // read the whole request (headers, then content-length bytes of body)
                let mut req = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = s.read(&mut buf).await.unwrap();
                    req.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&req).to_string();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let len = text[..h].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                        if req.len() >= h + 4 + len { break; }
                    }
                    if n == 0 { return; }
                }
                let path = String::from_utf8_lossy(&req).split_whitespace().nth(1).unwrap_or("").to_string();
                let word = "مرحبا".as_bytes();
                let (parts, finish): (Vec<Vec<u8>>, bool) = if path.starts_with("/split") {
                    let a = [br#"data: {"id":"c","choices":[{"delta":{"content":""#.as_slice(), &word[..3]].concat();
                    let b = [&word[3..], br#""}}]}

data: {"id":"c","choices":[],"usage":{"prompt_tokens":3,"completion_tokens":5}}

data: [DONE]

"#.as_slice()].concat();
                    (vec![a, b], true)
                } else if path.starts_with("/usage") {
                    (vec![b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_t".to_vec(), b"okens\":42}}\n\n".to_vec()], true)
                } else {
                    (vec![br#"data: {"id":"c","choices":[{"delta":{"content":"half"}}]}

"#.to_vec()], false)
                };
                s.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n").await.unwrap();
                for p in parts {
                    s.write_all(format!("{:x}\r\n", p.len()).as_bytes()).await.unwrap();
                    s.write_all(&p).await.unwrap();
                    s.write_all(b"\r\n").await.unwrap();
                    s.flush().await.unwrap();
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
                if finish { s.write_all(b"0\r\n\r\n").await.unwrap(); }
                // dropping the socket without the last chunk is the provider vanishing mid-answer
            });
        }
    });
    base
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_survive_split_chunks_and_report_a_dropped_provider() {
    let s = setup().await;
    let raw = raw_provider().await;
    let mut reg = s.ctx.registry().unwrap();
    for (id, path, format) in [("p-split", "split", "openai"), ("p-drop", "drop", "openai"), ("p-usage", "usage", "anthropic")] {
        reg.providers.push(Provider { id: id.into(), name: id.into(), base_url: format!("{raw}/{path}"), format: format.into(), models: vec![] });
        reg.routes.push(Route { id: path.into(), mode: "failover".into(), providers: vec![id.into()], default_provider: None });
        hrm_core::secrets::set_key(&hrm_core::secrets::provider_target(id), "k").unwrap();
    }
    s.ctx.save_registry(&reg).unwrap();

    // a character split across two chunks arrives whole
    let text = post_msg(&s, "split", &body("m", true), Some(&s.token)).await.text().await.unwrap();
    assert!(text.contains("مرحبا"), "{text}");
    assert!(text.contains("event: message_stop"));

    // a usage line split across two chunks is still counted
    post_msg(&s, "usage", &body("m", true), Some(&s.token)).await.text().await.unwrap();

    // a provider that drops mid-answer ends in an error event, not a normal finish
    let text = post_msg(&s, "drop", &body("m", true), Some(&s.token)).await.text().await.unwrap();
    assert!(text.contains("event: error") && text.contains("stopped mid-answer"), "{text}");
    assert!(!text.contains("message_stop"), "{text}");

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let log = s.ctx.db.route_log(100).unwrap();
    assert!(log.iter().any(|r| r["provider"] == "p-usage" && r["output"] == 42), "{log:?}");
    assert!(log.iter().any(|r| r["provider"] == "p-drop" && r["status"] == 502), "{log:?}");
    assert!(log.iter().any(|r| r["provider"] == "p-split" && r["output"] == 5 && r["status"] == 200), "{log:?}");

}

#[tokio::test(flavor = "multi_thread")]
async fn a_daily_cap_stops_a_routed_account_for_the_day() {
    let s = setup().await;
    // a dollar a token in and out, so one answer (11 in, 3 out) costs $14
    s.ctx.db.kv_set("pricing:catalog", &json!({"anthropic": {"models": {"claude-sonnet-5-5": {"cost": {"input": 1_000_000.0, "output": 1_000_000.0}}}}}).to_string()).unwrap();
    s.ctx.db.kv_set("pricing:at", &hrm_core::util::now_ms().to_string()).unwrap();
    let mut reg = s.ctx.registry().unwrap();
    reg.profiles.push(hrm_core::registry::Profile { name: "acct".into(), label: None, harness: "claude".into(), kind: hrm_core::registry::Kind::Routed, email: None, main: false,
        base_url: None, model: None, route: Some("fo".into()), daily_cap_usd: Some(10.0), created_at: 0 });
    s.ctx.save_registry(&reg).unwrap();
    // under the cap: it goes through
    assert_eq!(post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some(&s.token)).await.status(), 200);
    assert!((hrm_core::dash::spent_today(&s.ctx, "acct").unwrap() - 14.0).abs() < 1e-6);
    // over it: stopped, said why, and nothing reaches a provider
    let before = s.hits.anth.load(Ordering::SeqCst);
    let r = post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some(&s.token)).await;
    assert_eq!(r.status(), 429);
    assert!(r.text().await.unwrap().contains("daily cap of $10.00"));
    assert_eq!(s.hits.anth.load(Ordering::SeqCst), before);
    assert!(s.ctx.db.events(5).unwrap().iter().any(|e| e["kind"] == "cap"));
}
