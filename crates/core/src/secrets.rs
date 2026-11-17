//! API keys and saved logins live in the OS credential store, never in the registry.
//! Windows: Credential Manager generic credentials. macOS: the Keychain. Linux: the Secret Service (GNOME Keyring,
//! KWallet). A computer where that store cannot be reached (a headless Linux box, a locked keychain) keeps them in
//! one private file, `secrets.json` in the data folder. `HRM_CRED_PREFIX` lets tests use their own namespace, and
//! off Windows keeps them in that file alone, never in the real Keychain or Secret Service.

use anyhow::Result;

pub fn prefix() -> String { crate::testenv::var("HRM_CRED_PREFIX").unwrap_or_else(|| "HarnessManager".into()) }

/// Target name for a profile's API key.
pub fn key_target(profile: &str) -> String { format!("{}:key:{profile}", prefix()) }

/// Target name for a router provider's key.
pub fn provider_target(provider: &str) -> String { format!("{}:provider:{provider}", prefix()) }

/// Target name for one env or header value of an MCP server (`kind`: "env" or "header").
// shortcut: two names that differ only in letter case, or a ':' placed just so, give one target; the list file then keeps the value (see Registry::save). Encode the parts if that ever matters.
pub fn mcp_target(server: &str, kind: &str, key: &str) -> String { format!("{}:mcp:{server}:{kind}:{key}", prefix()) }

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

/// The tests' credentials live in the private file alone (see `imp`), so that is where they are deleted.
#[cfg(not(windows))]
pub fn delete_prefix(prefix: &str) -> Result<usize> {
    let mut n = 0;
    file::change(&imp::path(), |m| m.retain(|t, _| !t.starts_with(prefix) || { n += 1; false }))?;
    Ok(n)
}

/// The private file that stands in for a system credential store: target to base64 of the stored bytes, mode 0600.
#[cfg(any(not(windows), test))]
mod file {
    use std::collections::BTreeMap;
    use std::path::Path;

    use anyhow::{Context, Result};
    use base64::Engine;

    const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

    fn load(path: &Path) -> Result<BTreeMap<String, String>> {
        let text = crate::util::read_or_empty(path)?;
        if text.trim().is_empty() { return Ok(BTreeMap::new()); }
        serde_json::from_str(&text).with_context(|| format!("{} is not valid; fix it, or delete it and enter its keys again", path.display()))
    }

    pub fn get(path: &Path, target: &str) -> Option<Vec<u8>> { B64.decode(load(path).ok()?.get(target)?).ok() }

    pub fn put(path: &Path, target: &str, raw: &[u8]) -> Result<()> { change(path, |m| { m.insert(target.into(), B64.encode(raw)); }) }

    pub fn remove(path: &Path, target: &str) -> Result<()> { change(path, |m| { m.remove(target); }) }

    /// Load, change and save under the lock every Harness Manager process shares. A file that cannot be read is an
    /// error, never written over; one left without a credential is removed.
    pub fn change(path: &Path, f: impl FnOnce(&mut BTreeMap<String, String>)) -> Result<()> {
        let _lock = crate::util::NamedLock::take("credentials");
        let mut m = load(path)?;
        let was = m.clone();
        f(&mut m);
        if m == was { return Ok(()); }
        if m.is_empty() { return std::fs::remove_file(path).with_context(|| format!("remove {}", path.display())); }
        crate::util::write_atomic_private(path, serde_json::to_string_pretty(&m)?.as_bytes())
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use anyhow::{anyhow, Context, Result};

    use super::file;

    const SERVICE: &str = "HarnessManager";

    /// Why this process keeps credentials in the private file (None: the system store answers), decided by one probe
    /// at the first call. A store that fails later joins it, so a locked keychain costs one wait, not one per call.
    // shortcut: a store that failed stays out until the process ends (the app has to be restarted to use it again); probe again after a while if that shows
    static DOWN: Mutex<Option<Option<String>>> = Mutex::new(None);

    /// Tests never touch the real Keychain or Secret Service: under their prefix, and in this crate's own tests, the file alone.
    fn tests_only() -> bool { cfg!(test) || crate::testenv::var("HRM_CRED_PREFIX").is_some() }

    /// `secrets.json` in the data folder; for a test without a data folder of its own, a temp file named after its prefix.
    pub(super) fn path() -> PathBuf {
        if tests_only() && std::env::var_os("HRM_HOME").is_none() {
            return std::env::temp_dir().join(format!("{}.secrets.json", super::prefix().replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")));
        }
        crate::ctx::data_dir().join("secrets.json")
    }

    fn down() -> Option<String> {
        if tests_only() { return Some("the tests keep their credentials apart".into()); }
        // held through the probe: callers that ask together wait for the one answer
        let mut d = DOWN.lock().unwrap_or_else(|e| e.into_inner());
        if d.is_none() {
            // an item that is not there: a store that works answers "no such item"
            let why = keyring::Entry::store_status().as_ref().map_err(|e| e.to_string()).err()
                .or_else(|| keyring::Entry::new(SERVICE, "HarnessManager:probe").and_then(|e| e.get_secret()).err().filter(|e| !matches!(e, keyring::Error::NoEntry)).map(|e| e.to_string()));
            // one line a run, so the log tells where this computer's credentials are
            crate::log::info(&match &why { Some(w) => gone(w), None => format!("credentials: kept in {}", os_name()) });
            *d = Some(why);
        }
        d.clone().flatten()
    }

    fn gone(why: &str) -> String { format!("credentials: kept in {}; no system credential store was reachable: {why}", file_name()) }

    fn os_name() -> &'static str { if cfg!(target_os = "macos") { "the Keychain" } else { "the system keyring (Secret Service)" } }
    fn file_name() -> String { format!("a private file ({})", path().display()) }

    /// One call to the system store. None: this process uses the file. Ok(None): the store has no such item.
    fn os<T>(target: &str, f: impl FnOnce(&keyring::Entry) -> keyring::Result<T>) -> Option<Result<Option<T>>> {
        use keyring::Error::{NoDefaultStore, NoEntry, NoStorageAccess, PlatformFailure};
        if down().is_some() { return None; }
        match keyring::Entry::new(SERVICE, target).and_then(|e| f(&e)) {
            Ok(v) => Some(Ok(Some(v))),
            Err(NoEntry) => Some(Ok(None)),
            // locked, no way to ask the owner, the service gone: the file from here on
            Err(e @ (NoStorageAccess(_) | PlatformFailure(_) | NoDefaultStore)) => {
                crate::log::warn(&gone(&e.to_string()));
                *DOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Some(e.to_string()));
                None
            }
            // the store works and refuses this one (too long, a name it does not take): the caller hears it
            Err(e) => Some(Err(anyhow!("{e}"))),
        }
    }

    /// Stored as the user, a zero byte, then the secret: the stores here keep one value per item.
    fn split(raw: &[u8]) -> (String, Vec<u8>) {
        let at = raw.iter().position(|b| *b == 0);
        (String::from_utf8_lossy(&raw[..at.unwrap_or(0)]).into_owned(), raw[at.map(|i| i + 1).unwrap_or(0)..].to_vec())
    }

    pub fn write(target: &str, user: &str, secret: &[u8]) -> Result<()> {
        let raw = [user.as_bytes(), &[0], secret].concat();
        match os(target, |e| e.set_secret(&raw)) {
            // a copy the file took earlier would be read as the newer one
            Some(Ok(_)) => file::remove(&path(), target),
            Some(Err(e)) => Err(e).with_context(|| format!("could not save credential {target}")),
            None => file::put(&path(), target, &raw),
        }
    }

    pub fn read(target: &str) -> Option<(String, Vec<u8>)> {
        // the file holds only what was written while the system store was away, so its copy is the newer one: it moves
        // to the store, and leaves the file once it reads back from there
        if let Some(raw) = file::get(&path(), target) {
            if matches!(os(target, |e| e.set_secret(&raw)), Some(Ok(_))) && matches!(os(target, |e| e.get_secret()), Some(Ok(Some(b))) if b == raw) {
                let _ = file::remove(&path(), target);
            }
            return Some(split(&raw));
        }
        os(target, |e| e.get_secret())?.ok().flatten().map(|raw| split(&raw))
    }

    pub fn delete(target: &str) -> Result<()> {
        // shortcut: deleted while the system store is away, the store's own copy stays and is read again once it is back; keep a "deleted" mark in the file if that shows
        let stored = match os(target, |e| e.delete_credential()) { Some(Err(e)) => Err(e).with_context(|| format!("could not delete credential {target}")), _ => Ok(()) };
        file::remove(&path(), target)?;
        stored
    }

    pub fn store() -> super::Store {
        match down() {
            None => super::Store { kind: if cfg!(target_os = "macos") { "keychain" } else { "secret-service" }, name: os_name().into(), reason: None },
            Some(why) => super::Store { kind: "file", name: file_name(), reason: Some(why) },
        }
    }
}

pub use imp::{delete, read, write};

/// Where this computer keeps credentials. `kind`: "windows", "keychain", "secret-service" or "file"; `name` as a
/// sentence takes it ("the Keychain"); `reason`: why it is the private file and no system store.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Store { pub kind: &'static str, pub name: String, pub reason: Option<String> }

#[cfg(windows)]
pub fn store() -> Store { Store { kind: "windows", name: "Windows Credential Manager".into(), reason: None } }
#[cfg(not(windows))]
pub use imp::store;

/// A probe credential written, read back and deleted, and the store that took it: how the owner checks a computer.
pub fn check() -> Result<Store> {
    let t = format!("{}:probe:{}", prefix(), std::process::id());
    write(&t, "hrm", b"probe")?;
    let back = read(&t);
    delete(&t)?;
    if back != Some(("hrm".into(), b"probe".to_vec())) { anyhow::bail!("the probe credential did not read back from {}", store().name); }
    if read(&t).is_some() { anyhow::bail!("the probe credential could not be deleted from {}", store().name); }
    Ok(store())
}

/// A secret an earlier version kept as a file in the data folder, where a computer had no credential store: the
/// store's value, else the file's, which moves into the store. The file goes once the store holds what it held.
pub fn read_moving(target: &str, old: &std::path::Path) -> Option<Vec<u8>> {
    let stored = read(target).map(|(_, b)| b);
    let Ok(filed) = std::fs::read(old) else { return stored };
    if stored.is_none() {
        crate::util::make_private(old);
        if let Err(e) = write(target, "hrm", &filed) { crate::log::warn(&format!("{} stays a file: {e:#}", old.display())); }
    }
    if read(target).is_some_and(|(_, b)| b == filed) { let _ = std::fs::remove_file(old); }
    stored.or(Some(filed))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn own_prefix() { std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-core-{}", std::process::id())); }

    #[test]
    fn the_private_file_keeps_each_credential_and_goes_with_its_last_one() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("data").join("secrets.json");
        file::put(&f, "t:a", b"hrm\0one").unwrap();
        file::put(&f, "t:b", &[0, 255, 1]).unwrap();
        assert_eq!(file::get(&f, "t:a").as_deref(), Some(b"hrm\0one".as_slice()));
        assert_eq!(file::get(&f, "t:b"), Some(vec![0, 255, 1]));
        assert_eq!(file::get(&f, "t:c"), None);
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600, "its owner's alone"); }
        file::remove(&f, "t:a").unwrap();
        assert!(f.exists() && file::get(&f, "t:b").is_some());
        file::remove(&f, "t:b").unwrap();
        assert!(!f.exists());
        // a file that does not read as the store is an error, never written over
        std::fs::write(&f, "not json").unwrap();
        assert!(file::put(&f, "t:a", b"x").is_err() && file::remove(&f, "t:a").is_err());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "not json");
    }

    #[test]
    fn a_secret_kept_as_a_file_moves_into_the_store_and_the_file_goes_once_it_reads_back() {
        own_prefix();
        let d = tempfile::tempdir().unwrap();
        let (old, t) = (d.path().join("ring.key"), format!("{}:moving:{}", prefix(), crate::util::uuid4()));
        assert_eq!(read_moving(&t, &old), None);
        std::fs::write(&old, b"SEED").unwrap();
        assert_eq!(read_moving(&t, &old).as_deref(), Some(b"SEED".as_slice()));
        assert!(!old.exists(), "the file is gone");
        assert_eq!(read(&t), Some(("hrm".to_string(), b"SEED".to_vec())), "and the store holds its value");
        // the store's value wins, and a file that holds another one is left alone
        std::fs::write(&old, b"OTHER").unwrap();
        assert_eq!(read_moving(&t, &old).as_deref(), Some(b"SEED".as_slice()));
        assert_eq!(std::fs::read(&old).unwrap(), b"OTHER");
        delete(&t).unwrap();
    }

    #[test]
    fn check_round_trips_a_probe_and_names_the_store() {
        own_prefix();
        let s = check().unwrap();
        // off Windows a test's credentials are in the private file alone, never in the Keychain or the Secret Service
        assert_eq!((s.kind, s.reason.is_some()), if cfg!(windows) { ("windows", false) } else { ("file", true) }, "{s:?}");
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_a_user_and_its_secret_round_trip_and_delete_prefix_takes_only_its_own() {
        own_prefix();
        let id = crate::util::uuid4();
        let (a, b) = (format!("{}:unix-{id}:a", prefix()), format!("{}:unix-{id}:b", prefix()));
        write(&a, "someone", &[1, 0, 2, 0]).unwrap();
        set_key(&b, "sk-key").unwrap();
        assert_eq!(read(&a), Some(("someone".to_string(), vec![1, 0, 2, 0])), "a secret with zero bytes in it");
        assert_eq!(get_key(&b).as_deref(), Some("sk-key"));
        write(&a, "", b"again").unwrap();
        assert_eq!(read(&a), Some((String::new(), b"again".to_vec())));
        assert_eq!(delete_prefix(&format!("{}:unix-{id}:", prefix())).unwrap(), 2);
        assert!(read(&a).is_none() && read(&b).is_none());
        delete(&a).unwrap();
    }
}
