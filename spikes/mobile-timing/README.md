# Spike: from the alert to an accepted answer

**Question.** On the emulator, how long from a local high-priority notification (standing in for a push) to the app
showing a request fetched from the host and an answer accepted by the host, with the app in the background, warm and
cold? What does that mean for the 60 s question window, and what does the real push test still need?

**Answer.** The machine part is small. Tap to accepted answer: **0.51 s median warm, 2.15 s median cold**. Almost all
of a 60 s window is left for the person: noticing the alert, unlocking, reading, deciding.

## Set-up

- Emulator `Teksa_Consumer`: Android 17 (API 37.1), x86_64, Google Play image with 16 KB pages, 2 GB RAM, on this PC.
- App: `spikes/mobile-frb/hm_spike`, release build, already paired with `spikes/mobile-host`.
- `AskReceiver` (Kotlin) is the push stand-in. `am broadcast` makes it post a content-free alert, "A session needs
  you", on a high-importance channel, which is what the relay's push would show. Tapping it opens `MainActivity` with
  the ask id, and the Dart side fetches `/c1/requests` and posts `/c1/actions` through the Rust mutual-TLS client.
- `drive.py` runs each case: open the app, press HOME, open an ask on the host, post the alert, (cold only) kill the
  process with `am kill` while the alert waits, open the shade, tap the alert, wait for the receipt.
- **Warm:** the process and Flutter engine are alive in the background, the TLS client is kept.
  **Cold:** no process at the tap; Android starts it, Flutter starts the engine, the app reads its pairing file and
  opens a new TLS connection.
- The human's "Allow" tap is not simulated: the app answers as soon as the request is on screen. There is no lock
  screen on the emulator, so unlock time is not included either.

## Results (5 runs each, ms)

All times are on this PC's clock. The tap time is stamped just before `adb shell input tap`; that command took
41–124 ms, so every figure from the tap is **high** by up to that much.

| | Warm median (min–max) | Cold median (min–max) |
|---|---|---|
| Alert posted → listed by the notification manager | 192 (89–250) | 201 (168–262) |
| Tap → `GET /c1/requests` reaches the host | 203 (78–492) | 2130 (1228–2266) |
| Tap → request on screen (estimate, see below) | 233 (88–534) | 2140 (1253–2293) |
| **Tap → answer accepted by the host** | **511 (338–552)** | **2147 (1256–2301)** |

Inside the app (its own stopwatch, from the moment Dart gets the tapped intent):

| | Warm median | Cold median |
|---|---|---|
| Activity got the intent → Dart has it | 1 | 892 (engine start, `RustLib.init`, first frame) |
| → request fetched | 278 | 558 (pairing file read, new TLS client and handshake) |
| → request rendered | 308 | 577 |
| → receipt back | 391 | 583 |

"On screen" is the host's fetch time plus the app's own fetch-to-rendered interval; the app's "shown" mark travels on a
separate request and lands late, so it is not used directly. `results.json` has every run.

Two things stand out. In the cold case, starting the process and the Flutter engine is about 1.5 s of the 2.1 s. In
the warm case, the first call on the kept TLS connection after the app comes back took 180–370 ms where a fresh run
takes 5 ms, so the app should reconnect or ping as soon as it resumes.

## What this means for the 60 s window

Roughly, for a person with the phone at hand:

| Step | Estimate | Source |
|---|---|---|
| Computer opens the request → push sent → alert on the phone | ~1–3 s with FCM/APNs high priority | to be measured (below) |
| Person notices and unlocks | a few seconds to never | not machine time |
| Tap → request on screen | 0.2 s warm, 2.1 s cold | this spike, emulator |
| Read, decide, tap Allow → accepted | a few seconds + 0.3 s | this spike for the 0.3 s |

So 60 s works when the phone is near the person, and the app's part is about 2 s of it in the worst measured case.
15 s (the window without a phone) leaves little room once unlocking and reading are counted, which supports the plan's
choice. The window still runs out in real use (phone in a pocket, Doze, no network), so the app must show "expired"
plainly and the receipt must be the only truth, as the action contract already says.

## What the real push test still needs

- **Android (FCM):** a Firebase project, `google-services.json` for `net.abushanab.harnessmanager`, and the relay
  holding the service account. Measure high-priority delivery with the screen off and in Doze, and with the app
  restricted by battery settings. Check that a notification message posts while the app process is dead, and that a
  data message still wakes it to fetch.
- **iOS (APNs):** a paid Apple Developer membership for the APNs key, the push entitlement and a real device (the
  simulator does not receive real pushes). Live Activity push updates (ActivityKit push tokens) need the same.
  Measure alert to Face ID unlock to request shown, cold launch on a real iPhone, and the time-sensitive interruption
  level against Focus modes.
- **Both:** a real lock screen and biometric unlock in the loop, mid-range phones (the emulator runs on a fast PC),
  and Wi-Fi to cellular through the hub mailbox, which adds a round trip the direct LAN path here does not have.

## Run it

```
cd spikes/mobile-host && ./target/release/mobile-spike-host serve --code 482913
# app installed and paired (see spikes/mobile-frb/README.md), emulator running
cd spikes/mobile-timing && python drive.py 5
```
