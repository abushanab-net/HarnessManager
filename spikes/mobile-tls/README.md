# Spike: mutual TLS from the phone

**Question.** Can Dart (`dart:io` `SecurityContext` / `HttpClient`) do mutual TLS with an Ed25519 self-signed client
certificate and pin the computer's self-signed Ed25519 certificate by exact DER, against a rustls server configured
like `crates/core/src/sync/peer/listener.rs`? If not, does the TLS work from Rust through the flutter_rust_bridge crate?

**Answer.** `dart:io` cannot reach today's listener. The TLS works from Rust (rustls + reqwest, the same code as
`client.rs`) on the emulator. **Recommendation: do the companion API's TLS in Rust, inside the protocol crate.**

## What was tried

The server is `spikes/mobile-host` (aws-lc-rs provider, the host's Ed25519 certificate made exactly as
`identity::make_cert`, a client-certificate verifier that admits a certificate only when its DER equals a stored
phone's). Port 47802 serves that. Port 47803 serves the same routes with an ECDSA P-256 server certificate, to find
out which half of the handshake breaks.

`dart_cli/bin/mtls.dart` (Dart 3.13.2, the BoringSSL that ships in the Flutter engine), on this PC:

| Case | Result |
|---|---|
| Ed25519 server, paired client, right pin | `HandshakeException … SSLV3_ALERT_HANDSHAKE_FAILURE` |
| Ed25519 server, stranger client | same alert |
| Ed25519 server, wrong pin | same alert (never gets as far as the pin) |
| P-256 server, paired Ed25519 client, right pin | **HTTP 200** in 19 ms |
| P-256 server, stranger client | `TLSV1_ALERT_ACCESS_DENIED` (refused by the host's verifier) |
| P-256 server, paired client, wrong pin | `CERTIFICATE_VERIFY_FAILED` (refused by `badCertificateCallback`) |

The same app code on the emulator (`spikes/mobile-frb/hm_spike`, button "TLS check") fails the same way against
the Ed25519 listener: `SSLV3_ALERT_HANDSHAKE_FAILURE(tls_record.cc:489)`.

**Why.** OpenSSL shows it directly: `openssl s_client -sigalgs ECDSA+SHA256:…` gets alert 40 from the rustls server,
`-sigalgs ed25519` completes the handshake ("Peer signature type: ed25519"). rustls holds only an Ed25519 key, so it
needs the client to offer `ed25519` in `signature_algorithms`. BoringSSL's client does not offer it by default and
`dart:io` has no API to change the list. The client side is fine: BoringSSL signs with an Ed25519 client key when
the server asks (the P-256 rows), and `badCertificateCallback` gets `X509Certificate.der`, so exact-DER pinning works.

## Rust through the bridge

`hm_spike_core::api::tls::C1` builds a `rustls::ClientConfig` with a pinned-DER `ServerCertVerifier` and the phone's
certificate as client auth (copied from `client.rs`), and hands it to `reqwest::blocking` with
`use_preconfigured_tls`. Results:

| Where | First call (connect + handshake) | Reused connection | Stranger client | Wrong pin |
|---|---|---|---|---|
| PC, `cargo test -- --ignored` | 25 ms | 0 ms | refused by host | refused by client (`ApplicationVerificationFailure`) |
| Emulator, release build | 37 ms | 5 ms | — | — |

The timing spike's fetches and answers all go through this client.

## Recommendation

Use Rust (rustls) for the companion API, in `crates/companion-proto`, called from Dart through flutter_rust_bridge:

- The desktop listener stays as it is: one Ed25519 identity, one pinned certificate per side.
- The phone runs the same verifier code as `client.rs`, and pairing, sealing and TLS live in one crate both sides test.
- `/c1/events` (WebSocket) needs the same TLS; `tokio-tungstenite` with a preconfigured rustls connector fits there.

The fallback, if Rust networking becomes a problem (background isolates, proxies, platform networking features):
give the listener a second, ECDSA P-256 certificate and let a rustls `ResolvesServerCert` pick it when the client
does not offer `ed25519`. Then `dart:io` works with the phone's Ed25519 client certificate as shown above. It costs a
second key in the credential store and a second pin in the phone enrollment payload.

Things to watch with the Rust client:

- `reqwest`'s `rustls` feature pulls in `rustls-platform-verifier`, which on Android needs JNI set-up if it is ever
  used. With `use_preconfigured_tls` it is not used, but it is dead weight; `hyper-rustls` directly, or a reqwest
  feature set without the platform verifier, would drop it.
- `reqwest::blocking` starts its own runtime thread per client. The real crate should hold one async client on one
  runtime.
- After the app comes back from the background, the first call on the kept connection took 150–370 ms on the
  emulator (see the timing spike). Reconnect or ping on resume.

## Run it

```
cd spikes/mobile-host && cargo build --release
./target/release/mobile-spike-host client-cert ../mobile-tls/dart_cli/certs
./target/release/mobile-spike-host serve --code 482913
cd ../mobile-tls/dart_cli && dart run bin/mtls.dart
```
