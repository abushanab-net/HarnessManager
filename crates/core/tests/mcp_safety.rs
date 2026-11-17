//! MCP servers between the shared list, the credential store and the tools' own config files: no env or header
//! value is kept in the list file or in a backup, and a tool's entry keeps everything Harness Manager does not own.
// the credential store these tests read is Windows Credential Manager, the only one there is so far
#![cfg(windows)]

use std::sync::{Mutex, MutexGuard};

use hrm_core::registry::{Kind, McpServer, Profile};
use hrm_core::{items, secrets, Ctx};
use serde_json::{json, Value};

/// One test at a time, each with the credential names of this test run, removed when it ends (also when it fails).
struct Store { prefix: String, _one: MutexGuard<'static, ()> }
static ONE: Mutex<()> = Mutex::new(());

fn store() -> Store {
    let one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let prefix = format!("HRMTEST-mcp-{}", std::process::id());
    std::env::set_var("HRM_CRED_PREFIX", &prefix);
    Store { prefix, _one: one }
}

impl Drop for Store { fn drop(&mut self) { let _ = secrets::delete_prefix(&self.prefix); } }

/// A data folder and a home with a main profile of each of `tools`.
fn ctx(tools: &[&str]) -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
    let mut reg = ctx.registry().unwrap();
    for t in tools {
        reg.add_profile(Profile { name: format!("{t}-main"), label: None, harness: t.to_string(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }).unwrap();
    }
    ctx.save_registry(&reg).unwrap();
    (dir, ctx)
}

fn server(name: &str, env: &[(&str, &str)], headers: &[(&str, &str)]) -> McpServer {
    let map = |x: &[(&str, &str)]| x.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let stdio = headers.is_empty();
    McpServer { name: name.into(), transport: if stdio { "stdio" } else { "http" }.into(), command: stdio.then(|| "npx".to_string()), args: if stdio { vec!["-y".into(), "server-github".into()] } else { vec![] },
        env: map(env), url: (!stdio).then(|| "https://d.example/mcp".to_string()), headers: map(headers), ..Default::default() }
}

/// `profiles.json` inside a backup zip.
fn list_in_zip(ctx: &Ctx, name: &str) -> String {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::fs::File::open(hrm_core::backup::dir(ctx).join(name)).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name("profiles.json").unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn env_and_header_values_are_in_the_credential_store_not_in_the_list_file_or_a_backup() {
    let s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("GITHUB_TOKEN", "ghp_secret_1"), ("DEBUG", "")], &[])).unwrap();
    items::add_mcp(&mut reg, server("docs", &[], &[("Authorization", "Bearer secret_2")])).unwrap();
    ctx.save_registry(&reg).unwrap();
    let file = std::fs::read_to_string(ctx.registry_path()).unwrap();
    assert!(!file.contains("ghp_secret_1") && !file.contains("secret_2"), "a value is in the list file: {file}");
    assert!(file.contains("GITHUB_TOKEN") && file.contains("Authorization"), "the keys stay in the file: {file}");
    let zip = list_in_zip(&ctx, &hrm_core::backup::create(&ctx, "Manual").unwrap().name);
    assert!(!zip.contains("ghp_secret_1") && !zip.contains("secret_2"), "a value is in the backup: {zip}");
    // in memory nothing changed: every caller reads the values as before
    let reg = ctx.registry().unwrap();
    assert_eq!(reg.mcp[0].env["GITHUB_TOKEN"], "ghp_secret_1");
    assert_eq!(reg.mcp[0].env["DEBUG"], "");
    assert_eq!(reg.mcp[1].headers["Authorization"], "Bearer secret_2");
    assert_eq!(secrets::get_key(&format!("{}:mcp:github:env:GITHUB_TOKEN", s.prefix)).as_deref(), Some("ghp_secret_1"));
    assert_eq!(secrets::get_key(&format!("{}:mcp:docs:header:Authorization", s.prefix)).as_deref(), Some("Bearer secret_2"));
}

#[test]
fn removing_a_server_or_a_key_removes_its_credentials_and_renaming_moves_them() {
    let s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let cred = |server: &str, kind: &str, key: &str| secrets::get_key(&format!("{}:mcp:{server}:{kind}:{key}", s.prefix));
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("GITHUB_TOKEN", "ghp_1"), ("ORG", "acme")], &[])).unwrap();
    items::add_mcp(&mut reg, server("docs", &[], &[("Authorization", "Bearer 2")])).unwrap();
    ctx.save_registry(&reg).unwrap();
    assert_eq!((cred("github", "env", "ORG").as_deref(), cred("docs", "header", "Authorization").as_deref()), (Some("acme"), Some("Bearer 2")));
    // a key taken off one server; the other renamed, as an edit does it: removed, and added under the new name
    let mut reg = ctx.registry().unwrap();
    reg.mcp[0].env.remove("ORG");
    let mut docs = reg.mcp[1].clone();
    docs.name = "handbook".into();
    items::remove_item(&mut reg, "mcp", "docs").unwrap();
    items::add_mcp(&mut reg, docs).unwrap();
    ctx.save_registry(&reg).unwrap();
    assert_eq!((cred("github", "env", "ORG"), cred("docs", "header", "Authorization")), (None, None));
    assert_eq!(cred("handbook", "header", "Authorization").as_deref(), Some("Bearer 2"));
    let mut reg = ctx.registry().unwrap();
    assert_eq!(reg.mcp.iter().find(|m| m.name == "handbook").unwrap().headers["Authorization"], "Bearer 2");
    assert_eq!(reg.mcp.iter().find(|m| m.name == "github").unwrap().env["GITHUB_TOKEN"], "ghp_1");
    items::remove_item(&mut reg, "mcp", "github").unwrap();
    ctx.save_registry(&reg).unwrap();
    assert_eq!(cred("github", "env", "GITHUB_TOKEN"), None);
    assert_eq!(cred("handbook", "header", "Authorization").as_deref(), Some("Bearer 2"), "the other server's stays");
    // two names that Windows takes for one credential name: neither value is lost
    let mut reg = ctx.registry().unwrap();
    for (name, v) in [("Tools", "one"), ("tools", "two")] { reg.mcp.push(server(name, &[("K", v)], &[])); }
    ctx.save_registry(&reg).unwrap();
    let reg = ctx.registry().unwrap();
    let k = |name: &str| reg.mcp.iter().find(|m| m.name == name).unwrap().env["K"].clone();
    assert_eq!((k("Tools"), k("tools")), ("one".to_string(), "two".to_string()));
    assert!(std::fs::read_to_string(ctx.registry_path()).unwrap().contains("\"K\": \"two\""), "the second, which the store cannot keep apart, stays in the file");
    ctx.save_registry(&reg).unwrap();
    let reg = ctx.registry().unwrap();
    assert_eq!((reg.mcp[1].env["K"].as_str(), reg.mcp[2].env["K"].as_str()), ("one", "two"), "and a second save changes nothing");
    // renamed in letter case only, with a new value: the credential is replaced, not hidden behind the old spelling
    let mut reg = ctx.registry().unwrap();
    reg.mcp.truncate(1);
    reg.mcp[0].name = "Handbook".into();
    reg.mcp[0].headers.insert("Authorization".into(), "Bearer 3".into());
    ctx.save_registry(&reg).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp[0].headers["Authorization"], "Bearer 3");
    assert!(!std::fs::read_to_string(ctx.registry_path()).unwrap().contains("Bearer"), "the store took it");
}

#[test]
fn a_list_written_by_an_older_version_moves_its_values_at_the_first_start() {
    let s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let path = ctx.registry_path();
    // as 1.9.14 saved it: the values in the file
    let mut old: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    old["mcp"] = json!([{"name": "github", "transport": "stdio", "command": "npx", "env": {"GITHUB_TOKEN": "ghp_old"}},
        {"name": "docs", "transport": "http", "url": "https://d.example/mcp", "headers": {"Authorization": "Bearer old"}}]);
    std::fs::write(&path, serde_json::to_string_pretty(&old).unwrap()).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp[0].env["GITHUB_TOKEN"], "ghp_old", "a value still in the file is used as it is");
    // what the app and the command line run at every start
    hrm_core::import::first_run(&ctx).unwrap();
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(!file.contains("ghp_old") && !file.contains("Bearer old") && file.contains("GITHUB_TOKEN"), "{file}");
    let reg = ctx.registry().unwrap();
    assert_eq!((reg.mcp[0].env["GITHUB_TOKEN"].as_str(), reg.mcp[1].headers["Authorization"].as_str()), ("ghp_old", "Bearer old"));
    assert_eq!(secrets::get_key(&format!("{}:mcp:github:env:GITHUB_TOKEN", s.prefix)).as_deref(), Some("ghp_old"));
    // once: with nothing left to move, a start does not write the list
    let sig = hrm_core::util::file_sig(&path);
    std::thread::sleep(std::time::Duration::from_millis(30));
    hrm_core::import::first_run(&ctx).unwrap();
    ctx.save_registry(&ctx.registry().unwrap()).unwrap();
    assert_eq!(hrm_core::util::file_sig(&path), sig);
}

#[test]
fn a_value_the_store_refuses_stays_in_the_list_file_and_out_of_backups() {
    let _s = store();
    let (_d, ctx) = ctx(&["claude"]);
    // a Windows credential holds 2,560 bytes
    let big = "k".repeat(3000);
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("BIG", &big), ("SMALL", "ghp_small")], &[])).unwrap();
    ctx.save_registry(&reg).unwrap();
    let file = std::fs::read_to_string(ctx.registry_path()).unwrap();
    assert!(file.contains(&big) && !file.contains("ghp_small"), "the refused value is kept, in the file");
    assert_eq!(ctx.registry().unwrap().mcp[0].env["BIG"], big);
    let b = hrm_core::backup::create(&ctx, "Manual").unwrap();
    let zip = list_in_zip(&ctx, &b.name);
    assert!(!zip.contains(&big) && zip.contains("\"BIG\": \"\""), "its key is in the backup, its value is not: {zip}");
    // putting that backup back: its empty values do not replace the ones this computer has
    hrm_core::backup::restore(&ctx, &b.name).unwrap();
    let reg = ctx.registry().unwrap();
    assert_eq!((reg.mcp[0].env["BIG"].as_str(), reg.mcp[0].env["SMALL"].as_str()), (big.as_str(), "ghp_small"));
}

#[test]
fn a_restore_never_empties_a_value_this_computer_has() {
    use hrm_core::backup;
    let _s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("GITHUB_TOKEN", "ghp_1")], &[])).unwrap();
    ctx.save_registry(&reg).unwrap();
    let old = backup::create(&ctx, "Manual").unwrap();
    // since that backup: a new token, and one more server
    reg.mcp[0].env.insert("GITHUB_TOKEN".into(), "ghp_2".into());
    items::add_mcp(&mut reg, server("docs", &[], &[("Authorization", "Bearer 2")])).unwrap();
    ctx.save_registry(&reg).unwrap();
    backup::restore(&ctx, &old.name).unwrap();
    let reg = ctx.registry().unwrap();
    assert_eq!(reg.mcp.len(), 1, "the list is the backup's");
    assert_eq!(reg.mcp[0].env["GITHUB_TOKEN"], "ghp_2", "the backup's empty value did not replace this computer's");
    // docs is not in that backup; its value waits in the store, so putting the newer list back finds it
    let before = backup::list(&ctx).into_iter().find(|b| b.label == "Before restore").unwrap();
    backup::restore(&ctx, &before.name).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp.iter().find(|m| m.name == "docs").unwrap().headers["Authorization"], "Bearer 2");
    // the merging restore adds accounts and history; the values stay
    hrm_core::restore::run(&ctx, &hrm_core::restore::Source::Zip(backup::dir(&ctx).join(&old.name)), false).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp[0].env["GITHUB_TOKEN"], "ghp_2");
    let file = std::fs::read_to_string(ctx.registry_path()).unwrap();
    assert!(!file.contains("ghp_") && !file.contains("Bearer 2"), "{file}");
}
