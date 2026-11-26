//! One best-effort wake per enrolled handle when a request opens.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{ensure, Result};
use companion_proto::push::{self, Kind, Wake};
use futures_util::{stream, StreamExt};
use reqwest::Url;
use rusqlite::{Connection, OpenFlags};

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

pub(crate) fn opened(db: &Connection, request: &str) {
    let Some(path) = db.path().filter(|s| !s.is_empty()).map(PathBuf::from) else { return };
    let request = request.to_owned();
    // Even settings and enrollment reads happen off the hook's answer path.
    let _ = std::thread::Builder::new().name("phone-wake".into()).spawn(move || {
        let _ = notify(&path, request);
    });
}

fn notify(path: &Path, request: String) -> Result<()> {
    let settings: crate::ctx::Settings = serde_json::from_slice(&std::fs::read(path.with_file_name("settings.json"))?)?;
    if settings.push_relay_url.is_empty() {
        return Ok(());
    }
    let url = wake_url(&settings.push_relay_url)?;
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut query = db.prepare("select distinct relay_handle,relay_secret from phones where revoked_at is null and relay_handle is not null and relay_secret is not null")?;
    let handles = query
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if handles.is_empty() {
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        stream::iter(handles)
            .for_each_concurrent(8, |(relay_handle, secret)| {
                let (http, url, collapse_id) = (&http, url.clone(), request.clone());
                async move {
                    let mut key = [0; 32];
                    if hex::decode_to_slice(secret, &mut key).is_err() {
                        return;
                    }
                    let Ok(body) = serde_json::to_vec(&Wake { relay_handle, kind: Kind::Needs, collapse_id }) else {
                        return;
                    };
                    let ts = crate::util::now_ms() as u64;
                    let sig = push::signature(&key, "POST", push::WAKE_PATH, ts, &body);
                    let _ = http
                        .post(url)
                        .header("content-type", "application/json")
                        .header(push::TIMESTAMP, ts)
                        .header(push::SIGNATURE, sig)
                        .body(body)
                        .send()
                        .await;
                }
            })
            .await;
        Ok(())
    })
}
