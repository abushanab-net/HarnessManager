use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::ptr::{null, null_mut};

use anyhow::{bail, ensure, Context, Result};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};
use windows_sys::Win32::Security::{TOKEN_DUPLICATE, TOKEN_QUERY};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

const SEP: u16 = b';' as u16;
const SLASH: u16 = b'\\' as u16;
const QUOTE: u16 = b'"' as u16;

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain([0]).collect()
}

fn same_entry(a: &[u16], b: &[u16]) -> bool {
    let mut a = a.strip_prefix(&[QUOTE]).and_then(|a| a.strip_suffix(&[QUOTE])).unwrap_or(a);
    let mut b = b;
    while let Some(rest) = a.strip_suffix(&[SLASH]) { a = rest; }
    while let Some(rest) = b.strip_suffix(&[SLASH]) { b = rest; }
    a.len() == b.len() && (a == b || unsafe {
        CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1) == CSTR_EQUAL
    })
}

fn entries(path: &[u16]) -> impl Iterator<Item = &[u16]> {
    let mut quoted = false;
    path.split(move |&c| {
        if c == QUOTE { quoted = !quoted; }
        c == SEP && !quoted
    })
}

fn add_entry(path: &[u16], dir: &[u16]) -> Option<Vec<u16>> {
    if entries(path).any(|entry| same_entry(entry, dir)) { return None; }
    let mut updated = path.to_vec();
    if !path.is_empty() { updated.push(SEP); }
    updated.extend_from_slice(dir);
    Some(updated)
}

fn remove_entry(path: &[u16], entry: &[u16]) -> Option<Vec<u16>> {
    let mut entries: Vec<_> = entries(path).collect();
    let index = entries.iter().rposition(|&part| part == entry)?;
    entries.remove(index);
    Some(entries.join(&SEP))
}

fn check(code: u32) -> Result<()> {
    if code != ERROR_SUCCESS { return Err(std::io::Error::from_raw_os_error(code as i32).into()); }
    Ok(())
}

fn read(key: &[u16], name: &[u16]) -> Result<Option<(u32, Vec<u16>)>> {
    let mut size = 0;
    let mut kind = 0;
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let mut buf = Vec::<u16>::new();
    loop {
        let code = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), flags, &mut kind,
                if buf.is_empty() { null_mut() } else { buf.as_mut_ptr().cast() }, &mut size)
        };
        if code == ERROR_FILE_NOT_FOUND { return Ok(None); }
        if code == ERROR_MORE_DATA || (code == ERROR_SUCCESS && buf.is_empty()) {
            buf.resize((size as usize / 2) + 1, 0);
            size = (buf.len() * 2) as u32;
            continue;
        }
        check(code)?;
        ensure!(size >= 2 && size % 2 == 0, "invalid registry string");
        buf.truncate(size as usize / 2);
        ensure!(buf.pop() == Some(0) && !buf.contains(&0), "invalid registry string");
        return Ok(Some((kind, buf)));
    }
}

fn write(key: &[u16], name: &[u16], kind: u32, data: &[u16]) -> Result<()> {
    let data: Vec<_> = data.iter().copied().chain([0]).collect();
    check(unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), kind, data.as_ptr().cast(), (data.len() * 2) as u32) })
}

fn delete(key: &[u16], name: &[u16]) -> Result<()> {
    let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    if code == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(code) }
}

fn create(key: &[u16]) -> Result<()> {
    let mut opened = null_mut();
    check(unsafe { RegCreateKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, null(), 0, KEY_SET_VALUE, null(), &mut opened, null_mut()) })?;
    unsafe { RegCloseKey(opened) };
    Ok(())
}

struct Target {
    key: Vec<u16>,
    owned_key: Vec<u16>,
    test: bool,
}

impl Target {
    fn current() -> Result<Option<Self>> {
        if let Some(key) = crate::testenv::var("HRM_PATH_TEST_KEY") {
            ensure!(
                key.strip_prefix(r"Software\HarnessManager-PathTest-")
                    .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')),
                "invalid PATH test registry key"
            );
            return Ok(Some(Self { key: wide(&key), owned_key: wide(format!(r"{key}\Owned")), test: true }));
        }
        if std::env::var_os("HRM_HOME").is_some() { return Ok(None); }
        Ok(Some(Self { key: wide("Environment"), owned_key: wide(r"Software\HarnessManager\CliPath"), test: false }))
    }

    fn edit(&self, dir: &Path, add: bool) -> Result<bool> {
        let name = wide("Path");
        let entry_name = wide(dir);
        let entry = &entry_name[..entry_name.len() - 1];
        ensure!(!entry.is_empty() && !entry.contains(&SEP), "the install folder cannot be a PATH entry");

        let marker = if add { None } else {
            let Some((_, marker)) = read(&self.owned_key, &entry_name)? else { return Ok(false) };
            Some(marker)
        };
        let current = read(&self.key, &name)?;
        let (kind, path) = current.as_ref().map_or((REG_EXPAND_SZ, &[][..]), |(kind, path)| (*kind, path.as_slice()));
        if add {
            let Some(updated) = add_entry(path, entry) else { return Ok(false) };
            ensure!(path.iter().filter(|&&c| c == QUOTE).count() % 2 == 0, "the user PATH contains an unmatched quote");
            ensure!(updated.len() < 32_767, "the user PATH would exceed the Windows length limit");
            create(&self.owned_key)?;
            // Record the exact spelling before publishing it, so an interrupted install can be retried.
            let flag = if current.is_none() { b'1' } else { b'0' };
            let marker: Vec<_> = std::iter::once(flag as u16).chain(entry.iter().copied()).collect();
            write(&self.owned_key, &entry_name, REG_SZ, &marker)?;
            if let Err(error) = write(&self.key, &name, kind, &updated) {
                let _ = delete(&self.owned_key, &entry_name);
                return Err(error);
            }
            return Ok(true);
        }
        let marker = marker.context("missing PATH ownership record")?;
        let Some((&flag, owned_entry)) = marker.split_first() else { bail!("empty PATH ownership record") };
        let updated = remove_entry(path, owned_entry);
        if let Some(updated) = &updated {
            if updated.is_empty() && flag == b'1' as u16 { delete(&self.key, &name)?; }
            else { write(&self.key, &name, kind, updated)?; }
        }
        delete(&self.owned_key, &entry_name)?;
        Ok(updated.is_some())
    }
}

pub fn set(dir: &Path, add: bool) -> Result<()> {
    let Some(target) = Target::current()? else { return Ok(()) };
    let lock = crate::util::NamedLock::take("cli-path");
    ensure!(lock.held(), "another Harness Manager operation is changing PATH");
    if target.edit(dir, add).context("edit the current user's PATH")? && !target.test {
        unsafe {
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, wide("Environment").as_ptr() as isize,
                SMTO_ABORTIFHUNG, 1000, null_mut());
        }
    }
    Ok(())
}

pub(super) fn status(dir: &Path) -> Result<(Option<OsString>, bool, bool)> {
    let Some(target) = Target::current()? else { return Ok((None, false, false)) };
    let name = wide(dir);
    let entry = &name[..name.len() - 1];
    let raw = read(&target.key, &wide("Path"))?.map_or_else(Vec::new, |(_, path)| path);
    let can_add = !entries(&raw).any(|part| same_entry(part, entry));
    let can_remove = read(&target.owned_key, &name)?.is_some_and(|(_, marker)| {
        marker.split_first().is_some_and(|(_, entry)| entries(&raw).any(|part| part == entry))
    });
    let path = if target.test {
        OsString::from_wide(&raw)
    } else {
        fresh_path()?
    };
    Ok((Some(path), can_add, can_remove))
}

// Explorer refreshes its environment after WM_SETTINGCHANGE. This process may still have the old PATH.
fn fresh_path() -> Result<OsString> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error()).context("open user token");
    }
    let mut env = null_mut();
    let created = unsafe { CreateEnvironmentBlock(&mut env, token, 0) };
    let error = std::io::Error::last_os_error();
    unsafe { CloseHandle(token) };
    if created == 0 { return Err(error).context("read Windows environment"); }
    let mut path = OsString::new();
    let prefix = [b'P' as u16, b'A' as u16, b'T' as u16, b'H' as u16, b'=' as u16];
    unsafe {
        let mut start = env.cast::<u16>();
        while *start != 0 {
            let mut len = 0;
            while *start.add(len) != 0 { len += 1; }
            let line = std::slice::from_raw_parts(start, len);
            if line.len() >= prefix.len() && same_entry(&line[..prefix.len()], &prefix) {
                path = OsString::from_wide(&line[prefix.len()..]);
                break;
            }
            start = start.add(len + 1);
        }
        DestroyEnvironmentBlock(env);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_keep_every_other_code_unit() {
        let text = |s: &str| s.encode_utf16().collect::<Vec<_>>();
        let dir = text(r"C:\Tools\Härness Manager");
        let variant = text(r#"C:\other;"c:\TOOLS\hÄrness manager\";C:\last"#);
        assert!(add_entry(&variant, &dir).is_none());
        assert!(remove_entry(&variant, &dir).is_none());
        let quoted = text(r#""C:\entry;C:\Tools\Härness Manager;C:\last""#);
        assert!(remove_entry(&quoted, &dir).is_none());
        assert!(add_entry(&quoted, &dir).is_some());

        let mut original = text(&format!(r"%USERPROFILE%\{};C:\Tools\Härness Manager Old;; ", "x".repeat(2300)));
        original.extend([0xd800, SEP]);
        let added = add_entry(&original, &dir).unwrap();
        assert_eq!(&added[..original.len()], original);
        assert!(add_entry(&added, &dir).is_none());
        assert_eq!(remove_entry(&added, &dir).unwrap(), original);
        assert_eq!(remove_entry(&add_entry(&[], &dir).unwrap(), &dir), Some(vec![]));
    }
}
