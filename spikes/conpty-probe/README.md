# ConPTY probe

Windows-only standalone Rust crate. It uses the Windows ConPTY APIs, a background output reader, and a kill-on-close Job Object. It saves the child’s VT byte stream in `raw.vt`. In a console, `run` relays local input and output and forwards viewport size changes. Use `headless` for a fixed initial size of 100 × 30 and scripted control.

Build and start the probe from the worktree root.

~~~powershell
cargo build --manifest-path spikes/conpty-probe/Cargo.toml --locked
$probe = (Resolve-Path spikes/conpty-probe/target/debug/conpty-probe.exe).Path
$session = Join-Path $env:TEMP ("hrm-conpty-" + [guid]::NewGuid().ToString("N"))
& $probe run $session "$env:SystemRoot\System32\cmd.exe" /d /k
~~~

Set `$probe` and `$session` to the same paths in a second PowerShell process. Send the test input from that process.

~~~powershell
& $probe text $session 'echo input-from-the-second-process'
& $probe enter $session
& $probe interrupt $session
& $probe stop $session
~~~

Use a new session directory under a private temp directory. Keep its `control` file private. It holds the ephemeral loopback access token and is removed on normal teardown. The log contains whatever the tool displays. Keep test data in the fixture and scrub any excerpts before sharing. The probe closes the process tree after ten minutes or on `stop`. The latter is immediate teardown. `interrupt` sends the Ctrl+C byte to the running session.

- `bytes DIR FILE` injects up to 16 KiB as provided. For bracketed paste use `ESC [ 200 ~`, the text bytes, and `ESC [ 201 ~` without the spaces.
- `resize DIR COLS ROWS` changes the child geometry. With `run`, resizing the outer console also changes the child geometry.

## Repeat the isolated tool probes

Python 3 supplies the loopback fixture and project hooks. The runner creates new HRM and tool homes under `%TEMP%`, invokes the installed HRM launcher, and deletes the temp tree on exit. It uses fixed local responses for the harmless shell and question probes.

~~~powershell
$claude = (Get-Command claude.exe).Source
$codex = (Get-ChildItem "$env:APPDATA/npm/node_modules/@openai" -Recurse -Filter codex.exe |
    Select-Object -First 1).FullName
python spikes/conpty-probe/run-fixture.py claude question --tool-exe $claude
python spikes/conpty-probe/run-fixture.py codex trust --tool-exe $codex
~~~

Both tools support `terminal-first`, `hook-first`, `timeout`, and `terminal`. The last case checks injected resize, bracketed paste, and Ctrl+C in a delayed turn. Pass `--hrm` or `--probe` to select another binary.

Run the `manual` case in Windows Terminal for a desktop comparison. The runner launches the tool directly and waits for you to exit. It then launches the same command through the probe, using the same disposable home and project. See the [Phase 0 comparison checklist](../../docs/plans/remote-control/spikes.md#4-windows-conpty--partly).
