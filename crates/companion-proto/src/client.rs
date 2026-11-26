//! The companion API from the phone: one client per paired computer, keeping one connection open across calls. A
//! connection found closed before a request is opened again; a read that fails on the way is tried once more on a
//! new one, but an action never is (the contract: the phone asks for the receipt instead). After the app comes back
//! from the background the kept connection is usually dead: `resume` opens a fresh one at once.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::client::TlsStream;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
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
        let url = format!("wss://{host}:{}/c1/events", self.port);
        let (ws, _) = tokio::time::timeout(net::CONNECT, tokio_tungstenite::client_async_with_config(url, tls, Some(events_config()))).await
            .map_err(|_| anyhow!("the computer did not open the events"))??;
        Ok(Events::over(ws, PING_EVERY, ANSWER_WITHIN))
    }
}

/// An event is a few hundred bytes, a `needs` list a few kilobytes: nothing bigger is read.
pub const MAX_EVENT: usize = 256 * 1024;
/// A quiet socket is pinged this often, and must answer within `ANSWER_WITHIN`, or it is taken as gone (a phone that
/// moved networks keeps a half-open socket that would otherwise wait for ever).
pub const PING_EVERY: Duration = Duration::from_secs(20);
pub const ANSWER_WITHIN: Duration = Duration::from_secs(10);

fn events_config() -> WebSocketConfig { WebSocketConfig::default().max_message_size(Some(MAX_EVENT)).max_frame_size(Some(MAX_EVENT)) }

/// The open events socket.
pub struct Events<S = TlsStream<TcpStream>> {
    ws: WebSocketStream<S>,
    ping_every: Duration,
    answer_within: Duration,
    /// When a frame last came, and when the ping still unanswered went out.
    heard: Instant,
    pinged: Option<Instant>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Events<S> {
    /// Events over an open socket, with its heartbeat.
    pub fn over(ws: WebSocketStream<S>, ping_every: Duration, answer_within: Duration) -> Events<S> {
        Events { ws, ping_every, answer_within, heard: Instant::now(), pinged: None }
    }

    /// The next event, typed; None once the computer closed the socket (or cut it, when this phone was revoked); an
    /// error when it stopped answering, or sent more than an event can be.
    pub async fn next(&mut self) -> Option<Result<(Event, Update)>> {
        loop {
            let wait = match self.pinged {
                Some(at) => self.answer_within.saturating_sub(at.elapsed()),
                None => self.ping_every.saturating_sub(self.heard.elapsed()),
            };
            match tokio::time::timeout(wait, self.ws.next()).await {
                Ok(None) => return None,
                Ok(Some(Err(e))) => return Some(Err(e.into())),
                Ok(Some(Ok(m))) => {
                    (self.heard, self.pinged) = (Instant::now(), None);
                    match m {
                        Message::Text(t) => return Some(serde_json::from_str::<Event>(&t).map_err(Into::into).and_then(|e| { let u = e.update()?; Ok((e, u)) })),
                        Message::Close(_) => return None,
                        _ => continue,
                    }
                }
                Err(_) if self.pinged.is_some() => return Some(Err(anyhow!("the computer stopped answering"))),
                Err(_) => {
                    // a ping that cannot even be written (a socket backed up) counts against the same answer time
                    match tokio::time::timeout(self.answer_within, self.ws.send(Message::Ping(Vec::new().into()))).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => return Some(Err(e.into())),
                        Err(_) => return Some(Err(anyhow!("the computer stopped answering"))),
                    }
                    self.pinged = Some(Instant::now());
                }
            }
        }
    }

    pub async fn close(mut self) { let _ = tokio::time::timeout(self.answer_within, self.ws.send(Message::Close(None))).await; }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_path_segment_is_encoded() {
        assert_eq!(super::seg("claude-main_1.x~"), "claude-main_1.x~");
        assert_eq!(super::seg("a/b c?"), "a%2Fb%20c%3F");
    }

    use super::*;
    use tokio::net::TcpListener;

    /// A WebSocket server on loopback that does `serve` with each socket it takes.
    async fn server<F, Fut>(serve: F) -> String
    where F: FnOnce(WebSocketStream<TcpStream>) -> Fut + Send + 'static, Fut: std::future::Future<Output = ()> + Send {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (s, _) = l.accept().await.unwrap();
            serve(tokio_tungstenite::accept_async(s).await.unwrap()).await;
        });
        format!("ws://{addr}/c1/events")
    }

    async fn events_at(url: &str) -> Events<TcpStream> {
        let s = TcpStream::connect(url.trim_start_matches("ws://").trim_end_matches("/c1/events")).await.unwrap();
        let (ws, _) = tokio_tungstenite::client_async_with_config(url, s, Some(events_config())).await.unwrap();
        Events::over(ws, Duration::from_millis(100), Duration::from_millis(100))
    }

    /// N5: a computer that stops answering (a half-open socket) ends the events within a ping and its answer time.
    #[tokio::test]
    async fn a_silent_computer_ends_the_events_within_the_heartbeat() {
        let url = server(|ws| async move { let _held = ws; tokio::time::sleep(Duration::from_secs(10)).await; }).await;
        let mut ev = events_at(&url).await;
        let r = tokio::time::timeout(Duration::from_secs(2), ev.next()).await.expect("the heartbeat ends it");
        assert!(format!("{:#}", r.unwrap().unwrap_err()).contains("stopped answering"));
    }

    /// N5: a quiet computer that answers its pings keeps the events open.
    #[tokio::test]
    async fn a_quiet_computer_that_answers_stays_connected() {
        let url = server(|mut ws| async move {
            // reading answers the pings; an event after a while
            let read = async { while let Some(Ok(_)) = ws.next().await {} };
            let _ = tokio::time::timeout(Duration::from_millis(500), read).await;
            ws.send(Message::Text(serde_json::json!({"kind": "changed", "at": 1, "data": {}}).to_string().into())).await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }).await;
        let mut ev = events_at(&url).await;
        let (_, u) = tokio::time::timeout(Duration::from_secs(2), ev.next()).await.unwrap().unwrap().unwrap();
        assert_eq!(u, Update::Changed);
    }

    /// N5: a message bigger than an event can be is refused, not read.
    #[tokio::test]
    async fn an_oversized_event_is_refused() {
        let url = server(|mut ws| async move {
            let big = serde_json::json!({"kind": "changed", "at": 1, "data": {"x": "y".repeat(2 * MAX_EVENT)}}).to_string();
            let _ = ws.send(Message::Text(big.into())).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }).await;
        let mut ev = events_at(&url).await;
        let r = tokio::time::timeout(Duration::from_secs(2), ev.next()).await.unwrap();
        assert!(matches!(r, Some(Err(_))), "{:?}", r.map(|x| x.map(|(_, u)| u)));
    }

    /// A socket whose writes stop going through once `stall` is set (a backed-up connection).
    struct Stalling {
        inner: TcpStream,
        stall: Arc<std::sync::atomic::AtomicBool>,
    }

    impl AsyncRead for Stalling {
        fn poll_read(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
            std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for Stalling {
        fn poll_write(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
            if self.stall.load(std::sync::atomic::Ordering::SeqCst) { return std::task::Poll::Pending; }
            std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
        }
        fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
            if self.stall.load(std::sync::atomic::Ordering::SeqCst) { return std::task::Poll::Pending; }
            std::pin::Pin::new(&mut self.inner).poll_flush(cx)
        }
        fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
            std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    /// N5: a ping that cannot be written (the socket backed up) ends the events within the answer time, and closing
    /// such a socket returns too.
    #[tokio::test]
    async fn a_backed_up_socket_ends_the_events_and_closes_in_bounded_time() {
        let url = server(|ws| async move { let _held = ws; tokio::time::sleep(Duration::from_secs(10)).await; }).await;
        let stall = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s = Stalling { inner: TcpStream::connect(url.trim_start_matches("ws://").trim_end_matches("/c1/events")).await.unwrap(), stall: stall.clone() };
        let (ws, _) = tokio_tungstenite::client_async_with_config(url.as_str(), s, Some(events_config())).await.unwrap();
        stall.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut ev = Events::over(ws, Duration::from_millis(100), Duration::from_millis(100));
        let r = tokio::time::timeout(Duration::from_secs(2), ev.next()).await.expect("the heartbeat's write is bounded");
        assert!(format!("{:#}", r.unwrap().unwrap_err()).contains("stopped answering"));
        tokio::time::timeout(Duration::from_secs(2), ev.close()).await.expect("closing is bounded");
    }
}
