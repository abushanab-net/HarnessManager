use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

use crate::Ctx;

const MAX_TEXT: usize = 16 * 1024;
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

#[repr(u8)]
pub(super) enum Outcome {
    Sent = 0,
    Busy = 1,
    Closed = 2,
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
        input: impl FnOnce(Action<'_>) -> Outcome + Send + 'static,
    ) -> io::Result<()> {
        self.clients.retain(|client| !client.is_finished());
        match self.listener.accept() {
            Ok((stream, _)) if self.clients.len() < MAX_CLIENTS => {
                let cipher = self.cipher.clone();
                self.clients.push(thread::Builder::new().spawn(move || {
                    let _ = serve(stream, &cipher, input);
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

fn socket_timeout(stream: &TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))
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
    mut stream: TcpStream,
    cipher: &XChaCha20Poly1305,
    input: impl FnOnce(Action<'_>) -> Outcome,
) -> Result<()> {
    socket_timeout(&stream)?;
    // A fresh challenge for each connection prevents replay and keeps the token off the wire.
    // Separate nonce spaces for the request and the authenticated result.
    let challenge = random::<16>()?;
    stream.write_all(&challenge)?;
    let mut nonce = [0; 24];
    stream.read_exact(&mut nonce)?;
    ensure!(nonce[0] & 0x80 == 0, "invalid terminal nonce");
    const MAX_FRAME: usize = 1 + MAX_TEXT + TAG_LEN;
    let mut bytes = Vec::new();
    (&mut stream)
        .take((MAX_FRAME + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_FRAME, "terminal command is too large");
    let request = open(cipher, nonce, &challenge, &bytes)?;
    let outcome = input(Action::parse(&request)?);
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

pub(super) fn send(data: &Path, run: &str, action: Action<'_>) -> Result<()> {
    let request = action.encode()?;
    let (port, cipher) = endpoint(data, run)?;
    let address = (Ipv4Addr::LOCALHOST, port).into();
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)
        .with_context(|| format!("owned terminal for run '{run}' is no longer reachable"))?;
    socket_timeout(&stream)?;
    let mut challenge = [0; 16];
    stream
        .read_exact(&mut challenge)
        .context("owned terminal did not accept the connection")?;
    let mut nonce = random::<24>()?;
    nonce[0] &= 0x7f;
    stream.write_all(&nonce)?;
    stream.write_all(&seal(&cipher, nonce, &challenge, &request)?)?;
    stream.shutdown(Shutdown::Write)?;

    let mut response = [0; 1 + TAG_LEN];
    stream
        .read_exact(&mut response)
        .context("owned terminal did not confirm delivery")?;
    nonce[0] |= 0x80;
    match open(&cipher, nonce, &challenge, &response)?.as_slice() {
        [code] if *code == Outcome::Sent as u8 => Ok(()),
        [code] if *code == Outcome::Busy as u8 => {
            bail!("the terminal is busy writing input; try again")
        }
        [code] if *code == Outcome::Closed as u8 => bail!("the terminal input has closed"),
        _ => bail!("invalid terminal response"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_a_single_line_or_stop() {
        assert_eq!(
            Action::parse("réply".as_bytes()).unwrap(),
            Action::Reply("éply")
        );
        assert_eq!(Action::parse(b"s").unwrap(), Action::Stop);
        assert_eq!(Action::parse(b"r").unwrap(), Action::Reply(""));
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
}
