"""Timing spike driver: notification (stand-in for a push) -> tap -> app fetches the request over mutual TLS ->
request on screen -> answer accepted by the host. Every time is on this computer's clock: the tap is stamped here,
the rest by the spike host as the requests arrive (the app's own marks go to the host too).

Needs: `mobile-spike-host serve --code 482913` running, the spike app installed and paired, one emulator.
Usage: python drive.py [runs per mode, default 5]
"""
import json, re, subprocess, sys, time, urllib.request

PKG = "net.abushanab.hm_spike"
HOST = "http://127.0.0.1:47801"


def now():
    return time.time() * 1000


def sh(*a):
    return subprocess.run(["adb", "shell", *a], capture_output=True, text=True).stdout


def call(path, body=None):
    rq = urllib.request.Request(HOST + path, data=None if body is None else json.dumps(body).encode(),
                                headers={"content-type": "application/json"}, method="GET" if body is None else "POST")
    with urllib.request.urlopen(rq, timeout=10) as r:
        t = r.read()
        return json.loads(t) if t else None


def pid():
    return sh("pidof", PKG).strip()


def notification_bounds():
    """Centre of the expanded notification titled 'A session needs you', from a UI dump."""
    for _ in range(10):
        sh("uiautomator", "dump", "/sdcard/ui.xml")
        xml = sh("cat", "/sdcard/ui.xml")
        m = re.search(r'text="A session needs you"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', xml)
        if m:
            x1, y1, x2, y2 = map(int, m.groups())
            return (x1 + x2) // 2, (y1 + y2) // 2
        time.sleep(0.3)
    raise RuntimeError("the notification did not show in the shade")


def one(mode):
    # a known starting state: the app opened, then sent to the background with HOME
    sh("am", "start", "-W", "-n", f"{PKG}/.MainActivity")
    time.sleep(3)
    sh("input", "keyevent", "KEYCODE_HOME")
    time.sleep(2)
    ask = call("/spike/ask", {"what": "rm -rf build/"})
    t_post = now()
    sh("am", "broadcast", "-n", f"{PKG}/.AskReceiver", "--es", "ask", ask["ask_id"])
    # posted -> listed by the notification manager
    while "A session needs you" not in sh("dumpsys", "notification", "--noredact"):
        if now() - t_post > 10000:
            raise RuntimeError("no notification")
    t_listed = now()
    if mode == "cold":
        # the process dies while the alert waits, as when Android reclaims it or a push arrives for a dead app
        sh("am", "kill", PKG)
        time.sleep(1)
        if pid():
            sh("am", "kill", PKG)
            time.sleep(1)
        assert not pid(), "the app process is still alive"
    sh("cmd", "statusbar", "expand-notifications")
    x, y = notification_bounds()
    alive = bool(pid())
    t_tap = now()
    sh("input", "tap", str(x), str(y))
    t_tap_sent = now()
    deadline = now() + 30000
    while True:
        log = call("/spike/log")
        mine = [e for e in log if e.get("ask_id") == ask["ask_id"] or (e["event"] == "c1-requests" and e["at"] >= t_tap)]
        done = [e for e in mine if e["event"] == "app-answered"]
        if done or now() > deadline:
            break
        time.sleep(0.2)
    first = lambda ev: next((e for e in mine if e["event"] == ev), None)
    req, shown, action, answered = first("c1-requests"), first("app-shown"), first("c1-action"), first("app-answered")
    if not (req and shown and action):
        raise RuntimeError(f"incomplete run: {mine}")
    return {
        "mode": mode, "process_alive_at_tap": alive,
        "post_to_listed_ms": round(t_listed - t_post), "tap_cmd_ms": round(t_tap_sent - t_tap),
        "tap_to_fetch_ms": round(req["at"] - t_tap), # the app's "shown" mark rides a separate fire-and-forget request, so it lands late: shown is the host's
        # fetch time plus the app's own fetch -> rendered interval
        "tap_to_shown_ms": round(req["at"] - t_tap + shown["shown_ms"] - shown["fetch_ms"]),
        "tap_to_accepted_ms": round(action["at"] - t_tap), "receipt": action["status"],
        "app_intent_to_dart_ms": shown["intent_to_dart_ms"], "app_fetch_ms": shown["fetch_ms"], "app_shown_ms": shown["shown_ms"],
        "app_done_ms": answered["done_ms"] if answered else None, "reused_tls_client": shown["reused_client"],
    }


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    sh("pm", "grant", PKG, "android.permission.POST_NOTIFICATIONS")
    sh("settings", "put", "global", "heads_up_notifications_enabled", "1")
    results = []
    for mode in ["warm", "cold"]:
        for i in range(n):
            call("/spike/reset", {})
            r = one(mode)
            print(json.dumps(r), flush=True)
            results.append(r)
            sh("cmd", "statusbar", "collapse")
    for mode in ["warm", "cold"]:
        rs = [r for r in results if r["mode"] == mode]
        for k in ["tap_to_fetch_ms", "tap_to_shown_ms", "tap_to_accepted_ms", "post_to_listed_ms"]:
            v = sorted(r[k] for r in rs)
            print(f"{mode:5} {k:20} median {v[len(v) // 2]:5}  min {v[0]:5}  max {v[-1]:5}")
    json.dump(results, open("results.json", "w"), indent=1)


if __name__ == "__main__":
    main()
