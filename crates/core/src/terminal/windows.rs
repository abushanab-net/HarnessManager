use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::Console::*;
use windows_sys::Win32::System::JobObjects::*;
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::IO::CancelSynchronousIo;

use super::control::{Action, Command, Control, Outcome};
use crate::{launch::LaunchSpec, Ctx};

const TICK: Duration = Duration::from_millis(40);
const BRACKETED_PASTE: u8 = 1;
const WIN32_INPUT: u8 = 2;
// Win32 input records carry the virtual key, scan code, UTF-16 unit, down flag, modifiers and repeat.
const WIN32_CTRL_C: &[u8] = b"\x1b[67;46;3;1;8;1_\x1b[67;46;3;0;8;1_";
const WIN32_ENTER: &[u8] = b"\x1b[13;28;13;1;0;1_\x1b[13;28;13;0;0;1_";
pub const SHIM_COMMAND: &str = "__terminal-shim";

/// Batch launchers need Rust's Windows Command quoting, especially for metacharacters.
pub fn run_shim(argv: &[OsString]) -> Result<i32> {
    unsafe extern "system" fn handle(event: u32) -> i32 {
        // A registered handler is not inherited, so the tool can handle Ctrl+C normally.
        i32::from(event == CTRL_C_EVENT)
    }
    let (program, args) = argv.split_first().context("missing batch launcher")?;
    unsafe { win_ok(SetConsoleCtrlHandler(Some(handle), 1))? };
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("start {}", program.to_string_lossy()))?;
    Ok(status.code().unwrap_or(1))
}

pub(crate) fn run(ctx: &Ctx, id: &str, spec: &LaunchSpec) -> Result<i32> {
    let input = ConsoleMode::set(
        io::stdin().as_raw_handle(),
        ENABLE_EXTENDED_FLAGS
            | ENABLE_WINDOW_INPUT
            | ENABLE_MOUSE_INPUT
            | ENABLE_VIRTUAL_TERMINAL_INPUT,
    )?;
    let output = ConsoleMode::set(
        io::stdout().as_raw_handle(),
        ENABLE_PROCESSED_OUTPUT
            | ENABLE_WRAP_AT_EOL_OUTPUT
            | ENABLE_VIRTUAL_TERMINAL_PROCESSING
            | DISABLE_NEWLINE_AUTO_RETURN,
    )?;
    let mut size = output
        .as_ref()
        .map(ConsoleMode::size)
        .transpose()?
        .unwrap_or(COORD { X: 100, Y: 30 });
    let job = cleanup_job()?;
    let (pty, reader, writer) = Pty::open(size)?;
    let relay = Arc::new(Relay {
        writer: Mutex::new(writer),
        modes: AtomicU8::new(0),
        closed: AtomicBool::new(false),
        withheld: AtomicBool::new(false),
        db: ctx.data.join("hm.db"),
        run: id.to_string(),
    });
    let display = {
        let relay = relay.clone();
        let console = output.is_some();
        thread::Builder::new().spawn(move || relay_output(reader, &relay.modes, console))?
    };

    let mut keyboard = None;
    let result = (|| {
        let child = pty.spawn(spec, &job)?;
        let mut control = Control::listen(ctx, id)?;
        keyboard = Some({
            let relay = relay.clone();
            let console = input.is_some();
            thread::Builder::new().spawn(move || relay_input(&relay, console))?
        });
        loop {
            match unsafe { WaitForSingleObject(child.as_raw_handle(), TICK.as_millis() as u32) } {
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    unsafe { win_ok(GetExitCodeProcess(child.as_raw_handle(), &mut code))? };
                    return Ok(code as i32);
                }
                WAIT_TIMEOUT => {}
                _ => return Err(io::Error::last_os_error().into()),
            }
            ensure!(
                !display.is_finished(),
                "terminal output closed before the tool exited"
            );
            let input = relay.clone();
            control.accept(move |command| { let mut row = RunRow { db: &input.db, run: &input.run, held: None }; input.remote(command, &mut row) })?;
            if let Some(output) = &output {
                let next = output.size()?;
                if (size.X, size.Y) != (next.X, next.Y) {
                    let code = unsafe { ResizePseudoConsole(pty.0, next) };
                    ensure!(code >= 0, "ResizePseudoConsole failed ({code:#x})");
                    size = next;
                }
            }
        }
    })();

    relay.closed.store(true, Relaxed);
    drop(job);
    // Keep draining while the pseudoconsole closes, including on pre-24H2 Windows.
    drop(pty);
    if let Some(keyboard) = keyboard {
        cancel_input(keyboard);
    }
    let displayed = display
        .join()
        .map_err(|_| anyhow::anyhow!("terminal output relay panicked"))?;
    let code = result?;
    displayed.context("relay terminal output")?;
    Ok(code)
}

struct Relay {
    writer: Mutex<File>,
    modes: AtomicU8,
    closed: AtomicBool,
    /// A reply's text waits unsent in the input; the next local keystroke settles it.
    withheld: AtomicBool,
    /// The run's database and id: where remote input is decided.
    db: std::path::PathBuf,
    run: String,
}

/// Where remote input is decided: the run's database row, held for writing from the decision until the bytes are
/// out, so whatever else changes the run (its hooks write that row) waits for the input instead of slipping in
/// between the check and the first byte, or the text and its Enter.
trait Owner {
    /// Before the first byte: take the hold and decide; the outcome to answer when the input may not go.
    fn take(&mut self, command: &Command<'_>) -> Result<(), Outcome>;
    /// Before a reply's Enter, inside the hold: only time can have moved the guard.
    fn still(&self, command: &Command<'_>) -> bool;
    /// What was written, recorded in the hold before it is let go.
    fn done(&mut self, command: &Command<'_>, out: &Outcome);
}

struct RunRow<'a> {
    db: &'a std::path::Path,
    run: &'a str,
    held: Option<rusqlite::Connection>,
}

impl Owner for RunRow<'_> {
    fn take(&mut self, command: &Command<'_>) -> Result<(), Outcome> {
        let reply = matches!(command.action, Action::Reply(_));
        let held = rusqlite::Connection::open(self.db).and_then(|c| { c.busy_timeout(Duration::from_secs(2))?; c.execute_batch("begin immediate")?; Ok(c) });
        let Ok(c) = held else { return Err(Outcome::Refused) };
        let refused = if reply && crate::hooks::unsent_waits(&c, self.run) { Some(Outcome::Unsent) }
            else if command.guard.as_ref().is_some_and(|g| crate::hooks::guard_holds(&c, self.run, g, reply).is_err()) { Some(Outcome::Refused) }
            else { None };
        if let Some(out) = refused {
            let _ = c.execute_batch("rollback");
            return Err(out);
        }
        self.held = Some(c);
        Ok(())
    }

    fn still(&self, command: &Command<'_>) -> bool { command.guard.as_ref().is_none_or(|g| crate::util::now_ms() < g.deadline) }

    fn done(&mut self, command: &Command<'_>, out: &Outcome) {
        let Some(c) = self.held.take() else { return };
        let r = (|| -> anyhow::Result<()> {
            match (command.action, out) {
                // a Ctrl+C that went out, or may have (a write broken part way), ends the turn it claimed
                (Action::Stop, Outcome::Sent | Outcome::Broken) => {
                    if let Some(token) = command.guard.as_ref().and_then(|g| g.stop.as_deref()) { crate::hooks::stop_landed(&c, self.run, token)?; }
                }
                (Action::Reply(_), Outcome::Withheld) => crate::hooks::withheld(&c, self.run)?,
                _ => {}
            }
            c.execute_batch("commit")?;
            Ok(())
        })();
        if let Err(e) = r { crate::log::warn(&format!("terminal: recording remote input for {}: {e:#}", self.run)); }
    }
}

impl Relay {
    fn remote(&self, command: Command<'_>, owner: &mut dyn Owner) -> Outcome {
        let mut writer = match self.writer.try_lock() {
            Ok(writer) => writer,
            Err(TryLockError::WouldBlock) => return Outcome::Busy,
            Err(TryLockError::Poisoned(_)) => return Outcome::Closed,
        };
        if self.closed.load(Relaxed) {
            return Outcome::Closed;
        }
        if let Err(out) = owner.take(&command) {
            return out;
        }
        let out = {
            let still = || owner.still(&command);
            match write_remote_input(&mut *writer, command.action, self.modes.load(Relaxed), &still) {
                Ok(true) => Outcome::Sent,
                Ok(false) => Outcome::Withheld,
                Err(_) => Outcome::Broken,
            }
        };
        if matches!(out, Outcome::Withheld) { self.withheld.store(true, Relaxed); }
        owner.done(&command, &out);
        out
    }
}

/// The input's bytes, written; false when `before_enter` said no between a reply's text and its Enter.
fn write_remote_input(mut writer: impl Write, action: Action<'_>, modes: u8, before_enter: &dyn Fn() -> bool) -> io::Result<bool> {
    let win32 = modes & WIN32_INPUT != 0;
    match action {
        Action::Stop => writer.write_all(if win32 { WIN32_CTRL_C } else { b"\x03" }).map(|_| true),
        Action::Reply(text) => {
            let enter = if win32 { WIN32_ENTER } else { b"\r" };
            let mut bytes = if modes & BRACKETED_PASTE != 0 {
                [b"\x1b[200~", text.as_bytes(), b"\x1b[201~"].concat()
            } else {
                text.as_bytes().to_vec()
            };
            if win32 && !text.is_empty() {
                writer.write_all(&bytes)?;
                // shortcut: let Codex's 120 ms paste window expire, revisit if the tool exposes paste completion
                thread::sleep(Duration::from_millis(180));
                // the session may have changed while the text waited: the Enter is what submits it
                if !before_enter() {
                    return Ok(false);
                }
                writer.write_all(enter).map(|_| true)
            } else {
                bytes.extend_from_slice(enter);
                writer.write_all(&bytes).map(|_| true)
            }
        }
    }
}

fn cancel_input(thread: JoinHandle<io::Result<()>>) {
    // Cancellation can race a pending read. Retry until the relay sees the closed flag.
    while !thread.is_finished() {
        unsafe { CancelSynchronousIo(thread.as_raw_handle()) };
        thread::sleep(Duration::from_millis(1));
    }
    let _ = thread.join();
}

fn relay_input(relay: &Relay, console: bool) -> io::Result<()> {
    let input = io::stdin().as_raw_handle();
    let mut buf = [0; 4096];
    let mut wide = [0; 2049];
    let mut carry = 0;
    while !relay.closed.load(Relaxed) {
        let mut read = 0;
        let text;
        let bytes = if console {
            unsafe {
                win_ok(ReadConsoleW(
                    input,
                    wide[carry..].as_mut_ptr().cast(),
                    2048,
                    &mut read,
                    null(),
                ))?;
            }
            if read == 0 {
                break;
            }
            let len = carry + read as usize;
            carry = usize::from(matches!(wide[len - 1], 0xd800..=0xdbff));
            text = String::from_utf16_lossy(&wide[..len - carry]);
            wide[0] = wide[len - 1];
            text.as_bytes()
        } else {
            unsafe {
                win_ok(ReadFile(
                    input,
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                    &mut read,
                    null_mut(),
                ))?;
            }
            if read == 0 {
                break;
            }
            &buf[..read as usize]
        };
        // what a withheld reply left in the input is the person's at the terminal now
        if relay.withheld.swap(false, Relaxed) {
            let _ = rusqlite::Connection::open(&relay.db).and_then(|c| { c.busy_timeout(Duration::from_secs(2))?; Ok(c) }).map(|c| crate::hooks::terminal_typed(&c, &relay.run));
        }
        let mut writer = relay.writer.lock().unwrap();
        if !relay.closed.load(Relaxed) {
            writer.write_all(bytes)?;
        }
    }
    Ok(())
}

fn relay_output(mut reader: File, modes: &AtomicU8, console: bool) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    let mut buf = [0; 4096];
    let mut sequence = ModeSequence::default();
    let mut active = 0;
    let mut carry = 0;
    loop {
        let n = match reader.read(&mut buf[carry..]) {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => 0,
            other => other?,
        };
        if n == 0 {
            return Ok(());
        }
        let len = n + carry;
        let end = if console {
            // Rust's Windows console writer expects a full UTF-8 character at the boundary.
            match std::str::from_utf8(&buf[..len]) {
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                _ => len,
            }
        } else {
            len
        };
        for &b in &buf[..end] {
            if let Some((mask, enabled)) = sequence.byte(b) {
                active = if enabled { active | mask } else { active & !mask };
                modes.store(active, Relaxed);
            }
        }
        stdout.write_all(&buf[..end])?;
        stdout.flush()?;
        buf.copy_within(end..len, 0);
        carry = len - end;
    }
}

#[derive(Default)]
enum ModeSequence {
    #[default]
    Ground,
    Escape,
    Csi,
    Private {
        param: u16,
        mask: u8,
    },
}

impl ModeSequence {
    fn byte(&mut self, byte: u8) -> Option<(u8, bool)> {
        let mode_bit = |param| match param {
            2004 => BRACKETED_PASTE,
            9001 => WIN32_INPUT,
            _ => 0,
        };
        let mut changed = None;
        *self = match (&*self, byte) {
            (_, 0x1b) => Self::Escape,
            (Self::Escape, b'[') => Self::Csi,
            (Self::Escape, b'c') => {
                changed = Some((BRACKETED_PASTE | WIN32_INPUT, false));
                Self::Ground
            }
            (Self::Csi, b'?') => Self::Private {
                param: 0,
                mask: 0,
            },
            (Self::Private { param, mask }, b'0'..=b'9') => Self::Private {
                param: param
                    .saturating_mul(10)
                    .saturating_add(u16::from(byte - b'0')),
                mask: *mask,
            },
            (Self::Private { param, mask }, b';') => Self::Private {
                param: 0,
                mask: *mask | mode_bit(*param),
            },
            (Self::Private { param, mask }, b'h' | b'l') => {
                let mask = *mask | mode_bit(*param);
                if mask != 0 {
                    changed = Some((mask, byte == b'h'));
                }
                Self::Ground
            }
            _ => Self::Ground,
        };
        changed
    }
}

struct ConsoleMode(HANDLE, u32);

impl ConsoleMode {
    fn set(handle: HANDLE, next: u32) -> io::Result<Option<Self>> {
        let mut saved = 0;
        unsafe {
            if GetConsoleMode(handle, &mut saved) == 0 {
                return Ok(None); // redirected stdin or stdout
            }
            win_ok(SetConsoleMode(handle, next))?;
        }
        Ok(Some(Self(handle, saved)))
    }

    fn size(&self) -> io::Result<COORD> {
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        unsafe { win_ok(GetConsoleScreenBufferInfo(self.0, &mut info))? };
        Ok(COORD {
            X: info.srWindow.Right - info.srWindow.Left + 1,
            Y: info.srWindow.Bottom - info.srWindow.Top + 1,
        })
    }
}

impl Drop for ConsoleMode {
    fn drop(&mut self) {
        unsafe { SetConsoleMode(self.0, self.1) };
    }
}

struct Pty(HPCON);

impl Pty {
    fn open(size: COORD) -> Result<(Self, File, File)> {
        let (input, writer) = pipe()?;
        let (reader, output) = pipe()?;
        let mut pty = 0;
        let code = unsafe {
            CreatePseudoConsole(
                size,
                input.as_raw_handle(),
                output.as_raw_handle(),
                0,
                &mut pty,
            )
        };
        ensure!(code >= 0, "CreatePseudoConsole failed ({code:#x})");
        Ok((Self(pty), reader, writer))
    }

    fn spawn(&self, spec: &LaunchSpec, job: &OwnedHandle) -> Result<OwnedHandle> {
        let attributes = Attributes::new(self.0)?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of_val(&startup) as u32;
        // ConPTY supplies the child standard handles, even when the parent's are redirected.
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        startup.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        startup.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
        startup.lpAttributeList = attributes.ptr();

        let program = std::path::absolute(&spec.program)?;
        let mut argv = Vec::new();
        if program
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("cmd") || s.eq_ignore_ascii_case("bat"))
        {
            argv.extend([
                std::env::current_exe()?.into_os_string(),
                SHIM_COMMAND.into(),
                "--".into(),
            ]);
        }
        argv.push(program.into_os_string());
        argv.extend(spec.args.iter().map(OsString::from));
        let exe = argv[0].encode_wide().chain([0]).collect::<Vec<_>>();
        let mut command = command_line(&argv)?;
        let env = environment(spec)?;
        let mut process = PROCESS_INFORMATION::default();
        unsafe {
            win_ok(CreateProcessW(
                exe.as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                env.as_ptr().cast(),
                null(),
                &startup.StartupInfo,
                &mut process,
            ))
            .with_context(|| format!("start {} inside ConPTY", spec.program.display()))?;
            let child = OwnedHandle::from_raw_handle(process.hProcess);
            let thread = OwnedHandle::from_raw_handle(process.hThread);
            if let Err(e) = win_ok(AssignProcessToJobObject(
                job.as_raw_handle(),
                child.as_raw_handle(),
            )) {
                TerminateProcess(child.as_raw_handle(), 1);
                return Err(e).context("assign the ConPTY child to its cleanup job");
            }
            win_ok(i32::from(ResumeThread(thread.as_raw_handle()) != u32::MAX))
                .context("resume the ConPTY child")?;
            Ok(child)
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.0) };
    }
}

struct Attributes(Vec<usize>);

impl Attributes {
    fn new(pty: HPCON) -> io::Result<Self> {
        let mut size = 0;
        unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size) };
        let mut storage = vec![0; size.div_ceil(std::mem::size_of::<usize>())];
        unsafe {
            win_ok(InitializeProcThreadAttributeList(
                storage.as_mut_ptr().cast(),
                1,
                0,
                &mut size,
            ))?
        };
        let attributes = Self(storage);
        unsafe {
            win_ok(UpdateProcThreadAttribute(
                attributes.ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                pty as _,
                std::mem::size_of::<HPCON>(),
                null_mut(),
                null_mut(),
            ))?;
        }
        Ok(attributes)
    }

    fn ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.0.as_ptr().cast_mut().cast()
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.ptr()) };
    }
}

fn cleanup_job() -> io::Result<OwnedHandle> {
    let raw = unsafe { CreateJobObjectW(null(), null()) };
    win_ok(i32::from(!raw.is_null()))?;
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        win_ok(SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            std::mem::size_of_val(&limits) as u32,
        ))?;
    }
    Ok(job)
}

fn pipe() -> io::Result<(File, File)> {
    let (mut read, mut write) = (null_mut(), null_mut());
    unsafe {
        win_ok(CreatePipe(&mut read, &mut write, null(), 0))?;
        Ok((File::from_raw_handle(read), File::from_raw_handle(write)))
    }
}

fn win_ok(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn environment(spec: &LaunchSpec) -> Result<Vec<u16>> {
    // LaunchSpec's overrides use ASCII variable names, which Windows matches case-insensitively.
    let key = |s: &OsStr| s.to_ascii_uppercase().encode_wide().collect::<Vec<_>>();
    let mut env: BTreeMap<_, _> = std::env::vars_os()
        .map(|(k, v)| (key(&k), (k, v)))
        .collect();
    for (k, v) in spec.command().get_envs() {
        if let Some(v) = v {
            env.insert(key(k), (k.to_owned(), v.to_owned()));
        } else {
            env.remove(&key(k));
        }
    }
    let mut block = Vec::new();
    for (k, v) in env.into_values() {
        let entry = k.encode_wide().chain([b'=' as u16]).chain(v.encode_wide());
        let start = block.len();
        block.extend(entry);
        ensure!(!block[start..].contains(&0), "NUL in terminal environment");
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn command_line(args: &[OsString]) -> Result<Vec<u16>> {
    const QUOTE: u16 = b'"' as u16;
    const SLASH: u16 = b'\\' as u16;
    let mut command = Vec::new();
    for arg in args {
        if !command.is_empty() {
            command.push(b' ' as u16);
        }
        command.push(QUOTE);
        let mut slashes = 0;
        for unit in arg.encode_wide() {
            ensure!(unit != 0, "NUL in terminal command line");
            if unit == SLASH {
                slashes += 1;
                continue;
            }
            let count = if unit == QUOTE {
                2 * slashes + 1
            } else {
                slashes
            };
            command.extend(std::iter::repeat_n(SLASH, count));
            slashes = 0;
            command.push(unit);
        }
        command.extend(std::iter::repeat_n(SLASH, 2 * slashes));
        command.push(QUOTE);
    }
    command.push(0);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::Guard;

    #[test]
    fn follows_input_mode_toggles_across_reads() {
        let mut state = ModeSequence::default();
        let mut scan = |s: &[u8]| s.iter().filter_map(|&b| state.byte(b)).collect::<Vec<_>>();
        assert!(scan(b"hello\x1b[?1004;20").is_empty());
        assert_eq!(scan(b"04;9001h"), [(BRACKETED_PASTE | WIN32_INPUT, true)]);
        assert!(scan(b"2004l\x1b[?1999l").is_empty());
        assert_eq!(
            scan(b"\x1b[?2004;1006l\x1b[?9001l\x1b[?2004h\x1bc"),
            [
                (BRACKETED_PASTE, false),
                (WIN32_INPUT, false),
                (BRACKETED_PASTE, true),
                (BRACKETED_PASTE | WIN32_INPUT, false),
            ]
        );
    }

    #[test]
    fn encodes_paste_and_enter_independently() {
        let input = |action, modes| {
            let mut bytes = Vec::new();
            assert!(write_remote_input(&mut bytes, action, modes, &|| true).unwrap());
            bytes
        };
        assert_eq!(input(Action::Reply("abc"), 0), b"abc\r");
        assert_eq!(
            input(Action::Reply("abc"), BRACKETED_PASTE),
            b"\x1b[200~abc\x1b[201~\r"
        );
        assert_eq!(
            input(Action::Reply("abc"), WIN32_INPUT),
            [b"abc".as_slice(), WIN32_ENTER].concat()
        );
        assert_eq!(input(Action::Stop, BRACKETED_PASTE), b"\x03");
    }

    /// A test owner: says yes or no at the start, and yes to `still` this many times.
    struct Fixed {
        take: bool,
        still: std::cell::Cell<u32>,
        asked: std::cell::Cell<u32>,
    }

    impl Fixed {
        fn new(take: bool, still: u32) -> Fixed { Fixed { take, still: std::cell::Cell::new(still), asked: std::cell::Cell::new(0) } }
    }

    impl Owner for Fixed {
        fn take(&mut self, _: &Command<'_>) -> Result<(), Outcome> { if self.take { Ok(()) } else { Err(Outcome::Refused) } }
        fn still(&self, _: &Command<'_>) -> bool {
            self.asked.set(self.asked.get() + 1);
            let left = self.still.get();
            self.still.set(left.saturating_sub(1));
            left > 0
        }
        fn done(&mut self, _: &Command<'_>, _: &Outcome) {}
    }

    fn relay_on(writer: File, modes: u8, db: &std::path::Path, run: &str) -> Relay {
        Relay { writer: Mutex::new(writer), modes: AtomicU8::new(modes), closed: AtomicBool::new(false), withheld: AtomicBool::new(false), db: db.to_path_buf(), run: run.into() }
    }

    #[test]
    fn a_second_driver_is_refused_before_writing() {
        let (mut reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, std::path::Path::new("unused"), "r");
        let first = relay.writer.lock().unwrap();
        assert!(matches!(
            relay.remote(Command { action: Action::Reply("second"), guard: None }, &mut Fixed::new(true, u32::MAX)),
            Outcome::Busy
        ));
        drop(first);
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: None }, &mut Fixed::new(true, u32::MAX)), Outcome::Sent));
        drop(relay);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, [3]);
    }

    /// #3: a guard that no longer holds stops the input before its first byte, and between a reply's text and its
    /// Enter in Win32 input mode.
    #[test]
    fn a_guard_is_checked_before_the_first_byte_and_before_the_enter() {
        let (mut reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, WIN32_INPUT, std::path::Path::new("unused"), "r");
        assert!(matches!(relay.remote(Command { action: Action::Reply("before"), guard: None }, &mut Fixed::new(false, 0)), Outcome::Refused));
        let mut owner = Fixed::new(true, 0);
        assert!(matches!(relay.remote(Command { action: Action::Reply("typed"), guard: None }, &mut owner), Outcome::Withheld));
        assert_eq!(owner.asked.get(), 1, "asked before the Enter");
        drop(relay);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, b"typed", "the text, no Enter, and nothing of the refused one");
    }

    /// A data folder whose run `r` is working, as its hooks left it.
    fn run_row() -> (tempfile::TempDir, crate::Ctx) {
        let t = tempfile::tempdir().unwrap();
        let ctx = crate::Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness, state, version) values('r', 1, 1, 'p', 'claude', 'working', 4)", [])?)).unwrap();
        (t, ctx)
    }

    fn row<T: rusqlite::types::FromSql>(ctx: &crate::Ctx, sql: &str) -> T { ctx.db.read(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

    /// (c): the run's row is held from the decision until the input is out: a hook writing the run in between waits for
    /// it, so the turn cannot change between the check and the first byte, or the text and its Enter.
    #[test]
    fn a_hook_writing_the_run_waits_for_the_input_in_flight() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        let (_reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, WIN32_INPUT, &db, "r");
        let started = std::time::Instant::now();
        let hook = {
            let db = db.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(60));
                let c = rusqlite::Connection::open(&db).unwrap();
                c.busy_timeout(Duration::from_secs(5)).unwrap();
                c.execute("update run_state set state = 'waiting', version = version + 1 where run = 'r'", []).unwrap();
                started.elapsed()
            })
        };
        let out = relay.remote(Command { action: Action::Reply("go on"), guard: None }, &mut RunRow { db: &db, run: "r", held: None });
        let sent = started.elapsed();
        assert!(matches!(out, Outcome::Sent));
        let hooked = hook.join().unwrap();
        assert!(hooked >= sent, "the hook wrote at {hooked:?}, inside the input that ended at {sent:?}");
    }

    /// (c): a reply typed but not submitted keeps every remote reply out until the terminal settles it.
    #[test]
    fn text_left_unsent_keeps_remote_replies_out_until_the_terminal_settles_it() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        let (_reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, WIN32_INPUT, &db, "r");
        let g = Guard { version: 4, deadline: crate::util::now_ms() + 60, phone: None, stop: None };
        let out = relay.remote(Command { action: Action::Reply("typed"), guard: Some(g) }, &mut RunRow { db: &db, run: "r", held: None });
        assert!(matches!(out, Outcome::Withheld));
        assert_eq!(row::<i64>(&ctx, "select needs_terminal from run_state"), 1);
        assert!(matches!(relay.remote(Command { action: Action::Reply("next"), guard: None }, &mut RunRow { db: &db, run: "r", held: None }), Outcome::Unsent));
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: None }, &mut RunRow { db: &db, run: "r", held: None }), Outcome::Sent), "Stop is still possible");
        crate::hooks::terminal_typed(&rusqlite::Connection::open(&db).unwrap(), "r").unwrap();
        assert!(matches!(relay.remote(Command { action: Action::Reply("next"), guard: None }, &mut RunRow { db: &db, run: "r", held: None }), Outcome::Sent));
    }

    /// (b): the relay takes only the Stop whose claim is the run's, and ends that turn in the same write.
    #[test]
    fn the_relay_takes_only_the_stop_whose_claim_is_the_runs() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        let pid = std::process::id();
        ctx.db.with(|c| Ok(c.execute("update run_state set pid = ?1, started = ?2", rusqlite::params![pid, crate::procs::start_ms(pid).unwrap()])?)).unwrap();
        let (mut reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, &db, "r");
        let claim = crate::hooks::claim_stop(&rusqlite::Connection::open(&db).unwrap(), "r", None).unwrap().unwrap();
        let g = |token: &str| Guard { version: claim.version, deadline: claim.deadline, phone: None, stop: Some(token.into()) };
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: Some(g("another")) }, &mut RunRow { db: &db, run: "r", held: None }), Outcome::Refused));
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: Some(g(&claim.token)) }, &mut RunRow { db: &db, run: "r", held: None }), Outcome::Sent));
        let (state, token): (String, Option<String>) = ctx.db.read(|c| Ok(c.query_row("select state, stop_token from run_state", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
        assert_eq!((state.as_str(), token), ("done", None));
        drop(relay);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, [3], "one Ctrl+C");
    }
}
