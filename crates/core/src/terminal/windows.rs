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
        writer: Writer::new(writer)?,
        remote: Mutex::new(()),
        modes: AtomicU8::new(0),
        closed: AtomicBool::new(false),
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
            control.accept(move |command| { let mut row = RunRow { db: &input.db, run: &input.run, record: None }; input.remote(command, &mut row) })?;
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
    /// Every byte to the tool goes through here, the keyboard's and remote input alike.
    writer: Writer,
    /// One remote input at a time.
    remote: Mutex<()>,
    modes: AtomicU8,
    closed: AtomicBool,
    /// The run's database and id: where remote input is recorded.
    db: std::path::PathBuf,
    run: String,
}

/// How long a remote input's write may take before it counts as not known to be delivered.
const WRITE_DEADLINE: Duration = Duration::from_secs(2);

/// What became of one write.
#[derive(Debug, PartialEq)]
enum Written {
    Whole,
    /// Failed, or still not done at its deadline (then cancelled): some of it may have reached the tool.
    NotKnown,
}

/// The tool's input pipe, written by a thread of its own, so a write that blocks (the tool not reading) can be given
/// up on at a deadline, and cancelled, without anything else waiting on it.
/// Bytes to write, and where to say how it went.
type Job = (Vec<u8>, std::sync::mpsc::Sender<io::Result<()>>);

struct Writer {
    jobs: Mutex<std::sync::mpsc::Sender<Job>>,
    thread: JoinHandle<()>,
}

impl Writer {
    fn new(mut file: File) -> io::Result<Writer> {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let thread = thread::Builder::new().spawn(move || {
            for (bytes, done) in rx { let _ = done.send(file.write_all(&bytes)); }
        })?;
        Ok(Writer { jobs: Mutex::new(tx), thread })
    }

    fn write(&self, bytes: &[u8], within: Duration) -> Written {
        let (done, wait) = std::sync::mpsc::channel();
        if self.jobs.lock().unwrap_or_else(|e| e.into_inner()).send((bytes.to_vec(), done)).is_err() { return Written::NotKnown; }
        match wait.recv_timeout(within) {
            Ok(Ok(())) => Written::Whole,
            Ok(Err(_)) => Written::NotKnown,
            Err(_) => {
                // the write is stuck: cancel it, so the writer goes on with what comes next
                unsafe { CancelSynchronousIo(self.thread.as_raw_handle()) };
                Written::NotKnown
            }
        }
    }
}

/// Where remote input is decided and recorded: the run's database row, in short writes of their own and never
/// across the terminal write. Before the first byte the input is recorded as begun (the run then takes no other
/// remote input); after the bytes its outcome is recorded.
trait Owner {
    /// Before the first byte: decide, and record that the input begins; the outcome to answer when it may not.
    fn begin(&mut self, command: &Command<'_>) -> Result<(), Outcome>;
    /// Before a reply's Enter: the record is still this input's and nothing moved the run since it began.
    fn before_enter(&self, command: &Command<'_>) -> bool;
    /// The outcome, recorded; false when it could not be (the input then reads as not known).
    fn finish(&mut self, command: &Command<'_>, out: &Outcome) -> bool;
}

struct RunRow<'a> {
    db: &'a std::path::Path,
    run: &'a str,
    /// The input's record: its id and the run's version when it began.
    record: Option<(String, i64)>,
}

impl RunRow<'_> {
    fn open(&self) -> rusqlite::Result<rusqlite::Connection> {
        let c = rusqlite::Connection::open(self.db)?;
        c.busy_timeout(Duration::from_secs(2))?;
        Ok(c)
    }
}

impl Owner for RunRow<'_> {
    fn begin(&mut self, command: &Command<'_>) -> Result<(), Outcome> {
        let stop = matches!(command.action, Action::Stop);
        let mut c = self.open().map_err(|_| Outcome::Refused)?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|_| Outcome::Refused)?;
        match crate::hooks::begin_delivery(&tx, self.run, command.guard.as_ref(), stop) {
            Ok(record) => { tx.commit().map_err(|_| Outcome::Refused)?; self.record = Some(record); Ok(()) }
            Err(crate::hooks::Refusal::Unsent) => Err(Outcome::Unsent),
            Err(crate::hooks::Refusal::Unconfirmed) => Err(Outcome::Unconfirmed),
            Err(crate::hooks::Refusal::Refused) => Err(Outcome::Refused),
        }
    }

    fn before_enter(&self, command: &Command<'_>) -> bool {
        let Some((id, version)) = &self.record else { return false };
        self.open().is_ok_and(|c| crate::hooks::delivery_holds(&c, self.run, id, *version, command.guard.as_ref()))
    }

    fn finish(&mut self, command: &Command<'_>, out: &Outcome) -> bool {
        let Some((id, _)) = self.record.take() else { return true };
        let how = match out {
            Outcome::Sent => crate::hooks::Settled::Sent,
            Outcome::Withheld => crate::hooks::Settled::Withheld,
            _ => crate::hooks::Settled::Maybe,
        };
        let stop = matches!(command.action, Action::Stop);
        let r = self.open().map_err(anyhow::Error::from).and_then(|mut c| {
            let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            crate::hooks::finish_delivery(&tx, self.run, &id, stop, how)?;
            tx.commit()?;
            Ok(())
        });
        if let Err(e) = &r { crate::log::warn(&format!("terminal: recording remote input for {}: {e:#}", self.run)); }
        r.is_ok()
    }
}

impl Relay {
    fn remote(&self, command: Command<'_>, owner: &mut dyn Owner) -> Outcome {
        let _one = match self.remote.try_lock() {
            Ok(one) => one,
            Err(TryLockError::WouldBlock) => return Outcome::Busy,
            Err(TryLockError::Poisoned(_)) => return Outcome::Closed,
        };
        if self.closed.load(Relaxed) {
            return Outcome::Closed;
        }
        if let Err(out) = owner.begin(&command) {
            return out;
        }
        let out = remote_input(&self.writer, command.action, self.modes.load(Relaxed), &|| owner.before_enter(&command));
        // an outcome that could not be recorded is not known to anyone: never "sent"
        if !owner.finish(&command, &out) && matches!(out, Outcome::Sent) {
            return Outcome::Broken;
        }
        out
    }
}

/// The input written, each step within its deadline: `Sent`, `Withheld` when `before_enter` said no between a
/// reply's text and its Enter, or `Broken` when a write failed or did not finish (it may have reached the tool).
fn remote_input(writer: &Writer, action: Action<'_>, modes: u8, before_enter: &dyn Fn() -> bool) -> Outcome {
    let win32 = modes & WIN32_INPUT != 0;
    let write = |bytes: &[u8]| writer.write(bytes, WRITE_DEADLINE) == Written::Whole;
    match action {
        Action::Stop => if write(if win32 { WIN32_CTRL_C } else { b"\x03" }) { Outcome::Sent } else { Outcome::Broken },
        Action::Reply(text) => {
            let enter = if win32 { WIN32_ENTER } else { b"\r" };
            let mut bytes = if modes & BRACKETED_PASTE != 0 {
                [b"\x1b[200~", text.as_bytes(), b"\x1b[201~"].concat()
            } else {
                text.as_bytes().to_vec()
            };
            if win32 && !text.is_empty() {
                if !write(&bytes) { return Outcome::Broken; }
                // shortcut: let Codex's 120 ms paste window expire, revisit if the tool exposes paste completion
                thread::sleep(Duration::from_millis(180));
                // the session may have changed while the text waited: the Enter is what submits it
                if !before_enter() {
                    return Outcome::Withheld;
                }
                if write(enter) { Outcome::Sent } else { Outcome::Broken }
            } else {
                bytes.extend_from_slice(enter);
                if write(&bytes) { Outcome::Sent } else { Outcome::Broken }
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
        // the keyboard waits for the tool as long as it takes; a key settles nothing remote input left behind
        if !relay.closed.load(Relaxed) && relay.writer.write(bytes, Duration::from_secs(3600)) != Written::Whole {
            return Err(io::Error::other("the tool's input closed"));
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

    /// The bytes the relay writes for an input, as read from the other end of a pipe.
    fn written(action: Action<'_>, modes: u8) -> Vec<u8> {
        let (mut reader, writer) = pipe().unwrap();
        let w = Writer::new(writer).unwrap();
        assert_eq!(remote_input(&w, action, modes, &|| true), Outcome::Sent);
        drop(w);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn encodes_paste_and_enter_independently() {
        assert_eq!(written(Action::Reply("abc"), 0), b"abc\r");
        assert_eq!(written(Action::Reply("abc"), BRACKETED_PASTE), b"\x1b[200~abc\x1b[201~\r");
        assert_eq!(written(Action::Reply("abc"), WIN32_INPUT), [b"abc".as_slice(), WIN32_ENTER].concat());
        assert_eq!(written(Action::Stop, BRACKETED_PASTE), b"\x03");
    }

    /// A test owner: yes or no at the start, yes before the Enter this many times, and whether its outcome is recorded.
    struct Fixed {
        begin: bool,
        enter: std::cell::Cell<u32>,
        asked: std::cell::Cell<u32>,
        recorded: bool,
    }

    impl Fixed {
        fn new(begin: bool, enter: u32) -> Fixed { Fixed { begin, enter: std::cell::Cell::new(enter), asked: std::cell::Cell::new(0), recorded: true } }
    }

    impl Owner for Fixed {
        fn begin(&mut self, _: &Command<'_>) -> Result<(), Outcome> { if self.begin { Ok(()) } else { Err(Outcome::Refused) } }
        fn before_enter(&self, _: &Command<'_>) -> bool {
            self.asked.set(self.asked.get() + 1);
            let left = self.enter.get();
            self.enter.set(left.saturating_sub(1));
            left > 0
        }
        fn finish(&mut self, _: &Command<'_>, _: &Outcome) -> bool { self.recorded }
    }

    fn relay_on(writer: File, modes: u8, db: &std::path::Path, run: &str) -> Relay {
        Relay { writer: Writer::new(writer).unwrap(), remote: Mutex::new(()), modes: AtomicU8::new(modes), closed: AtomicBool::new(false), db: db.to_path_buf(), run: run.into() }
    }

    #[test]
    fn a_second_driver_is_refused_before_writing() {
        let (mut reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, std::path::Path::new("unused"), "r");
        let first = relay.remote.lock().unwrap();
        assert!(matches!(relay.remote(Command { action: Action::Reply("second"), guard: None }, &mut Fixed::new(true, u32::MAX)), Outcome::Busy));
        drop(first);
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: None }, &mut Fixed::new(true, u32::MAX)), Outcome::Sent));
        drop(relay);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, [3]);
    }

    /// #3: a refused input writes nothing, and a reply whose run moved between its text and its Enter keeps the Enter.
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

    /// (1): an outcome that could not be recorded is never "sent".
    #[test]
    fn an_outcome_that_could_not_be_recorded_reads_not_known() {
        let (_reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, std::path::Path::new("unused"), "r");
        let mut owner = Fixed::new(true, u32::MAX);
        owner.recorded = false;
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: None }, &mut owner), Outcome::Broken));
    }

    /// A data folder whose run `r` (this process) is working, as its hooks left it.
    fn run_row() -> (tempfile::TempDir, crate::Ctx) {
        let t = tempfile::tempdir().unwrap();
        let ctx = crate::Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let pid = std::process::id();
        ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness, state, version) values('r', ?1, ?2, 'p', 'claude', 'working', 4)",
            rusqlite::params![pid, crate::procs::start_ms(pid).unwrap()])?)).unwrap();
        (t, ctx)
    }

    fn row<T: rusqlite::types::FromSql>(ctx: &crate::Ctx, sql: &str) -> T { ctx.db.read(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap() }

    fn reply_now(relay: &Relay, db: &std::path::Path, text: &str, guard: Option<Guard>) -> Outcome {
        relay.remote(Command { action: Action::Reply(text), guard }, &mut RunRow { db, run: "r", record: None })
    }

    /// (1): a stalled pipe (the tool not reading) holds no database lock: the write is given up at its deadline, the
    /// input reads as not known, and the run takes no other remote input until that is settled.
    #[test]
    fn a_stalled_pipe_holds_no_lock_and_leaves_the_run_unconfirmed() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        // a pipe nobody reads, and a reply bigger than its buffer
        let (_reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, &db, "r");
        let big = "x".repeat(MAX_TEXT_FOR_TEST);
        let hook = {
            let db = db.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                let c = rusqlite::Connection::open(&db).unwrap();
                c.busy_timeout(Duration::from_millis(200)).unwrap();
                // a hook's write while the terminal write is stuck: the database is free
                c.execute("update run_state set reason = 'hooked' where run = 'r'", []).is_ok()
            })
        };
        let started = std::time::Instant::now();
        assert!(matches!(reply_now(&relay, &db, &big, None), Outcome::Broken));
        assert!(started.elapsed() < WRITE_DEADLINE + Duration::from_secs(2), "given up at its deadline");
        assert!(hook.join().unwrap(), "the hook waited for the terminal write");
        assert_eq!((row::<String>(&ctx, "select delivery_state from run_state"), row::<i64>(&ctx, "select needs_terminal from run_state")), ("unknown".into(), 1));
        assert!(matches!(reply_now(&relay, &db, "next", None), Outcome::Unconfirmed));
        assert!(matches!(relay.remote(Command { action: Action::Stop, guard: None }, &mut RunRow { db: &db, run: "r", record: None }), Outcome::Unconfirmed));
    }

    const MAX_TEXT_FOR_TEST: usize = 16 * 1024;

    /// (3): text a reply left unsent keeps remote replies out until the person at the computer settles it.
    #[test]
    fn text_left_unsent_keeps_remote_replies_out_until_the_terminal_settles_it() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        let (_reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, WIN32_INPUT, &db, "r");
        let g = Guard { version: 4, deadline: crate::util::now_ms() + 60_000, phone: None, stop: None };
        let mut held = RunRow { db: &db, run: "r", record: None };
        // the run moves between the text and the Enter
        let mover = { let db = db.clone(); thread::spawn(move || { thread::sleep(Duration::from_millis(60)); rusqlite::Connection::open(&db).unwrap().execute("update run_state set version = version + 1", []).unwrap(); }) };
        assert!(matches!(relay.remote(Command { action: Action::Reply("typed"), guard: Some(g) }, &mut held), Outcome::Withheld));
        mover.join().unwrap();
        assert_eq!(row::<i64>(&ctx, "select needs_terminal from run_state"), 1);
        assert!(matches!(reply_now(&relay, &db, "next", None), Outcome::Unsent));
        crate::hooks::settle_at_terminal(&rusqlite::Connection::open(&db).unwrap(), "r").unwrap();
        assert!(matches!(reply_now(&relay, &db, "next", None), Outcome::Sent));
    }

    /// (2): the relay takes a Stop only with the run's own claim; one with none, or another's, writes nothing.
    #[test]
    fn the_relay_takes_only_the_stop_whose_claim_is_the_runs() {
        let (_t, ctx) = run_row();
        let db = ctx.data.join("hm.db");
        let (mut reader, writer) = pipe().unwrap();
        let relay = relay_on(writer, 0, &db, "r");
        let claim = crate::hooks::claim_stop(&rusqlite::Connection::open(&db).unwrap(), "r", None).unwrap().unwrap();
        let g = |token: &str| Guard { version: claim.version, deadline: claim.deadline, phone: None, stop: Some(token.into()) };
        let stop = |guard: Option<Guard>| relay.remote(Command { action: Action::Stop, guard }, &mut RunRow { db: &db, run: "r", record: None });
        assert!(matches!(stop(None), Outcome::Refused), "no claim");
        assert!(matches!(stop(Some(g("another"))), Outcome::Refused));
        assert!(matches!(stop(Some(g(&claim.token))), Outcome::Sent));
        let (state, token, delivery): (String, Option<String>, Option<String>) = ctx.db.read(|c| Ok(c.query_row("select state, stop_token, delivery_state from run_state", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
        assert_eq!((state.as_str(), token, delivery), ("done", None, None));
        drop(relay);
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, [3], "one Ctrl+C");
    }
}
