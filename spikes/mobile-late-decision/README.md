# Spike: a permission decision that arrives after the terminal answered

**Question.** Claude Code shows its permission prompt in the terminal while the `PermissionRequest` hook (hrm's
`hooks::handle`) waits for the app's or the phone's answer. If the person answers in the terminal first and the hook
prints a decision afterwards, does Claude Code act on it? And what happens to the hook process?

**Answer.** Claude Code 2.1.332 uses only the first answer. A hook decision that arrives after the terminal answered is
read without error and ignored, in both directions. A hook decision that arrives first closes the prompt, and terminal
keys after that change nothing. The trouble is on hrm's side: after a terminal **No** Claude Code reports no event at
all, so hrm's request stays open and a phone answer in that gap would be stored, printed and reported as accepted
while nothing happens.

## Set-up

- Claude Code **2.1.332** (`claude --version`), launched by `hrm run` 1.13.2 inside the ConPTY probe
  (`spikes/conpty-probe`) against its loopback model fixture: disposable homes under `%TEMP%`, no real model calls.
- The fixture model asks for `echo fixture-shell-ok | tee -a late-marker.txt` (an `ask` rule makes sure it prompts).
  Each run of the command adds a line to the marker file, so the line count is the number of times it ran.
- The project's `PermissionRequest` hook (matcher `Bash`, timeout 120 s) records its pid, waits for `decision.txt`,
  then prints exactly what `hooks::decision()` prints:
  `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`, or `deny` with
  `"message":"Denied in Harness Manager."`. It logs whether the write to stdout succeeded.
- A second hook on `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `Notification`, `UserPromptSubmit`, `Stop` and
  `SessionEnd` logs every event, to see what Claude Code reports after each terminal answer.
- The terminal "No" is **Esc**, the prompt's "No (esc)". Selecting the numbered "No" entry was not tried.
- `late.py` runs everything; `results.json` and `results-idle.json` hold every run.

## Results (each case 3 times, all runs alike)

| Case | Order | Command ran | Late input acted on? | Hook process | Events reported |
|---|---|---|---|---|---|
| A | terminal **No**, then hook **allow** 3 s later | 0 times | **No.** The hook printed allow, its stdout write succeeded, nothing ran and nothing went to the model | alive 3 s after the Esc; ran to the end | `UserPromptSubmit`, `PreToolUse` only. No `Stop`, no `PostToolUseFailure`, no `Notification` |
| B | terminal **Yes**, then hook **deny** 3 s later | once | **No.** Nothing changed on screen, no "Denied in Harness Manager", no new model request | alive; ran to the end | `PostToolUse`, `Stop` |
| C | hook **allow**, then terminal **No** (Esc) | once | **No.** The hook's allow closed the prompt and the command ran; the late Esc did nothing | returned normally | `PostToolUse`, `Stop` |

How long Claude Code keeps the hook after the terminal answers (A and B with a 75 s watch, 3 runs each, the late
decision held back until the end):

| After | Hook process | Events within 75 s |
|---|---|---|
| terminal **No** | **ended 10.2 s, 13.5 s, 14.2 s after the Esc**, without printing (well before its own 100 s wait and the 120 s hook timeout, so Claude Code ended it) | none after `PreToolUse`; no `idle_prompt` either |
| terminal **Yes** | **left running** for the full 75 s; Claude Code neither killed it nor closed its pipe (the late deny was written without error) | `PostToolUse`, `Stop`, then `Notification idle_prompt` |

So: first answer wins, whichever side gives it. Claude Code does not stop a hook that lost to a terminal "Yes"; it
lets it run until it returns or reaches its timeout. After a terminal "No" it ends the hook some 10–14 s later.

## What it means for a phone answer that arrives after the terminal answered

Claude Code never acts on the late answer, so the danger is a wrong receipt, not a wrong action. hrm only knows the
terminal answered from the events it receives (`hooks::handle` closes open asks on later events):

- **Terminal Yes.** `PostToolUse` closes the request, and only once the command has **finished**. Until then the
  request is open and the hook is alive, so a phone "deny" sent while an approved command is still running is stored,
  printed and reported as accepted, but the command keeps running. For a long build or test command that window is
  as long as the command.
- **Terminal No.** No event comes at all. The request stays open until Claude Code ends the hook 10–14 s later; after
  that `answer()` refuses because `hook_is_live` is false. In those 10–14 s a phone "allow" is reported as accepted
  while nothing runs. There is no later `Stop` or `idle_prompt` to close it.
- **Phone first.** The phone's answer is the one that counts, and the terminal prompt goes away. This is the case
  the receipts already describe correctly.

What the receipt contract (`POST /c1/actions`) should take from this:

1. "accepted" should mean "the computer delivered the answer to Claude Code", not "Claude Code acted on it". For an
   allow, confirm it with the `PostToolUse` of that very call (same tool and input) and say "done" only then. For a deny,
   the confirmation is that the prompt is gone and no `PostToolUse` for that call follows.
2. hrm owns the terminal of a session it launched (`terminal.rs`), so it can read the prompt's state from the screen:
   when "Do you want to proceed?" is no longer on screen, the terminal has answered, and the request should close at
   once with "already handled". That covers both gaps above. It is the most direct signal; the spike did not build it.
3. Until one of those exists, the phone should say what is known: "Sent to the computer" for a late or uncertain
   delivery, rather than "Approved".

## Run it

```
cargo build --manifest-path spikes/conpty-probe/Cargo.toml --locked
python spikes/mobile-late-decision/late.py run --repeat 3
LATE_WAIT=75 python spikes/mobile-late-decision/late.py run --repeat 3 --cases A,B --out results-idle.json
```

Defaults: `claude.exe` from `%USERPROFILE%\.local\bin`, `hrm.exe` from `%LOCALAPPDATA%\Harness Manager`; override
with `--tool-exe`, `--hrm`, `--probe`. Under a minute per run, 75 s more with `LATE_WAIT=75`.
