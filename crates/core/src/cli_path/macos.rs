fn stable_bundle(path: &str, temp: &str) -> bool {
    let Some((bundle, bin)) = path.rsplit_once("/Contents/MacOS/") else { return false };
    path.starts_with('/') && bundle.ends_with(".app") && bin == "hrm"
        && !path.chars().any(char::is_control)
        && !["/Volumes/", "/private/tmp/", "/tmp/", temp].iter().any(|p| !p.is_empty() && path.starts_with(p))
        && !path.split('/').any(|part| part == "AppTranslocation")
}

#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use anyhow::{ensure, Context, Result};

#[cfg(target_os = "macos")]
const LINK: &str = "/usr/local/bin/hrm";
#[cfg(target_os = "macos")]
const OWNED: &str = "cli-path:mac-link";

#[cfg(target_os = "macos")]
fn source(file: &Path) -> Option<PathBuf> {
    let file = file.canonicalize().ok()?;
    let temp = std::env::temp_dir().canonicalize().ok();
    let temp = temp.as_ref().and_then(|p| p.to_str()).map(|s| format!("{}/", s.trim_end_matches('/'))).unwrap_or_default();
    stable_bundle(file.to_str()?, &temp).then_some(file)
}

#[cfg(target_os = "macos")]
pub(super) fn status(ctx: &crate::Ctx, file: &Path) -> Result<super::Status> {
    let on_path = super::on_terminal_path(file);
    let enabled = std::env::var_os("HRM_HOME").is_none();
    let link = Path::new(LINK);
    let owned = ctx.db.kv_get(OWNED)?.filter(|s| !s.is_empty());
    let can_add = enabled && !on_path && source(file).is_some()
        && matches!(link.symlink_metadata(), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
    let can_remove = enabled && owned.is_some_and(|p| link.read_link().is_ok_and(|target| target == Path::new(&p)));
    Ok(super::Status { on_path, can_add, can_remove, offer: can_add && ctx.db.kv_get("cli-path:offered")?.is_none() })
}

#[cfg(target_os = "macos")]
pub(super) fn set(ctx: &crate::Ctx, file: &Path, add: bool) -> Result<()> {
    let link = Path::new(LINK);
    let target = if add {
        let target = source(file).context("move Harness Manager.app off the disk image and into Applications first")?;
        if link.read_link().is_ok_and(|p| p == target) { return Ok(()); }
        ensure!(matches!(link.symlink_metadata(), Err(e) if e.kind() == std::io::ErrorKind::NotFound), "{LINK} is already in use");
        target.to_str().context("invalid app path")?.to_owned()
    } else {
        let target = ctx.db.kv_get(OWNED)?.context("Harness Manager did not create this link")?;
        ensure!(link.read_link().is_ok_and(|p| p == Path::new(&target)), "{LINK} no longer points to the saved app");
        target
    };
    if add { ctx.db.kv_set(OWNED, &target)?; }
    if let Err(e) = privileged_link(add, &target) {
        if add && !link.read_link().is_ok_and(|p| p == Path::new(&target)) { let _ = ctx.db.kv_set(OWNED, ""); }
        return Err(e);
    }
    if !add { ctx.db.kv_set(OWNED, "")?; }
    Ok(())
}

#[cfg(target_os = "macos")]
fn privileged_link(add: bool, target: &str) -> Result<()> {
    // AppleScript quotes the two arguments for the shell. The privileged command only creates or removes the link.
    const SCRIPT: &str = r#"
on run argv
    set targetPath to quoted form of item 2 of argv
    set linkPath to quoted form of item 3 of argv
    if item 1 of argv is "add" then
        do shell script "/bin/mkdir -p /usr/local/bin && [ ! -e " & linkPath & " ] && [ ! -L " & linkPath & " ] && /bin/ln -sh " & targetPath & " " & linkPath with administrator privileges
    else
        do shell script "[ -L " & linkPath & " ] && [ \"$(/usr/bin/readlink " & linkPath & ")\" = " & targetPath & " ] && /bin/rm " & linkPath with administrator privileges
    end if
end run
"#;
    let result = std::process::Command::new("/usr/bin/osascript").args(["-e", SCRIPT, if add { "add" } else { "remove" }, target, LINK]).output()?;
    ensure!(result.status.success(), "could not change {LINK}: {}", String::from_utf8_lossy(&result.stderr).trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_only_to_an_installed_app() {
        assert!(stable_bundle("/Applications/Harness Manager's.app/Contents/MacOS/hrm", ""));
        assert!(stable_bundle("/Users/me/Applications/Harness Manager.app/Contents/MacOS/hrm", ""));
        for path in [
            "/Volumes/Harness Manager/Harness Manager.app/Contents/MacOS/hrm",
            "/private/tmp/Harness Manager.app/Contents/MacOS/hrm",
            "/private/var/folders/AppTranslocation/9/d/Harness Manager.app/Contents/MacOS/hrm",
            "/home/temp/Harness Manager.app/Contents/MacOS/hrm",
            "/target/release/hrm",
        ] {
            assert!(!stable_bundle(path, "/home/temp/"), "{path}");
        }
    }
}
