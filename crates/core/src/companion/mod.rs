//! The companion API (`/c1`) a paired phone reads and acts through, served by the direct connection's listener
//! with mutual TLS. Only a phone's certificate reaches it (a computer's is refused here, as a phone's is on `/v1`);
//! every request reads the phone's row again, so a revoked phone or a grant switched off holds at once. Every
//! answer carries `computer` and `observed_at`. The routes and their grants: docs/plans/mobile/PLAN.md.

pub mod actions;
pub mod conn;
mod events;
mod history;
pub mod mailbox;
#[cfg(test)]
mod mailbox_tests;
pub mod push;
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
        .route("/c1/projects/{key}", get(project))
        .route("/c1/sessions", get(sessions))
        .route("/c1/requests", get(requests))
        .route("/c1/events", get(events::events))
        .route_layer(middleware::from_fn(need(|g| g.summaries, "Summaries")));
    let transcripts = Router::new().route("/c1/sessions/{harness}/{profile}/{id}/transcript", get(transcript)).route_layer(middleware::from_fn(need(|g| g.transcripts, "Transcripts")));
    let control = Router::new().route("/c1/actions", post(actions::post)).route("/c1/actions/{action_id}", get(actions::get)).route_layer(middleware::from_fn(need(|g| g.control, "Control")));
    any.merge(summaries).merge(transcripts).merge(control).route_layer(middleware::from_fn_with_state(ctx.clone(), phone_only)).with_state(ctx)
}

async fn hello(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Json<api::Hello> { Json(blocking(move || hello_of(&ctx, &p)).await) }

pub(crate) fn hello_of(ctx: &Ctx, p: &Phone) -> api::Hello {
    // the other computers' names and systems are a summary: only with Summaries
    let computers = if p.grants.summaries {
        crate::journal::peers(ctx).unwrap_or_default().into_iter().map(|k| api::KnownComputer { id: k.id, name: k.name, os: k.os, last_seen: k.last_seen }).collect()
    } else { vec![] };
    let now = now_ms();
    api::Hello { computer: computer(ctx), observed_at: now, app_version: env!("CARGO_PKG_VERSION").into(), api: api::API, grants: p.grants, server_time: now, computers, push_needs_reregistering: p.push_needs_reregistering }
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
    let asks: Vec<(i64, Option<String>)> = runs.iter().filter_map(|r| r.ask.as_ref()).map(|a| phone_view(a, now)).collect();
    let answerable: Vec<i64> = asks.iter().filter(|a| a.1.is_none()).map(|a| a.0).collect();
    let views = service::list_views(ctx)?;
    let listed = recommend::listed(ctx);
    let views: Vec<service::ProfileView> = views.into_iter().filter(|p| listed.contains(&p.harness)).collect();
    // a session runs when the desktop sees its console or its `hrm run` reports it (working, waiting or at its
    // prompt); one seen both ways, by its session or its folder under the same account, counts once
    let consoles: Vec<(&str, &crate::model::RunningConsole)> = views.iter().flat_map(|p| p.running.iter().map(move |c| (p.name.as_str(), c))).collect();
    let same = |r: &hooks::Run, (profile, c): &(&str, &crate::model::RunningConsole)| *profile == r.profile
        && (c.session_id.as_deref().is_some_and(|s| !s.is_empty() && s == r.session_id) || (c.session_id.is_none() && c.cwd.as_deref().is_some_and(|d| !d.is_empty() && d == r.cwd)));
    let running = consoles.len() + runs.iter().filter(|r| !consoles.iter().any(|c| same(r, c))).count();
    let here = service::overview(ctx, Some(30), None, Some("here"))?;
    let daily_tokens = here.heatmap[here.heatmap.len().saturating_sub(30)..].iter().map(|d| api::DayTokens { day: d.day.clone(), tokens: d.tokens }).collect();
    let next_reset = views.iter().flat_map(|p| p.quota.iter().flat_map(|q| &q.tiers).filter_map(move |t| t.resets_at.filter(|r| *r > now).map(|r| (r, p, t))))
        .min_by_key(|x| x.0).map(|(at, p, t)| json!({"profile": p.name, "name": p.label, "window": t.label, "at": at}));
    Ok(api::Overview {
        computer: computer(ctx), observed_at: now, needs_you: answerable.len() as u32, soonest_expiry: answerable.iter().copied().min(), at_computer: (asks.len() - answerable.len()) as u32,
        running: running as u32, next_reset, today: today(ctx)?, use_next: recommend::use_next(ctx, &views, now)?, today_cost: here.today_cost, daily_tokens,
        request_days: Some(history::request_days(ctx)?),
    })
}

async fn overview(State(ctx): State<Arc<Ctx>>) -> Response { answer(blocking(move || overview_of(&ctx).and_then(|o| Ok(serde_json::to_value(o)?))).await) }

fn accounts_of(ctx: &Ctx, only: Option<&str>) -> anyhow::Result<Vec<api::Account>> {
    let now = now_ms();
    let listed = recommend::listed(ctx);
    let views: Vec<service::ProfileView> = service::list_views(ctx)?.into_iter().filter(|p| listed.contains(&p.harness) && only.is_none_or(|n| p.name == n)).collect();
    let forecast = crate::dash::forecasts(ctx, now)?;
    views.iter().map(|p| {
        let a = recommend::acct(p, &forecast, now);
        let plan = match p.kind { crate::registry::Kind::ApiKey => Some("API key".to_string()), crate::registry::Kind::Routed => Some("Routed".to_string()), _ => p.identity.plan.clone() };
        let read_at = p.quota.as_ref().and_then(|q| q.fetched_at);
        let windows = a.windows.iter().map(|w| {
            let at_reset = forecast.get(&format!("{}|{}", p.name, w.id)).and_then(|f| f["at_reset"].as_f64()).filter(|_| w.open);
            api::Window { id: w.id.clone(), label: w.label.clone(), left: w.left, resets_at: w.resets_at, hours: window_hours(&w.id),
                left_at_reset: at_reset.map(|u| (100.0 - u).clamp(0.0, 100.0).round() as i64), runs_out_at: w.out_ms.map(|m| now + m) }
        }).collect();
        let (past_resets, run_outs) = history::quota(ctx, &p.name, p.identity.email.as_deref().or(p.expected_email.as_deref()), now)?;
        Ok(api::Account { profile: p.name.clone(), name: p.label.clone(), tool: p.harness.clone(), plan, windows,
            read_at, reading_age_ms: read_at.map(|t| (now - t).max(0)), login_attention: a.needs_login, tokens_7d: p.tokens_7d,
            past_resets: Some(past_resets), run_outs: Some(run_outs) })
    }).collect()
}

/// A limit window's length from its id: the 5-hour window, a week (the account's or a model's), or Codex's
/// `window:<minutes>`; None for one the id does not say.
fn window_hours(id: &str) -> Option<f64> {
    let (base, rest) = id.split_once(':').unwrap_or((id, ""));
    match base {
        "session" => Some(5.0),
        "week" => Some(168.0),
        "window" => rest.parse::<f64>().ok().map(|m| m / 60.0),
        _ => None,
    }
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

async fn project(State(ctx): State<Arc<Ctx>>, Path(key): Path<String>) -> Response {
    answer(blocking(move || Ok(serde_json::to_value(history::project(&ctx, &key)?)?)).await)
}

/// Reply and Stop for a run: not from a phone in wave 1.
pub(crate) fn run_actions() -> Value {
    let n = json!({"ok": false, "reason": actions::NOT_YET});
    json!({"reply": n, "stop": n})
}

async fn sessions(State(ctx): State<Arc<Ctx>>, Extension(_p): Extension<Phone>, Query(q): Query<Cursor>) -> Response {
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
            let mut v = json!(api::Session { harness: s.harness.clone(), profile: s.profile.clone(), id: s.id.clone(), title: s.title.clone(), project: s.project_name.clone(),
                project_key: (!s.project_key.is_empty()).then(|| history::project_key(&s.project_key)), computer: me.clone(),
                state: run.map(|r| r.state.clone()).unwrap_or_default(), open: s.open, last_active_at: s.last_active_at });
            if let Some(r) = run { v["run"] = json!(r.run); v["state_version"] = json!(r.version); v["actions"] = run_actions(); }
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
    // whether this read may be kept is decided before it starts: a file written in the last two seconds can change
    // again without its size or time showing it (OpenCode's log keeps its size), however long the read takes
    let keep = sig.iter().all(|s| crate::util::still(*s));
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
    // and it is kept only when the files are still as they were when it began
    let after: Sig = [crate::util::file_sig(std::path::Path::new(&t.meta.source_path)), crate::util::file_sig(&std::path::PathBuf::from(format!("{}-wal", t.meta.source_path)))];
    if keep && after == sig { *last = Some((t.meta.source_path.clone(), sig, t.clone())); }
    Ok(t)
}

/// A turn that fits `budget` bytes: its long texts cut to 2,000 characters, then its steps left out, then its answers
/// after the first 20 (with their places), and at last only its prompt, cut to fit; `truncated` says it was cut.
fn bounded(mut v: Value, budget: usize) -> Value {
    fn cut(v: &mut Value, chars: usize) {
        match v {
            Value::String(t) if t.chars().count() > chars => { *t = t.chars().take(chars).collect::<String>() + "…"; }
            Value::Array(a) => a.iter_mut().for_each(|x| cut(x, chars)),
            Value::Object(o) => o.values_mut().for_each(|x| cut(x, chars)),
            _ => {}
        }
    }
    let fits = |v: &Value| v.to_string().len() <= budget;
    if fits(&v) { return v; }
    v["truncated"] = json!(true);
    cut(&mut v, 2000);
    if !fits(&v) { let n = v["steps"].as_array().map(Vec::len).unwrap_or(0); v["steps"] = json!([]); v["steps_left_out"] = json!(n); }
    if !fits(&v) {
        let n = v["answer"].as_array().map(Vec::len).unwrap_or(0);
        for k in ["answer", "answer_pos"] { if let Some(a) = v[k].as_array_mut() { a.truncate(20); } }
        v["answers_left_out"] = json!(n.saturating_sub(20));
    }
    if !fits(&v) {
        // a character takes at most 6 bytes in JSON (``): the prompt alone, cut to what the budget holds
        let mut p = json!({"prompt": v["prompt"], "prompt_at": v["prompt_at"], "answer_at": v["answer_at"], "model": v["model"], "steps": [], "answer": [], "answer_pos": [], "truncated": true});
        // measured, and the prompt and the model cut shorter until the turn fits; at last both go
        let mut chars = 2000;
        while !fits(&p) && chars > 0 {
            for k in ["prompt", "model"] { cut(&mut p[k], chars); }
            chars /= 2;
        }
        if !fits(&p) { p["prompt"] = json!(""); p["model"] = json!(""); }
        v = p;
    }
    v
}

/// One page of a transcript from turn `from`: at most `TURNS_PER_PAGE` turns, the whole answer within `PAGE_BYTES`.
/// The page's own fields are measured as they will be written (names come from the session, of any length), and the
/// turns get what is left.
fn page(ctx: &Ctx, harness: &str, profile: &str, id: &str, t: &crate::model::Transcript, from: usize) -> anyhow::Result<Value> {
    let from = from.min(t.turns.len());
    let title: String = t.meta.title.chars().take(200).collect();
    let envelope = |turns: Vec<Value>, next: Option<usize>| stamped(ctx, json!({"harness": harness, "profile": profile, "id": id, "title": title, "after": from, "turns": turns, "next": next, "total": t.turns.len()}));
    // the largest the fields can be: `next` at its longest, and room for the clock's digits to grow
    let budget = PAGE_BYTES.saturating_sub(envelope(vec![], Some(usize::MAX)).to_string().len() + 32);
    if budget < 4096 { anyhow::bail!("this session's names are too long to page"); }
    let (mut turns, mut bytes) = (vec![], 0);
    for turn in &t.turns[from..] {
        let v = bounded(serde_json::to_value(turn)?, budget);
        let n = v.to_string().len() + 1;
        if turns.len() >= TURNS_PER_PAGE || (!turns.is_empty() && bytes + n > budget) { break; }
        bytes += n;
        turns.push(v);
    }
    let end = from + turns.len();
    Ok(envelope(turns, (end < t.turns.len()).then_some(end)))
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
/// When a request stops being a phone's to answer: its own expiry, or `MAX_REQUEST_SECONDS` after it opened.
pub(crate) fn phone_expiry(created: i64, expires: i64) -> i64 { expires.min(created + i64::from(hooks::MAX_REQUEST_SECONDS) * 1000) }

pub(crate) const AT_COMPUTER: &str = "Answer it at the computer: a phone answers a request within 5 minutes of its opening";

/// A request as every phone view shows it (list, `needs` event, overview): its phone expiry, and why it is the
/// computer's to answer once that passed.
pub(crate) fn phone_view(a: &hooks::Ask, now: i64) -> (i64, Option<String>) {
    let expires = phone_expiry(a.created, a.expires);
    (expires, (expires <= now).then(|| AT_COMPUTER.to_string()))
}

/// How long a question whose window ended stays listed, as expired.
const ENDED_KEPT_MS: i64 = 10 * 60_000;

/// The open requests, then the questions whose window ended in the last ten minutes. Claude's last line before a
/// request is transcript content: given only with `transcripts`.
pub(crate) fn requests_of(ctx: &Ctx, transcripts: bool) -> anyhow::Result<Vec<api::Request>> {
    let now = now_ms();
    let runs = hooks::runs(ctx)?;
    let ended = hooks::ended_questions(ctx, now - ENDED_KEPT_MS)?;
    let open = runs.iter().filter_map(|r| r.ask.as_ref().map(|a| (r, a, false)));
    let late = ended.iter().filter_map(|(run, a)| runs.iter().find(|r| &r.run == run).map(|r| (r, a, true)));
    Ok(open.chain(late).map(|(r, a, ended)| request(ctx, r, a, now, transcripts, ended)).collect())
}

/// One request as a phone is shown it: what it asks (a permission's command or file, and its description; a
/// question's text and options; nothing else of the tool's input), when it opened, the session (its folder, its
/// title and Claude's last line before it when its transcript is here; a session without one is still named).
fn request(ctx: &Ctx, r: &hooks::Run, a: &hooks::Ask, now: i64, transcripts: bool, ended: bool) -> api::Request {
    let (expires, at_computer) = if ended { (a.expires, None) } else { phone_view(a, now) };
    let t = (!r.session_id.is_empty()).then(|| transcript_of(ctx, &r.harness, &r.profile, &r.session_id).ok()).flatten();
    let title = t.as_ref().map(|t| t.meta.title.chars().take(200).collect::<String>()).filter(|s| !s.is_empty());
    let last_line = t.filter(|_| transcripts).and_then(|t| {
        let text = t.turns.last()?.answer.iter().rev().find(|s| !s.trim().is_empty())?;
        Some(text.lines().rev().map(str::trim).find(|l| !l.is_empty())?.chars().take(300).collect())
    });
    let description = (a.kind == "permission").then(|| a.input["description"].as_str()).flatten().map(|d| d.chars().take(500).collect());
    api::Request { input: summary(&a.kind, &a.input), run: r.run.clone(), ask_id: a.id.clone(), kind: a.kind.clone(), tool: a.tool.clone(), what: a.what.clone(), profile: r.profile.clone(),
        expires, state_version: r.version, at_computer, state: if ended { "expired" } else { "open" }.into(), ended_at: ended.then_some(a.expires), opened_at: a.created,
        session_id: r.session_id.clone(), cwd: r.cwd.clone(), title, last_line, description }
}

async fn requests(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Response {
    answer(blocking(move || Ok(stamped(&ctx, json!({"requests": requests_of(&ctx, p.grants.transcripts)?, "actions": {"answer": may_answer(&p)}})))).await)
}

#[derive(Deserialize)]
struct PushEnrollment { relay_handle: String, secret: Option<String> }

async fn push_put(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>, Json(h): Json<PushEnrollment>) -> Response {
    let secret = h.secret;
    if secret.as_ref().is_some_and(|s| hex::decode_to_slice(s, &mut [0; 32]).is_err()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"the relay secret must be 64 hex digits"}))).into_response();
    }
    if !phones::valid_handle(&h.relay_handle) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"the relay handle must be 48 hex digits"}))).into_response();
    }
    let h = h.relay_handle.to_ascii_lowercase();
    answer(blocking(move || { phones::set_push(&ctx, &p, Some(&h), secret.as_deref())?; Ok(stamped(&ctx, json!({"ok": true}))) }).await)
}

async fn push_delete(State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Response {
    answer(blocking(move || { phones::set_push(&ctx, &p, None, None)?; Ok(stamped(&ctx, json!({"ok": true}))) }).await)
}
