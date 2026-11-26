# Spike host

A stand-in for the desktop, shared by the three mobile spikes. Everything binds to 127.0.0.1; the Android emulator
reaches it as `10.0.2.2`.

| Port | What |
|---|---|
| 47801 | Plain HTTP. `/pair/hello`, `/pair/start`, `/pair/finish` with the same JSON, SPAKE2 roles and sealing as `crates/core/src/sync/peer/pairing.rs`; the phone's certificate goes to `host-data/phones/`. Also `/spike/*`: the timing driver's controls and the event log. |
| 47802 | Mutual TLS configured like `listener.rs`: aws-lc-rs, the host's Ed25519 self-signed certificate made as `identity::make_cert`, client certificates admitted by exact DER. Toy `/c1/hello`, `/c1/requests`, `/c1/actions` (with stored receipts). |
| 47803 | The same routes with an ECDSA P-256 server certificate, for the dart:io comparison. |

```
cargo build --release
./target/release/mobile-spike-host serve --code 482913        # prints one JSON line per event
./target/release/mobile-spike-host client-cert <dir>          # a client identity for the Dart CLI test
```

It differs from the desktop where the spikes do not need it: no database, no two-minute expiry, no ending after three
wrong codes (it counts them), and the host's identity seed sits in `host-data/host.seed`.
