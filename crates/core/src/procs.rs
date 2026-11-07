//! Process checks: is a PID alive, and which processes of a harness are running.

#[cfg(windows)]
pub fn alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        ok && code == STILL_ACTIVE as u32
    }
}

#[cfg(not(windows))]
pub fn alive(pid: u32) -> bool { std::path::Path::new(&format!("/proc/{pid}")).exists() }

/// Processes with one of these names (and, when given, this program file): (pid, start ms, working folder).
pub fn by_name(names: &[&str], exe: Option<&std::path::Path>) -> Vec<(u32, i64, Option<String>)> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let hits: Vec<Pid> = names_now().iter().filter(|(_, n)| names.iter().any(|x| n == x)).map(|(p, _)| Pid::from_u32(*p)).collect();
    if hits.is_empty() { return vec![]; }
    // the folder and the program file only for the few that match: reading them for every process of a busy
    // computer (1,600 of them) took seconds, once per account
    let Ok(mut sys) = System::new() else { return vec![] };
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&hits), true, ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always).with_exe(UpdateKind::Always));
    hits.iter()
        .filter_map(|pid| sys.process(*pid))
        .filter(|p| exe.map(|e| p.exe().map(|x| x.to_string_lossy().eq_ignore_ascii_case(&e.to_string_lossy())).unwrap_or(false)).unwrap_or(true))
        .map(|p| (p.pid().as_u32(), p.start_time() as i64 * 1000, p.cwd().map(|c| c.display().to_string())))
        .collect()
}

/// Every process as (pid, lowercase name without .exe), from one scan shared for two seconds: the accounts of a
/// page all ask at once, and a scan opens every process.
fn names_now() -> std::sync::Arc<Vec<(u32, String)>> {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    static LAST: Mutex<Option<(Instant, Arc<Vec<(u32, String)>>)>> = Mutex::new(None);
    // held while scanning, so the accounts asking together wait for one scan instead of each making their own
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, v)) = last.as_ref() { if at.elapsed() < Duration::from_secs(2) { return v.clone(); } }
    let v: Arc<Vec<(u32, String)>> = Arc::new(System::new().map(|mut sys| {
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
        sys.processes().values().map(|p| (p.pid().as_u32(), p.name().to_string_lossy().to_ascii_lowercase().trim_end_matches(".exe").to_string())).collect()
    }).unwrap_or_default());
    *last = Some((Instant::now(), v.clone()));
    v
}

/// When a running process started (epoch ms), to tell a reused pid from the process that wrote a pid file.
/// Asked of that one process: sysinfo lists every process to find it, half a second on a busy computer.
#[cfg(windows)]
pub fn start_ms(pid: u32) -> Option<i64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() { return None; }
        let zero = || FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero(), zero(), zero(), zero());
        let ok = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        CloseHandle(h);
        // FILETIME: 100 ns since 1601
        ok.then(|| (((created.dwHighDateTime as i64) << 32) | created.dwLowDateTime as i64) / 10_000 - 11_644_473_600_000)
    }
}

#[cfg(not(windows))]
pub fn start_ms(pid: u32) -> Option<i64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new().ok()?;
    let pid = Pid::from_u32(pid);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
    sys.process(pid).map(|p| p.start_time() as i64 * 1000)
}

/// Working folder of a running process, when the OS lets us read it.
pub fn cwd_of(pid: u32) -> Option<String> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new().ok()?;
    let pid = Pid::from_u32(pid);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always));
    sys.process(pid).and_then(|p| p.cwd()).map(|c| c.display().to_string()).filter(|s| !s.is_empty())
}

pub fn kill(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h.is_null() { anyhow::bail!("cannot open process {pid}"); }
        let ok = TerminateProcess(h, 0) != 0;
        CloseHandle(h);
        if !ok { anyhow::bail!("cannot stop process {pid}"); }
    }
    #[cfg(not(windows))]
    { std::process::Command::new("kill").arg(pid.to_string()).status()?; }
    Ok(())
}
