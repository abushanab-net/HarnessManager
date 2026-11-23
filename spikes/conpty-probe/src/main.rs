use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
mod pty;

use pty::{Pty, Size};
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::{Console::*, JobObjects::*};

const MAX_INPUT: usize = 16 * 1024;
const TICK: Duration = Duration::from_millis(40);
const IO_TIMEOUT: Duration = Duration::from_secs(3);
type PtyInput = Arc<Mutex<File>>;

fn win_ok(ok: i32) -> io::Result<()> {
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct Console {
    input: HANDLE,
    output: HANDLE,
    input_mode: u32,
    output_mode: u32,
    input_cp: u32,
    output_cp: u32,
}

impl Console {
    fn open() -> io::Result<Self> {
        let input = io::stdin().as_raw_handle();
        let output = io::stdout().as_raw_handle();
        let (mut input_mode, mut output_mode) = (0, 0);
        unsafe {
            win_ok(GetConsoleMode(input, &mut input_mode))?;
            win_ok(GetConsoleMode(output, &mut output_mode))?;
        }
        let console = Self {
            input,
            output,
            input_mode,
            output_mode,
            input_cp: unsafe { GetConsoleCP() },
            output_cp: unsafe { GetConsoleOutputCP() },
        };
        unsafe {
            win_ok(SetConsoleCP(65001))?;
            win_ok(SetConsoleOutputCP(65001))?;
            win_ok(SetConsoleMode(
                console.input,
                ENABLE_EXTENDED_FLAGS
                    | ENABLE_WINDOW_INPUT
                    | ENABLE_MOUSE_INPUT
                    | ENABLE_VIRTUAL_TERMINAL_INPUT,
            ))?;
            win_ok(SetConsoleMode(
                console.output,
                ENABLE_PROCESSED_OUTPUT
                    | ENABLE_WRAP_AT_EOL_OUTPUT
                    | ENABLE_VIRTUAL_TERMINAL_PROCESSING
                    | DISABLE_NEWLINE_AUTO_RETURN,
            ))?;
        }
        Ok(console)
    }

    fn size(&self) -> io::Result<Size> {
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        unsafe { win_ok(GetConsoleScreenBufferInfo(self.output, &mut info))? };
        Ok(size(
            (info.srWindow.Right - info.srWindow.Left + 1) as u16,
            (info.srWindow.Bottom - info.srWindow.Top + 1) as u16,
        ))
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        unsafe {
            SetConsoleMode(self.input, self.input_mode);
            SetConsoleMode(self.output, self.output_mode);
            SetConsoleCP(self.input_cp);
            SetConsoleOutputCP(self.output_cp);
        }
    }
}

fn size(cols: u16, rows: u16) -> Size {
    Size { cols, rows }
}

fn relay_input(input: PtyInput) -> io::Result<()> {
    let console = io::stdin().as_raw_handle();
    let mut buffer = [0; 4096];
    loop {
        let mut read = 0;
        unsafe {
            win_ok(ReadFile(
                console,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            ))?;
        }
        if read == 0 {
            return Ok(());
        }
        input.lock().unwrap().write_all(&buffer[..read as usize])?;
    }
}

fn relay_output(mut reader: File, mut log: File, visible: bool) -> io::Result<()> {
    let console = io::stdout().as_raw_handle();
    let mut buffer = [0; 4096];
    loop {
        let read = match reader.read(&mut buffer) {
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => 0,
            result => result?,
        };
        if read == 0 {
            return Ok(());
        }
        let bytes = &buffer[..read];
        log.write_all(bytes)?;
        if visible {
            // Keep a split UTF-8 character together across two PTY reads.
            let mut pending = bytes.to_vec();
            while let Err(err) = std::str::from_utf8(&pending) {
                if err.error_len().is_some() {
                    break;
                }
                let mut next = [0];
                reader.read_exact(&mut next)?;
                log.write_all(&next)?;
                pending.push(next[0]);
            }
            let mut left = pending.as_slice();
            while !left.is_empty() {
                let mut written = 0;
                unsafe {
                    win_ok(WriteFile(
                        console,
                        left.as_ptr(),
                        left.len() as u32,
                        &mut written,
                        std::ptr::null_mut(),
                    ))?;
                }
                if written == 0 {
                    return Err(io::ErrorKind::WriteZero.into());
                }
                left = &left[written as usize..];
            }
        }
    }
}

fn new_job() -> io::Result<OwnedHandle> {
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
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

fn stop_job(job: &OwnedHandle) -> io::Result<()> {
    let raw = job.as_raw_handle();
    unsafe { win_ok(TerminateJobObject(raw, 1))? };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            win_ok(QueryInformationJobObject(
                raw,
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut accounting).cast(),
                std::mem::size_of_val(&accounting) as u32,
                std::ptr::null_mut(),
            ))?;
        }
        if accounting.ActiveProcesses == 0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "job still has live children",
            ));
        }
        thread::sleep(TICK);
    }
}

enum Action {
    Input(Vec<u8>),
    Resize(Size),
    Stop,
}

fn action(op: u8, data: Vec<u8>) -> Result<Action> {
    match (op, data.as_slice()) {
        (b'i', _) => Ok(Action::Input(data)),
        (b's', []) => Ok(Action::Stop),
        (b'r', [a, b, c, d]) => {
            let cols = u16::from_le_bytes([*a, *b]);
            let rows = u16::from_le_bytes([*c, *d]);
            ensure!(
                (1..=i16::MAX as u16).contains(&cols) && (1..=i16::MAX as u16).contains(&rows),
                "invalid size"
            );
            Ok(Action::Resize(size(cols, rows)))
        }
        _ => bail!("invalid request"),
    }
}

fn read_action(stream: &mut TcpStream, token: &[u8; 32]) -> Result<Action> {
    let mut header = [0; 35];
    stream.read_exact(&mut header)?;
    ensure!(&header[..32] == token, "invalid token");
    let len = u16::from_le_bytes([header[33], header[34]]) as usize;
    ensure!(len <= MAX_INPUT, "request too large");
    let mut payload = vec![0; len];
    stream.read_exact(&mut payload)?;
    action(header[32], payload)
}

fn listen(listener: TcpListener, token: [u8; 32], input: PtyInput, control: mpsc::Sender<Action>) {
    for mut stream in listener.incoming().map_while(Result::ok) {
        let result = (|| -> Result<()> {
            stream.set_read_timeout(Some(IO_TIMEOUT))?;
            stream.set_write_timeout(Some(IO_TIMEOUT))?;
            match read_action(&mut stream, &token)? {
                Action::Input(data) => input.lock().unwrap().write_all(&data)?,
                other => control.send(other).context("session ended")?,
            }
            Ok(())
        })();
        let _ = stream.write_all(&[u8::from(result.is_ok())]);
    }
}

fn run(dir: &Path, args: &[OsString], headless: bool) -> Result<u32> {
    ensure!(
        Path::new(&args[0]).is_absolute(),
        "use an absolute executable path"
    );
    fs::create_dir(dir).context("session directory must be new")?;
    let dir = dir.canonicalize()?;
    let log = File::create(dir.join("raw.vt"))?;
    let console = if !headless && io::stdin().is_terminal() && io::stdout().is_terminal() {
        Some(Console::open().context("enable console VT mode")?)
    } else {
        None
    };
    let mut dimensions = console
        .as_ref()
        .map(Console::size)
        .transpose()?
        .unwrap_or_else(|| size(100, 30));
    let job = new_job()?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let mut token = [0u8; 32];
    let result = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            token.as_mut_ptr(),
            token.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    ensure!(result == 0, "BCryptGenRandom failed");

    let (pty, reader, writer) = Pty::open(dimensions)?;
    let input = Arc::new(Mutex::new(writer));
    let visible = console.is_some();
    let output = thread::spawn(move || relay_output(reader, log, visible));
    let (control, receiver) = mpsc::channel();
    let session = (|| -> Result<u32> {
        let child = pty.spawn(args, job.as_raw_handle())?;
        let mut control_file = Vec::from(listener.local_addr()?.port().to_le_bytes());
        control_file.extend(token);
        fs::write(dir.join("control.tmp"), control_file)?;
        thread::spawn({
            let input = input.clone();
            move || listen(listener, token, input, control)
        });
        if visible {
            thread::spawn(move || {
                let _ = relay_input(input);
            });
        }
        fs::rename(dir.join("control.tmp"), dir.join("control"))?;
        wait(
            &child,
            &pty,
            console.as_ref(),
            &receiver,
            &mut dimensions,
            &output,
        )
    })();

    let stopped = stop_job(&job);
    drop(job);
    drop(pty);
    let _ = fs::remove_file(dir.join("control"));
    let _ = fs::remove_file(dir.join("control.tmp"));
    output
        .join()
        .map_err(|_| anyhow::anyhow!("output relay panicked"))??;
    stopped.context("wait for the probe process tree to exit")?;
    session
}

fn wait(
    child: &OwnedHandle,
    pty: &Pty,
    console: Option<&Console>,
    control: &mpsc::Receiver<Action>,
    dimensions: &mut Size,
    output: &thread::JoinHandle<io::Result<()>>,
) -> Result<u32> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut last_local = *dimensions;
    loop {
        if let Some(code) = pty::exit_code(child)? {
            return Ok(code);
        }
        ensure!(
            Instant::now() < deadline,
            "ten-minute session limit reached"
        );
        ensure!(
            !output.is_finished(),
            "PTY output relay ended before the child"
        );
        let next = match control.recv_timeout(TICK) {
            Ok(Action::Stop) => return Ok(0),
            Ok(Action::Resize(next)) => Some(next),
            _ => None,
        };
        let local = console
            .map(Console::size)
            .transpose()?
            .unwrap_or(last_local);
        let next = if local != last_local {
            local
        } else {
            next.unwrap_or(*dimensions)
        };
        last_local = local;
        if next != *dimensions {
            pty.resize(next)?;
            *dimensions = next;
        }
    }
}

fn send(dir: &Path, op: u8, bytes: &[u8]) -> Result<u32> {
    ensure!(bytes.len() <= MAX_INPUT, "input exceeds {MAX_INPUT} bytes");
    let control = fs::read(dir.join("control")).context("read probe session endpoint")?;
    ensure!(control.len() == 34, "invalid session endpoint");
    let port = u16::from_le_bytes([control[0], control[1]]);
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    let mut request = control[2..].to_vec();
    request.push(op);
    request.extend((bytes.len() as u16).to_le_bytes());
    request.extend(bytes);
    stream.write_all(&request)?;
    let mut reply = [0];
    stream.read_exact(&mut reply)?;
    ensure!(reply == [1], "probe rejected request");
    Ok(0)
}

fn main_result() -> Result<u32> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if let [command, dir, data @ ..] = args.as_slice() {
        let dir = Path::new(dir);
        match (command.to_str(), data) {
            (Some("run"), [_, ..]) => return run(dir, data, false),
            (Some("headless"), [_, ..]) => return run(dir, data, true),
            (Some("text"), [text]) => {
                return send(
                    dir,
                    b'i',
                    text.to_str().context("expected UTF-8 text")?.as_bytes(),
                );
            }
            (Some("bytes"), [path]) => {
                let mut bytes = Vec::new();
                File::open(path)?
                    .take((MAX_INPUT + 1) as u64)
                    .read_to_end(&mut bytes)?;
                return send(dir, b'i', &bytes);
            }
            (Some("enter"), []) => return send(dir, b'i', b"\r"),
            (Some("interrupt"), []) => return send(dir, b'i', b"\x03"),
            (Some("resize"), [cols, rows]) => {
                let dimension = |arg: &OsString| -> Result<[u8; 2]> {
                    Ok(arg
                        .to_str()
                        .context("missing dimension")?
                        .parse::<u16>()?
                        .to_le_bytes())
                };
                return send(dir, b'r', &[dimension(cols)?, dimension(rows)?].concat());
            }
            (Some("stop"), []) => return send(dir, b's', &[]),
            _ => {}
        }
    }
    bail!(
        "usage: conpty-probe <run|headless> DIR EXE [ARGS...]\n       conpty-probe <text|bytes|enter|interrupt|resize|stop> DIR [VALUE...]"
    )
}

fn main() {
    match main_result() {
        Ok(code) => std::process::exit(code as i32),
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_resize_and_stop_payloads_are_rejected() {
        for payload in [vec![0, 0, 20, 0], vec![0, 128, 20, 0], vec![10, 0]] {
            assert!(action(b'r', payload).is_err());
        }
        assert!(action(b's', vec![3]).is_err());
        assert!(matches!(action(b'r', vec![80, 0, 25, 0]).unwrap(),
            Action::Resize(dimensions) if dimensions == size(80, 25)));
    }
}
