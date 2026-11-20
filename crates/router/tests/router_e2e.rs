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

async fn limited() -> Response { (StatusCode::TOO_MANY_REQUESTS, [("retry-after", "7")], "slow down").into_response() }

async fn denied() -> Response { (StatusCode::UNAUTHORIZED, "the key is not valid").into_response() }

/// A failure the way some OpenAI-format providers send it: status 200, an `error` and no `choices`.
async fn oai_err() -> Response { Json(json!({"error": {"message": "the model is overloaded", "code": 503}})).into_response() }

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

/// Answers with what arrived, so a test sees what the router sent on. A streamed Responses answer ends with its usage.
async fn echo(headers: HeaderMap, Json(req): Json<Value>) -> Response {
    let h = |k: &str| headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let usage = json!({"input_tokens": 100, "output_tokens": 7, "input_tokens_details": {"cached_tokens": 60}});
    if req["stream"] == true {
        let done = json!({"type": "response.completed", "response": {"id": "resp_1", "usage": usage}});
        return ([("content-type", "text/event-stream")], format!("event: response.created
data: {{\"type\":\"response.created\"}}

event: response.completed
data: {done}

")).into_response();
    }
    Json(json!({"usage": usage, "got": req, "auth": h("authorization"), "x_api_key": h("x-api-key"), "beta": h("anthropic-beta"), "agent": h("user-agent")})).into_response()
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
        .route("/limited/v1/messages", post(limited))
        .route("/denied/v1/messages", post(denied))
        .route("/oaierr/chat/completions", post(oai_err))
        .route("/anth/v1/messages", post(anth))
        .route("/oai/chat/completions", post(oai))
        .route("/own/v1/messages", post(echo))
        .route("/own/responses", post(echo))
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
        prov("p-limited", "limited", "anthropic", &[]),
        prov("p-denied", "denied", "anthropic", &[]),
        prov("p-oaierr", "oaierr", "openai", &[]),
    ];
    reg.routes = vec![
        Route { id: "fo".into(), mode: "failover".into(), providers: vec!["p-fail".into(), "p-anth".into()], default_provider: None },
        Route { id: "bad".into(), mode: "failover".into(), providers: vec!["p-bad".into(), "p-anth".into()], default_provider: None },
        Route { id: "oai".into(), mode: "failover".into(), providers: vec!["p-oai".into()], default_provider: None },
        Route { id: "agg".into(), mode: "aggregate".into(), providers: vec!["p-anth".into(), "p-oai".into()], default_provider: Some("p-anth".into()) },
        Route { id: "limited".into(), mode: "failover".into(), providers: vec!["p-fail".into(), "p-limited".into()], default_provider: None },
        Route { id: "denied".into(), mode: "aggregate".into(), providers: vec!["p-denied".into()], default_provider: None },
        Route { id: "oaierr".into(), mode: "failover".into(), providers: vec!["p-oaierr".into(), "p-anth".into()], default_provider: None },
        Route { id: "oaierr1".into(), mode: "failover".into(), providers: vec!["p-oaierr".into()], default_provider: None },
    ];
    ctx.save_registry(&reg).unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-anth"), "key-anth").unwrap();
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-oai"), "key-oai").unwrap();
    for id in ["p-fail", "p-bad", "p-limited", "p-denied", "p-oaierr"] { hrm_core::secrets::set_key(&hrm_core::secrets::provider_target(id), "x").unwrap(); }
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
    let r = post_msg(&s, "fo", &body("claude-sonnet-5-5", false), Some("wrong")).await;
    assert_eq!(r.status(), 401);
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["type"], "authentication_error");

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
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["type"], "invalid_request_error");
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
    let r = post_msg(&s, "agg", &body("hm-nope--x", false), Some(&s.token)).await;
    assert_eq!(r.status(), 404);
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["type"], "not_found_error");

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

#[tokio::test(flavor = "multi_thread")]
async fn a_providers_error_keeps_its_status_its_type_and_its_retry_after() {
    let s = setup().await;

    // every provider failed: the last refusal comes back with its retry-after, where it was a 502 a tool retries at once
    let r = post_msg(&s, "limited", &body("m", false), Some(&s.token)).await;
    assert_eq!((r.status().as_u16(), r.headers().get("retry-after").and_then(|v| v.to_str().ok())), (429, Some("7")));
    let v: Value = r.json().await.unwrap();
    assert_eq!((v["type"].as_str(), v["error"]["type"].as_str()), (Some("error"), Some("rate_limit_error")), "{v}");
    assert!(v["error"]["message"].as_str().unwrap().contains("p-limited answered 429"), "{v}");

    // a key the provider refuses is an authentication error, and no retry-after is made up for it
    let r = post_msg(&s, "denied", &body("m", false), Some(&s.token)).await;
    assert_eq!((r.status().as_u16(), r.headers().get("retry-after")), (401, None));
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["type"], "authentication_error");

    // an OpenAI-format error with status 200 is a failure: the next provider answers
    let before = s.hits.anth.load(Ordering::SeqCst);
    let r = post_msg(&s, "oaierr", &body("m", false), Some(&s.token)).await;
    assert_eq!(r.status(), 200);
    assert_eq!(r.json::<Value>().await.unwrap()["content"][0]["text"], "from anthropic");
    assert_eq!(s.hits.anth.load(Ordering::SeqCst), before + 1);

    // with no other provider it is an error with the provider's reason, never an empty answer marked complete
    let r = post_msg(&s, "oaierr1", &body("m", false), Some(&s.token)).await;
    assert_eq!(r.status(), 502);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["error"]["type"], "api_error");
    assert!(v["error"]["message"].as_str().unwrap().contains("the model is overloaded"), "{v}");
    let text = post_msg(&s, "oaierr1", &body("m", true), Some(&s.token)).await.text().await.unwrap();
    assert!(text.contains("event: error") && text.contains("the model is overloaded"), "{text}");
    assert!(!text.contains("message_stop"), "{text}");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let log = s.ctx.db.route_log(100).unwrap();
    assert!(log.iter().any(|r| r["provider"] == "p-limited" && r["status"] == 429), "{log:?}");
    assert!(!log.iter().any(|r| r["provider"] == "p-oaierr" && r["status"] == 200), "a failure is not logged as an answer: {log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn health_names_the_program_only_to_the_tokens_holder() {
    let s = setup().await;
    let get = |token: Option<&str>| { let mut r = reqwest::Client::new().get(format!("{}/health", s.base)); if let Some(t) = token { r = r.bearer_auth(t); } r.send() };
    assert_eq!(get(None).await.unwrap().text().await.unwrap(), "ok");
    assert_eq!(get(Some("wrong")).await.unwrap().text().await.unwrap(), "ok");
    let named = get(Some(&s.token)).await.unwrap().text().await.unwrap();
    assert!(named.strip_prefix("ok\n").is_some_and(|exe| !exe.is_empty()), "the program file follows for the caller with the token");
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
                } else if path.starts_with("/cached") {
                    (vec![b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":1000,\"cache_creation_input_tokens\":100,\"output_tokens\":1}}}\n\n".to_vec(),
                        b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":5}}\n\n".to_vec()], true)
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
async fn a_streamed_answer_counts_its_cached_tokens_toward_the_days_spend() {
    let s = setup().await;
    let raw = raw_provider().await;
    // a dollar a token; cached reads at a tenth and cache writes at a quarter more, as the catalog's defaults
    s.ctx.db.kv_set("pricing:catalog", &json!({"anthropic": {"models": {"claude-sonnet-5-5": {"cost": {"input": 1_000_000.0, "output": 1_000_000.0}}}}}).to_string()).unwrap();
    s.ctx.db.kv_set("pricing:at", &hrm_core::util::now_ms().to_string()).unwrap();
    let mut reg = s.ctx.registry().unwrap();
    reg.providers.push(Provider { id: "p-cached".into(), name: "p-cached".into(), base_url: format!("{raw}/cached"), format: "anthropic".into(), models: vec![] });
    reg.routes.push(Route { id: "cached".into(), mode: "failover".into(), providers: vec!["p-cached".into()], default_provider: None });
    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target("p-cached"), "k").unwrap();
    s.ctx.save_registry(&reg).unwrap();
    post_msg(&s, "cached", &body("claude-sonnet-5-5", true), Some(&s.token)).await.text().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let log = s.ctx.db.route_log(10).unwrap();
    assert!(log.iter().any(|r| r["provider"] == "p-cached" && r["input"] == 1110 && r["output"] == 5), "the log shows every token: {log:?}");
    // 10 in + 5 out + 1000 read at $0.10 + 100 written at $1.25
    let spent = hrm_core::dash::spent_today(&s.ctx, "acct").unwrap();
    assert!((spent - 240.0).abs() < 1e-6, "{spent}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daily_cap_stops_a_routed_account_for_the_day() {
    let s = setup().await;
    // a dollar a token in and out, so one answer (11 in, 3 out) costs $14
    s.ctx.db.kv_set("pricing:catalog", &json!({"anthropic": {"models": {"claude-sonnet-5-5": {"cost": {"input": 1_000_000.0, "output": 1_000_000.0}}}}}).to_string()).unwrap();
    s.ctx.db.kv_set("pricing:at", &hrm_core::util::now_ms().to_string()).unwrap();
    let mut reg = s.ctx.registry().unwrap();
    reg.profiles.push(hrm_core::registry::Profile { name: "acct".into(), label: None, harness: "claude".into(), kind: hrm_core::registry::Kind::Routed, email: None, main: false,
        base_url: None, model: None, route: Some("fo".into()), daily_cap_usd: Some(10.0), intro: None, created_at: 0 });
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

#[tokio::test(flavor = "multi_thread")]
async fn an_api_key_account_with_its_own_endpoint_is_its_own_route_for_claude_code_and_for_codex() {
    use hrm_core::registry::{Kind, Profile};
    let s = setup().await;
    let mut reg = s.ctx.registry().unwrap();
    let up = reg.providers[0].base_url.trim_end_matches("/fail").to_string();
    let acct = |name: &str, harness: &str, model: Option<&str>, intro: Option<&str>| Profile { name: name.into(), label: None, harness: harness.into(), kind: Kind::ApiKey, email: None, main: false,
        base_url: Some(format!("{up}/own")), model: model.map(String::from), route: None, daily_cap_usd: None, intro: intro.map(String::from), created_at: 0 };
    reg.profiles.push(acct("mine", "claude", Some("local-m"), Some("You are the team's assistant.")));
    reg.profiles.push(acct("cdx", "codex", None, Some("")));
    reg.profiles.push(acct("asis", "claude", None, None));
    s.ctx.save_registry(&reg).unwrap();
    for n in ["mine", "cdx", "asis"] { hrm_core::secrets::set_key(&hrm_core::secrets::key_target(n), &format!("key-{n}")).unwrap(); }
    let post = |path: &str, b: Value| reqwest::Client::new().post(format!("{}/r/{path}", s.base)).bearer_auth(&s.token)
        .header("anthropic-beta", "claude-code-20250219,interleaved-thinking-2025-05-14").header("user-agent", "claude-cli/2.1.329 (external, cli)").json(&b).send();
    let claude = json!({"model": "claude-haiku-5-5", "max_tokens": 9, "output_config": {"effort": "high"}, "metadata": {"user_id": "device"}, "messages": [{"role": "user", "content": "hi"}], "system": [
        {"type": "text", "text": "x-anthropic-billing-header: cc_version=2.1.329; cc_entrypoint=cli;"},
        {"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}, {"type": "text", "text": "Use the tools."}]});

    // Claude Code: the account's endpoint, key (both ways: a gateway may take a bearer only) and model; its intro replaced, its own ids gone
    let v: Value = post("~/mine/v1/messages", claude.clone()).await.unwrap().json().await.unwrap();
    assert_eq!((v["auth"].as_str(), v["x_api_key"].as_str()), (Some("Bearer key-mine"), Some("key-mine")), "{v}");
    assert_eq!(v["got"]["model"], "local-m", "its own endpoint knows its own model only");
    assert_eq!(v["got"]["system"], json!([{"type": "text", "text": "You are the team's assistant."}, {"type": "text", "text": "Use the tools."}]));
    assert!(v["got"].get("output_config").is_none(), "a field an endpoint of one's own may refuse: {v}");
    assert!(v["got"].get("metadata").is_none() && v["beta"] == "interleaved-thinking-2025-05-14" && !v["agent"].as_str().unwrap().contains("claude"), "{v}");

    // no intro setting: the request goes on as the tool wrote it
    let v: Value = post("~/asis/v1/messages", claude.clone()).await.unwrap().json().await.unwrap();
    assert_eq!((&v["got"]["system"], &v["got"]["metadata"], v["beta"].as_str()), (&claude["system"], &claude["metadata"], Some("claude-code-20250219,interleaved-thinking-2025-05-14")));

    // Codex: Responses, passed on as they are; "" removes the intro
    let codex = |stream: bool| json!({"model": "qwen", "stream": stream, "client_metadata": {"x-codex-installation-id": "i"}, "instructions": "You are Codex, based on GPT-5.
Be exact.", "input": [
        {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "hi"}]}]});
    let v: Value = post("~/cdx/v1/responses", codex(false)).await.unwrap().json().await.unwrap();
    assert_eq!((v["auth"].as_str(), v["got"]["model"].as_str(), v["got"]["instructions"].as_str()), (Some("Bearer key-cdx"), Some("qwen"), Some("Be exact.")), "{v}");
    assert!(v["got"].get("client_metadata").is_none() && v["got"]["input"] == codex(false)["input"], "{v}");
    let text = post("~/cdx/v1/responses", codex(true)).await.unwrap().text().await.unwrap();
    assert!(text.starts_with("event: response.created") && text.contains("event: response.completed"), "the events come back as they were sent: {text}");

    // an account without an endpoint of its own has no such route, and Codex cannot ask an Anthropic-format provider
    assert_eq!(post("~/acct/v1/messages", claude.clone()).await.unwrap().status(), 404);
    let r = post("fo/acct/v1/responses", codex(false)).await.unwrap();
    assert_eq!(r.status(), 502);
    assert!(r.text().await.unwrap().contains("Codex needs an OpenAI-format provider"));

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let log = s.ctx.db.route_log(100).unwrap();
    assert!(log.iter().any(|r| r["provider"] == "~mine" && r["model"] == "local-m" && r["status"] == 200), "{log:?}");
    assert_eq!(log.iter().filter(|r| r["provider"] == "~cdx" && r["input"] == 100 && r["output"] == 7).count(), 2, "plain and streamed: {log:?}");
}
