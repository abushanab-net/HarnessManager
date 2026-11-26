# Mobile companion, phase 0: what the spikes found

WP-M0, branch `mobile/spikes`, 26 Nov 2026. Android only: this run had a Windows PC, the `Teksa_Consumer` emulator
(Android 17, x86_64, 16 KB pages), no Mac, no Apple developer account and no Firebase project. Each spike has its
own README with the steps and every number.

| Folder | What |
|---|---|
| `mobile-host/` | Rust stand-in for the desktop: `pairing.rs` routes and messages, a `listener.rs`-style rustls listener, toy `/c1` routes |
| `mobile-frb/` | Flutter app + Rust crate through flutter_rust_bridge 2.13.0: pairing, TLS, the notification stand-in |
| `mobile-tls/` | `dart:io` mutual-TLS tests and the Rust alternative |
| `mobile-timing/` | Driver and results for alert → request shown → answer accepted |

## The answers

**1. Rust crate in a Flutter Android app (`mobile-frb`).** Works. The desktop's `spake2` 0.4.0, `chacha20poly1305`
0.11.0, `ed25519-dalek` 3.0.0 and `rcgen` 0.14.10 (with aws-lc-rs, as on the desktop) build for arm64, armv7 and
x86_64 through cargokit and are called from Dart. On the emulator the app paired with the Rust host in the exact
`pairing.rs` format: wrong code refused with 403, right code paired in 27 ms, the host's certificate pinned, no ring
key sent. Build: 17 min 39 s the first time (NDK 28.2 download, Gradle's first run), 130 s clean with warm caches,
52 s after a Dart-only change. Size added per ABI (stored uncompressed in the APK; compressed in brackets):
arm64 5.52 MB (2.49), armv7 3.41 MB (1.89), x86_64 6.78 MB (2.70). The arm64 split APK is 20.8 MB in total.

**2. Mutual TLS (`mobile-tls`).** `dart:io` cannot talk to today's listener: BoringSSL never offers `ed25519` in
`signature_algorithms`, and rustls, holding only the Ed25519 certificate, answers alert 40. Dart has no API to
change that. The client half is fine: `dart:io` presents an Ed25519 client certificate and pins by exact DER when the
server's certificate is ECDSA P-256 (HTTP 200; stranger and wrong pin refused). From Rust through the bridge (rustls
+ reqwest, the verifier copied from `client.rs`) it works on the emulator: 37 ms first call, 5 ms on the kept
connection. **Recommendation: TLS in Rust, inside `crates/companion-proto`.** The listener stays as it is and the
phone runs the same pinning code as the desktop. Fallback: a second P-256 certificate on the listener for clients
that do not offer `ed25519`.

**3. Timing (`mobile-timing`).** From tapping the alert to the host accepting the answer, release build, 5 runs each:
**warm 511 ms median (338–552), cold 2147 ms median (1256–2301)**, upper bounds by up to ~0.1 s of adb overhead.
Cold is mostly process and Flutter engine start (~1.5 s). The app's part of a 60 s window is about 2 s in the worst
measured case; the rest belongs to push delivery and the person. 60 s is workable with the phone at hand; 15 s is
tight once unlock and reading are counted. Still to measure: real FCM delivery (needs a Firebase project), APNs and
Live Activity push (need the paid Apple membership and a real iPhone), real lock screens, mid-range phones, and the
hub mailbox path.

## How the app should depend on `crates/companion-proto`

1. **`crates/companion-proto` stays plain Rust, with no flutter_rust_bridge in it.** It holds the identity and
   certificate making, SPAKE2 and sealing, the pinned rustls client config and verifier, and the `/c1` and action
   types. The desktop's `pairing.rs` and `identity.rs` call it, so there is one implementation that both test.
2. **The app repository has a thin wrapper crate** (as `mobile-frb/hm_spike/rust` here): `crate-type = ["cdylib",
   "staticlib"]`, `flutter_rust_bridge = "=2.13.0"`, and `companion-proto` as a **git dependency pinned to a tag**
   (`git = "ssh://git@github.com/<owner>/HarnessManager.git", tag = "companion-proto-v1.0.0"`), with its `Cargo.lock`
   committed. The wrapper's `api/` only adapts types for the bridge (opaque handles for SPAKE2 state and the client).
3. **Build with cargokit** (frb's default, proven here): Gradle and the Xcode pod phase run cargo. Commit the
   generated Dart and Rust glue and have CI check that `flutter_rust_bridge_codegen generate` changes nothing, so CI
   does not need `cargo-expand`. Leave cargokit's signed precompiled binaries and frb's newer `native-assets` backend
   for later, if build time or Swift Package Manager push us there.
4. **Private repository:** cargo needs `CARGO_NET_GIT_FETCH_WITH_CLI=true` and a read-only deploy key for
   HarnessManager on developer machines and runners. If that becomes a nuisance, a git submodule with a `path`
   dependency gives the same pinning.
5. **CI on the home runners** (`homek8-mobile`; the Mac as the project's own self-hosted label for iOS): the image
   needs Flutter 3.47.x, a JDK 21 (Gradle ran on Android Studio's JBR 21 here; JDK 27 was not tried), Android
   SDK 36 with NDK 28.2.13676358, Rust with the three Android targets. A small CI check should compare the versions
   of the shared crypto crates between the two lockfiles, since the app's lock is separate from the desktop's.

## Risks found

- **dart:io and Ed25519 servers** (above). Settled by doing TLS in Rust; the WebSocket for `/c1/events` must then be
  Rust too (`tokio-tungstenite` with a preconfigured rustls connector).
- **Networking in Rust inside a mobile app.** The first call on a kept connection after the app resumed took
  180–370 ms instead of 5 ms: reconnect or ping on resume. `reqwest::blocking` starts a runtime thread per client;
  the real crate should keep one async client on one runtime. reqwest's `rustls` feature brings in
  `rustls-platform-verifier` (Android JNI set-up if ever used); it is unused here and could be dropped with
  `hyper-rustls`.
- **Plain HTTP for pairing.** The spike sets `android:usesCleartextTraffic="true"` for the whole app. Android's
  network security config cannot name arbitrary LAN addresses, and on iOS ATS and the local network prompt apply.
  Whether `dart:io` enforces Android's cleartext policy was not tested; doing the pairing HTTP in Rust as well avoids
  the question on both platforms. The sealed payloads keep the content safe either way.
- **Toolchain drift.** Flutter 3.47 asks for NDK 28.2 and installed it on the first build; the documented 27.1 is not
  used. The 16 KB page size was handled (cargokit links with `-z max-page-size=16384`); keep testing on a 16 KB image.
- **Size.** About 2.5 MB more to download and 5.5 MB more installed on arm64. Not yet measured: aws-lc-rs against
  `ring`, and reqwest against plain hyper.
- **Cold start.** ~1.5 s of engine start on a fast PC's emulator; slower phones will take longer. An Allow button on
  the notification itself would skip it, but would need request details in the push, which the plan rules out.

## To check on the Mac for iOS

- Xcode and CocoaPods; `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`; run the same app
  with `flutter build ios --simulator`, then on an own iPhone with a free Apple ID (7-day signing).
- cargokit builds through a CocoaPods script phase (`rust_builder/ios/*.podspec`). This Flutter has Swift Package
  Manager on: check the pod-based FFI plugin still builds, or move to frb's `native-assets` backend.
- aws-lc-rs for the iOS targets (aws-lc-sys with Xcode's clang). If it fails, the phone can use rustls' `ring`
  provider; TLS interop with the desktop does not depend on the provider.
- `staticlib` linking with `-force_load`: check the bridge symbols survive dead stripping in release, and the size per
  architecture after app thinning.
- The same `TLS check` on iOS: `dart:io` uses the same BoringSSL, so the Ed25519 failure is expected; confirm it.
- Info.plist: `NSLocalNetworkUsageDescription`, `NSBonjourServices` with `_hrm._tcp` (from `mdns.rs`), the ATS rule for
  the plain-HTTP pairing port (`NSAllowsLocalNetworking`), and the seed in the Keychain
  (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`).
- The timing run with a local `UNUserNotificationCenter` alert as the push stand-in: cold launch from a tap on a real
  iPhone, with Face ID in the loop.
- With the paid membership: APNs, the time-sensitive interruption level, and Live Activity push updates.
