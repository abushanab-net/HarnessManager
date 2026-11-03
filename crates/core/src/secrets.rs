//! API keys and saved logins live in the OS credential store, never in the registry.
//! Windows: Credential Manager generic credentials. `HRM_CRED_PREFIX` lets tests use their own namespace.

use anyhow::Result;

pub fn prefix() -> String { std::env::var("HRM_CRED_PREFIX").unwrap_or_else(|_| "HarnessManager".into()) }

/// Target name for a profile's API key.
pub fn key_target(profile: &str) -> String { format!("{}:key:{profile}", prefix()) }

/// Target name for a router provider's key.
pub fn provider_target(provider: &str) -> String { format!("{}:provider:{provider}", prefix()) }

pub fn set_key(target: &str, secret: &str) -> Result<()> {
    if secret.trim().is_empty() { anyhow::bail!("the key is empty"); }
    write(target, "hrm", secret.as_bytes())
}

pub fn get_key(target: &str) -> Option<String> { read(target).map(|(_, b)| String::from_utf8_lossy(&b).into_owned()) }

#[cfg(windows)]
mod imp {
    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND, FILETIME};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

    pub fn write(target: &str, user: &str, secret: &[u8]) -> Result<()> {
        let t = wide(target);
        let u = wide(user);
        let cred = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: t.as_ptr() as *mut u16,
            Comment: std::ptr::null_mut(),
            LastWritten: FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 },
            CredentialBlobSize: secret.len() as u32,
            CredentialBlob: secret.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: u.as_ptr() as *mut u16,
        };
        // Concurrent writers can lose each other's updates in Credential Manager: serialize our own
        // processes with a system-wide mutex, then read back and write again if the value did not stick.
        let _lock = crate::util::NamedLock::take("credentials");
        for _ in 0..5 {
            if unsafe { CredWriteW(&cred, 0) } == 0 {
                bail!("could not save credential {target} (error {})", unsafe { GetLastError() });
            }
            if read(target).map(|(_, b)| b == secret).unwrap_or(false) {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        bail!("could not save credential {target}: it did not read back")
    }

    pub fn read(target: &str) -> Option<(String, Vec<u8>)> {
        let t = wide(target);
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        if unsafe { CredReadW(t.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) } == 0 {
            return None;
        }
        let out = unsafe {
            let c = &*p;
            let blob = if c.CredentialBlob.is_null() || c.CredentialBlobSize == 0 { vec![] } else {
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
            };
            let mut user = String::new();
            if !c.UserName.is_null() {
                let mut n = 0;
                while *c.UserName.add(n) != 0 { n += 1; }
                user = String::from_utf16_lossy(std::slice::from_raw_parts(c.UserName, n));
            }
            CredFree(p as *const _);
            (user, blob)
        };
        Some(out)
    }

    pub fn delete(target: &str) -> Result<()> {
        let _lock = crate::util::NamedLock::take("credentials");
        let t = wide(target);
        if unsafe { CredDeleteW(t.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let e = unsafe { GetLastError() };
            if e != ERROR_NOT_FOUND {
                bail!("could not delete credential {target} (error {e})");
            }
        }
        Ok(())
    }
}

/// Delete every credential whose target starts with `prefix` (test cleanup).
#[cfg(windows)]
pub fn delete_prefix(prefix: &str) -> Result<usize> {
    use windows_sys::Win32::Security::Credentials::{CredEnumerateW, CredFree, CREDENTIALW};
    let filter: Vec<u16> = format!("{prefix}*").encode_utf16().chain(std::iter::once(0)).collect();
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    if unsafe { CredEnumerateW(filter.as_ptr(), 0, &mut count, &mut list) } == 0 {
        return Ok(0);
    }
    let mut targets = vec![];
    unsafe {
        for i in 0..count as usize {
            let c = &**list.add(i);
            let mut n = 0;
            while *c.TargetName.add(n) != 0 { n += 1; }
            targets.push(String::from_utf16_lossy(std::slice::from_raw_parts(c.TargetName, n)));
        }
        CredFree(list as *const _);
    }
    for t in &targets { delete(t)?; }
    Ok(targets.len())
}

#[cfg(not(windows))]
pub fn delete_prefix(_prefix: &str) -> Result<usize> { Ok(0) }

#[cfg(not(windows))]
mod imp {
    // shortcut: credential store only on Windows for now; macOS Keychain / Secret Service come with those builds.
    use anyhow::{bail, Result};
    pub fn write(_t: &str, _u: &str, _s: &[u8]) -> Result<()> { bail!("credential store not supported on this platform yet") }
    pub fn read(_t: &str) -> Option<(String, Vec<u8>)> { None }
    pub fn delete(_t: &str) -> Result<()> { Ok(()) }
}

pub use imp::{delete, read, write};

/// Decrypt a PowerShell `ConvertFrom-SecureString` string (DPAPI, current user), for importing
/// keys saved by the old `claude-accounts.ps1` launcher.
#[cfg(windows)]
pub fn dpapi_unprotect_hex(hex_str: &str) -> Result<String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let bytes = hex::decode(hex_str.trim())?;
    let input = CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = unsafe { CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut out) };
    if ok == 0 {
        anyhow::bail!("this key was saved by another Windows user or computer and cannot be read here");
    }
    let raw = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
    unsafe { LocalFree(out.pbData as _) };
    // SecureString plaintext is UTF-16LE
    let u16s: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    Ok(String::from_utf16_lossy(&u16s))
}

#[cfg(not(windows))]
pub fn dpapi_unprotect_hex(_hex: &str) -> Result<String> { anyhow::bail!("DPAPI is Windows only") }
