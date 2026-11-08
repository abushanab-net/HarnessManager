//! The local router: `http://127.0.0.1:<port>/r/<route>/<profile>`. API-key providers only; subscription
//! logins never pass through it. Requests carry the launch token; failover uses a circuit breaker per
//! provider; aggregate routes pick the provider from a `hm-<provider>--<model>` id and never fall back.

pub mod convert;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use hrm_core::registry::{Provider, Route};
use hrm_core::Ctx;
use serde_json::{json, Value};

const FAILS_TO_OPEN: u32 = 3;
const OPEN_FOR: Duration = Duration::from_secs(60);

#[derive(Default, Clone)]
struct Breaker {
    failures: u32,
    open_until: Option<Instant>,
}

pub struct AppState {
    ctx: Arc<Ctx>,
    client: reqwest::Client,
    breakers: Mutex<HashMap<String, Breaker>>,
}

impl AppState {
    fn allowed(&self, provider: &str) -> bool {
        let mut map = self.breakers.lock().unwrap();
        let Some(b) = map.get_mut(provider) else { return true };
        match b.open_until {
            // half-open: this request is the one probe; others skip the provider until it reports back
            Some(t) if Instant::now() >= t => { b.open_until = Some(Instant::now() + OPEN_FOR); true }
            Some(_) => false,
            None => true,
        }
    }

    fn report(&self, provider: &str, ok: bool) {
        let mut map = self.breakers.lock().unwrap();
        let b = map.entry(provider.into()).or_default();
        let was_open = b.open_until.is_some();
        if ok {
            *b = Breaker::default();
            if was_open { let _ = self.ctx.db.event("ok", "router", &format!("{provider} answers again; the router uses it once more.")); }
        } else {
            b.failures += 1;
            if b.failures >= FAILS_TO_OPEN {
                b.open_until = Some(Instant::now() + OPEN_FOR);
                if b.failures == FAILS_TO_OPEN {
                    let _ = self.ctx.db.event("bad", "router", &format!("{provider} paused after {FAILS_TO_OPEN} failures in a row."));
                }
            }
        }
        let open = b.open_until.map(|t| Instant::now() < t).unwrap_or(false);
        // when it is tried again, as a time Harness Manager can show
        let retry_at = b.open_until.map(|t| hrm_core::util::now_ms() + t.saturating_duration_since(Instant::now()).as_millis() as i64);
        let _ = self.ctx.db.kv_set(&format!("breaker:{provider}"), &json!({"state": if open { "open" } else { "closed" }, "failures": b.failures, "retry_at": retry_at}).to_string());
    }
}

pub fn router(ctx: Arc<Ctx>) -> Router {
    let state = Arc::new(AppState { ctx, client: reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).read_timeout(Duration::from_secs(300)).build().unwrap(), breakers: Mutex::new(HashMap::new()) });
    Router::new()
        // and which program serves, so Harness Manager can replace a router from another build
        .route("/health", get(|| async { format!("ok\n{}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()) }))
        .route("/r/{route}/{profile}/v1/messages", post(messages))
        .route("/r/{route}/{profile}/v1/messages/count_tokens", post(count_tokens))
        .route("/r/{route}/{profile}/v1/models", get(models))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}

/// Serve on 127.0.0.1 only: the router injects keys, so it must never listen on the network.
pub async fn serve(ctx: Arc<Ctx>, port: u16) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    ctx.db.kv_set("router:pid", &std::process::id().to_string())?;
    serve_on(ctx, listener).await
}

pub async fn serve_on(ctx: Arc<Ctx>, listener: tokio::net::TcpListener) -> Result<()> {
    axum::serve(listener, router(ctx)).await?;
    Ok(())
}

fn err(status: StatusCode, kind: &str, msg: impl Into<String>) -> Response {
    (status, Json(json!({"type": "error", "error": {"type": kind, "message": msg.into()}}))).into_response()
}

fn authorized(st: &AppState, headers: &HeaderMap) -> bool {
    let Ok(Some(token)) = st.ctx.db.kv_get("router:token") else { return false };
    let bearer = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim);
    let key = headers.get("x-api-key").and_then(|v| v.to_str().ok());
    bearer == Some(token.as_str()) || key == Some(token.as_str())
}

/// Model a provider should receive: the requested one if it serves it, else its first model.
fn model_for(p: &Provider, requested: &str) -> String {
    if p.models.is_empty() || p.models.iter().any(|m| m == requested) { requested.into() } else { p.models[0].clone() }
}

/// Providers to try, in order, each with the model to send.
fn candidates(reg: &hrm_core::Registry, route: &Route, model: &str) -> Result<Vec<(Provider, String)>, Response> {
    let find = |id: &str| reg.providers.iter().find(|p| p.id == id).cloned();
    if route.mode == "aggregate" {
        if let Some(rest) = model.strip_prefix("hm-") {
            let Some((pid, m)) = rest.split_once("--") else { return Err(err(StatusCode::NOT_FOUND, "not_found_error", format!("unknown model {model}"))) };
            if !route.providers.iter().any(|x| x == pid) {
                return Err(err(StatusCode::NOT_FOUND, "not_found_error", format!("provider {pid} is not in route {}", route.id)));
            }
            let p = find(pid).ok_or_else(|| err(StatusCode::NOT_FOUND, "not_found_error", format!("unknown provider {pid}")))?;
            return Ok(vec![(p, m.to_string())]);
        }
        let pid = route.default_provider.clone().or_else(|| route.providers.first().cloned()).unwrap_or_default();
        let p = find(&pid).ok_or_else(|| err(StatusCode::NOT_FOUND, "not_found_error", "route has no default provider"))?;
        let m = model_for(&p, model);
        return Ok(vec![(p, m)]);
    }
    Ok(route.providers.iter().filter_map(|id| find(id)).map(|p| { let m = model_for(&p, model); (p, m) }).collect())
}

#[allow(clippy::too_many_arguments)]
fn log(st: &AppState, route: &str, profile: &str, provider: &str, model: &str, status: u16, started: Instant, first: Option<i64>, input: i64, output: i64) {
    let _ = st.ctx.db.log_route(route, profile, provider, model, status as i64, started.elapsed().as_millis() as i64, first, input, output);
}

async fn messages(State(st): State<Arc<AppState>>, Path((route_id, profile)): Path<(String, String)>, headers: HeaderMap, body: Bytes) -> Response {
    if !authorized(&st, &headers) {
        return err(StatusCode::UNAUTHORIZED, "authentication_error", "missing or wrong router token");
    }
    let Ok(req) = serde_json::from_slice::<Value>(&body) else { return err(StatusCode::BAD_REQUEST, "invalid_request_error", "body is not JSON") };
    let reg = match st.ctx.registry() { Ok(r) => r, Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, "api_error", e.to_string()) };
    let Some(route) = reg.routes.iter().find(|r| r.id == route_id).cloned() else { return err(StatusCode::NOT_FOUND, "not_found_error", format!("no route {route_id}")) };
    // a daily spend cap: nothing more goes out today once it is reached (prices may be fetched, so off the async thread)
    if let Some(cap) = reg.profiles.iter().find(|p| p.name == profile).and_then(|p| p.daily_cap_usd) {
        let (c2, who) = (st.ctx.clone(), profile.clone());
        let spent = tokio::task::spawn_blocking(move || hrm_core::dash::spent_today(&c2, &who)).await.ok().and_then(|r| r.ok()).unwrap_or(0.0);
        if spent >= cap {
            let _ = st.ctx.db.event("warn", "cap", &format!("{profile} reached its ${cap:.2} daily cap. The router stops for it until midnight."));
            return err(StatusCode::TOO_MANY_REQUESTS, "rate_limit_error", format!("{profile} reached its daily cap of ${cap:.2} (${spent:.2} spent today); it resets at midnight"));
        }
    }
    let requested = req["model"].as_str().unwrap_or("").to_string();
    let list = match candidates(&reg, &route, &requested) { Ok(l) => l, Err(r) => return r };
    let stream = req["stream"] == true;
    let mut last_error = String::from("no provider is available");
    let failover = route.mode == "failover";
    // providers that failed this request, for the failover event
    let mut tried: Vec<String> = vec![];
    for (p, model) in list {
        if failover && !st.allowed(&p.id) {
            last_error = format!("{} is paused after repeated failures", p.id);
            continue;
        }
        let started = Instant::now();
        // the registry syncs between computers, the keys do not
        let Some(key) = hrm_core::secrets::get_key(&hrm_core::secrets::provider_target(&p.id)) else {
            last_error = format!("no key for {} is saved on this computer; add it under API keys & routing", p.id);
            continue;
        };
        let base = p.base_url.trim_end_matches('/');
        let rb = if p.format == "openai" {
            st.client.post(format!("{base}/chat/completions")).bearer_auth(&key).json(&convert::anthropic_to_openai(&req, &model))
        } else {
            let mut body = req.clone();
            body["model"] = json!(model);
            let mut b = st.client.post(format!("{base}/v1/messages")).header("x-api-key", &key).header("anthropic-version",
                headers.get("anthropic-version").and_then(|v| v.to_str().ok()).unwrap_or("2023-06-01"));
            if let Some(beta) = headers.get("anthropic-beta").and_then(|v| v.to_str().ok()) { b = b.header("anthropic-beta", beta); }
            b.json(&body)
        };
        let resp = match rb.send().await {
            Ok(r) => r,
            Err(e) => {
                st.report(&p.id, false);
                tried.push(p.id.clone());
                log(&st, &route_id, &profile, &p.id, &model, 0, started, None, 0, 0);
                last_error = format!("{}: {e}", p.id);
                if failover { continue } else { break }
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let code = status.as_u16();
            let text = resp.text().await.unwrap_or_default();
            log(&st, &route_id, &profile, &p.id, &model, code, started, None, 0, 0);
            // the next provider would reject the same request; do not fail over on these
            if matches!(code, 400 | 413 | 422) {
                st.report(&p.id, true);
                return err(status, "invalid_request_error", format!("{}: {}", p.id, hrm_core::util::truncate(&text, 400)));
            }
            st.report(&p.id, false);
            tried.push(p.id.clone());
            last_error = format!("{} answered {code}: {}", p.id, hrm_core::util::truncate(&text, 200));
            if failover { continue } else { return err(status, "api_error", last_error) }
        }
        st.report(&p.id, true);
        if failover && !tried.is_empty() {
            let _ = st.ctx.db.event("warn", "router", &format!("{profile} failed over from {} to {}. No request lost.", tried.join(" and "), p.id));
        }
        return if stream {
            stream_back(st.clone(), resp, p, model, route_id.clone(), profile.clone(), started)
        } else {
            let v: Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    log(&st, &route_id, &profile, &p.id, &model, 502, started, None, 0, 0);
                    return err(StatusCode::BAD_GATEWAY, "api_error", format!("{} sent a reply that is not JSON: {e}", p.id));
                }
            };
            let out = if p.format == "openai" { convert::openai_to_anthropic(&v, &model) } else { v };
            // not streamed: the first chunk is the whole answer
            let ms = started.elapsed().as_millis() as i64;
            // shortcut: router_log has no cache column, so cache hits (DeepSeek) are counted into input; split them when it gets one
            let input = out["usage"]["input_tokens"].as_i64().unwrap_or(0) + out["usage"]["cache_read_input_tokens"].as_i64().unwrap_or(0);
            log(&st, &route_id, &profile, &p.id, &model, 200, started, Some(ms), input, out["usage"]["output_tokens"].as_i64().unwrap_or(0));
            Json(out).into_response()
        };
    }
    err(StatusCode::BAD_GATEWAY, "api_error", last_error)
}

/// Relay a streaming answer, converting OpenAI chunks to Anthropic events on the fly.
fn stream_back(st: Arc<AppState>, resp: reqwest::Response, p: Provider, model: String, route: String, profile: String, started: Instant) -> Response {
    let openai = p.format == "openai";
    let mut upstream = resp.bytes_stream();
    let s = async_stream_like(move |tx| async move {
        // bytes, cut at line ends only: a line, or a multi-byte character, split across chunks waits for its rest
        let mut buf: Vec<u8> = Vec::new();
        let mut conv = convert::StreamConv::new(&model);
        let (mut input, mut output) = (0i64, 0i64);
        let mut broken = None;
        let mut first: Option<i64> = None;
        while let Some(chunk) = upstream.next().await {
            let chunk = match chunk { Ok(c) => c, Err(e) => { broken = Some(e.to_string()); break } };
            if first.is_none() { first = Some(started.elapsed().as_millis() as i64); }
            // Anthropic format: passed through unchanged, read on the way for the usage numbers
            if !openai && tx.send(Ok(chunk.clone())).await.is_err() { log(&st, &route, &profile, &p.id, &model, 499, started, first, input, output); return; }
            buf.extend_from_slice(&chunk);
            while let Some(i) = buf.iter().position(|b| *b == b'\n') {
                let raw: Vec<u8> = buf.drain(..=i).collect();
                let line = String::from_utf8_lossy(&raw);
                let Some(data) = line.trim().strip_prefix("data:").map(str::trim) else { continue };
                if !openai {
                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                        if let Some(i) = v["message"]["usage"]["input_tokens"].as_i64() { input = i; }
                        if let Some(o) = v["usage"]["output_tokens"].as_i64() { output = o; }
                    }
                    continue;
                }
                let events = if data == "[DONE]" { conv.finish() } else { serde_json::from_str::<Value>(data).map(|v| conv.feed(&v)).unwrap_or_default() };
                for e in events {
                    if tx.send(Ok(Bytes::from(e))).await.is_err() {
                        // 499: the client went away (Esc in Claude Code); the tokens so far were still used
                        log(&st, &route, &profile, &p.id, &model, 499, started, first, conv.input_tokens, conv.output_tokens);
                        return;
                    }
                }
            }
        }
        if openai {
            input = conv.input_tokens;
            output = conv.output_tokens;
        }
        if let Some(e) = broken.or_else(|| conv.error.take()) {
            // never end a cut-off answer as if it were complete
            let ev = json!({"type": "error", "error": {"type": "api_error", "message": format!("{} stopped mid-answer: {e}", p.id)}});
            let _ = tx.send(Ok(Bytes::from(format!("event: error\ndata: {ev}\n\n")))).await;
            log(&st, &route, &profile, &p.id, &model, 502, started, first, input, output);
            return;
        }
        if openai {
            for e in conv.finish() { let _ = tx.send(Ok(Bytes::from(e))).await; }
        }
        log(&st, &route, &profile, &p.id, &model, 200, started, first, input, output);
    });
    Response::builder().header("content-type", "text/event-stream").header("cache-control", "no-cache").body(Body::from_stream(s)).unwrap()
}

/// A small channel-backed stream, so the relay above can be written as a plain loop.
fn async_stream_like<F, Fut>(f: F) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(f(tx));
    futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
}

async fn count_tokens(State(st): State<Arc<AppState>>, Path((route_id, _profile)): Path<(String, String)>, headers: HeaderMap, body: Bytes) -> Response {
    if !authorized(&st, &headers) {
        return err(StatusCode::UNAUTHORIZED, "authentication_error", "missing or wrong router token");
    }
    let reg = st.ctx.registry().unwrap_or_default();
    let mut req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let model = req["model"].as_str().unwrap_or("").to_string();
    // the first provider the route would send this model to, with that provider's own model id (aggregate
    // routes name models `hm-<provider>--<model>`, which no provider knows)
    let first = reg.routes.iter().find(|r| r.id == route_id).and_then(|r| candidates(&reg, r, &model).ok()).and_then(|c| c.into_iter().next());
    if let Some((p, m)) = first.filter(|(p, _)| p.format == "anthropic") {
        req["model"] = json!(m);
        let key = hrm_core::secrets::get_key(&hrm_core::secrets::provider_target(&p.id)).unwrap_or_default();
        let mut call = st.client.post(format!("{}/v1/messages/count_tokens", p.base_url.trim_end_matches('/'))).header("x-api-key", key)
            .header("anthropic-version", headers.get("anthropic-version").and_then(|v| v.to_str().ok()).unwrap_or("2023-06-01"))
            .header("content-type", "application/json").body(req.to_string());
        if let Some(b) = headers.get("anthropic-beta").and_then(|v| v.to_str().ok()) { call = call.header("anthropic-beta", b); }
        if let Ok(r) = call.send().await {
            if r.status().is_success() {
                if let Ok(v) = r.json::<Value>().await { return Json(v).into_response(); }
            }
        }
    }
    // shortcut: rough estimate (4 characters a token) when the provider cannot count
    Json(json!({"input_tokens": body.len() / 4})).into_response()
}

async fn models(State(st): State<Arc<AppState>>, Path((route_id, _profile)): Path<(String, String)>, headers: HeaderMap) -> Response {
    if !authorized(&st, &headers) {
        return err(StatusCode::UNAUTHORIZED, "authentication_error", "missing or wrong router token");
    }
    let reg = st.ctx.registry().unwrap_or_default();
    let Some(route) = reg.routes.iter().find(|r| r.id == route_id) else { return err(StatusCode::NOT_FOUND, "not_found_error", "no such route") };
    Json(json!({"data": model_list(&reg, route), "has_more": false})).into_response()
}

/// The models a route offers. Aggregate routes prefix each with its provider so every id names one provider.
pub fn model_list(reg: &hrm_core::Registry, route: &Route) -> Vec<Value> {
    let mut out = vec![];
    for pid in &route.providers {
        let Some(p) = reg.providers.iter().find(|p| &p.id == pid) else { continue };
        for m in &p.models {
            let id = if route.mode == "aggregate" { format!("hm-{}--{m}", p.id) } else { m.clone() };
            out.push(json!({"type": "model", "id": id, "display_name": format!("{m} ({})", p.name)}));
        }
    }
    out
}

