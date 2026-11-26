"""Does Claude Code act on a PermissionRequest hook decision that arrives after the terminal answered?

Reuses spikes/conpty-probe (ConPTY probe, loopback model fixture, disposable homes). The fixture model asks for
`echo fixture-shell-ok | tee -a late-marker.txt`, so every run of the command adds a line to the marker file. The
hook waits for `decision.txt`, then prints what `hooks::decision()` prints for allow or deny.

  python late.py run [--repeat 3]                         A, B, C; the late decision 3 s after the terminal
  LATE_WAIT=75 python late.py run --cases A,B --out results-idle.json   watch the hook and events for 75 s
  python late.py serve | hook PROJECT    (used by the runner)
"""

import argparse
import ctypes
import importlib.util
import json
import os
import re
import sys
import time
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROBE_DIR = HERE.parent / "conpty-probe"
sys.dont_write_bytecode = True  # keeps conpty-probe free of __pycache__
sys.path.insert(0, str(PROBE_DIR))
CMD = "echo fixture-shell-ok | tee -a late-marker.txt"


def load_runner():
    spec = importlib.util.spec_from_file_location("run_fixture", PROBE_DIR / "run-fixture.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def serve():
    import fixture as base
    original = base.anthropic
    base.anthropic = lambda body: json.loads(json.dumps(original(body)).replace("echo fixture-shell-ok", CMD))
    with ThreadingHTTPServer(("127.0.0.1", 0), base.Handler) as server:
        print(server.server_port, flush=True)
        server.serve_forever()


def hook(project):
    """hooks::decision() output, once decision.txt appears; every step logged with a wall-clock stamp."""
    event = json.load(sys.stdin)
    log = project / "hook-events.txt"
    note = lambda s: log.open("a", encoding="utf-8").write(f"{time.time():.3f} {s}\n")
    (project / "hook-pid.txt").write_text(str(os.getpid()), encoding="utf-8")
    note(f"entered {event['hook_event_name']} {event['tool_name']} pid={os.getpid()}")
    release = project / "decision.txt"
    until = time.monotonic() + 100
    while not release.exists() and time.monotonic() < until:
        time.sleep(0.05)
    answer = release.read_text(encoding="utf-8").strip() if release.exists() else "none"
    d = {"behavior": "allow"} if answer == "allow" else {"behavior": "deny", "message": "Denied in Harness Manager."}
    note(f"printing {answer}")
    try:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": d}}), flush=True)
        sys.stdout.close()
        note(f"returned {answer} (stdout write ok)")
    except OSError as e:
        note(f"returned {answer} (stdout write failed: {e})")


def log_event(project):
    """Every other event Claude Code reports, to see what follows each terminal answer."""
    e = json.load(sys.stdin)
    extra = e.get("notification_type") or e.get("tool_name") or ""
    with (project / "other-events.txt").open("a", encoding="utf-8") as f:
        f.write(f"{e['hook_event_name']} {extra}".strip() + chr(10))


OTHER = ["PreToolUse", "PostToolUse", "PostToolUseFailure", "Notification", "UserPromptSubmit", "Stop", "SessionEnd"]


def alive(pid):
    k = ctypes.windll.kernel32
    h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not h:
        return False
    code = ctypes.c_ulong()
    k.GetExitCodeProcess(h, ctypes.byref(code))
    k.CloseHandle(h)
    return code.value == 259  # STILL_ACTIVE


def one(rf, args, case):
    with rf.workspace() as root:
        project = root / "project"
        project.mkdir()
        env = rf.environment(root, args)
        with rf.background([sys.executable, __file__, "serve"], env, project, stdout=rf.sp.PIPE, text=True) as model:
            port = int(model.stdout.readline())
            name, extra = rf.configure(root, project, args, f"http://127.0.0.1:{port}")
            rf.write_json(project / ".claude/settings.local.json", {
                "permissions": {"ask": [f"Bash({CMD})"]},
                "hooks": {"PermissionRequest": [{"matcher": "Bash", "hooks": [
                    {"type": "command", "command": sys.executable, "args": [__file__, "hook", str(project)], "timeout": 120}]}],
                    **{e: [{"hooks": [{"type": "command", "command": sys.executable, "args": [__file__, "log", str(project)]}]}] for e in OTHER}},
            })
            other = lambda: rf.read(project / "other-events.txt").strip().splitlines()
            hooklog, modellog, marker = project / "hook-events.txt", project / "model-events.txt", project / "late-marker.txt"
            runs = lambda: rf.read(marker).count("fixture-shell-ok")
            esc = project / "esc.bin"
            esc.write_bytes(b"\x1b")
            with rf.terminal(root, "session", [args.hrm, "run", name, "--", *extra], args, env, project) as (raw, key, proc):
                screen = lambda: rf.ANSI.sub("", rf.read(raw))
                flat = lambda: re.sub(r"\s+", "", screen().lower())
                rf.wait_for(lambda: "Claude Code v" in screen(), 30)
                rf.submit(key, "permission-probe")
                rf.wait_for(lambda: "entered PermissionRequest" in rf.read(hooklog), 20)
                rf.wait_for(lambda: "doyouwanttoproceed" in flat(), 20)
                pid = int(rf.read(project / "hook-pid.txt"))
                r = {"case": case}
                if case in ("A", "B"):
                    time.sleep(1)
                    mark = len(screen())
                    key("bytes", esc) if case == "A" else key("enter")
                    if case == "B":
                        rf.wait_for(lambda: "tool result allowed" in rf.read(modellog), 20)
                    waited = float(os.environ.get("LATE_WAIT", "3"))  # longer, to see whether an idle notification follows a "No"
                    t0, died = time.monotonic(), None
                    while time.monotonic() - t0 < waited:
                        if died is None and not alive(pid):
                            died = round(time.monotonic() - t0, 1)
                        time.sleep(0.2)
                    r.update(waited_s=waited, hook_died_after_terminal_s=died)
                    r.update(terminal_answer="No (Esc)" if case == "A" else "Yes (Enter)", runs_after_terminal=runs(),
                             hook_alive_after_terminal=alive(pid), model_after_terminal=rf.read(modellog).strip().splitlines(), events_after_terminal=other(),
                             screen_after_terminal=screen()[mark:][-240:].strip())
                    (project / "decision.txt").write_text("allow" if case == "A" else "deny", encoding="utf-8")
                    time.sleep(6)
                    r.update(hook_decision="allow" if case == "A" else "deny", runs_after_hook=runs(),
                             hook_alive_after_decision=alive(pid), hook_log=[l.split(" ", 1)[1] for l in rf.read(hooklog).splitlines()],
                             model_after_hook=rf.read(modellog).strip().splitlines(),
                             harness_message_on_screen="Denied in Harness Manager" in screen(), events_after_hook=other())
                else:
                    time.sleep(1)
                    (project / "decision.txt").write_text("allow", encoding="utf-8")
                    rf.wait_for(lambda: "tool result allowed" in rf.read(modellog), 20)
                    time.sleep(1)
                    r.update(hook_decision="allow", runs_after_hook=runs(), prompt_closed=flat().rfind("doyouwanttoproceed") < flat().rfind("fixture-result-seen"))
                    mark = len(screen())
                    key("bytes", esc)  # a late terminal "No"
                    time.sleep(4)
                    r.update(terminal_answer="No (Esc), late", runs_after_terminal=runs(), hook_log=[l.split(" ", 1)[1] for l in rf.read(hooklog).splitlines()],
                             model_after_terminal=rf.read(modellog).strip().splitlines(), events=other(),
                             screen_after_terminal=screen()[mark:][-240:].strip())
                r["session_alive"] = proc.poll() is None
                return r


def run(args):
    rf = load_runner()
    args.tool, args.case = "claude", "late"
    results = []
    for case in args.cases.split(","):
        for i in range(args.repeat):
            r = one(rf, args, case)
            r["n"] = i + 1
            print(json.dumps(r), flush=True)
            results.append(r)
    (HERE / args.out).write_text(json.dumps(results, indent=1), encoding="utf-8")


if __name__ == "__main__":
    if sys.argv[1] == "serve":
        serve()
    elif sys.argv[1] == "log":
        log_event(Path(sys.argv[2]))
    elif sys.argv[1] == "hook":
        hook(Path(sys.argv[2]))
    else:
        p = argparse.ArgumentParser(description=__doc__)
        p.add_argument("run")
        p.add_argument("--repeat", type=int, default=3)
        p.add_argument("--cases", default="A,B,C")
        p.add_argument("--out", default="results.json")
        p.add_argument("--tool-exe", type=Path, default=Path(os.environ["USERPROFILE"]) / ".local/bin/claude.exe")
        p.add_argument("--hrm", type=Path, default=Path(os.environ["LOCALAPPDATA"]) / "Harness Manager/hrm.exe")
        p.add_argument("--probe", type=Path, default=PROBE_DIR / "target/debug/conpty-probe.exe")
        run(p.parse_args())
