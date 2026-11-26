//! The companion API (`/c1`) a paired phone reads and acts through, served by the direct connection's listener
//! with mutual TLS. Only a phone's certificate reaches it (a computer's is refused here, as a phone's is on `/v1`);
//! every request reads the phone's row again, so a revoked phone or a grant switched off holds at once. Every
//! answer carries `computer` and `observed_at`. The routes and their grants: docs/plans/mobile/PLAN.md.

pub mod actions;
pub mod conn;
mod events;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use companion_proto::api::{self, Grants};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::phones::{self, Phone};
use crate::service::{self, SessionFilter};
use crate::util::now_ms;
use crate::{hooks, recommend, Ctx};

use conn::Caller;

/// Sessions a page of `/c1/sessions` holds.
const SESSIONS_PER_PAGE: i64 = 50;
/// A transcript page: at most this many turns, and no more turns once this many bytes are taken.
const TURNS_PER_PAGE: usize = 20;
const PAGE_BYTES: usize = 512 * 1024;

pub(crate) fn computer(ctx: &Ctx) -> api::Computer { api::Computer { id: ctx.settings.device_id.clone(), name: ctx.settings.device_name.clone() } }

/// `computer` and `observed_at` added to an answer.
fn stamped(ctx: &Ctx, mut v: Value) -> Value {
    v["computer"] = json!(computer(ctx));
    v["observed_at"] = json!(now_ms());
    v
}

/// The database and file reads are blocking work: off the runtime's threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T { tokio::task::spawn_blocking(f).await.expect("blocking work") }

fn answer(r: anyhow::Result<Value>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({"error": format!("{e:#}")}))).into_response(),
    }
}

fn forbidden(why: &str) -> Response { (StatusCode::FORBIDDEN, Json(json!({"error": why}))).into_response() }

/// Phones only, and only one that is not revoked: its row goes beside the request.
async fn phone_only(State(ctx): State<Arc<Ctx>>, caller: Option<Extension<Caller>>, mut req: Request, next: Next) -> Response {
    let Some(Extension(Caller::Phone { id, generation })) = caller else { return forbidden("the companion API is for paired phones") };
    let p = blocking(move || { let p = phones::get(&ctx, &id).ok().flatten(); if p.is_some() { phones::seen(&ctx, &id); } p }).await;
    match p {
        // the enrollment this connection showed its certificate for, not one paired again under the same id since
        Some(p) if p.revoked_at.is_none() && p.generation == generation => { req.extensions_mut().insert(p); next.run(req).await }
        _ => forbidden("this phone was revoked or paired again"),
    }
}

/// Paired computers only (the `/v1` routes).
pub(crate) async fn peers_only(caller: Option<Extension<Caller>>, req: Request, next: Next) -> Response {
    match caller { Some(Extension(Caller::Peer)) => next.run(req).await, _ => forbidden("these routes are for paired computers") }
}

/// A layer that lets a request through when the phone holds the grant.
fn need(grant: fn(&Grants) -> bool, name: &'static str) -> impl Fn(Extension<Phone>, Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Clone {
    move |Extension(p): Extension<Phone>, req: Request, next: Next| {
        Box::pin(async move { if grant(&p.grants) { next.run(req).await } else { forbidden(&format!("{name} is not granted to this phone")) } })
    }
}

/// The `/c1` routes, each behind its grant and all behind `phone_only`.
pub fn router(ctx: Arc<Ctx>) -> Router {
    let any = Router::new().route("/c1/hello", get(hello)).route("/c1/push", put(push_put).delete(push_delete));
    let summaries = Router::new()
        .route("/c1/overview", get(overview))
        .route("/c1/accounts", get(accounts))
        .route("/c1/accounts/{profile}", get(account))
        .route("/c1/sessions", get(sessions))
        .route("/c1/requests", get(requests))
        .route("/c1/events", get(events::events))
        .route_layer(middleware::from_fn(need(|g| g.summaries, "Summaries")));
    let transcripts = Router::new().route("/c1/sessions/{harness}/{profile}/{id}/transcript", get(transcript)).route_layer(middleware::from_fn(need(|g| g.transcripts, "Transcripts")));
    let control = Router::new().route("/c1/actions", post(actions::post)).route("/c1/actions/{action_id}", get(actions::get)).route_layer(middleware::from_fn(need(|g| g.control, "Control")));
    any.merge(summaries).merge(transcripts).merge(control).route_layer(middleware::from_fn_with_state(ctx.clone(), phone_only)).with_state(ctx)
}

async fn hello(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Json<api::Hello> {
    let now = now_ms();
    Json(api::Hello { computer: computer(&ctx), observed_at: now, app_version: env!("CARGO_PKG_VERSION").into(), api: api::API, grants: p.grants, server_time: now })
}

/// What this phone may do with requests now, and why not.
fn may_answer(p: &Phone) -> api::Allowed { api::Allowed { ok: p.grants.control, reason: (!p.grants.control).then(|| "Control is not granted to this phone; grant it on the computer".into()) } }

/// The day's tokens and requests on this computer.
fn today(ctx: &Ctx) -> anyhow::Result<Value> {
    let day = service::last_days(1).remove(0);
    ctx.db.read(|c| Ok(c.query_row("select coalesce(sum(input + output + cache_read + cache_write), 0), count(*) from usage where day = ?1 and device = ?2",
        rusqlite::params![day, ctx.db.device], |r| Ok(json!({"tokens": r.get::<_, i64>(0)?, "requests": r.get::<_, i64>(1)?})))?))
}

pub(crate) fn overview_of(ctx: &Ctx) -> anyhow::Result<api::Overview> {
    let now = now_ms();
    let runs = hooks::runs(ctx)?;
    let asks: Vec<&hooks::Ask> = runs.iter().filter_map(|r| r.ask.as_ref()).collect();
    let views = service::list_views(ctx)?;
    let listed = recommend::listed(ctx);
    let views: Vec<service::ProfileView> = views.into_iter().filter(|p| listed.contains(&p.harness)).collect();
    let next_reset = views.iter().flat_map(|p| p.quota.iter().flat_map(|q| &q.tiers).filter_map(move |t| t.resets_at.filter(|r| *r > now).map(|r| (r, p, t))))
        .min_by_key(|x| x.0).map(|(at, p, t)| json!({"profile": p.name, "name": p.label, "window": t.label, "at": at}));
    Ok(api::Overview {
        computer: computer(ctx), observed_at: now, needs_you: asks.len() as u32, soonest_expiry: asks.iter().map(|a| a.expires).min(),
        running: views.iter().map(|p| p.running.len() as u32).sum(), next_reset, today: today(ctx)?, use_next: recommend::use_next(ctx, &views, now)?,
    })
}

async fn overview(State(ctx): State<Arc<Ctx>>) -> Response { answer(blocking(move || overview_of(&ctx).and_then(|o| Ok(serde_json::to_value(o)?))).await) }

fn accounts_of(ctx: &Ctx, only: Option<&str>) -> anyhow::Result<Vec<api::Account>> {
    let now = now_ms();
    let listed = recommend::listed(ctx);
    let views: Vec<service::ProfileView> = service::list_views(ctx)?.into_iter().filter(|p| listed.contains(&p.harness) && only.is_none_or(|n| p.name == n)).collect();
    let forecast = crate::dash::forecasts(ctx, now)?;
    Ok(views.iter().map(|p| {
        let a = recommend::acct(p, &forecast, now);
        let plan = match p.kind { crate::registry::Kind::ApiKey => Some("API key".to_string()), crate::registry::Kind::Routed => Some("Routed".to_string()), _ => p.identity.plan.clone() };
        let read_at = p.quota.as_ref().and_then(|q| q.fetched_at);
        api::Account { profile: p.name.clone(), name: p.label.clone(), tool: p.harness.clone(), plan, windows: a.windows.iter().map(|w| api::Window { id: w.id.clone(), label: w.label.clone(), left: w.left, resets_at: w.resets_at }).collect(),
            read_at, reading_age_ms: read_at.map(|t| (now - t).max(0)), login_attention: a.needs_login, tokens_7d: p.tokens_7d }
    }).collect())
}

async fn accounts(State(ctx): State<Arc<Ctx>>) -> Response {
    answer(blocking(move || Ok(stamped(&ctx, json!({"accounts": accounts_of(&ctx, None)?}))) ).await)
}

async fn account(State(ctx): State<Arc<Ctx>>, Path(profile): Path<String>) -> Response {
    answer(blocking(move || {
        let a = accounts_of(&ctx, Some(&profile))?.into_iter().next().ok_or_else(|| anyhow::anyhow!("no account is called {profile}"))?;
        Ok(stamped(&ctx, json!({"account": a})))
    }).await)
}

#[derive(Deserialize)]
struct Cursor {
    cursor: Option<i64>,
}

/// Reply and Stop for a run, by today's rules: an owned terminal; no reply while a prompt waits; Stop only while a
/// turn runs or a prompt waits (at the idle prompt Ctrl+C would exit the tool).
pub(crate) fn run_actions(p: &Phone, r: &hooks::Run) -> Value {
    let no = |why: &str| json!({"ok": false, "reason": why});
    if !p.grants.control { let n = no("Control is not granted to this phone; grant it on the computer"); return json!({"reply": n, "stop": n}); }
    if !r.terminal { let n = no("Needs Harness Manager's own terminal"); return json!({"reply": n, "stop": n}); }
    if r.stopping { let n = no("a Stop is already on its way to this session"); return json!({"reply": n, "stop": n}); }
    // an open request blocks a reply whatever the state says: the reply's Enter would answer its prompt
    json!({"reply": if r.state == "waiting" || r.ask.is_some() { no("Answer the request first") } else { json!({"ok": true}) },
        "stop": if r.state == "working" || r.state == "waiting" { json!({"ok": true}) } else { no("Nothing to stop: the session is at its prompt") }})
}

async fn sessions(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Query(q): Query<Cursor>) -> Response {
    // the page times its size is the query's offset: a cursor past that is no page
    let page = q.cursor.unwrap_or(0);
    if !(0..i64::MAX / SESSIONS_PER_PAGE).contains(&page) { return (StatusCode::BAD_REQUEST, Json(json!({"error": "no such page"}))).into_response(); }
    answer(blocking(move || {
        let f = SessionFilter { scope: Some("here".into()), per: Some(SESSIONS_PER_PAGE), page: Some(page), ..Default::default() };
        let list = service::sessions(&ctx, &f)?;
        let runs = hooks::runs(&ctx)?;
        let me = computer(&ctx);
        let items: Vec<Value> = list.iter().map(|s| {
            let run = runs.iter().find(|r| !s.id.is_empty() && r.session_id == s.id && r.profile == s.profile);
            let mut v = json!(api::Session { harness: s.harness.clone(), profile: s.profile.clone(), id: s.id.clone(), title: s.title.clone(), project: s.project_name.clone(), computer: me.clone(),
                state: run.map(|r| r.state.clone()).unwrap_or_default(), open: s.open, last_active_at: s.last_active_at });
            if let Some(r) = run { v["run"] = json!(r.run); v["state_version"] = json!(r.version); v["actions"] = run_actions(&p, r); }
            v
        }).collect();
        let next = (items.len() as i64 == SESSIONS_PER_PAGE).then_some(page + 1);
        Ok(stamped(&ctx, json!({"sessions": items, "next_cursor": next})))
    }).await)
}

#[derive(Deserialize)]
struct After {
    after: Option<usize>,
}

/// The transcript last read, by its file's path and signature: a phone pages through one session, and each page
/// would otherwise read and parse the whole file again. The signature takes the write-ahead log beside the file too:
/// OpenCode writes there while its database file stays as it was.
/// shortcut: one transcript kept; a small cache per phone if several phones page different sessions at once
type Sig = [Option<(i64, u64)>; 2];
static LAST: std::sync::Mutex<Option<(String, Sig, Arc<crate::model::Transcript>)>> = std::sync::Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// Test seams: run between choosing this computer's row and reading it; count the reads.
    pub(crate) static BEFORE_READ: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
    pub(crate) static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// This computer's own copy of a session, read from the very row chosen (another computer's session of the same id
/// is that computer's to serve), and kept only once it is seen to be that row's.
fn transcript_of(ctx: &Ctx, harness: &str, profile: &str, id: &str) -> anyhow::Result<Arc<crate::model::Transcript>> {
    let f = SessionFilter { profile: Some(profile.into()), id: Some(id.into()), scope: Some("here".into()), ..Default::default() };
    let here = |s: &crate::model::SessionMeta| s.harness == harness && s.id == id && s.profile == profile && (s.device.is_empty() || s.device == ctx.db.device);
    let s = service::sessions(ctx, &f)?.into_iter().find(here).ok_or_else(|| anyhow::anyhow!("no such session"))?;
    let path = std::path::Path::new(&s.source_path);
    let sig: Sig = [crate::util::file_sig(path), crate::util::file_sig(&std::path::PathBuf::from(format!("{}-wal", s.source_path)))];
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, g, tr)) = last.as_ref() { if p == &s.source_path && g == &sig && here(&tr.meta) { return Ok(tr.clone()); } }
    #[cfg(test)]
    if let Some(f) = BEFORE_READ.with(|b| b.borrow_mut().take()) { f(); }
    let reg = ctx.registry()?;
    let p = reg.get(profile)?;
    let mut t = if s.archived { crate::archive::transcript(&s)? } else { crate::harness::of(p).transcript(ctx, p, &s)? };
    t.meta = s;
    if !here(&t.meta) { anyhow::bail!("no such session"); }
    #[cfg(test)]
    READS.with(|r| r.set(r.get() + 1));
    let t = Arc::new(t);
    *last = Some((t.meta.source_path.clone(), sig, t.clone()));
    Ok(t)
}

/// What a page holds besides its turns (names, title cut to 200 characters, the counts): kept out of the turns' budget,
/// so a whole page stays within `PAGE_BYTES`.
const ENVELOPE: usize = 4 * 1024;

/// A turn that fits a page: its long texts cut to 2,000 characters, then its steps left out, then its answers after
/// the first 20 (with their places), and at last only the prompt; `truncated` says it was cut.
fn bounded(mut v: Value) -> Value {
    fn cut(v: &mut Value) {
        match v {
            Value::String(t) if t.chars().count() > 2000 => { *t = t.chars().take(2000).collect::<String>() + "…"; }
            Value::Array(a) => a.iter_mut().for_each(cut),
            Value::Object(o) => o.values_mut().for_each(cut),
            _ => {}
        }
    }
    let fits = |v: &Value| v.to_string().len() <= PAGE_BYTES - ENVELOPE;
    if fits(&v) { return v; }
    v["truncated"] = json!(true);
    cut(&mut v);
    if !fits(&v) { let n = v["steps"].as_array().map(Vec::len).unwrap_or(0); v["steps"] = json!([]); v["steps_left_out"] = json!(n); }
    if !fits(&v) {
        let n = v["answer"].as_array().map(Vec::len).unwrap_or(0);
        for k in ["answer", "answer_pos"] { if let Some(a) = v[k].as_array_mut() { a.truncate(20); } }
        v["answers_left_out"] = json!(n.saturating_sub(20));
    }
    if !fits(&v) {
        v = json!({"prompt": v["prompt"], "prompt_at": v["prompt_at"], "answer_at": v["answer_at"], "model": v["model"], "steps": [], "answer": [], "answer_pos": [], "truncated": true});
    }
    v
}

/// One page of a transcript from turn `from`: at most `TURNS_PER_PAGE` turns, all of it within `PAGE_BYTES`.
fn page(ctx: &Ctx, harness: &str, profile: &str, id: &str, t: &crate::model::Transcript, from: usize) -> anyhow::Result<Value> {
    let from = from.min(t.turns.len());
    let (mut turns, mut bytes) = (vec![], 0);
    for turn in &t.turns[from..] {
        let v = bounded(serde_json::to_value(turn)?);
        let n = v.to_string().len() + 1;
        if turns.len() >= TURNS_PER_PAGE || (!turns.is_empty() && bytes + n > PAGE_BYTES - ENVELOPE) { break; }
        bytes += n;
        turns.push(v);
    }
    let end = from + turns.len();
    let title: String = t.meta.title.chars().take(200).collect();
    Ok(stamped(ctx, json!({"harness": harness, "profile": profile, "id": id, "title": title, "after": from, "turns": turns, "next": (end < t.turns.len()).then_some(end), "total": t.turns.len()})))
}

async fn transcript(State(ctx): State<Arc<Ctx>>, Path((harness, profile, id)): Path<(String, String, String)>, Query(q): Query<After>) -> Response {
    answer(blocking(move || {
        let t = transcript_of(&ctx, &harness, &profile, &id)?;
        page(&ctx, &harness, &profile, &id, &t, q.after.unwrap_or(0))
    }).await)
}

/// What a phone is shown of a request's tool input: a question's text and its options; nothing of a permission's
/// (a file's new content, say), whose command or file is already in `what`.
fn summary(kind: &str, input: &Value) -> Value {
    if kind != "question" { return json!({}); }
    let qs: Vec<Value> = input["questions"].as_array().into_iter().flatten().map(|q| json!({
        "question": q["question"].as_str().unwrap_or(""), "multiSelect": q["multiSelect"] == true,
        "options": q["options"].as_array().into_iter().flatten().map(|o| json!({"label": o["label"].as_str().unwrap_or(""), "description": o["description"].as_str().unwrap_or("")})).collect::<Vec<_>>(),
    })).collect();
    json!({"questions": qs})
}

/// The open requests, each with the state version an answer must name.
pub(crate) fn requests_of(ctx: &Ctx) -> anyhow::Result<Vec<api::Request>> {
    Ok(hooks::runs(ctx)?.into_iter().filter_map(|r| r.ask.clone().map(|a| api::Request {
        input: summary(&a.kind, &a.input), run: r.run.clone(), ask_id: a.id, kind: a.kind, tool: a.tool, what: a.what, profile: r.profile.clone(), expires: a.expires, state_version: r.version,
    })).collect())
}

async fn requests(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Response {
    answer(blocking(move || Ok(stamped(&ctx, json!({"requests": requests_of(&ctx)?, "actions": {"answer": may_answer(&p)}})))).await)
}

async fn push_put(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Json(h): Json<api::PushHandle>) -> Response {
    let h = h.relay_handle.trim().to_string();
    if h.is_empty() || h.len() > 200 { return (StatusCode::BAD_REQUEST, Json(json!({"error": "give the relay handle"}))).into_response(); }
    answer(blocking(move || { phones::set_push(&ctx, &p, Some(&h))?; Ok(stamped(&ctx, json!({"ok": true}))) }).await)
}

async fn push_delete(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Response {
    answer(blocking(move || { phones::set_push(&ctx, &p, None)?; Ok(stamped(&ctx, json!({"ok": true}))) }).await)
}
