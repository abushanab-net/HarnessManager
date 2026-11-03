//! Starting a harness as a profile: in this console (`hrm run`) or in a new terminal window.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::util::which;

/// Everything needed to start a harness for one profile. Secrets go in `env_set` only inside the
/// `hrm run` process, never on a command line.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env_set: Vec<(String, String)>,
    pub env_unset: Vec<String>,
    pub cwd: Option<PathBuf>,
}

impl LaunchSpec {
    pub fn command(&self) -> Command {
        let mut c = Command::new(&self.program);
        c.args(&self.args);
        for k in &self.env_unset {
            c.env_remove(k);
        }
        for (k, v) in &self.env_set {
            c.env(k, v);
        }
        if let Some(d) = &self.cwd {
            c.current_dir(d);
        }
        c
    }

    /// Run in this console and return the exit code. Ctrl+C belongs to the child.
    pub fn run(&self) -> Result<i32> {
        ignore_ctrl_c();
        let status = self.command().stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit())
            .status().with_context(|| format!("start {}", self.program.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}

#[cfg(windows)]
fn ignore_ctrl_c() {
    // The harness handles Ctrl+C itself; hrm must stay alive until it exits.
    unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 1) };
}
#[cfg(not(windows))]
fn ignore_ctrl_c() {}

/// Resolve a harness binary: `HRM_BIN_<ID>` (tests point this at the fake harness), else PATH.
pub fn resolve_program(harness_id: &str, default_name: &str) -> Result<PathBuf> {
    let var = format!("HRM_BIN_{}", harness_id.to_ascii_uppercase());
    if let Ok(p) = std::env::var(&var) {
        return Ok(PathBuf::from(p));
    }
    match which(default_name) {
        Some(p) => Ok(p),
        None => bail!("{default_name} is not installed or not on PATH"),
    }
}

/// The `hrm` executable that terminals run: `HRM_EXE`, else `hrm.exe` next to the current executable.
pub fn hrm_exe() -> PathBuf {
    if let Ok(p) = std::env::var("HRM_EXE") {
        return PathBuf::from(p);
    }
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hrm"));
    let name = if cfg!(windows) { "hrm.exe" } else { "hrm" };
    if me.file_name().map(|n| n == name).unwrap_or(false) {
        return me;
    }
    let sibling = me.with_file_name(name);
    if sibling.exists() { sibling } else { which("hrm").unwrap_or_else(|| PathBuf::from(name)) }
}

#[derive(Serialize)]
struct Recorded<'a> {
    title: &'a str,
    args: &'a [String],
    cwd: Option<&'a Path>,
}

/// The 8.3 form of an existing path (`C:\Users\me\AppData\Local\HARNES~1\hrm.exe`), which has no spaces;
/// the path itself when it has no spaces or the drive keeps no short names.
pub fn short_path(p: &Path) -> PathBuf {
    #[cfg(windows)]
    if p.to_string_lossy().contains(' ') {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut buf = vec![0u16; 1024];
        let n = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n > 0 && n < buf.len() {
            let short = PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]));
            if !short.to_string_lossy().contains(' ') { return short; }
        }
    }
    p.to_path_buf()
}

/// Open a new terminal window running `hrm <args>`. `terminal` is "wt" or "console".
/// `HRM_TERMINAL_LOG` records the request instead (end-to-end tests).
pub fn open_terminal(terminal: &str, title: &str, hrm_args: &[String], cwd: Option<&Path>) -> Result<()> {
    if let Ok(log) = std::env::var("HRM_TERMINAL_LOG") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
        writeln!(f, "{}", serde_json::to_string(&Recorded { title, args: hrm_args, cwd })?)?;
        return Ok(());
    }
    // Windows Terminal re-reads the command line and can lose the quotes around a path with a space
    // (the installed `...\Harness Manager\hrm.exe`), then looks for `...\Harness`: give it the short form
    let hrm = short_path(&hrm_exe());
    // a new console opens in Windows Terminal by itself when it is Windows' default terminal, with our exact
    // arguments; wt.exe re-parses its command line and sometimes runs "hrm.exe run x" as one file name
    if terminal == "wt" && !wt_is_default_terminal() {
        if let Some(wt) = which("wt") {
            let mut c = Command::new(wt);
            let esc = |s: &str| s.replace(';', "\\;");
            c.args(["-w", "new", "new-tab", "--title", &esc(title)]);
            if let Some(d) = cwd {
                c.arg("-d").arg(esc(&d.display().to_string()));
            }
            c.arg(&hrm).args(hrm_args.iter().map(|a| a.replace(';', "\\;")));
            c.spawn().context("start Windows Terminal")?;
            return Ok(());
        }
    }
    new_console(&hrm, hrm_args, cwd)
}

#[cfg(windows)]
fn new_console(hrm: &Path, args: &[String], cwd: Option<&Path>) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x10;
    let mut c = Command::new(hrm);
    c.args(args).creation_flags(CREATE_NEW_CONSOLE);
    // not the app's own folder (System32 when Explorer starts it): the home folder, as a terminal profile would
    if let Some(d) = cwd.map(Path::to_path_buf).or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from)) {
        c.current_dir(d);
    }
    c.spawn().context("open a console window")?;
    Ok(())
}

/// Windows 11's "Default terminal application" is Windows Terminal (Settings > System > For developers > Terminal).
#[cfg(windows)]
pub fn wt_is_default_terminal() -> bool {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let mut buf = [0u16; 64];
    let mut len = (buf.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, w(r"Console\%%Startup").as_ptr(), w("DelegationTerminal").as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len)
    } == 0;
    // the GUIDs of Windows Terminal and Windows Terminal Preview; {00000000-...} means "let Windows decide" (conhost)
    let v = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]).to_ascii_uppercase();
    ok && (v == "{E12CFF52-A866-4C77-9A90-F570A7AA2C6B}" || v == "{86633F1F-6454-40EC-89CE-DA4EBA977EE2}")
}
#[cfg(not(windows))]
pub fn wt_is_default_terminal() -> bool { false }

#[cfg(not(windows))]
fn new_console(hrm: &Path, args: &[String], cwd: Option<&Path>) -> Result<()> {
    // shortcut: Linux/macOS terminal launchers (x-terminal-emulator, Terminal.app) come with those builds.
    let mut c = Command::new("x-terminal-emulator");
    c.arg("-e").arg(hrm).args(args);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    c.spawn().context("open a terminal")?;
    Ok(())
}

/// The Windows sign-in entry that starts Harness Manager in the tray (HKCU\...\Run). Tests point
/// HRM_AUTOSTART_KEY at a key of their own.
#[cfg(windows)]
pub mod autostart {
    use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ};
    const VALUE: &str = "HarnessManager";
    fn w(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
    fn key() -> String { std::env::var("HRM_AUTOSTART_KEY").unwrap_or_else(|_| r"Software\Microsoft\Windows\CurrentVersion\Run".into()) }

    /// The command Windows runs at sign-in, if the entry is there.
    pub fn get() -> Option<String> {
        let mut buf = vec![0u16; 2048];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe { RegGetValueW(HKEY_CURRENT_USER, w(&key()).as_ptr(), w(VALUE).as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) } == 0;
        ok.then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
    }

    /// On: start `exe --tray` at sign-in. Off: remove the entry.
    pub fn set(on: bool, exe: &std::path::Path) -> anyhow::Result<()> {
        let r = if on {
            let cmd = w(&format!("\"{}\" --tray", exe.display()));
            unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, w(&key()).as_ptr(), w(VALUE).as_ptr(), REG_SZ, cmd.as_ptr().cast(), (cmd.len() * 2) as u32) }
        } else {
            let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, w(&key()).as_ptr(), w(VALUE).as_ptr()) };
            if r == 2 { 0 } else { r } // already gone
        };
        if r != 0 { anyhow::bail!("could not change the sign-in entry (error {r})"); }
        Ok(())
    }
}

/// shortcut: launch at sign-in is Windows-only; add a LaunchAgent / XDG autostart entry when macOS or Linux is supported.
#[cfg(not(windows))]
pub mod autostart {
    pub fn get() -> Option<String> { None }
    pub fn set(_on: bool, _exe: &std::path::Path) -> anyhow::Result<()> {
        anyhow::bail!("launch at sign-in is only available on Windows for now")
    }
}
