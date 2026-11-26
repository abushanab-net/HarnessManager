//! One best-effort wake per enrolled handle when a request opens.
use std::{sync::Arc, time::Duration};

use crate::{hooks::Run, phones, secrets, Ctx};
use anyhow::{ensure, Result};
use companion_proto::push::{self, Kind, Wake};
use futures_util::{stream, StreamExt};
use reqwest::Url;

pub(crate) fn wake_url(origin: &str) -> Result<Url> {
    let url = Url::parse(origin)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    ensure!(
        (url.scheme() == "https" || (url.scheme() == "http" && loopback))
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "give the HTTPS origin of the push relay"
    );
    Ok(url.join(push::WAKE_PATH)?)
}

/// The desktop's one-second snapshots contain only requests whose hook is still alive.
pub fn new_requests(previous: &[Run], current: &[Run]) -> Vec<String> {
    current
        .iter()
        .filter_map(|r| r.ask.as_ref())
        .filter(|a| !previous.iter().filter_map(|r| r.ask.as_ref()).any(|old| old.id == a.id))
        .map(|a| a.id.clone())
        .collect()
}

pub async fn notify(ctx: Arc<Ctx>, requests: Vec<String>) -> Result<()> {
    if requests.is_empty() || ctx.settings.push_relay_url.is_empty() {
        return Ok(());
    }
    let url = wake_url(&ctx.settings.push_relay_url)?;
    let reader = ctx.clone();
    let (handles, requests) = tokio::task::spawn_blocking(move || -> Result<(Vec<_>, Vec<_>)> {
        let (slots, claimed) = reader.db.with(|c| {
            let tx = c.transaction()?;
            let mut claimed = Vec::new();
            for id in requests {
                // A restarted desktop may see the same open hook again.
                if tx.execute(
                    "update run_ask set push_claimed=1 where id=? and state='open' and expires>? and push_claimed=0",
                    rusqlite::params![id, crate::util::now_ms()],
                )? != 0
                {
                    claimed.push(id);
                }
            }
            if claimed.is_empty() {
                return Ok((Vec::new(), claimed));
            }
            let mut st = tx.prepare(
                "select relay_handle,push_key_id from phones where revoked_at is null and push_key_id is not null",
            )?;
            let rows = st
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(st);
            tx.commit()?;
            Ok((rows, claimed))
        })?;
        let handles = slots
            .into_iter()
            .filter_map(|(handle, slot)| {
                let secret = secrets::get_key(&phones::push_target(&slot))?;
                let mut key = [0; 32];
                hex::decode_to_slice(secret, &mut key).ok()?;
                Some((handle, slot, key))
            })
            .collect();
        Ok((handles, claimed))
    })
    .await??;
    if handles.is_empty() {
        return Ok(());
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    stream::iter(handles)
        .for_each_concurrent(8, |(relay_handle, slot, key)| {
            let (http, url, requests, ctx) = (&http, &url, &requests, ctx.clone());
            async move {
                for request in requests {
                    let Ok(body) = serde_json::to_vec(&Wake {
                        relay_handle: relay_handle.clone(),
                        kind: Kind::Needs,
                        collapse_id: request.clone(),
                    }) else {
                        return;
                    };
                    let ts = crate::util::now_ms() as u64;
                    let sig = push::signature(&key, "POST", push::WAKE_PATH, ts, &body);
                    let response = http
                        .post(url.clone())
                        .header("content-type", "application/json")
                        .header(push::TIMESTAMP, ts)
                        .header(push::SIGNATURE, sig)
                        .body(body)
                        .send()
                        .await;
                    if response
                        .is_ok_and(|r| matches!(r.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE))
                    {
                        let _ = tokio::task::spawn_blocking(move || phones::rejected_push(&ctx, &slot)).await;
                        return;
                    }
                }
            }
        })
        .await;
    Ok(())
}
