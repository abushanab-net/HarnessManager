//! The installed CLI's entry on a terminal's PATH.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

#[cfg(windows)]
pub mod windows;
#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;

#[derive(Default, Serialize)]
pub struct Status {
    pub on_path: bool,
    pub can_add: bool,
    pub can_remove: bool,
    pub offer: bool,
}

fn sidecar(exe: &Path) -> Option<PathBuf> {
    let file = exe.with_file_name(if cfg!(windows) { "hrm.exe" } else { "hrm" });
    file.is_file().then_some(file)
}

fn same_file(first: &Path, second: &Path) -> bool {
    first.canonicalize().ok().is_some_and(|p| second.canonicalize().is_ok_and(|q| p == q))
}

#[cfg(not(windows))]
fn on_terminal_path(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let mount = std::env::var_os("APPDIR").map(PathBuf::from);
    crate::util::terminal_path().and_then(|path| std::env::split_paths(&path)
        .filter(|dir| mount.as_ref().is_none_or(|mount| !dir.starts_with(mount)))
        .map(|dir| dir.join("hrm"))
        .find(|candidate| candidate.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
    ).is_some_and(|found| same_file(&found, file))
}

pub fn status(ctx: &crate::Ctx) -> Result<Status> {
    let _ = ctx;
    let exe = std::env::current_exe()?;
    let Some(file) = sidecar(&exe) else { return Ok(Status::default()) };
    #[cfg(windows)]
    {
        let dir = file.parent().context("find install folder")?;
        let (path, can_add, can_remove) = windows::status(dir)?;
        let found = path.and_then(|path| crate::util::which_on("hrm", &path));
        let on_path = found.as_ref().is_some_and(|p| same_file(p, &file));
        Ok(Status { on_path, can_add: can_add && !on_path, can_remove, offer: false })
    }
    #[cfg(target_os = "linux")]
    {
        if linux::image_for(&exe).is_some() && std::env::var_os("HRM_HOME").is_none() {
            return linux::status(ctx);
        }
        Ok(Status { on_path: on_terminal_path(&file), ..Status::default() })
    }
    #[cfg(target_os = "macos")]
    {
        macos::status(ctx, &file)
    }
}

pub fn set(ctx: &crate::Ctx, add: bool) -> Result<()> {
    let _ = ctx;
    let exe = std::env::current_exe()?;
    let file = sidecar(&exe).context("the bundled hrm is missing")?;
    #[cfg(windows)]
    return windows::set(file.parent().context("find install folder")?, add);
    #[cfg(not(windows))]
    {
        if std::env::var_os("HRM_HOME").is_some() { bail!("PATH changes are disabled with HRM_HOME set"); }
        let lock = crate::util::NamedLock::take("cli-path");
        anyhow::ensure!(lock.held(), "another Harness Manager operation is changing PATH");
        #[cfg(target_os = "linux")]
        return linux::set(ctx, &file, &linux::image_for(&exe).context("the package manager owns this copy of hrm")?, add);
        #[cfg(target_os = "macos")]
        return macos::set(ctx, &file, add);
    }
}

pub fn dismiss_offer(ctx: &crate::Ctx) -> Result<()> {
    if cfg!(target_os = "macos") { return ctx.db.kv_set("cli-path:offered", "1"); }
    bail!("there is no PATH offer on this platform")
}

#[cfg(target_os = "linux")]
pub fn appimage_start(ctx: &crate::Ctx) -> Result<()> {
    if std::env::var_os("HRM_HOME").is_some() { return Ok(()); }
    let exe = std::env::current_exe()?;
    let Some(image) = linux::image_for(&exe) else { return Ok(()) };
    let lock = crate::util::NamedLock::take("cli-path");
    anyhow::ensure!(lock.held(), "another Harness Manager operation is changing PATH");
    if ctx.db.kv_get("cli-path:appimage-disabled")?.is_none_or(|s| s != "1") {
        linux::install(ctx, &sidecar(&exe).context("the bundled hrm is missing")?, &image)?;
    }
    Ok(())
}

/// A PATH launch may reach hrm through a link on macOS or a standalone copy on Linux.
pub fn desktop_exe(ctx: &crate::Ctx) -> Result<PathBuf> {
    let exe = crate::launch::hrm_exe();
    #[cfg(target_os = "linux")]
    if same_file(&exe, &linux::destination(&ctx.home)) {
        return Ok(PathBuf::from(ctx.db.kv_get("cli-path:appimage")?.context("start the Harness Manager AppImage first")?));
    }
    let _ = ctx;
    let exe = exe.canonicalize().unwrap_or(exe);
    Ok(exe.with_file_name(if cfg!(windows) { "HarnessManager.exe" } else { "HarnessManager" }))
}
