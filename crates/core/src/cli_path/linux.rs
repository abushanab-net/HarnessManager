use std::path::{Path, PathBuf};

use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};

pub(super) fn destination(home: &Path) -> PathBuf { home.join(".local/bin/hrm") }

#[cfg(target_os = "linux")]
pub(super) fn image_for(exe: &Path) -> Option<PathBuf> {
    let mount = PathBuf::from(std::env::var_os("APPDIR")?);
    let image = PathBuf::from(std::env::var_os("APPIMAGE")?);
    (mount.is_absolute() && exe.starts_with(mount) && image.is_absolute()).then_some(image)
}

fn digest(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }

fn installed_hash(dest: &Path) -> Result<Option<String>> {
    match dest.symlink_metadata() {
        Ok(m) => {
            ensure!(m.is_file(), "{} is already in use", dest.display());
            Ok(Some(digest(&std::fs::read(dest)?)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn owned(markers: &str, hash: &str) -> bool { markers.split_whitespace().any(|s| s == hash) }

#[cfg(target_os = "linux")]
fn markers(ctx: &crate::Ctx) -> Result<String> { Ok(ctx.db.kv_get("cli-path:appimage-hash")?.unwrap_or_default()) }

#[cfg(target_os = "linux")]
pub(crate) fn installed_cli(ctx: &crate::Ctx) -> Option<PathBuf> {
    let dest = destination(&ctx.home);
    let hash = installed_hash(&dest).ok().flatten()?;
    owned(&markers(ctx).ok()?, &hash).then_some(dest)
}

#[cfg(target_os = "linux")]
pub(super) fn status(ctx: &crate::Ctx) -> Result<super::Status> {
    let dest = destination(&ctx.home);
    let owned = installed_cli(ctx).is_some();
    Ok(super::Status {
        on_path: owned && super::on_terminal_path(&dest),
        can_add: matches!(dest.symlink_metadata(), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
        can_remove: owned,
        offer: false,
    })
}

#[cfg(target_os = "linux")]
pub(super) fn install(ctx: &crate::Ctx, source: &Path, image: &Path) -> Result<()> {
    let dest = destination(&ctx.home);
    let prev = installed_hash(&dest)?;
    let bytes = std::fs::read(source)?;
    let next = digest(&bytes);
    if let Some(prev) = &prev {
        ensure!(owned(&markers(ctx)?, prev), "{} is already in use", dest.display());
    }
    if prev.as_deref() != Some(&next) {
        // Either copy belongs to us if the app exits before the rename or the final DB write.
        ctx.db.kv_set("cli-path:appimage-hash", &format!("{} {next}", prev.as_deref().unwrap_or_default()))?;
        crate::util::write_atomic_executable(&dest, &bytes)?;
        ctx.db.kv_set("cli-path:appimage-hash", &next)?;
    }
    ctx.db.kv_set("cli-path:appimage", image.to_str().ok_or_else(|| anyhow::anyhow!("AppImage path is not UTF-8"))?)
}

#[cfg(target_os = "linux")]
pub(super) fn set(ctx: &crate::Ctx, source: &Path, image: &Path, add: bool) -> Result<()> {
    if add {
        install(ctx, source, image)?;
        return ctx.db.kv_set("cli-path:appimage-disabled", "0");
    }
    let dest = destination(&ctx.home);
    let current = installed_hash(&dest)?;
    if let Some(hash) = &current {
        ensure!(owned(&markers(ctx)?, &hash), "{} is already in use", dest.display());
    }
    ctx.db.kv_set("cli-path:appimage-disabled", "1")?;
    if current.is_some() {
        std::fs::remove_file(&dest)?;
    }
    ctx.db.kv_set("cli-path:appimage-hash", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_injected_home_and_only_recognizes_recorded_binary_bytes() {
        let home = tempfile::tempdir().unwrap();
        let dest = destination(home.path());
        assert_eq!(dest, home.path().join(".local").join("bin").join("hrm"));
        assert_eq!(installed_hash(&dest).unwrap(), None);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"old").unwrap();
        let old = installed_hash(&dest).unwrap().unwrap();
        let new = digest(b"new");
        assert!(owned(&format!("{old} {new}"), &old));
        assert!(owned(&format!("{old} {new}"), &new));
        assert!(!owned(&new, &old));
        assert!(!owned(&old, &digest(b"other")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn installs_updates_and_removes_only_the_managed_executable() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let ctx = crate::Ctx::open_at(&home.path().join("data"), home.path()).unwrap();
        let src = home.path().join("bundled-hrm");
        let image = home.path().join("Harness Manager.AppImage");
        let dest = destination(home.path());
        std::fs::write(&src, b"old").unwrap();
        assert_eq!(installed_cli(&ctx), None);
        install(&ctx, &src, &image).unwrap();
        assert_eq!(installed_cli(&ctx), Some(dest.clone()));
        let [_, settings] = crate::hooks::settings_args(&ctx, &[]).unwrap().unwrap();
        let settings: serde_json::Value = serde_json::from_slice(&std::fs::read(settings).unwrap()).unwrap();
        let hook = format!("{} hook", crate::launch::sh_quote(&dest.to_string_lossy()));
        assert_eq!(settings["hooks"]["PermissionRequest"][0]["hooks"][0]["command"], hook);
        assert_eq!(std::fs::read(&dest).unwrap(), b"old");
        assert_ne!(dest.metadata().unwrap().permissions().mode() & 0o111, 0);
        std::fs::write(&src, b"new").unwrap();
        install(&ctx, &src, &image).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        set(&ctx, &src, &image, false).unwrap();
        assert!(!dest.exists());
        assert_eq!(ctx.db.kv_get("cli-path:appimage-disabled").unwrap().as_deref(), Some("1"));
        std::os::unix::fs::symlink(&src, &dest).unwrap();
        assert_eq!(installed_cli(&ctx), None);
        assert!(install(&ctx, &src, &image).is_err());
        assert!(set(&ctx, &src, &image, false).is_err());
        assert!(dest.symlink_metadata().unwrap().is_symlink());
    }
}
