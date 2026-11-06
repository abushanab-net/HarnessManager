//! The tools themselves: every copy of each on this computer (on PATH and in the folders the installers use),
//! how each copy was installed, its version, the newest version from the vendor, and installing, updating or
//! removing a copy in a terminal window the user can watch.
//!
//! What each vendor ships (read from their installer scripts and docs, November 2026):
//! - Claude Code: `irm https://claude.ai/install.ps1 | iex` / `curl -fsSL https://claude.ai/install.sh | bash`, to
//!   `~/.local/bin/claude`; the newest version is the text at downloads.claude.ai/claude-code-releases/latest;
//!   `claude update`; also winget, brew casks, apt/dnf/apk and npm.
//! - Codex: `irm https://chatgpt.com/codex/install.ps1 | iex` / `curl -fsSL https://chatgpt.com/codex/install.sh | sh`,
//!   to `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin` or `~/.local/bin`; newest from releases.openai.com/codex/channels/latest
//!   (`tag_name` rust-vX.Y.Z); no update command, the installer is run again; also a brew cask and npm.
//! - OpenCode: `curl -fsSL https://opencode.ai/install | bash` (1) or `.../v2/install` (2), bash only, to `~/.opencode/bin`;
//!   newest from the npm registry (`opencode-ai` for 1, `@opencode/cli` for 2); `opencode upgrade`; also brew, scoop, choco, npm.
//! - Antigravity CLI: `irm https://antigravity.google/cli/install.ps1 | iex` / `curl -fsSL https://antigravity.google/cli/install.sh | bash`,
//!   to `%LOCALAPPDATA%\agy\bin` or `~/.local/bin`; it updates itself in the background and has no update command,
//!   so a forced update removes the file and runs the installer again; newest from the installer's own manifest.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::harness;
use crate::util::which;

/// How a copy of a tool got onto this computer, read from where its file sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind { Native, Npm, Brew, Winget, Scoop, Choco, Unknown }

/// What the vendor ships for one tool.
struct Installer {
    /// Folders the vendor's installer writes to (Windows), relative to the env var named first.
    win_dirs: &'static [(&'static str, &'static str)],
    /// Folders on macOS and Linux, relative to home ("~") or absolute.
    unix_dirs: &'static [&'static str],
    /// The vendor's installer, as a PowerShell command and as a shell command; empty when there is none for that OS.
    win_install: &'static str,
    unix_install: &'static str,
    /// Where the vendor's page is, for a tool that must be installed by hand.
    site: &'static str,
    npm: &'static [&'static str],
    winget: &'static str,
    brew: &'static str,
    scoop: &'static str,
    choco: &'static str,
    /// The tool's own update command, for a native copy; empty when the installer is run again instead.
    self_update: &'static str,
    /// The tool updates itself in the background.
    auto_updates: bool,
}

const CLAUDE: Installer = Installer {
    win_dirs: &[("USERPROFILE", r".local\bin")], unix_dirs: &["~/.local/bin"],
    win_install: "irm https://claude.ai/install.ps1 | iex", unix_install: "curl -fsSL https://claude.ai/install.sh | bash",
    site: "https://code.claude.com/docs/en/install",
    npm: &["@anthropic-ai/claude-code"], winget: "Anthropic.ClaudeCode", brew: "--cask claude-code", scoop: "", choco: "",
    self_update: "update", auto_updates: true,
};
const CODEX: Installer = Installer {
    win_dirs: &[("LOCALAPPDATA", r"Programs\OpenAI\Codex\bin"), ("USERPROFILE", r".local\bin")], unix_dirs: &["~/.local/bin"],
    win_install: "irm https://chatgpt.com/codex/install.ps1 | iex", unix_install: "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
    site: "https://github.com/openai/codex",
    npm: &["@openai/codex"], winget: "", brew: "--cask codex", scoop: "", choco: "",
    self_update: "", auto_updates: false,
};
const OPENCODE: Installer = Installer {
    win_dirs: &[("USERPROFILE", r".opencode\bin"), ("USERPROFILE", r".local\bin")], unix_dirs: &["~/.opencode/bin", "~/.local/bin"],
    // the script is bash only: on Windows it needs Git Bash or MSYS
    win_install: "", unix_install: "curl -fsSL https://opencode.ai/v2/install | bash",
    site: "https://opencode.ai/docs/",
    npm: &["@opencode/cli", "opencode-ai"], winget: "", brew: "opencode", scoop: "opencode", choco: "opencode",
    self_update: "upgrade", auto_updates: true,
};
const AGY: Installer = Installer {
    win_dirs: &[("LOCALAPPDATA", r"agy\bin"), ("ProgramFiles", r"Google\antigravity-cli"), ("USERPROFILE", r".local\bin")], unix_dirs: &["~/.local/bin"],
    win_install: "irm https://antigravity.google/cli/install.ps1 | iex", unix_install: "curl -fsSL https://antigravity.google/cli/install.sh | bash",
    site: "https://antigravity.google/docs/getting-started?tab=cli",
    npm: &[], winget: "", brew: "--cask antigravity-cli", scoop: "", choco: "",
    self_update: "", auto_updates: true,
};
// Pi and Oh My Pi ship on npm only (the first Pi package name is deprecated in favour of the second, both still install)
const PI: Installer = Installer {
    win_dirs: &[], unix_dirs: &[],
    win_install: "", unix_install: "",
    site: "https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent",
    npm: &["@earendil-works/pi-coding-agent", "@mariozechner/pi-coding-agent"], winget: "", brew: "", scoop: "", choco: "",
    self_update: "", auto_updates: false,
};
const OMP: Installer = Installer {
    win_dirs: &[], unix_dirs: &[],
    win_install: "", unix_install: "",
    site: "https://omp.sh/docs",
    npm: &["@oh-my-pi/pi-coding-agent"], winget: "", brew: "", scoop: "", choco: "",
    self_update: "", auto_updates: false,
};

fn installer(id: &str) -> Option<&'static Installer> {
    match id { "claude" => Some(&CLAUDE), "codex" => Some(&CODEX), "opencode" => Some(&OPENCODE), "agy" => Some(&AGY), "pi" => Some(&PI), "omp" => Some(&OMP), _ => None }
}

/// One file of a tool on this computer.
#[derive(Debug, Clone, Serialize)]
pub struct Copy {
    pub path: String,
    pub version: Option<String>,
    pub kind: Kind,
    /// The npm package behind an npm copy.
    pub package: Option<String>,
    /// The copy a terminal gets when it types the tool's name (first on PATH).
    pub runs: bool,
}

/// A way to install a tool that this computer can run now.
#[derive(Debug, Clone, Serialize)]
pub struct Method { pub id: String, pub label: String, pub command: String }

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub id: String,
    pub label: String,
    /// The copy that runs: its path, version and kind (None when the tool is not installed).
    pub path: Option<String>,
    pub version: Option<String>,
    pub kind: Option<Kind>,
    /// Kept for older readers of the JSON: "npm", "self" (updates itself) or none.
    pub manager: Option<String>,
    pub package: Option<String>,
    pub download: Option<String>,
    pub auto_updates: bool,
    /// Every copy found, the one that runs first.
    pub copies: Vec<Copy>,
    /// The command Update runs for the copy that runs, and what it does.
    pub update: Option<Method>,
    /// Ways to install it that this computer can run now, the vendor's own first.
    pub install: Vec<Method>,
}

/// `x.y.z` out of a version line such as "codex-cli 0.166.0" or "opencode v2.2.5".
pub fn version_in(s: &str) -> Option<String> {
    s.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '"' || c == ',')
        .map(|w| w.trim_start_matches('v'))
        .find(|w| w.split('.').count() >= 2 && w.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())))
        .map(String::from)
}

/// The npm package and version behind a tool installed in npm's global folder (`<prefix>\claude.cmd` with
/// `<prefix>\node_modules\<package>`), read from its package.json.
fn npm_install(id: &str, path: &Path) -> Option<(String, String)> {
    let modules = path.parent()?.join("node_modules");
    installer(id)?.npm.iter().find_map(|p| {
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

/// The folders the vendor's installer and the package managers put this tool in, on this OS.
fn known_dirs(ins: &Installer) -> Vec<PathBuf> {
    let mut v = vec![];
    if cfg!(windows) {
        for (var, rel) in ins.win_dirs { if let Some(b) = std::env::var_os(var) { v.push(PathBuf::from(b).join(rel)); } }
        if let Some(a) = std::env::var_os("APPDATA") { v.push(PathBuf::from(a).join("npm")); }
        if let Some(l) = std::env::var_os("LOCALAPPDATA") { v.push(PathBuf::from(l).join(r"Microsoft\WinGet\Links")); }
        if let Some(h) = std::env::var_os("USERPROFILE") { v.push(PathBuf::from(h).join(r"scoop\shims")); }
        if let Some(p) = std::env::var_os("ProgramData") { v.push(PathBuf::from(p).join(r"chocolatey\bin")); }
    } else {
        let home = dirs::home_dir().unwrap_or_default();
        for d in ins.unix_dirs { v.push(if let Some(r) = d.strip_prefix("~/") { home.join(r) } else { PathBuf::from(d) }); }
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/home/linuxbrew/.linuxbrew/bin"] { v.push(PathBuf::from(d)); }
        v.push(home.join(".npm-global/bin"));
    }
    v
}

/// How a copy was installed, from its path (links followed) and its neighbours.
pub fn kind_of(id: &str, path: &Path) -> Kind {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = real.display().to_string().to_ascii_lowercase().replace('/', "\\");
    if s.contains("\\node_modules\\") || npm_install(id, path).is_some() { return Kind::Npm; }
    if s.contains("\\scoop\\") { return Kind::Scoop; }
    if s.contains("\\chocolatey\\") { return Kind::Choco; }
    if s.contains("\\winget\\") || s.contains("\\windowsapps\\") { return Kind::Winget; }
    if s.contains("\\cellar\\") || s.contains("\\caskroom\\") || s.contains("\\homebrew\\") || s.contains("\\linuxbrew\\") { return Kind::Brew; }
    let native = [r"\.local\bin\", r"\.local\share\claude\", r"\programs\openai\codex\", r"\agy\bin\", r"\google\antigravity-cli\", r"\.opencode\bin\", r"\.codex\packages\"];
    if native.iter().any(|n| s.contains(n)) { return Kind::Native; }
    Kind::Unknown
}

fn q(p: &str) -> String { format!("\"{p}\"") }
/// A PowerShell command line as cmd.exe runs it (the installers are PowerShell on Windows).
fn ps(cmd: &str) -> String { format!("powershell -NoProfile -ExecutionPolicy Bypass -Command \"{}\"", cmd.replace('"', "'")) }

/// What Update runs for a copy, by how it was installed.
fn update_method(id: &str, ins: &Installer, c: &Copy) -> Option<Method> {
    let m = |id: &str, label: &str, command: String| Some(Method { id: id.into(), label: label.into(), command });
    let native_cmd = if cfg!(windows) { if ins.win_install.is_empty() { String::new() } else { ps(ins.win_install) } } else { ins.unix_install.to_string() };
    match c.kind {
        Kind::Npm => m("npm", "Update with npm", format!("npm install -g {}@latest", c.package.clone().or_else(|| ins.npm.first().map(|s| s.to_string())).unwrap_or_default())),
        Kind::Winget if !ins.winget.is_empty() => m("winget", "Update with winget", format!("winget upgrade {}", ins.winget)),
        Kind::Brew if !ins.brew.is_empty() => m("brew", "Update with Homebrew", format!("brew upgrade {}", ins.brew)),
        Kind::Scoop if !ins.scoop.is_empty() => m("scoop", "Update with Scoop", format!("scoop update {}", ins.scoop)),
        Kind::Choco if !ins.choco.is_empty() => m("choco", "Update with Chocolatey", format!("choco upgrade {} -y", ins.choco)),
        _ if !ins.self_update.is_empty() => m("self", "Run its updater", format!("{} {}", q(&c.path), ins.self_update)),
        // no update command: the installer is run again, over the file it finds (Antigravity's refuses an existing one, so it goes first)
        _ if !native_cmd.is_empty() => {
            let remove = if id == "agy" { if cfg!(windows) { format!("Remove-Item -Force '{}'; ", c.path) } else { format!("rm -f {} && ", q(&c.path)) } } else { String::new() };
            let command = if cfg!(windows) { if remove.is_empty() { native_cmd } else { ps(&format!("{remove}{}", ins.win_install)) } } else { format!("{remove}{}", ins.unix_install) };
            m("native", if remove.is_empty() { "Run the vendor's installer again" } else { "Remove it and run the vendor's installer" }, command)
        }
        _ => None,
    }
}

/// The ways this computer can install the tool now: the vendor's own first, then the package managers on PATH.
fn install_methods(ins: &Installer) -> Vec<Method> {
    let mut v = vec![];
    let has = |name: &str| which(name).is_some() || (cfg!(windows) && which(&format!("{name}.cmd")).is_some());
    if cfg!(windows) && !ins.win_install.is_empty() { v.push(Method { id: "native".into(), label: "The vendor's installer".into(), command: ps(ins.win_install) }); }
    if !cfg!(windows) && !ins.unix_install.is_empty() { v.push(Method { id: "native".into(), label: "The vendor's installer".into(), command: ins.unix_install.into() }); }
    if cfg!(windows) && ins.win_install.is_empty() && !ins.unix_install.is_empty() && has("bash") { v.push(Method { id: "native".into(), label: "The vendor's installer (in Git Bash)".into(), command: format!("bash -lc {}", q(ins.unix_install)) }); }
    if cfg!(windows) && !ins.winget.is_empty() && has("winget") { v.push(Method { id: "winget".into(), label: "winget".into(), command: format!("winget install {}", ins.winget) }); }
    if !cfg!(windows) && !ins.brew.is_empty() && has("brew") { v.push(Method { id: "brew".into(), label: "Homebrew".into(), command: format!("brew install {}", ins.brew) }); }
    if cfg!(windows) && !ins.scoop.is_empty() && has("scoop") { v.push(Method { id: "scoop".into(), label: "Scoop".into(), command: format!("scoop install {}", ins.scoop) }); }
    if cfg!(windows) && !ins.choco.is_empty() && has("choco") { v.push(Method { id: "choco".into(), label: "Chocolatey".into(), command: format!("choco install {} -y", ins.choco) }); }
    if let Some(p) = ins.npm.first() { if has("npm") { v.push(Method { id: "npm".into(), label: "npm".into(), command: format!("npm install -g {p}@latest") }); } }
    v
}

pub fn list() -> Vec<AppInfo> {
    let found: Vec<_> = harness::all().iter().map(|h| {
        let id = h.id();
        let runs = crate::launch::resolve_program(id, h.binary()).ok();
        // tests pin the tool with HRM_BIN_<ID>: then the real folders of this computer are left alone
        let pinned = std::env::var_os(format!("HRM_BIN_{}", id.to_ascii_uppercase())).is_some();
        let mut paths: Vec<PathBuf> = runs.iter().cloned().collect();
        if let (Some(ins), false) = (installer(id), pinned) {
            for d in known_dirs(ins) {
                for ext in if cfg!(windows) { vec![".exe", ".cmd"] } else { vec![""] } {
                    let p = d.join(format!("{}{ext}", h.binary()));
                    if p.is_file() && !paths.iter().any(|x| same_file(x, &p)) { paths.push(p); }
                }
            }
        }
        (*h, paths)
    }).collect();
    // versions are read side by side: a tool's --version can take a second or two
    std::thread::scope(|s| {
        let jobs: Vec<_> = found.iter().map(|(h, paths)| s.spawn(move || {
            let id = h.id();
            let ins = installer(id);
            let copies: Vec<Copy> = std::thread::scope(|s2| {
                let js: Vec<_> = paths.iter().enumerate().map(|(i, p)| s2.spawn(move || {
                    let npm = npm_install(id, p);
                    let version = match &npm { Some((_, v)) => Some(v.clone()), None => run_version(p) };
                    Copy { path: p.display().to_string(), version, kind: kind_of(id, p), package: npm.map(|(pk, _)| pk), runs: i == 0 }
                })).collect();
                js.into_iter().filter_map(|j| j.join().ok()).collect()
            });
            let first = copies.first();
            let update = match (ins, first) { (Some(ins), Some(c)) => update_method(id, ins, c), _ => None };
            AppInfo {
                id: id.into(), label: h.label().into(),
                path: first.map(|c| c.path.clone()), version: first.and_then(|c| c.version.clone()), kind: first.map(|c| c.kind),
                manager: match (first.map(|c| c.kind), ins) { (Some(Kind::Npm), _) => Some("npm".into()), (Some(_), Some(i)) if !i.self_update.is_empty() => Some("self".into()), (None, Some(i)) if !i.npm.is_empty() => Some("npm".into()), _ => None },
                package: first.and_then(|c| c.package.clone()).or_else(|| ins.and_then(|i| i.npm.first().map(|p| p.to_string()))),
                download: ins.map(|i| i.site.to_string()),
                auto_updates: ins.map(|i| i.auto_updates).unwrap_or(false),
                copies, update,
                install: ins.map(install_methods).unwrap_or_default(),
            }
        })).collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || match (std::fs::canonicalize(a), std::fs::canonicalize(b)) { (Ok(x), Ok(y)) => x == y, _ => false }
}

fn http(url: &str) -> Result<String> {
    let text = reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build()?
        .get(url).header("User-Agent", "Harness Manager").send()?.error_for_status()?.text()?;
    Ok(text)
}

/// The newest version the vendor offers for a tool; `installed` picks OpenCode's major line.
pub fn newest(id: &str, installed: Option<&str>) -> Result<String> {
    let json = |t: String| -> Result<serde_json::Value> { Ok(serde_json::from_str(&t)?) };
    match id {
        "claude" => version_in(http("https://downloads.claude.ai/claude-code-releases/latest")?.trim()).context("no version in the answer"),
        "codex" => {
            let tag = http("https://releases.openai.com/codex/channels/latest").and_then(json).ok().and_then(|v| v["tag_name"].as_str().map(String::from))
                .or_else(|| http("https://api.github.com/repos/openai/codex/releases/latest").and_then(json).ok().and_then(|v| v["tag_name"].as_str().map(String::from)))
                .context("no release found")?;
            version_in(tag.trim_start_matches("rust-v")).context("no version in the tag")
        }
        "opencode" => {
            let v2 = installed.and_then(|v| v.split('.').next()).and_then(|m| m.parse::<u32>().ok()).map(|m| m >= 2).unwrap_or(true);
            let v = json(http(if v2 { "https://registry.npmjs.org/@opencode/cli/latest" } else { "https://registry.npmjs.org/opencode-ai/latest" })?)?;
            v["version"].as_str().map(String::from).context("no version in the answer")
        }
        "agy" => {
            let platform = format!("{}_{}", match std::env::consts::OS { "windows" => "windows", "macos" => "darwin", _ => "linux" }, if std::env::consts::ARCH == "aarch64" { "arm64" } else { "amd64" });
            let v = json(http(&format!("https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/{platform}.json"))?)?;
            v["version"].as_str().map(String::from).context("no version in the manifest")
        }
        "pi" | "omp" => {
            let v = json(http(&format!("https://registry.npmjs.org/{}/latest", installer(id).and_then(|i| i.npm.first()).unwrap_or(&"")))?)?;
            v["version"].as_str().map(String::from).context("no version in the answer")
        }
        _ => bail!("no version source for {id}"),
    }
}

/// The newest version of each tool, from its vendor, side by side; None where the check failed.
pub fn latest(apps: &[AppInfo]) -> Vec<(String, Option<String>, Option<String>)> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = apps.iter().filter(|a| installer(&a.id).is_some()).map(|a| s.spawn(move || match newest(&a.id, a.version.as_deref()) {
            Ok(v) => (a.id.clone(), Some(v), None),
            Err(e) => (a.id.clone(), None, Some(format!("{e:#}"))),
        })).collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    })
}

/// "2.10.0" is newer than "2.9.3".
pub fn newer(a: &str, b: &str) -> bool {
    let n = |s: &str| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    n(a) > n(b)
}

/// The count again from the versions installed now against the last check's newest: an update applied since that
/// check leaves the count; the stored one follows, so the badge is right at the next launch too.
pub fn recount(ctx: &crate::Ctx, apps: &[AppInfo], latest: &mut Option<serde_json::Value>) -> anyhow::Result<u64> {
    let Some(l) = latest else { return Ok(0) };
    let n = apps.iter().filter(|a| matches!((l["versions"][a.id.as_str()].as_str(), &a.version), (Some(v), Some(c)) if newer(v, c))).count() as u64;
    if l["updates"].as_u64() != Some(n) { l["updates"] = n.into(); ctx.db.kv_set("apps:latest", &l.to_string())?; }
    Ok(n)
}

/// Ask each vendor for the newest versions, remember them with the time, and count the tools that have a newer one.
pub fn check(ctx: &crate::Ctx) -> anyhow::Result<serde_json::Value> {
    let apps = list();
    let latest = latest(&apps);
    let updates = apps.iter().filter(|a| latest.iter().any(|(id, v, _)| id == &a.id && matches!((v, &a.version), (Some(l), Some(c)) if newer(l, c)))).count();
    let versions: serde_json::Map<String, serde_json::Value> = latest.iter().map(|(id, v, _)| (id.clone(), serde_json::json!(v))).collect();
    let errors: serde_json::Map<String, serde_json::Value> = latest.iter().filter_map(|(id, _, e)| e.as_ref().map(|e| (id.clone(), serde_json::json!(e)))).collect();
    let v = serde_json::json!({"at": crate::util::now_ms(), "versions": versions, "errors": errors, "updates": updates});
    ctx.db.kv_set("apps:latest", &v.to_string())?;
    for a in &apps {
        if let (Some(Some(l)), Some(c)) = (v["versions"].get(&a.id).map(|x| x.as_str()), &a.version) {
            if newer(l, c) { let _ = ctx.db.event("warn", "update", &format!("{} {l} is out. You have {c}.", a.label)); }
        }
    }
    Ok(v)
}

/// Update the copy that runs, in a new terminal window, so its progress and any error stay in view.
pub fn update(id: &str) -> Result<String> {
    let a = list().into_iter().find(|a| a.id == id).ok_or_else(|| anyhow::anyhow!("unknown tool {id}"))?;
    let m = match (&a.update, a.path.is_some()) {
        (Some(m), true) => m.clone(),
        (_, false) => a.install.first().cloned().ok_or_else(|| anyhow::anyhow!("{} cannot be installed from here; get it from {}", a.label, a.download.as_deref().unwrap_or("its website")))?,
        _ => bail!("{} cannot be updated from here; get it from {}", a.label, a.download.as_deref().unwrap_or("its website")),
    };
    open_shell(&m.command)?;
    Ok(m.command)
}

/// Install a tool one of the ways this computer can, in a new terminal window.
pub fn install(id: &str, method: &str) -> Result<String> {
    let a = list().into_iter().find(|a| a.id == id).ok_or_else(|| anyhow::anyhow!("unknown tool {id}"))?;
    let m = a.install.iter().find(|m| m.id == method).ok_or_else(|| anyhow::anyhow!("{} cannot be installed with {method} on this computer", a.label))?;
    open_shell(&m.command)?;
    Ok(m.command.clone())
}

/// Remove a second copy of a tool: an npm one is uninstalled, a native one deleted. The copy that runs stays.
pub fn remove_copy(id: &str, path: &str) -> Result<String> {
    let a = list().into_iter().find(|a| a.id == id).ok_or_else(|| anyhow::anyhow!("unknown tool {id}"))?;
    let c = a.copies.iter().find(|c| c.path == path).ok_or_else(|| anyhow::anyhow!("{} has no copy at {path}", a.label))?;
    if c.runs && a.copies.len() > 1 { bail!("that is the copy your terminals run; remove the other one, or change PATH first"); }
    let cmd = match c.kind {
        Kind::Npm => format!("npm uninstall -g {}", c.package.clone().unwrap_or_default()),
        Kind::Native | Kind::Unknown => if cfg!(windows) { ps(&format!("Remove-Item -Force '{}'", c.path)) } else { format!("rm -f {}", q(&c.path)) },
        _ => bail!("remove that copy with the package manager that installed it"),
    };
    open_shell(&cmd)?;
    Ok(cmd)
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
        .current_dir(std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir)).spawn()?;
    Ok(())
}
#[cfg(not(windows))]
fn open_shell(line: &str) -> Result<()> {
    if let Ok(log) = std::env::var("HRM_TERMINAL_LOG") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
        writeln!(f, "{}", serde_json::json!({"shell": line}))?;
        return Ok(());
    }
    crate::launch::shell_window(&format!("{line}; echo; read -rp 'Finished. Press Enter to close this window.'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_each_tools_own_wording() {
        assert_eq!(version_in("2.1.316 (Claude Code)").as_deref(), Some("2.1.316"));
        assert_eq!(version_in("codex-cli 0.166.0").as_deref(), Some("0.166.0"));
        assert_eq!(version_in("opencode v2.2.5").as_deref(), Some("2.2.5"));
        assert_eq!(version_in("1.4.12\n").as_deref(), Some("1.4.12"));
        assert_eq!(version_in("rust-v0.166.0").as_deref(), None, "the prefix is stripped by the caller");
        assert_eq!(version_in("no version here"), None);
        assert!(newer("2.10.0", "2.9.3") && !newer("2.1.316", "2.1.316") && !newer("1.9", "1.10"));
    }

    #[test]
    fn the_install_kind_is_read_from_where_the_file_sits() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(kind_of("claude", &p(r"C:\Users\me\.local\bin\claude.exe")), Kind::Native);
        assert_eq!(kind_of("codex", &p(r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe")), Kind::Native);
        assert_eq!(kind_of("agy", &p(r"C:\Users\me\AppData\Local\agy\bin\agy.exe")), Kind::Native);
        assert_eq!(kind_of("opencode", &p("/home/me/.opencode/bin/opencode")), Kind::Native);
        assert_eq!(kind_of("claude", &p(r"C:\Users\me\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code\cli.js")), Kind::Npm);
        assert_eq!(kind_of("opencode", &p(r"C:\Users\me\scoop\shims\opencode.exe")), Kind::Scoop);
        assert_eq!(kind_of("opencode", &p(r"C:\ProgramData\chocolatey\bin\opencode.exe")), Kind::Choco);
        assert_eq!(kind_of("claude", &p("/opt/homebrew/Caskroom/claude-code/2.1.318/claude")), Kind::Brew);
        assert_eq!(kind_of("codex", &p(r"D:\tools\codex.exe")), Kind::Unknown);
    }

    #[test]
    fn each_kind_updates_with_what_installed_it() {
        let c = |kind: Kind, path: &str, package: Option<&str>| Copy { path: path.into(), version: None, kind, package: package.map(String::from), runs: true };
        let cmd = |id: &str, ins: &Installer, c: Copy| update_method(id, ins, &c).map(|m| m.command).unwrap_or_default();
        assert_eq!(cmd("claude", &CLAUDE, c(Kind::Npm, "x", Some("@anthropic-ai/claude-code"))), "npm install -g @anthropic-ai/claude-code@latest");
        assert_eq!(cmd("claude", &CLAUDE, c(Kind::Native, r"C:\u\claude.exe", None)), "\"C:\\u\\claude.exe\" update");
        assert_eq!(cmd("opencode", &OPENCODE, c(Kind::Scoop, "x", None)), "scoop update opencode");
        assert_eq!(cmd("pi", &PI, c(Kind::Npm, "x", Some("@mariozechner/pi-coding-agent"))), "npm install -g @mariozechner/pi-coding-agent@latest", "an older Pi package name updates in place");
        assert_eq!(install_methods(&OMP).iter().map(|m| m.id.as_str()).filter(|i| *i != "npm").count(), 0, "Oh My Pi installs with npm only");
        // Codex has no update command: the installer runs again; Antigravity's refuses an existing file, so it is removed first
        let codex = cmd("codex", &CODEX, c(Kind::Native, "x", None));
        assert!(codex.contains("chatgpt.com/codex/install"), "{codex}");
        let agy = cmd("agy", &AGY, c(Kind::Native, r"C:\u\agy.exe", None));
        assert!(agy.contains("Remove-Item") || agy.contains("rm -f"), "{agy}");
        assert!(agy.contains("antigravity.google/cli/install"), "{agy}");
    }
}
