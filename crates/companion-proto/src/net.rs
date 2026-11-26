//! The phone's connection to a computer: rustls presenting the phone's certificate and accepting exactly the
//! computer's, pinned at pairing by its DER (as the desktop pins a peer in `client.rs`), and HTTP/1.1 over it with
//! hyper. The same TLS stream carries the `/c1/events` WebSocket. Plain HTTP is used only for the pairing routes,
//! whose payloads are sealed.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::client::conn::http1::SendRequest;
use hyper_util::rt::TokioIo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::api::{ConnectError, Refused};
use crate::PhoneIdentity;

pub(crate) const CONNECT: Duration = Duration::from_secs(5);
pub(crate) const ANSWER: Duration = Duration::from_secs(20);
/// The most of one answer read: a transcript page is at most half a megabyte.
const MAX_ANSWER: usize = 8 * 1024 * 1024;

/// Accepts exactly one certificate: the computer's.
#[derive(Debug)]
struct Pinned {
    der: Vec<u8>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _server_name: &ServerName<'_>, _ocsp: &[u8], _now: UnixTime) -> Result<ServerCertVerified, Error> {
        if end_entity.as_ref() == self.der.as_slice() { Ok(ServerCertVerified::assertion()) } else { Err(Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)) }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.provider.signature_verification_algorithms.supported_schemes() }
}

/// The TLS client: this phone's certificate shown, the computer's (`desk_cert_pem`, from pairing) the only one taken.
pub fn tls_config(me: &PhoneIdentity, desk_cert_pem: &str) -> Result<Arc<rustls::ClientConfig>> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let der = crate::identity::pem_to_der(desk_cert_pem)?;
    let (_, mine) = me.cert()?;
    Ok(Arc::new(rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions()?.dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { der, provider }))
        .with_client_auth_cert(vec![CertificateDer::from(mine)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(me.key_der())))?))
}

/// `host:port` (an IPv6 literal in brackets) split.
pub(crate) fn split(address: &str) -> Result<(String, u16)> {
    let (h, p) = address.trim().rsplit_once(':').context("give the computer as host:port")?;
    Ok((h.trim_matches(['[', ']']).to_string(), p.parse().context("the port in host:port is a number")?))
}

pub(crate) async fn tcp(host: &str, port: u16) -> Result<TcpStream> {
    let s = tokio::time::timeout(CONNECT, TcpStream::connect((host, port))).await.map_err(|_| anyhow!("{host}:{port} did not answer"))?
        .with_context(|| format!("could not reach {host}:{port}"))?;
    let _ = s.set_nodelay(true);
    Ok(s)
}

/// A TLS session with the computer.
pub(crate) async fn tls(cfg: &Arc<rustls::ClientConfig>, host: &str, port: u16) -> Result<TlsStream<TcpStream>> {
    let tcp = tcp(host, port).await?;
    // the certificate is pinned, so the name only goes into SNI, which the computer does not read
    let name = ServerName::try_from(host.to_string()).unwrap_or_else(|_| ServerName::try_from("harness-manager.invalid").expect("a valid name"));
    tokio::time::timeout(CONNECT, tokio_rustls::TlsConnector::from(cfg.clone()).connect(name, tcp)).await.map_err(|_| anyhow!("{host}:{port} did not finish the handshake"))?
        .with_context(|| format!("{host}:{port} refused this phone, or is not the computer it paired with"))
}

/// A failure on the way to an answer, typed: `Revoked` when the computer sent a TLS alert (as it does to a removed
/// phone, on the handshake or on the first read after it), else `Unreachable`. A `Refused` (an HTTP answer) and an
/// error already typed pass through.
pub(crate) fn classify(e: anyhow::Error) -> anyhow::Error {
    if e.downcast_ref::<Refused>().is_some() || e.downcast_ref::<ConnectError>().is_some() { return e; }
    let why = format!("{e:#}");
    if alerted(&e) { ConnectError::Revoked(why).into() } else { ConnectError::Unreachable(why).into() }
}

/// The computer sent a TLS alert: it refused this phone.
pub(crate) fn alerted(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        let tls = c.downcast_ref::<Error>().or_else(|| c.downcast_ref::<std::io::Error>().and_then(|io| io.get_ref()).and_then(|i| i.downcast_ref::<Error>()));
        matches!(tls, Some(Error::AlertReceived(_)))
    })
}

pub(crate) type Sender = SendRequest<Full<Bytes>>;

/// HTTP/1.1 over a stream; the connection runs on the current tokio runtime until either side drops it.
pub(crate) async fn handshake<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(io: S) -> Result<Sender> {
    let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io)).await?;
    tokio::spawn(async move { let _ = conn.await; });
    Ok(sender)
}

pub(crate) fn request(method: &str, host: &str, path: &str, body: Option<Vec<u8>>) -> Result<hyper::Request<Full<Bytes>>> {
    let mut r = hyper::Request::builder().method(method).uri(path).header("host", host);
    if body.is_some() { r = r.header("content-type", "application/json"); }
    Ok(r.body(Full::new(Bytes::from(body.unwrap_or_default())))?)
}

/// One request sent and its whole answer read: `(status, body)`.
pub(crate) async fn send(sender: &mut Sender, req: hyper::Request<Full<Bytes>>) -> Result<(u16, Bytes)> {
    tokio::time::timeout(ANSWER, async {
        let r = sender.send_request(req).await?;
        let status = r.status().as_u16();
        let body = Limited::new(r.into_body(), MAX_ANSWER).collect().await.map_err(|e| anyhow!("reading the answer: {e}"))?.to_bytes();
        Ok((status, body))
    }).await.map_err(|_| anyhow!("the computer did not answer in time"))?
}

/// A JSON answer, or `Refused` with the status and the computer's reason.
pub(crate) fn json<T: serde::de::DeserializeOwned>((status, body): (u16, Bytes)) -> Result<T> {
    if !(200..300).contains(&status) {
        let error = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_default();
        return Err(Refused { status, error }.into());
    }
    serde_json::from_slice(&body).context("the computer's answer")
}
