# Phase 0 remote-control spikes

Windows 11 build 26300, Harness Manager 1.12.1 at `7a3f28f`, Claude Code 2.1.330, Codex CLI 0.171.0. The installed `hrm.exe` launched the installed tools. Its `run` path inherits console stdio from its parent, so the same launcher also works as the ConPTY child.

The live profiles were excluded. At this revision `hrm run` opens the HRM database in read/write mode before dispatch and calls profile preparation. Claude also persists folder trust and session state in its profile. Codex persists hook trust and session state in its profile. The probes used disposable `HRM_HOME` and `HRM_PROFILES_HOME` directories under `%TEMP%`, synthetic profiles, and a loopback model fixture. The hooks, prompts, shell execution, terminal and interrupt handling were from the installed clients. No vendor model turns were used.

The [probe README](../../../spikes/conpty-probe/README.md) gives build and replay commands. The [evidence directory](spike-logs/) contains selected terminal and side-log excerpts. VT control sequences and local paths were removed, and spacing was normalized where needed.

## 1. Claude Code interactive permission — yes

A Bash `PermissionRequest` command hook waited for a decision file. The project had an explicit ask rule for `echo fixture-shell-ok`. Claude showed the terminal prompt while the hook was still blocked.

| Order | Observed result |
| --- | --- |
| Terminal approval first, hook denial later | After the prompt settled, Enter approved the command. The model fixture received the tool result with the hook still blocked. The hook subsequently reported deny. The completed tool result stood. |
| Terminal denial first, hook approval later | Selecting No produced `Interrupted · What should Claude do instead?` while the hook was blocked. The hook subsequently reported allow. The command stayed unrun. |
| Hook denial first, terminal Enter later | The hook closed the visible dialog and Claude showed `Denied by PermissionRequest hook`. The later Enter did not change the decision. |
| Four-second hook timeout | The hook produced no decision before the deadline. The terminal dialog remained available. Enter approved the command and the tool result reached the fixture. A replay released the waiting Python child after observing the timeout, and that child wrote a return marker. |

Two earlier attempts sent Enter close to the first dialog paint. The late hook denial decided those attempts. The successful terminal-first runs waited about four seconds before sending the key. The minimum reliable time after paint and the cause of the early behavior are open.

Replay `python spikes/conpty-probe/run-fixture.py claude terminal-first --tool-exe <claude.exe>`, then use `hook-first` and `timeout`. The underlying launch was `hrm run claude-fixture -- --settings fixture-env.json --model haiku --permission-mode default`. The fresh Claude folder prompt defaulted to exit in the headless probe. Menu navigation flipped forward and back on the injected arrow. The successful scenarios pre-seeded trust for the disposable project only. [Claude permission evidence](spike-logs/claude-permission.txt).

**Plan impact.** Phase 2 can keep local and remote Claude choices available together. Closing a card on a terminal decision requires a signal independent of hook exit. A late remote answer can arrive after local execution. Invalidate it per request. Bound the hook command's own wait and leave the regular prompt available on expiry. Phase 3 and Phase 5 inherit these rules.

## 2. Claude Code interactive question — yes

The loopback fixture issued `AskUserQuestion` during an interactive turn. The `PreToolUse` hook returned `permissionDecision: "allow"` and `updatedInput` with the original questions and an `answers` map. Claude showed the recorded answer and the model fixture received `Blue` in the tool result. No answer keystroke was sent. This was an ordinary interactive launch without `-p`.

Replay `python spikes/conpty-probe/run-fixture.py claude question --tool-exe <claude.exe>`. [Claude question evidence](spike-logs/claude-question.txt). [Claude Code hooks reference](https://code.claude.com/docs/en/hooks#tools-that-require-user-interaction).

**Plan impact.** Phase 2 can include Claude question cards through this hook. It still needs a question-specific expiry and a check of terminal fallback while that hook waits.

## 3. Codex interactive permission and trust — partly

Codex loaded the disposable project's `.codex/hooks.json` after folder trust. On a shell escalation, the TUI reported a running hook and showed no terminal approval choices while the hook waited. Enter sent in that interval did not approve the command. A hook denial resolved the request directly, including when Enter was sent afterward. With a six-second hook timeout, Codex reported the timeout and then opened its ordinary approval dialog. Enter approved the command on that dialog. In the replay, the waiting Python hook child was still alive after the timeout and wrote its return marker when released.

The startup folder prompt and the hook prompt were separate. The second offered review, trust all, or continue with hooks inactive. The review browser showed one installed `PermissionRequest` handler with one to review. Its detail view showed the event, matcher, source, command, mode, timeout and trust status. Selecting `Trust all and continue` activated it. Trust survived a restart, a change to the referenced Python script's bytes, and a change to top-level JSON description metadata. Changing the handler timeout brought hook review back.

For a non-managed command hook from project JSON, the installed version keys trust by source path, event, matcher-group position and handler position. It compares the stored hash with the normalized handler declaration, which includes the matcher and handler settings such as the command and timeout. This is separate from folder trust. A later HRM edit to the registered command or timeout needs review. Replacing the script at an unchanged command path did not. The tested scope is a project hook. [Codex hook discovery and hash](https://github.com/openai/codex/blob/rust-v0.171.0/codex-rs/hooks/src/engine/discovery.rs), [Codex hook state key](https://github.com/openai/codex/blob/rust-v0.171.0/codex-rs/hooks/src/lib.rs), [Codex hook docs](https://learn.chatgpt.com/docs/hooks).

Replay `python spikes/conpty-probe/run-fixture.py codex timeout --tool-exe <codex.exe>`, then use `terminal-first`, `hook-first`, and `trust`. The replay launches `hrm run codex-fixture -- --no-daemon --ask-for-approval on-request --sandbox read-only permission-probe`. The scripted response requests approval for the local echo command. The timeout replay cancels from the terminal after the dialog appears. In the exploratory approval run Codex ran the echo command, and Windows held the temporary project directory open during cleanup. `--no-daemon` avoids the Windows socket path length limit with temporary homes. [Codex permission and trust evidence](spike-logs/codex-permission-trust.txt).

**Plan impact.** Phase 1 needs a user-visible review step for a new Codex declaration and should keep the declaration stable when rotating per-session addresses. Phase 2 needs a short expiry or an explicit way to release the hook without a decision so local approval can appear. Bound the hook command's own wait so it can exit independently of Codex's timeout. A simultaneous terminal-first acceptance criterion applies to Claude only. Phase 3 and Phase 5 need the same Codex handoff.

## 4. Windows ConPTY — partly

The probe used the Windows APIs through `windows-sys` 0.61.2. A preliminary `portable-pty` 0.9.0 headless attempt stalled before output with its default cursor-inheritance flag. The direct Windows API probe worked with flags set to zero. The child starts suspended and joins a cleanup job before it can spawn descendants. [Microsoft ConPTY reference](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session).

| Check | Evidence |
| --- | --- |
| Direct and relayed input | In a hidden native console, both tools accepted the same Win32 keyboard events in direct and foreground-relayed runs. Both displayed the fixture response. Codex used `--no-daemon` in this comparison. |
| Resize | Starting at 100 × 30, the second process requested 72 × 22 and both TUIs emitted redraws. A shell child reported both exact sizes. The hidden native window normally stayed at 120 × 30. With Codex's `--no-alt-screen` option, resizing the outer console to 72 × 22 visibly reflowed the relayed Hooks browser in the console capture. Physical Windows Terminal window resizing remains open. |
| Paste and second-process reply | Both TUI streams enabled bracketed paste. Both received a three-line UTF-8 paste in one prompt, with no model request before Enter. A separate plain-text input started each delayed second turn. |
| Second-process interrupt | Both accepted injected Ctrl+C during the delayed turn and stayed open. Claude restored its composer before a fixture reply. Codex displayed `Conversation interrupted`. |
| Mouse and colours | Direct and relayed console captures showed multiple colour attributes for both tools. Both emitted truecolour SGR as ConPTY children. Claude emitted SGR mouse mode during the observed run. An SGR mouse-mode request was absent in the Codex capture. Mouse input, actual colour fidelity and platform clipboard parity need a desktop comparison. |

Replay `python spikes/conpty-probe/run-fixture.py claude terminal --tool-exe <claude.exe>` and the equivalent `codex terminal` command. The automated comparison used the second process's `resize`, `bytes`, `text`, `enter` and `interrupt` commands. [ConPTY evidence](spike-logs/windows-conpty.txt).

For a desktop comparison, start the README's `manual` case in Windows Terminal. It starts direct and wrapped runs in the same terminal with the same disposable project. Make these checks in each run.

1. Resize from wide to narrow and back with a long input or open permission dialog. Check reflow, wrapping and selection position.
2. Paste a line containing spaces and a non-ASCII character with the normal clipboard shortcut. Paste three lines the same way. Check characters, line breaks and that the buffer waits for Enter. Inspect `raw.vt` in the wrapped run for bracketed-paste mode and compare with the scripted paste.
3. Scroll transcript history. Click and drag to select text, then test click and wheel inside a tool menu where supported. Compare which events the tool consumes and which Windows Terminal consumes.
4. Keep the terminal theme fixed. Compare prompt, status, selection and error colours side by side. Check for flicker and a restored cursor and input mode after exit.

**Plan impact.** Phase 4 can prototype typed replies and Stop behind its setting, using direct ConPTY calls as a candidate. Keep feature acceptance open until the desktop comparison passes. Phase 5 can rely on typed reply and interrupt after Phase 4 validates the terminal behavior.
