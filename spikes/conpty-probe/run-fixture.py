"""Run an installed CLI through hrm with disposable homes and a loopback model."""

import argparse
import json
import os
import re
import shutil
import subprocess as sp
import sys
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixture.py"
ANSI = re.compile(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]")


def read(path):
    return path.read_text("utf-8", errors="replace") if path.exists() else ""


def wait_for(check, seconds=10):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if check():
            return
        time.sleep(0.05)
    raise TimeoutError("expected fixture evidence did not arrive")


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value), encoding="utf-8")


def environment(root, args):
    inherited = {"SYSTEMROOT", "WINDIR", "COMSPEC", "PROGRAMFILES", "PROGRAMFILES(X86)",
                 "PROGRAMDATA", "COMMONPROGRAMFILES", "PATH", "PATHEXT", "NUMBER_OF_PROCESSORS"}
    env = {key: value for key, value in os.environ.items() if key.upper() in inherited}
    home = root / "home"
    paths = {"HRM_HOME": root / "hrm", "HRM_PROFILES_HOME": home, "HOME": home,
             "USERPROFILE": home, "APPDATA": home / "AppData/Roaming",
             "LOCALAPPDATA": home / "AppData/Local", "TEMP": root / "tmp", "TMP": root / "tmp"}
    for path in paths.values():
        path.mkdir(parents=True, exist_ok=True)
    env.update({key: str(path) for key, path in paths.items()})
    env["PATH"] = str(Path(sys.executable).parent) + os.pathsep + env.get("PATH", "")
    env.update(TERM="xterm-256color", COLORTERM="truecolor", NO_PROXY="localhost,127.0.0.1",
               DISABLE_TELEMETRY="1", DISABLE_ERROR_REPORTING="1", DISABLE_AUTOUPDATER="1",
               DISABLE_NONESSENTIAL_MODEL_CALLS="1", CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
               POWERSHELL_TELEMETRY_OPTOUT="1", DOTNET_CLI_TELEMETRY_OPTOUT="1")
    bash = Path(env.get("ProgramFiles", r"C:\Program Files")) / "Git/bin/bash.exe"
    if bash.is_file():
        env["CLAUDE_CODE_GIT_BASH_PATH"] = str(bash)
    env[f"HRM_BIN_{args.tool.upper()}"] = str(args.tool_exe.resolve(strict=True))
    return env


def configure(root, project, args, url):
    name = f"{args.tool}-fixture"
    write_json(root / "hrm/profiles.json", {"profiles": [
        {"name": name, "harness": args.tool, "kind": "subscription"}
    ]})
    config = root / "home" / f".{args.tool}-{name}"
    config.mkdir()
    timeout = (4 if args.tool == "claude" else 6) if args.case == "timeout" else 25
    if args.tool == "claude":
        # The Windows fixture is owned by this runner, and its trust is disposable.
        write_json(config / ".claude.json", {"hasCompletedOnboarding": True, "projects": {
            str(project): {"hasTrustDialogAccepted": True},
            project.as_posix(): {"hasTrustDialogAccepted": True},
        }})
        write_json(project / "fixture-env.json", {"env": {
            "ANTHROPIC_BASE_URL": url, "ANTHROPIC_AUTH_TOKEN": "probe-local-only",
        }})
        event = "PreToolUse" if args.case == "question" else "PermissionRequest"
        matcher = "AskUserQuestion" if args.case == "question" else "Bash"
        handler = {"type": "command", "command": sys.executable,
                   "args": [str(FIXTURE), "hook", str(project)], "timeout": timeout}
        write_json(project / ".claude/settings.local.json", {
            "permissions": {"ask": ["Bash(echo fixture-shell-ok)"]},
            "hooks": {event: [{"matcher": matcher, "hooks": [handler]}]},
        })
        return name, ["--settings", "fixture-env.json", "--model", "haiku", "--permission-mode", "default"]

    (config / "config.toml").write_text(
        'model = "gpt-5.4-mini"\nmodel_provider = "fixture"\n'
        'cli_auth_credentials_store = "file"\ncheck_for_update_on_startup = false\n'
        '[model_providers.fixture]\nname = "Fixture"\n'
        f'base_url = "{url}/v1"\nwire_api = "responses"\n'
        '[windows]\nsandbox = "unelevated"\n', encoding="utf-8")
    executable = project / "hook_fixture.py"
    shutil.copyfile(FIXTURE, executable)
    handler = {"type": "command", "command": f'{Path(sys.executable).name} "{executable}" hook "{project}"',
               "timeout": timeout}
    write_json(project / ".codex/hooks.json", {"hooks": {
        "PermissionRequest": [{"matcher": "Bash", "hooks": [handler]}],
    }})
    extra = ["--no-daemon", "--ask-for-approval", "on-request", "--sandbox", "read-only"]
    if args.case not in ("terminal", "trust", "manual"):
        extra.append("permission-probe")
    return name, extra


@contextmanager
def background(command, env, cwd, **streams):
    proc = sp.Popen(command, env=env, cwd=cwd, creationflags=sp.CREATE_NO_WINDOW, **streams)
    try:
        yield proc
    finally:
        if proc.poll() is None:
            proc.kill()
        proc.wait(timeout=5)


@contextmanager
def workspace():
    temporary = tempfile.TemporaryDirectory(prefix="hrm-rc-phase0-")
    try:
        yield Path(temporary.name)
    finally:
        for attempt in range(30):
            try:
                temporary.cleanup()
                break
            except PermissionError:
                if attempt == 29:
                    raise
                time.sleep(0.1)


@contextmanager
def terminal(root, name, command, args, env, project):
    directory = root / name
    def key(op, *values):
        sp.run([args.probe, op, directory, *values], env=env, check=True, timeout=5,
               creationflags=sp.CREATE_NO_WINDOW)
    with background([args.probe, "headless", directory, *command], env, project) as proc:
        try:
            wait_for(lambda: (directory / "control").exists())
            yield directory / "raw.vt", key, proc
        finally:
            if proc.poll() is None and (directory / "control").exists():
                key("stop")
                proc.wait(timeout=5)


def submit(key, prompt):
    key("text", prompt)
    time.sleep(0.75)
    key("enter")


def check_permission(args, project, screen, key):
    hook = project / "hook-events.txt"
    model = project / "model-events.txt"
    wait_for(lambda: "PermissionRequest Bash entered" in read(hook))
    question = "doyouwanttoproceed" if args.tool == "claude" else "wouldyouliketorunthefollowing"
    prompt = lambda: question in re.sub(r"\s+", "", screen().lower())
    if args.tool == "claude":
        wait_for(prompt)
    else:
        assert not prompt()
    print(f"Terminal prompt while hook blocked = {prompt()}")
    if args.case == "timeout":
        time.sleep(4.5 if args.tool == "claude" else 6.5)
        wait_for(prompt)
        assert "returned" not in read(hook)
        print("Terminal prompt after hook timeout")
        (project / "decision.txt").write_text("fallthrough", encoding="utf-8")
        if args.tool == "claude":
            key("enter")
            wait_for(lambda: "Claude tool result allowed" in read(model), 18)
            wait_for(lambda: "fixture-result-seen" in screen(), 18)
        else:
            escape = project / "esc.bin"
            escape.write_bytes(b"\x1b")
            key("bytes", escape)
            wait_for(lambda: "fixture-result-seen" in screen() or "interrupted" in screen().lower())
            print("Terminal Escape sent after Codex displayed approval choices")
    elif args.case == "terminal-first":
        time.sleep(4)
        key("enter")
        if args.tool == "claude":
            wait_for(lambda: "Claude tool result allowed" in read(model))
            assert "returned" not in read(hook)
            print("Claude tool result allowed before hook returned")
        (project / "decision.txt").write_text("deny", encoding="utf-8")
        wait_for(lambda: "returned deny" in read(hook))
        if args.tool == "codex":
            wait_for(lambda: "fixture-hook-denied" in screen())
    else:
        (project / "decision.txt").write_text("deny", encoding="utf-8")
        wait_for(lambda: "fixture-hook-denied" in screen())
        key("enter")
        print("Terminal Enter sent after hook denial")
    print(read(hook).strip(), read(model).strip(), sep="\n")


def check_terminal(root, project, raw, screen, key, proc):
    model = project / "model-events.txt"
    before = raw.stat().st_size
    key("resize", "72", "22")
    wait_for(lambda: raw.stat().st_size > before)
    print(f"Output grew after resize from {before} to {raw.stat().st_size} bytes")
    paste = root / "paste.bin"
    paste.write_bytes(b"\x1b[200~probe-first\nprobe-second\nprobe-third\x1b[201~")
    key("bytes", paste)
    time.sleep(0.6)
    assert not read(model)
    key("enter")
    time.sleep(0.7)
    if not read(model):
        key("enter")
    wait_for(lambda: "prompt multiline" in read(model))
    wait_for(lambda: "fixture-text-seen" in screen())
    submit(key, "interrupt-probe")
    time.sleep(0.6)
    if "interrupt request started" not in read(model):
        key("enter")
    wait_for(lambda: "interrupt request started" in read(model), 4)
    before_interrupt = len(screen())
    key("interrupt")
    wait_for(lambda: "interrupt" in screen()[before_interrupt:].lower(), 3)
    print(read(model).strip())
    print(f"Session alive after injected Ctrl+C = {proc.poll() is None}")
    print("Tail after Ctrl+C", screen()[before_interrupt:][-300:].strip())


def run(args):
    with workspace() as root:
        project = root / "project"
        project.mkdir()
        env = environment(root, args)
        with background([sys.executable, FIXTURE, "serve"], env, project, stdout=sp.PIPE,
                        text=True) as model:
            port = int(model.stdout.readline())
            profile, extra = configure(root, project, args, f"http://127.0.0.1:{port}")
            command = [args.hrm, "run", profile, "--", *extra]
            if args.case == "manual":
                print("Direct launch. Exit the tool to start the ConPTY comparison.", flush=True)
                sp.run(command, env=env, cwd=project, check=False)
                print(f"ConPTY launch. Probe session directory {root / 'session'}", flush=True)
                sp.run([args.probe, "run", root / "session", *command], env=env, cwd=project,
                       check=True)
                return

            def clean(raw):
                return ANSI.sub("", read(raw))

            with terminal(root, "session", command, args, env, project) as (raw, key, proc):
                screen = lambda: clean(raw)
                if args.tool == "claude":
                    wait_for(lambda: "Claude Code v" in screen())
                else:
                    wait_for(lambda: "Trust and continue" in screen())
                    key("enter")
                    wait_for(lambda: "hook is new or changed" in screen())
                    print("Codex displayed separate project and hook trust prompts")
                    key("text", "2")
                    time.sleep(0.3)
                    key("enter")
                    wait_for(lambda: "gpt-5.4-mini" in screen())
                if args.case == "terminal":
                    check_terminal(root, project, raw, screen, key, proc)
                elif args.case == "question":
                    submit(key, "question-probe")
                    wait_for(lambda: "Claude tool result Blue" in read(project / "model-events.txt"))
                    print(read(project / "hook-events.txt").strip())
                    print("Claude tool result Blue")
                elif args.case != "trust":
                    if args.tool == "claude":
                        submit(key, "permission-probe")
                    check_permission(args, project, screen, key)

            if args.case == "trust":
                config = project / ".codex/hooks.json"
                for change in ("same", "script-only", "metadata", "handler-timeout"):
                    if change == "script-only":
                        with (project / "hook_fixture.py").open("a", encoding="utf-8") as f:
                            f.write("\n# Replacement fixture script bytes.\n")
                    elif change in ("metadata", "handler-timeout"):
                        hooks = json.loads(read(config))
                        if change == "metadata":
                            hooks["description"] = "Metadata only"
                        else:
                            hooks["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] += 1
                        write_json(config, hooks)
                    with terminal(root, change, command, args, env, project) as (raw, _, _):
                        wait_for(lambda: any(s in clean(raw) for s in ("gpt-5.4-mini", "hook is new or changed")))
                        needs_review = "hook is new or changed" in clean(raw)
                        print(f"Restart after {change}: hook review = {needs_review}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tool", choices=["claude", "codex"])
    parser.add_argument("case", choices=["terminal-first", "hook-first", "timeout", "question",
                                         "trust", "terminal", "manual"])
    parser.add_argument("--tool-exe", type=Path, required=True)
    parser.add_argument("--hrm", type=Path, default=Path(os.environ["LOCALAPPDATA"]) / "Harness Manager/hrm.exe")
    parser.add_argument("--probe", type=Path, default=HERE / "target/debug/conpty-probe.exe")
    options = parser.parse_args()
    options.probe = options.probe.resolve(strict=True)
    options.hrm = options.hrm.resolve(strict=True)
    if (options.case == "trust" and options.tool != "codex") or (options.case == "question" and options.tool != "claude"):
        parser.error("trust is Codex-only and question is Claude-only")
    run(options)
