//! The companion API from the phone: one client per paired computer, keeping one connection open across calls. A
//! connection found closed before a request is opened again; a read that fails on the way is tried once more on a
//! new one, but an action never is (the contract: the phone asks for the receipt instead). After the app comes back
//! from the background the kept connection is usually dead: `resume` opens a fresh one at once.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::client::TlsStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::api::{AccountOne, Accounts, Action, Event, Hello, Overview, PushHandle, Receipt, Refused, Requests, SessionPage, TranscriptPage, Update};
use crate::net::{self, Sender};
use crate::PhoneIdentity;

pub struct Client {
    tls: Arc<rustls::ClientConfig>,
    host: String,
    port: u16,
    kept: Mutex<Option<Sender>>,
}

/// A path segment as the routes take it: what the computer named (an account, a session id), encoded.
fn seg(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

impl Client {
    /// A client for the computer at `address` (`host:port`, one of its address hints), whose certificate
    /// (`desk_cert_pem`) the pairing gave.
    pub fn new(me: &PhoneIdentity, desk_cert_pem: &str, address: &str) -> Result<Client> {
        let (host, port) = net::split(address)?;
        Ok(Client { tls: net::tls_config(me, desk_cert_pem)?, host, port, kept: Mutex::new(None) })
    }

    async fn open(&self) -> Result<Sender> { net::handshake(net::tls(&self.tls, &self.host, self.port).await?).await }

    /// One request on the kept connection (HTTP/1.1 carries one at a time), opened again when it no longer works.
    async fn call(&self, method: &str, path: &str, body: Option<Vec<u8>>) -> Result<(u16, bytes::Bytes)> {
        let req = || net::request(method, &self.host, path, body.clone());
        let mut kept = self.kept.lock().await;
        let alive = match kept.as_mut() { Some(s) => !s.is_closed() && s.ready().await.is_ok(), None => false };
        if !alive { *kept = Some(self.open().await?); }
        let s = kept.as_mut().expect("opened");
        match net::send(s, req()?).await {
            Ok(r) => Ok(r),
            // an action may have been done: never sent twice
            Err(e) if method == "POST" => { *kept = None; Err(e) }
            Err(_) => {
                let s = kept.insert(self.open().await?);
                net::send(s, req()?).await
            }
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> { net::json(self.call("GET", path, None).await?) }

    /// Back from the background: the kept connection is dropped and a fresh one greets the computer.
    pub async fn resume(&self) -> Result<Hello> {
        *self.kept.lock().await = None;
        self.hello().await
    }

    pub async fn hello(&self) -> Result<Hello> { self.get("/c1/hello").await }

    pub async fn overview(&self) -> Result<Overview> { self.get("/c1/overview").await }

    pub async fn accounts(&self) -> Result<Accounts> { self.get("/c1/accounts").await }

    pub async fn account(&self, profile: &str) -> Result<AccountOne> { self.get(&format!("/c1/accounts/{}", seg(profile))).await }

    /// A page of sessions; `cursor` from the page before (`next_cursor`).
    pub async fn sessions(&self, cursor: Option<i64>) -> Result<SessionPage> {
        self.get(&match cursor { Some(c) => format!("/c1/sessions?cursor={c}"), None => "/c1/sessions".into() }).await
    }

    /// A transcript page from turn `after` (0 when None); `next` names the following page.
    pub async fn transcript(&self, harness: &str, profile: &str, id: &str, after: Option<usize>) -> Result<TranscriptPage> {
        self.get(&format!("/c1/sessions/{}/{}/{}/transcript?after={}", seg(harness), seg(profile), seg(id), after.unwrap_or(0))).await
    }

    pub async fn requests(&self) -> Result<Requests> { self.get("/c1/requests").await }

    /// An action, sent once: its receipt. When this fails, ask `receipt` with the same id; if that fails too, the
    /// phone shows "delivery unknown".
    pub async fn act(&self, a: &Action) -> Result<Receipt> { net::json(self.call("POST", "/c1/actions", Some(serde_json::to_vec(a)?)).await?) }

    /// The stored receipt of an action; None when the computer never got it.
    pub async fn receipt(&self, action_id: &str) -> Result<Option<Receipt>> {
        match self.get(&format!("/c1/actions/{}", seg(action_id))).await {
            Ok(r) => Ok(Some(r)),
            Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status == 404) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The relay handle the computer wakes this phone through.
    pub async fn push_register(&self, relay_handle: &str) -> Result<()> {
        net::json::<serde_json::Value>(self.call("PUT", "/c1/push", Some(serde_json::to_vec(&PushHandle { relay_handle: relay_handle.into() })?)).await?).map(|_| ())
    }

    pub async fn push_remove(&self) -> Result<()> { net::json::<serde_json::Value>(self.call("DELETE", "/c1/push", None).await?).map(|_| ()) }

    /// What changes on the computer, as it happens: a WebSocket on a connection of its own.
    pub async fn events(&self) -> Result<Events> {
        let tls = net::tls(&self.tls, &self.host, self.port).await?;
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        let (ws, _) = tokio::time::timeout(net::CONNECT, tokio_tungstenite::client_async(format!("wss://{host}:{}/c1/events", self.port), tls)).await
            .map_err(|_| anyhow!("the computer did not open the events"))??;
        Ok(Events { ws })
    }
}

/// The open events socket.
pub struct Events {
    ws: WebSocketStream<TlsStream<TcpStream>>,
}

impl Events {
    /// The next event, typed; None once the computer closed the socket (or cut it, when this phone was revoked).
    pub async fn next(&mut self) -> Option<Result<(Event, Update)>> {
        loop {
            match self.ws.next().await? {
                Ok(Message::Text(t)) => {
                    return Some(serde_json::from_str::<Event>(&t).map_err(Into::into).and_then(|e| { let u = e.update()?; Ok((e, u)) }));
                }
                Ok(Message::Close(_)) => return None,
                Ok(_) => continue,
                Err(e) => return Some(Err(e.into())),
            }
        }
    }

    pub async fn close(mut self) { let _ = self.ws.send(Message::Close(None)).await; }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_path_segment_is_encoded() {
        assert_eq!(super::seg("claude-main_1.x~"), "claude-main_1.x~");
        assert_eq!(super::seg("a/b c?"), "a%2Fb%20c%3F");
    }
}
