//! Who is at the other end of a connection to the listener, told apart by the certificate it showed: a paired
//! computer or a phone. A phone's connections are kept here so revoking it cuts them at once: the stream under the
//! TLS session is wrapped, and once cut it fails every read and write, which ends the HTTP connection and any
//! WebSocket on it. The listener that holds them acknowledges a revocation, whichever process made it.

use std::io;
use std::path::{Path, PathBuf};
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
    /// A phone's enrollment: its id and the generation the certificate was enrolled with.
    Phone { id: String, generation: String },
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

/// A phone connection open now, on the listener of the data folder `data`.
struct Live {
    data: PathBuf,
    phone: String,
    generation: String,
    flag: Weak<Flag>,
}

/// Every phone connection open in this process. Registering drops the entries of closed connections.
static LIVE: Mutex<Vec<Live>> = Mutex::new(Vec::new());
/// The data folders whose listener runs in this process: their phones' connections are all here.
static LISTENING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn live() -> std::sync::MutexGuard<'static, Vec<Live>> { LIVE.lock().unwrap_or_else(|e| e.into_inner()) }

/// The listener for `data` runs in this process (on), or stopped (off).
pub(crate) fn listening(data: &Path, on: bool) {
    let mut l = LISTENING.lock().unwrap_or_else(|e| e.into_inner());
    l.retain(|d| d != data);
    if on { l.push(data.to_path_buf()); }
}

fn register(data: &Path, phone: &str, generation: &str, flag: &Arc<Flag>) {
    let mut l = live();
    l.retain(|e| e.flag.strong_count() > 0);
    l.push(Live { data: data.to_path_buf(), phone: phone.into(), generation: generation.into(), flag: Arc::downgrade(flag) });
}

/// Cut the open connections that `which` picks among this listener's.
fn cut_where(data: &Path, which: impl Fn(&Live) -> bool) {
    live().retain(|e| match e.flag.upgrade() {
        Some(f) if e.data == data && which(e) => { f.cut.store(true, Ordering::SeqCst); f.waker.wake(); false }
        Some(_) => true,
        None => false,
    });
}

/// Cut every open connection of a phone. True when this process holds the listener, so none is left anywhere; false
/// when another process does (its `sweep` cuts them within a second and acknowledges it).
pub fn cut(ctx: &Ctx, phone_id: &str) -> bool {
    cut_where(&ctx.data, |e| e.phone == phone_id);
    LISTENING.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|d| d == &ctx.data)
}

/// A connection of this phone is open now.
pub fn connected(ctx: &Ctx, phone_id: &str) -> bool {
    live().iter().any(|e| e.data == ctx.data && e.phone == phone_id && e.flag.upgrade().is_some_and(|f| !f.cut.load(Ordering::SeqCst)))
}

/// The enrollment still stands: not revoked, not replaced by a pairing again.
fn current(ctx: &Ctx, phone: &str, generation: &str) -> bool {
    crate::phones::get(ctx, phone).ok().flatten().is_some_and(|p| p.revoked_at.is_none() && p.generation == generation)
}

/// Every second, from the listener: the connections of phones revoked or paired again since (from any process) are
/// cut, and each revocation is acknowledged once this listener holds no connection of that phone.
pub(crate) fn sweep(ctx: &Ctx) {
    let held: Vec<(String, String)> = live().iter().filter(|e| e.data == ctx.data && e.flag.strong_count() > 0).map(|e| (e.phone.clone(), e.generation.clone())).collect();
    let stale: Vec<(String, String)> = held.into_iter().filter(|(p, g)| !current(ctx, p, g)).collect();
    cut_where(&ctx.data, |e| stale.iter().any(|(p, g)| &e.phone == p && &e.generation == g));
    let unacked: Vec<String> = ctx.db.read(|c| Ok(c.prepare("select phone_id from phones where revoked_at is not null and cut_at is null")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).unwrap_or_default();
    if !unacked.is_empty() {
        cut_where(&ctx.data, |e| unacked.contains(&e.phone));
        crate::phones::cut_done(ctx, &unacked);
    }
}

/// The certificate's owner: a phone's enrollment not revoked, or a paired computer; None for anything else, and
/// for a certificate both claim.
fn caller_of(ctx: &Ctx, der: &[u8]) -> Option<Caller> {
    let pems: Vec<String> = ctx.db.read(|c| Ok(c.prepare("select cert_pem from peers where coalesce(cert_pem, '') <> ''")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).ok()?;
    let peer = pems.iter().any(|p| crate::sync::peer::identity::pem_to_der(p).ok().as_deref() == Some(der));
    match (crate::phones::by_cert(ctx, der), peer) {
        (Some((id, generation)), false) => Some(Caller::Phone { id, generation }),
        (None, true) => Some(Caller::Peer),
        _ => None,
    }
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
            let f2 = flag.clone();
            let caller = tokio::task::spawn_blocking(move || {
                let caller = caller_of(&ctx, &der)?;
                if let Caller::Phone { id, generation } = &caller {
                    register(&ctx.data, id, generation, &f2);
                    // registered first, then looked at again: a revocation either cut it or is seen here
                    if !current(&ctx, id, generation) { return None; }
                }
                Some(caller)
            }).await.ok().flatten().ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "not a paired computer or phone"))?;
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
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let (a, mut b) = tokio::io::duplex(64);
            let flag = Arc::new(Flag::default());
            let mut s = Cuttable { inner: a, flag: flag.clone() };
            register(&ctx.data, "phone-cut", "g1", &flag);
            b.write_all(b"x").await.unwrap();
            let mut one = [0u8; 1];
            s.read_exact(&mut one).await.unwrap();
            assert!(connected(&ctx, "phone-cut"));
            let waiting = tokio::spawn(async move { let mut buf = [0u8; 1]; s.read(&mut buf).await });
            tokio::task::yield_now().await;
            assert!(!cut(&ctx, "phone-cut"), "no listener here: not acknowledged");
            let r = tokio::time::timeout(std::time::Duration::from_secs(2), waiting).await.expect("woken").unwrap();
            assert_eq!(r.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
            assert!(!connected(&ctx, "phone-cut"));
            drop(b);
        });
    }

    #[test]
    fn closed_connections_leave_the_registry_as_new_ones_come() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("reg");
        for _ in 0..100 { register(&data, "phone-churn", "g", &Arc::new(Flag::default())); }
        let mine = live().iter().filter(|e| e.data == data).count();
        assert!(mine <= 1, "{mine} entries kept for closed connections");
    }
}
