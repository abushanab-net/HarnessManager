use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Write via a temp file in the same folder and rename, so readers never see half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // pid plus a counter: two threads of one process never share a temp file
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("hrm-tmp-{}-{n}", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = written.with_context(|| format!("write {}", tmp.display())).and_then(|_| {
        std::fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))
    }) {
        let _ = std::fs::remove_file(&tmp); // no stray temp files after a failed save
        return Err(e);
    }
    Ok(())
}

/// A file's text, or "" when it does not exist. Any other failure (locked, not yet downloaded by OneDrive)
/// is an error, so callers never mistake an unreadable file for an empty one and write over it.
pub fn read_or_empty(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("could not read {}; left unchanged", path.display())),
    }
}

/// A system-wide named mutex shared by every Harness Manager process (and thread); released on drop.
/// Re-taking it on a thread that already holds it does not block.
pub struct NamedLock(#[cfg(windows)] windows_sys::Win32::Foundation::HANDLE);

impl NamedLock {
    pub fn take(name: &str) -> NamedLock {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
            let w: Vec<u16> = format!(r"Local\HarnessManager-{name}").encode_utf16().chain(std::iter::once(0)).collect();
            let h = unsafe { CreateMutexW(std::ptr::null(), 0, w.as_ptr()) };
            // shortcut: after 10 s we go on without the lock rather than hang; a holder that slow is stuck
            if !h.is_null() { unsafe { WaitForSingleObject(h, 10_000) }; }
            NamedLock(h)
        }
        #[cfg(not(windows))]
        { let _ = name; NamedLock() }
    }
}

#[cfg(windows)]
impl Drop for NamedLock {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;
        if !self.0.is_null() { unsafe { ReleaseMutex(self.0); CloseHandle(self.0); } }
    }
}

pub fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

/// Find an executable on PATH, honouring PATHEXT on Windows (claude, codex and opencode are `.cmd` shims).
pub fn which(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()).split(';').map(|e| e.to_ascii_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&std::env::var_os("PATH")?) {
        for ext in &exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// A Mac app started from the Dock or Finder gets launchd's PATH (`/usr/bin:/bin:...`), not the one the user's
/// shell sets up, so tools installed by npm, Homebrew or their own installers are not found, and neither is the
/// node they run on. Take the login shell's PATH, then the usual install folders, once at startup.
#[cfg(not(windows))]
pub fn adopt_shell_path() {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = vec![];
    if let Some(p) = login_shell_path() { dirs.extend(std::env::split_paths(&p)); }
    if let Some(p) = std::env::var_os("PATH") { dirs.extend(std::env::split_paths(&p)); }
    if let Some(h) = &home {
        dirs.extend([".local/bin", ".npm-global/bin", ".bun/bin", ".opencode/bin", ".volta/bin"].map(|d| h.join(d)));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| !d.as_os_str().is_empty() && seen.insert(d.clone()));
    if let Ok(p) = std::env::join_paths(dirs) { std::env::set_var("PATH", p); }
}

/// PATH as the user's shell has it. Interactive too: nvm and most PATH lines live in .zshrc or .bashrc.
#[cfg(not(windows))]
fn login_shell_path() -> Option<String> {
    use std::io::Read;
    const MARK: &str = "__HRM_PATH__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut child = std::process::Command::new(shell).args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
    // a shell profile that waits for something must not stop the app from starting
    let start = std::time::Instant::now();
    while child.try_wait().ok()?.is_none() {
        if start.elapsed() > std::time::Duration::from_secs(5) { let _ = child.kill(); return None; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    out.split(MARK).nth(1).filter(|p| !p.is_empty()).map(String::from)
}

/// Valid profile and item names: letters, digits, `-` and `_`, as they also become folder names.
pub fn valid_name(name: &str) -> bool {
    // CON, NUL, COM1 ... name devices on Windows, not folders
    let l = name.to_ascii_lowercase();
    let device = matches!(l.as_str(), "con" | "prn" | "aux" | "nul")
        || (l.len() == 4 && (l.starts_with("com") || l.starts_with("lpt")) && l.as_bytes()[3].is_ascii_digit() && l.as_bytes()[3] != b'0');
    !device && !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn file_sig(path: &Path) -> Option<(i64, u64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((mtime, m.len()))
}

/// Collect files under `dir` whose name satisfies `keep`, recursively.
pub fn walk_files(dir: &Path, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk_files(&p, keep, out),
            Ok(_) if keep(&p) => out.push(p),
            _ => {}
        }
    }
}

pub fn truncate(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n).collect::<String>().trim_end()) }
}

#[cfg(all(test, not(windows)))]
mod unix_tests {
    #[test]
    fn unix_shell_path_adds_the_install_folders_and_keeps_the_system_ones() {
        // as the Dock starts an app
        std::env::set_var("PATH", "/usr/bin:/bin");
        super::adopt_shell_path();
        let p = std::env::var("PATH").unwrap();
        let dirs: Vec<_> = p.split(':').collect();
        let home = std::env::var("HOME").unwrap();
        for d in ["/usr/bin", "/bin", "/opt/homebrew/bin", "/usr/local/bin", &format!("{home}/.local/bin")] {
            assert!(dirs.contains(&d), "{d} missing from {p}");
        }
        assert_eq!(dirs.len(), dirs.iter().collect::<std::collections::HashSet<_>>().len(), "no folder twice: {p}");
        // the login shell answered: whatever it adds (nvm, Homebrew, npm's prefix) comes first
        println!("PATH after: {p}");
    }
}
