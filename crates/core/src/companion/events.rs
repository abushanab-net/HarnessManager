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
    ws.on_upgrade(move |socket| run(socket, ctx, p.id))
}

/// What the phone is told about: the open requests, each run's state, a stamp of usage and limits; None once the
/// phone may no longer listen.
struct Look {
    needs: Value,
    runs: Vec<(String, String, i64)>,
    changed: (i64, i64),
}

fn look(ctx: &Ctx, phone: &str) -> Option<Look> {
    phones::get(ctx, phone).ok().flatten().filter(|p| p.revoked_at.is_none() && p.grants.summaries)?;
    let runs = hooks::runs(ctx).unwrap_or_default();
    let needs: Vec<Value> = runs.iter().filter_map(|r| r.ask.as_ref().map(|a| json!({"run": r.run, "ask_id": a.id, "expires": a.expires, "state_version": r.version}))).collect();
    let changed = ctx.db.read(|c| Ok(c.query_row("select (select coalesce(max(rowid), 0) from usage), (select coalesce(max(updated_at), 0) from quota)", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap_or((0, 0));
    Some(Look { needs: json!(needs), runs: runs.iter().map(|r| (r.run.clone(), r.state.clone(), r.version)).collect(), changed })
}

fn event(kind: &str, data: Value) -> Message { Message::Text(serde_json::to_string(&Event { kind: kind.into(), at: now_ms(), data }).unwrap_or_default().into()) }

async fn run(mut socket: WebSocket, ctx: Arc<Ctx>, phone: String) {
    let mut last: Option<Look> = None;
    loop {
        let (c, id) = (ctx.clone(), phone.clone());
        let Some(now) = super::blocking(move || look(&c, &id)).await else {
            let _ = socket.send(event("revoked", json!({}))).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        };
        let mut out = vec![];
        if last.as_ref().is_none_or(|l| l.needs != now.needs) { out.push(event("needs", json!({"count": now.needs.as_array().map(Vec::len).unwrap_or(0), "requests": now.needs}))); }
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
