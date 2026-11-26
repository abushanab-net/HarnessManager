//! Who is at the other end of a connection to the listener, told apart by the certificate it showed: a paired
//! computer or a phone. A phone's connections are kept here so revoking it cuts them at once: the stream under the
//! TLS session is wrapped, and once cut it fails every read and write, which ends the HTTP connection and any
//! WebSocket on it.

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};

use axum::middleware::AddExtension;
use axum::Extension;
use axum_server::accept::Accept;
use axum_server::tls_rustls::RustlsAcceptor;
use futures_util::future::BoxFuture;
use futures_util::task::AtomicWaker;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tower_layer::Layer;

use crate::Ctx;

/// The caller of a request, put beside it by `Acceptor`.
#[derive(Clone, Debug, PartialEq)]
pub enum Caller {
    Peer,
    Phone(String),
}

#[derive(Default)]
struct Flag {
    cut: AtomicBool,
    waker: AtomicWaker,
}

/// A stream that fails once cut, waking whoever waits on it.
pub struct Cuttable<I> {
    inner: I,
    flag: Arc<Flag>,
}

impl<I> Cuttable<I> {
    fn check(&self, cx: &Context<'_>) -> Option<io::Error> {
        self.flag.waker.register(cx.waker());
        self.flag.cut.load(Ordering::SeqCst).then(|| io::Error::new(io::ErrorKind::ConnectionAborted, "the phone was revoked"))
    }
}

impl<I: AsyncRead + Unpin> AsyncRead for Cuttable<I> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if let Some(e) = self.check(cx) { return Poll::Ready(Err(e)); }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<I: AsyncWrite + Unpin> AsyncWrite for Cuttable<I> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        if let Some(e) = self.check(cx) { return Poll::Ready(Err(e)); }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(e) = self.check(cx) { return Poll::Ready(Err(e)); }
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> { Pin::new(&mut self.inner).poll_shutdown(cx) }
}

/// Every phone connection open now, by phone id; a dropped connection leaves a dead entry, swept at the next cut.
static LIVE: Mutex<Vec<(String, Weak<Flag>)>> = Mutex::new(Vec::new());

/// Cut every open connection of a phone. True: none of them is left.
pub fn cut(phone_id: &str) -> bool {
    let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    live.retain(|(id, w)| match w.upgrade() {
        Some(f) if id == phone_id => { f.cut.store(true, Ordering::SeqCst); f.waker.wake(); false }
        Some(_) => true,
        None => false,
    });
    true
}

/// A connection of this phone is open now.
pub fn connected(phone_id: &str) -> bool {
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(id, w)| id == phone_id && w.upgrade().is_some_and(|f| !f.cut.load(Ordering::SeqCst)))
}

/// The certificate's owner: a phone not revoked, else a paired computer; None for anything else (a phone revoked
/// while its handshake ran).
fn caller_of(ctx: &Ctx, der: &[u8]) -> Option<Caller> {
    if let Some(id) = crate::phones::by_cert(ctx, der) { return Some(Caller::Phone(id)); }
    let pems: Vec<String> = ctx.db.read(|c| Ok(c.prepare("select cert_pem from peers where coalesce(cert_pem, '') <> ''")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).ok()?;
    pems.iter().any(|p| crate::sync::peer::identity::pem_to_der(p).ok().as_deref() == Some(der)).then_some(Caller::Peer)
}

/// The listener's TLS acceptor, with the caller put beside every request of the connection.
#[derive(Clone)]
pub struct Acceptor {
    pub inner: RustlsAcceptor,
    pub ctx: Arc<Ctx>,
}

impl<I, S> Accept<I, S> for Acceptor
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Send + 'static,
{
    type Stream = <RustlsAcceptor as Accept<Cuttable<I>, S>>::Stream;
    type Service = AddExtension<S, Caller>;
    type Future = BoxFuture<'static, io::Result<(Self::Stream, Self::Service)>>;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        let (inner, ctx) = (self.inner.clone(), self.ctx.clone());
        Box::pin(async move {
            let flag = Arc::new(Flag::default());
            let (stream, service) = inner.accept(Cuttable { inner: stream, flag: flag.clone() }, service).await?;
            let der = stream.get_ref().1.peer_certificates().and_then(|c| c.first()).map(|c| c.to_vec()).unwrap_or_default();
            let caller = tokio::task::spawn_blocking(move || caller_of(&ctx, &der)).await.ok().flatten()
                .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "not a paired computer or phone"))?;
            if let Caller::Phone(id) = &caller { LIVE.lock().unwrap_or_else(|e| e.into_inner()).push((id.clone(), Arc::downgrade(&flag))); }
            Ok((stream, Extension(caller).layer(service)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn a_cut_connection_fails_the_read_it_waits_in_and_is_no_longer_counted() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let (a, mut b) = tokio::io::duplex(64);
            let flag = Arc::new(Flag::default());
            let mut s = Cuttable { inner: a, flag: flag.clone() };
            LIVE.lock().unwrap().push(("phone-cut".into(), Arc::downgrade(&flag)));
            b.write_all(b"x").await.unwrap();
            let mut one = [0u8; 1];
            s.read_exact(&mut one).await.unwrap();
            assert!(connected("phone-cut"));
            let waiting = tokio::spawn(async move { let mut buf = [0u8; 1]; s.read(&mut buf).await });
            tokio::task::yield_now().await;
            assert!(cut("phone-cut"));
            let r = tokio::time::timeout(std::time::Duration::from_secs(2), waiting).await.expect("woken").unwrap();
            assert_eq!(r.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
            assert!(!connected("phone-cut"));
            drop(b);
        });
    }
}
