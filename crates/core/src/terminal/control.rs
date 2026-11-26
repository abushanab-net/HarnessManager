use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, ensure, Context, Result};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rusqlite::{params, Connection, OpenFlags};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

use super::{Guard, MaybeDelivered, Withheld};
use crate::Ctx;

const MAX_TEXT: usize = 16 * 1024;
/// The most a command's guard takes on the wire.
const MAX_GUARD: usize = 1024;
const TAG_LEN: usize = 16;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CLIENTS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Action<'a> {
    Reply(&'a str),
    Stop,
}

impl<'a> Action<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self> {
        match bytes {
            [b's'] => Ok(Self::Stop),
            [b'r', text @ ..] if text.len() <= MAX_TEXT => {
                let text = std::str::from_utf8(text)?;
                ensure!(
                    !text.chars().any(char::is_control),
                    "a reply must be a single line of text without control characters"
                );
                // a bare Enter would answer an open dialog with its highlighted choice
                ensure!(!text.trim().is_empty(), "a reply needs some text");
                Ok(Self::Reply(text))
            }
            _ => bail!("invalid terminal command"),
        }
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let bytes = match self {
            Self::Reply(text) => {
                ensure!(
                    text.len() <= MAX_TEXT,
                    "a reply must be at most {MAX_TEXT} bytes"
                );
                [&b"r"[..], text.as_bytes()].concat()
            }
            Self::Stop => vec![b's'],
        };
        Action::parse(&bytes)?;
        Ok(bytes)
    }
}

/// An action, with the guard a phone's action carries: `g<guard json>\0` before the action's own bytes.
#[derive(Debug, PartialEq)]
pub(super) struct Command<'a> {
    pub action: Action<'a>,
    pub guard: Option<Guard>,
}

impl<'a> Command<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self> {
        let (guard, rest) = match bytes.split_first() {
            Some((b'g', tail)) => {
                let end = tail.iter().position(|&b| b == 0).filter(|&n| n <= MAX_GUARD).context("invalid terminal guard")?;
                (Some(serde_json::from_slice(&tail[..end]).context("invalid terminal guard")?), &tail[end + 1..])
            }
            _ => (None, bytes),
        };
        Ok(Command { action: Action::parse(rest)?, guard })
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = vec![];
        if let Some(g) = &self.guard {
            bytes.push(b'g');
            bytes.extend(serde_json::to_vec(g)?);
            bytes.push(0);
        }
        bytes.extend(self.action.encode()?);
        Command::parse(&bytes)?;
        Ok(bytes)
    }
}

/// What the relay did: written whole; nothing written (busy, closed, or the guard no longer held); a reply typed but
/// its Enter withheld; or a write that failed part way, so the input may have arrived.
#[repr(u8)]
pub(super) enum Outcome {
    Sent = 0,
    Busy = 1,
    Closed = 2,
    Refused = 3,
    Withheld = 4,
    Broken = 5,
}

pub(super) struct Control {
    listener: TcpListener,
    cipher: XChaCha20Poly1305,
    clients: Vec<JoinHandle<()>>,
}

impl Control {
    pub(super) fn listen(ctx: &Ctx, run: &str) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let key = random::<32>()?;
        let stored = dpapi(&key, true)?;
        ctx.db.with(|c| {
            let changed = c.execute(
                "update run_state set terminal_port = ?1, terminal_token = ?2 where run = ?3",
                params![port, stored, run],
            )?;
            ensure!(changed == 1, "could not register the terminal endpoint");
            Ok(())
        })?;
        Ok(Self {
            listener,
            cipher: XChaCha20Poly1305::new(&key.into()),
            clients: Vec::new(),
        })
    }

    pub(super) fn accept(
        &mut self,
        input: impl FnOnce(Command<'_>) -> Outcome + Send + 'static,
    ) -> io::Result<()> {
        self.clients.retain(|client| !client.is_finished());
        match self.listener.accept() {
            Ok((stream, _)) if self.clients.len() < MAX_CLIENTS => {
                let cipher = self.cipher.clone();
                let deadline = Instant::now() + IO_TIMEOUT;
                self.clients.push(thread::Builder::new().spawn(move || {
                    let _ = serve(stream, deadline, &cipher, input);
                })?);
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|e| anyhow!("terminal control random bytes: {e}"))?;
    Ok(bytes)
}

fn dpapi(bytes: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    use std::ptr::{null, null_mut};

    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    unsafe {
        let ok = if protect {
            CryptProtectData(
                &input,
                null(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let bytes = if output.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        LocalFree(output.pbData.cast());
        Ok(bytes)
    }
}

struct ConnectionIo {
    stream: TcpStream,
    deadline: Instant,
}

impl ConnectionIo {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline.checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .map(|left| left.max(Duration::from_millis(1)))
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "terminal connection deadline elapsed"))
    }
}

impl Read for ConnectionIo {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(buf)
    }
}

impl Write for ConnectionIo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> { self.stream.flush() }
}

fn seal(cipher: &XChaCha20Poly1305, nonce: [u8; 24], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>> {
    cipher
        .encrypt(&XNonce::from(nonce), Payload { msg, aad })
        .map_err(|_| anyhow!("could not seal terminal command"))
}

fn open(cipher: &XChaCha20Poly1305, nonce: [u8; 24], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>> {
    cipher
        .decrypt(&XNonce::from(nonce), Payload { msg, aad })
        .map_err(|_| anyhow!("terminal authentication failed"))
}

fn serve(
    stream: TcpStream,
    deadline: Instant,
    cipher: &XChaCha20Poly1305,
    input: impl FnOnce(Command<'_>) -> Outcome,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    let mut stream = ConnectionIo { stream, deadline };
    // A fresh challenge for each connection prevents replay and keeps the token off the wire.
    // Separate nonce spaces for the request and the authenticated result.
    let challenge = random::<16>()?;
    stream.write_all(&challenge)?;
    let mut nonce = [0; 24];
    stream.read_exact(&mut nonce)?;
    ensure!(nonce[0] & 0x80 == 0, "invalid terminal nonce");
    const MAX_FRAME: usize = 2 + MAX_GUARD + 1 + MAX_TEXT + TAG_LEN;
    let mut bytes = Vec::new();
    (&mut stream)
        .take((MAX_FRAME + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_FRAME, "terminal command is too large");
    let request = open(cipher, nonce, &challenge, &bytes)?;
    stream.remaining()?;
    let outcome = input(Command::parse(&request)?);
    nonce[0] |= 0x80;
    stream.write_all(&seal(cipher, nonce, &challenge, &[outcome as u8])?)?;
    Ok(())
}

fn endpoint(data: &Path, run: &str) -> Result<(u16, XChaCha20Poly1305)> {
    let path = data.join("hm.db");
    let unavailable = || anyhow!("run '{run}' has no owned terminal");
    if !path.is_file() {
        return Err(unavailable());
    }
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    c.busy_timeout(IO_TIMEOUT)?;
    if !c
        .prepare("select 1 from pragma_table_info('run_state') where name = 'terminal_token'")?
        .exists([])?
    {
        return Err(unavailable());
    }
    let mut stmt = c.prepare("select pid, started, terminal_port, terminal_token from run_state
        where run = ?1 or (?1 <> '' and session_id = ?1 and not exists (select 1 from run_state where run = ?1))")?;
    let rows = stmt.query_map([run], |r| {
        Ok((
            r.get::<_, u32>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<u16>>(2)?,
            r.get::<_, Option<Vec<u8>>>(3)?,
        ))
    })?;
    let mut live = None;
    for row in rows {
        let (pid, started, port, token) = row?;
        if crate::procs::start_ms(pid).is_some_and(|time| started == 0 || started == time) {
            ensure!(
                live.is_none(),
                "session '{run}' has several live runs. Use the run ID"
            );
            live = Some((port, token));
        }
    }
    let (Some(port), Some(token)) = live.ok_or_else(unavailable)? else {
        return Err(unavailable());
    };
    let key =
        dpapi(&token, false).context("cannot open the terminal token for this Windows user")?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key).map_err(|_| anyhow!("invalid terminal token"))?;
    Ok((port, cipher))
}

/// Send a command to the run's relay. Every error before the command is written whole means it was not delivered; one
/// after (no answer, a garbled one, or a write that broke) is `MaybeDelivered`.
pub(super) fn send(data: &Path, run: &str, action: Action<'_>, guard: Option<&Guard>) -> Result<()> {
    let request = Command { action, guard: guard.cloned() }.encode()?;
    let (port, cipher) = endpoint(data, run)?;
    let address = (Ipv4Addr::LOCALHOST, port).into();
    let deadline = Instant::now() + IO_TIMEOUT;
    let stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)
        .with_context(|| format!("owned terminal for run '{run}' is no longer reachable"))?;
    let mut stream = ConnectionIo { stream, deadline };
    let mut challenge = [0; 16];
    stream
        .read_exact(&mut challenge)
        .context("owned terminal did not accept the connection")?;
    let mut nonce = random::<24>()?;
    nonce[0] &= 0x7f;
    stream.write_all(&nonce)?;
    stream.write_all(&seal(&cipher, nonce, &challenge, &request)?)?;
    // from here on the relay holds the command: what it did can only be told by its answer
    let maybe = |why: &str| anyhow::Error::new(MaybeDelivered(why.to_string()));
    let _ = stream.stream.shutdown(Shutdown::Write);

    let mut response = [0; 1 + TAG_LEN];
    stream
        .read_exact(&mut response)
        .map_err(|_| maybe("owned terminal did not confirm delivery"))?;
    nonce[0] |= 0x80;
    match open(&cipher, nonce, &challenge, &response).map_err(|_| maybe("invalid terminal response"))?.as_slice() {
        [code] if *code == Outcome::Sent as u8 => Ok(()),
        [code] if *code == Outcome::Busy as u8 => {
            bail!("the terminal is busy writing input; try again")
        }
        [code] if *code == Outcome::Closed as u8 => bail!("the terminal input has closed"),
        [code] if *code == Outcome::Refused as u8 => bail!("the session changed before the input could reach it; look again"),
        [code] if *code == Outcome::Withheld as u8 => Err(Withheld("the reply was typed into the session but not sent: it changed before Enter; the text waits in its input".into()).into()),
        [code] if *code == Outcome::Broken as u8 => Err(maybe("the terminal failed while writing the input")),
        _ => Err(maybe("invalid terminal response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_elapsed_connection_deadline_applies_to_reads_and_writes() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let mut conn = ConnectionIo { stream, deadline: Instant::now() };
        assert_eq!(conn.read_exact(&mut [0]).unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(conn.write_all(&[0]).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn parses_only_a_single_line_or_stop() {
        assert_eq!(
            Action::parse("réply".as_bytes()).unwrap(),
            Action::Reply("éply")
        );
        assert_eq!(Action::parse(b"s").unwrap(), Action::Stop);
        assert!(Action::parse(b"r").is_err(), "an empty reply would press Enter in an open dialog");
        assert!(Action::parse(b"r  ").is_err());
        for bytes in [
            b"stop".as_slice(),
            b"x",
            b"rfoo\nbar",
            b"r\xff",
            b"r\x1b[201~",
            b"r\x03",
        ] {
            assert!(Action::parse(bytes).is_err());
        }
        assert!(Action::parse(&[b'r'; MAX_TEXT + 2]).is_err());
    }

    #[test]
    fn a_wrong_token_or_replayed_command_cannot_write() {
        let key = XChaCha20Poly1305::new(&[7; 32].into());
        let wrong = XChaCha20Poly1305::new(&[9; 32].into());
        let nonce = [1; 24];
        let request = seal(&key, nonce, b"challenge", b"rhello").unwrap();
        assert_eq!(
            open(&key, nonce, b"challenge", &request).unwrap(),
            b"rhello"
        );
        assert!(open(&wrong, nonce, b"challenge", &request).is_err());
        assert!(open(&key, nonce, b"another challenge", &request).is_err());
        let protected = dpapi(&[7; 32], true).unwrap();
        assert_eq!(dpapi(&protected, false).unwrap(), [7; 32]);
    }

    #[test]
    fn a_guard_rides_before_the_action_and_reads_back() {
        let g = Guard { version: 7, deadline: 99, phone: Some(("p".into(), "g".into())) };
        let bytes = Command { action: Action::Reply("hi"), guard: Some(g.clone()) }.encode().unwrap();
        assert_eq!(Command::parse(&bytes).unwrap(), Command { action: Action::Reply("hi"), guard: Some(g) });
        assert_eq!(Command::parse(b"s").unwrap(), Command { action: Action::Stop, guard: None });
        assert!(Command::parse(b"g{\"version\":1}").is_err(), "a guard without its end");
    }

    /// N2: the command reached the relay and its answer was lost: what became of it is not known, never "failed".
    #[test]
    fn an_acknowledgement_lost_after_the_command_left_reads_as_maybe_delivered() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let pid = std::process::id();
        ctx.db.with(|c| Ok(c.execute("insert into run_state(run, pid, started, profile, harness) values('r', ?1, ?2, 'p', 'claude')", params![pid, crate::procs::start_ms(pid).unwrap()])?)).unwrap();
        let control = Control::listen(&ctx, "r").unwrap();
        control.listener.set_nonblocking(false).unwrap();
        let relay = thread::spawn(move || {
            let (mut s, _) = control.listener.accept().unwrap();
            s.write_all(&[0; 16]).unwrap();
            // the whole command arrives; the relay then goes away without a word
            let mut command = Vec::new();
            let _ = s.read_to_end(&mut command);
            command.len()
        });
        let e = send(&ctx.data, "r", Action::Reply("hello"), None).unwrap_err();
        assert!(relay.join().unwrap() > 24, "the command was sent");
        assert!(e.downcast_ref::<MaybeDelivered>().is_some(), "{e:#}");
    }
}
