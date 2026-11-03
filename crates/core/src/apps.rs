//! The tools themselves: where each is installed, its version, the newest one, and installing or updating it
//! in a terminal window the user can watch (npm for npm installs, else the tool's own updater).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::harness;

/// npm packages a tool can come from, the usual one first (OpenCode has moved between names).
fn packages(id: &str) -> &'static [&'static str] {
    match id {
        "claude" => &["@anthropic-ai/claude-code"],
        "codex" => &["@openai/codex"],
        "opencode" => &["@opencode/cli", "opencode-ai"],
        _ => &[],
    }
}

/// The tool's own update command, for installs npm does not manage.
fn self_update(id: &str) -> Option<&'static str> {
    match id { "claude" => Some("update"), "agy" => Some("update"), "opencode" => Some("upgrade"), _ => None }
}

/// Where to get a tool npm cannot install.
fn download_page(id: &str) -> Option<&'static str> {
    match id { "agy" => Some("https://antigravity.google/"), _ => None }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub id: String,
    pub label: String,
    pub path: Option<String>,
    pub version: Option<String>,
    /// "npm" (with `package`), "self" (the tool updates itself), or none.
    pub manager: Option<String>,
    pub package: Option<String>,
    pub download: Option<String>,
}

/// `x.y.z` out of a version line such as "codex-cli 0.166.0" or "opencode v2.2.5".
pub fn version_in(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .map(|w| w.trim_start_matches('v'))
        .find(|w| w.split('.').count() >= 2 && w.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())))
        .map(String::from)
}

/// The npm package and version behind a tool installed in npm's global folder (`<prefix>\claude.cmd` with
/// `<prefix>\node_modules\<package>`), read from its package.json.
fn npm_install(id: &str, path: &Path) -> Option<(String, String)> {
    let modules = path.parent()?.join("node_modules");
    packages(id).iter().find_map(|p| {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(modules.join(p).join("package.json")).ok()?).ok()?;
        Some((p.to_string(), v["version"].as_str()?.to_string()))
    })
}

fn run_version(path: &Path) -> Option<String> {
    let mut c = std::process::Command::new(path);
    c.arg("--version").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); } // CREATE_NO_WINDOW
    let mut child = c.spawn().ok()?;
    // a tool that waits for input or a login must not hang the page
    let start = std::time::Instant::now();
    loop {
        if child.try_wait().ok()?.is_some() { break; }
        if start.elapsed() > Duration::from_secs(10) { let _ = child.kill(); return None; }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().ok()?;
    version_in(&String::from_utf8_lossy(&out.stdout)).or_else(|| version_in(&String::from_utf8_lossy(&out.stderr)))
}

pub fn list() -> Vec<AppInfo> {
    let found: Vec<_> = harness::all().iter().map(|h| (*h, crate::launch::resolve_program(h.id(), h.binary()).ok())).collect();
    // versions are read side by side: a tool's --version can take a second or two
    std::thread::scope(|s| {
        let jobs: Vec<_> = found.iter().map(|(h, path)| s.spawn(move || {
            let id = h.id();
            let npm = path.as_deref().and_then(|p| npm_install(id, p));
            let version = match (&npm, path) { (Some((_, v)), _) => Some(v.clone()), (None, Some(p)) => run_version(p), _ => None };
            AppInfo {
                id: id.into(), label: h.label().into(), path: path.as_ref().map(|p| p.display().to_string()), version,
                manager: if npm.is_some() || (path.is_none() && !packages(id).is_empty()) { Some("npm".into()) } else if path.is_some() && self_update(id).is_some() { Some("self".into()) } else { None },
                package: npm.map(|(p, _)| p).or_else(|| packages(id).first().map(|p| p.to_string())),
                download: download_page(id).map(String::from),
            }
        })).collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    })
}

/// The newest version of each npm package, from the registry (`npm view <package> version`).
pub fn latest(apps: &[AppInfo]) -> Vec<(String, Option<String>)> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = apps.iter().filter(|a| a.manager.as_deref() == Some("npm")).map(|a| s.spawn(move || {
            let mut c = std::process::Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
            c.args(["view", a.package.as_deref().unwrap_or(""), "version"]).stdin(std::process::Stdio::null());
            #[cfg(windows)]
            { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
            let v = c.output().ok().filter(|o| o.status.success()).and_then(|o| version_in(&String::from_utf8_lossy(&o.stdout)));
            (a.id.clone(), v)
        })).collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    })
}

/// "2.10.0" is newer than "2.9.3".
pub fn newer(a: &str, b: &str) -> bool {
    let n = |s: &str| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    n(a) > n(b)
}

/// Ask npm for the newest versions, remember them with the time, and count the tools that have a newer one.
pub fn check(ctx: &crate::Ctx) -> anyhow::Result<serde_json::Value> {
    let apps = list();
    let latest = latest(&apps);
    let updates = apps.iter().filter(|a| latest.iter().any(|(id, v)| id == &a.id && matches!((v, &a.version), (Some(l), Some(c)) if newer(l, c)))).count();
    let versions: serde_json::Map<String, serde_json::Value> = latest.into_iter().map(|(id, v)| (id, serde_json::json!(v))).collect();
    let v = serde_json::json!({"at": crate::util::now_ms(), "versions": versions, "updates": updates});
    ctx.db.kv_set("apps:latest", &v.to_string())?;
    for a in &apps {
        if let (Some(Some(l)), Some(c)) = (v["versions"].get(&a.id).map(|x| x.as_str()), &a.version) {
            if newer(l, c) { let _ = ctx.db.event("warn", "update", &format!("{} {l} is out. You have {c}.", a.label)); }
        }
    }
    Ok(v)
}

/// Install or update a tool in a new terminal window, so its progress and any error stay in view.
pub fn update(id: &str) -> Result<String> {
    let a = list().into_iter().find(|a| a.id == id).ok_or_else(|| anyhow::anyhow!("unknown tool {id}"))?;
    let cmdline = match a.manager.as_deref() {
        Some("npm") => format!("npm install -g {}@latest", a.package.as_deref().unwrap_or_default()),
        Some("self") => format!("\"{}\" {}", a.path.as_deref().unwrap_or_default(), self_update(id).unwrap_or("update")),
        _ => bail!("{} cannot be installed from here{}", a.label, a.download.as_deref().map(|d| format!("; get it from {d}")).unwrap_or_default()),
    };
    open_shell(&cmdline)?;
    Ok(cmdline)
}

#[cfg(windows)]
fn open_shell(line: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;
    // tests record the command instead of running npm or an updater
    if let Ok(log) = std::env::var("HRM_TERMINAL_LOG") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
        writeln!(f, "{}", serde_json::json!({"shell": line}))?;
        return Ok(());
    }
    // cmd /c "..." keeps the inner quotes; a new console opens in the default terminal
    let line = format!("{line} & echo. & echo Finished. Press any key to close this window. & pause >nul");
    std::process::Command::new("cmd.exe").raw_arg(format!("/c \"{line}\"")).creation_flags(0x10)
        .current_dir(std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)).spawn()?;
    Ok(())
}
#[cfg(not(windows))]
fn open_shell(line: &str) -> Result<()> {
    crate::launch::shell_window(&format!("{line}; echo; read -rp 'Finished. Press Enter to close this window.'"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions_are_read_from_each_tools_own_wording() {
        assert_eq!(super::version_in("2.1.316 (Claude Code)").as_deref(), Some("2.1.316"));
        assert_eq!(super::version_in("codex-cli 0.166.0").as_deref(), Some("0.166.0"));
        assert_eq!(super::version_in("opencode v2.2.5").as_deref(), Some("2.2.5"));
        assert_eq!(super::version_in("1.4.12\n").as_deref(), Some("1.4.12"));
        assert_eq!(super::version_in("no version here"), None);
        assert!(super::newer("2.10.0", "2.9.3") && !super::newer("2.1.316", "2.1.316") && !super::newer("1.9", "1.10"));
    }
}
