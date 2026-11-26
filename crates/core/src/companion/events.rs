//! `GET /c1/events`: a WebSocket that tells the phone what changed, so it reads again instead of polling. Every
//! second the computer looks at its open requests (`needs`), each run's state (`run-state`) and the newest usage row
//! and limits reading (`changed`; the usage by rowid, a look at the end of the table and not a scan). A phone revoked
//! or without Summaries any more is told `revoked` and the socket closes.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::Extension;
use companion_proto::api::Event;
use serde_json::{json, Value};

use crate::phones::{self, Phone};
use crate::util::now_ms;
use crate::{hooks, Ctx};

pub(super) async fn events(ws: WebSocketUpgrade, State(ctx): State<Arc<Ctx>>, Extension(p): Extension<Phone>) -> Response {
    // the phone only answers pings and closes: nothing it sends needs more than a small frame
    ws.max_message_size(16 * 1024).max_frame_size(16 * 1024).on_upgrade(move |socket| run(socket, ctx, p))
}

/// What the phone is told about: the open requests, each run's state, a stamp of usage and limits; None once the
/// phone may no longer listen.
struct Look {
    needs: Value,
    runs: Vec<(String, String, i64)>,
    changed: (i64, i64),
}

fn look(ctx: &Ctx, phone: &Phone) -> Option<Look> {
    phones::get(ctx, &phone.id).ok().flatten().filter(|p| p.revoked_at.is_none() && p.generation == phone.generation && p.grants.summaries)?;
    let runs = hooks::runs(ctx).unwrap_or_default();
    let now = now_ms();
    let needs: Vec<Value> = runs.iter().filter_map(|r| r.ask.as_ref().map(|a| {
        let (expires, at_computer) = super::phone_view(a, now);
        json!(companion_proto::api::Need { run: r.run.clone(), ask_id: a.id.clone(), expires, state_version: r.version, at_computer })
    })).collect();
    let changed = ctx.db.read(|c| Ok(c.query_row("select (select coalesce(max(rowid), 0) from usage), (select coalesce(max(updated_at), 0) from quota)", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap_or((0, 0));
    Some(Look { needs: json!(needs), runs: runs.iter().map(|r| (r.run.clone(), r.state.clone(), r.version)).collect(), changed })
}

/// What the `revoked` event says: when the computer removed the phone, why (removed, paired again, Summaries
/// withdrawn), and which computer.
pub(super) fn revoked_data(ctx: &Ctx, phone: &Phone) -> Value {
    let row = phones::get(ctx, &phone.id).ok().flatten();
    let why = match &row {
        Some(r) if r.revoked_at.is_some() => "revoked",
        Some(r) if r.generation != phone.generation => "paired again",
        Some(_) => "summaries withdrawn",
        None => "removed",
    };
    json!({"revoked_at": row.as_ref().and_then(|r| r.revoked_at), "why": why, "by": super::computer(ctx)})
}

fn event(kind: &str, data: Value) -> Message { Message::Text(serde_json::to_string(&Event { kind: kind.into(), at: now_ms(), data }).unwrap_or_default().into()) }

async fn run(mut socket: WebSocket, ctx: Arc<Ctx>, phone: Phone) {
    let mut last: Option<Look> = None;
    loop {
        let (c, id) = (ctx.clone(), phone.clone());
        let Some(now) = super::blocking(move || look(&c, &id)).await else {
            let (c, id) = (ctx.clone(), phone.clone());
            let data = super::blocking(move || revoked_data(&c, &id)).await;
            let _ = socket.send(event("revoked", data)).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        };
        let mut out = vec![];
        // the count is what a phone may answer; a request past its window is listed, marked for the computer
        let count = now.needs.as_array().map(|n| n.iter().filter(|r| r.get("at_computer").is_none()).count()).unwrap_or(0);
        if last.as_ref().is_none_or(|l| l.needs != now.needs) { out.push(event("needs", json!({"count": count, "requests": now.needs}))); }
        for r in &now.runs {
            if last.as_ref().is_none_or(|l| !l.runs.contains(r)) { out.push(event("run-state", json!({"run": r.0, "state": r.1, "state_version": r.2}))); }
        }
        for r in last.iter().flat_map(|l| &l.runs).filter(|r| !now.runs.iter().any(|n| n.0 == r.0)) { out.push(event("run-state", json!({"run": r.0, "state": "ended", "state_version": r.2}))); }
        if last.as_ref().is_some_and(|l| l.changed != now.changed) { out.push(event("changed", json!({}))); }
        for m in out { if socket.send(m).await.is_err() { return; } }
        last = Some(now);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            m = socket.recv() => match m { Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return, _ => {} },
        }
    }
}
