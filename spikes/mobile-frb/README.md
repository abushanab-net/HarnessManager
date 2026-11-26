# Spike: the desktop's pairing crypto in a Flutter app through flutter_rust_bridge

**Question.** Can a small Rust crate using the desktop's `spake2`, `chacha20poly1305`, `ed25519-dalek` and `rcgen`
(the workspace Cargo.lock versions) be compiled into a Flutter Android app through flutter_rust_bridge and called
from Dart, and does it interoperate with the desktop's exact pairing messages?

**Answer.** Yes. The app on the emulator paired with a Rust host on this PC (reached as `10.0.2.2`) using the
`pairing.rs` message format: a wrong code was refused with 403, the right code paired in 27 ms, both sides opened
each other's sealed payloads, and the host's certificate was then pinned for mutual TLS.

## What is here

- `hm_spike/rust/` — the crate (`hm_spike_core`). `api/pairing.rs`: the phone's identity (Ed25519 seed → PKCS#8 →
  `rcgen` self-signed certificate, same fields as `identity::make_cert`), SPAKE2 side A as an opaque object, and
  `seal`/`open` byte-for-byte as `pairing.rs`. `api/tls.rs`: the mutual-TLS client (see `../mobile-tls`). The
  workspace `Cargo.lock` was copied in, so the shared crates resolve to the desktop's versions: spake2 0.4.0,
  chacha20poly1305 0.11.0, ed25519-dalek 3.0.0, rcgen 0.14.10 (aws_lc_rs), rustls 0.23.46, aws-lc-rs 1.19.0,
  reqwest 0.13.5.
- `hm_spike/lib/main.dart` — HTTP in Dart (`dart:io`), crypto in Rust. On first start it pairs (wrong code first, then
  the right one), stores the seed and the host's certificate, runs the TLS check, and reports to the host.
- `hm_spike/android/.../AskReceiver.kt`, `MainActivity.kt` — the notification stand-in for the timing spike.
- `hm_spike/integration_test/simple_test.dart` — on-device check that identity, SPAKE2 start and sealing cross the
  bridge (deterministic certificate from a seed, wrong key refused).
- `../mobile-host` — the Rust host: `/pair/hello`, `/pair/start`, `/pair/finish` with the same JSON, SPAKE2 roles and
  sealing as `pairing.rs`, and the rustls listener configured like `listener.rs`.

The interop is the real exchange: the phone is SPAKE2 side A with identities (phone id, host id), the host side B
with the code as password; the phone's identity payload is sealed first and the host seals its own only after it
opens the phone's. The host's payload is the desktop's `payload()` without `ring`: a phone never gets the ring key.

## Results on the emulator

Emulator `Teksa_Consumer`: Android 17 (API 37.1), x86_64, 16 KB pages. Release build.

| Step | Time |
|---|---|
| `RustLib.init()` (load the library, set up the bridge) | 19 ms |
| Phone identity: Ed25519 key + rcgen certificate (225 B DER) | < 1 ms |
| Pairing with the right code: hello + start + finish + open, 3 HTTP round trips | 27 ms (1031 ms on the very first launch after install) |
| Wrong code | host answers 403 "the code did not match" at `finish` |
| Mutual TLS `/c1/hello` from Rust, first / reused connection | 37 ms / 5 ms |
| Integration test (`flutter test integration_test -d emulator-5554`) | passes |

The 16 KB page size was no problem: cargokit links the Android libraries with `-z max-page-size=16384`.

## Build

Steps, on Windows with Flutter 3.47.2, Rust 1.100, Android SDK at `C:\Android\android-sdk`:

```
cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked     # 1 min 16 s
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
flutter_rust_bridge_codegen create hm_spike --org net.abushanab --rust-crate-name hm_spike_core
# write rust/src/api/*.rs, then
flutter_rust_bridge_codegen generate        # 66 s the first time (it installs cargo-expand)
flutter build apk --release                  # Gradle runs cargokit, which runs cargo for each ABI
flutter test integration_test -d emulator-5554
```

flutter_rust_bridge 2.13.0 was the current stable (2.14 is in beta). The crate needs an empty `[workspace]` table only
because the spike sits inside this repository's workspace.

| Build | Time |
|---|---|
| First release build on this machine | 17 min 39 s (mostly installing NDK 28.2.13676358, which Flutter 3.47 asks for, and Gradle's first run) |
| Clean release build after `flutter clean`, caches warm (Rust for three ABIs from scratch) | 130 s |
| Release build after a Dart-only change | 52 s |
| Split per ABI, nothing changed | 22 s |
| Debug build + install + run the integration test | 122 s |

Gradle ran on Android Studio's bundled JBR, which Flutter picks before `JAVA_HOME` (JDK 27 here).

## Size added by the Rust library

The library carries aws-lc-rs (the desktop's crypto provider), rustls, reqwest/hyper/tokio, spake2, ed25519-dalek and
rcgen, stripped. AGP stores `.so` files uncompressed in the APK, so the in-APK size is the raw size; the download from
the Play Store is compressed, which the deflate column approximates.

| ABI | `libhm_spike_core.so` in the APK | deflate -9 | Split APK total |
|---|---|---|---|
| arm64-v8a | 5.52 MB | 2.49 MB | 20.8 MB |
| armeabi-v7a | 3.41 MB | 1.89 MB | 16.2 MB |
| x86_64 | 6.78 MB | 2.70 MB | 23.4 MB |

For comparison, `libflutter.so` is 11.75 MB on arm64. Not measured: how much of the 5.5 MB is aws-lc against `ring`,
and how much is reqwest. Dropping reqwest for `hyper-rustls`, and the unused `rustls-platform-verifier`, are the first
places to look if size matters.
