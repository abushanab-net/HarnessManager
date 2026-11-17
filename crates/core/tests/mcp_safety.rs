//! MCP servers between the shared list, the credential store and the tools' own config files: no env or header
//! value is kept in the list file or in a backup, and a tool's entry keeps everything Harness Manager does not own.
// the credential store these tests read is Windows Credential Manager, the only one there is so far
#![cfg(windows)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use hrm_core::harness;
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

/// The file a tool keeps its MCP servers in.
fn mcp_file(ctx: &Ctx, tool: &str) -> PathBuf {
    let reg = ctx.registry().unwrap();
    let p = reg.get(&format!("{tool}-main")).unwrap();
    harness::of(p).item_paths(ctx, p).mcp.unwrap().0
}

fn server(name: &str, env: &[(&str, &str)], headers: &[(&str, &str)]) -> McpServer {
    let map = |x: &[(&str, &str)]| x.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let stdio = headers.is_empty();
    McpServer { name: name.into(), transport: if stdio { "stdio" } else { "http" }.into(), command: stdio.then(|| "npx".to_string()), args: if stdio { vec!["-y".into(), "server-github".into()] } else { vec![] },
        env: map(env), url: (!stdio).then(|| "https://d.example/mcp".to_string()), headers: map(headers), ..Default::default() }
}

/// The same two servers as each tool writes them by hand: a table of its own for env, keys Harness Manager does not
/// know, and a server turned off.
fn fixture(tool: &str) -> String {
    match tool {
        "codex" => "model = \"gpt-6\"\n\n[mcp_servers.github]\ncommand = \"npx\"\nargs = [\"-y\", \"server-github\"]\nstartup_timeout_sec = 20\nenabled = false\n\n[mcp_servers.github.env]\nGITHUB_TOKEN = \"ghp_x\"\n\n[mcp_servers.docs]\nurl = \"https://d.example/mcp\"\nbearer_token_env_var = \"DOCS_TOKEN\"\nhttp_headers = { \"X-Team\" = \"a\" }\n".into(),
        "opencode" => json!({"$schema": "https://opencode.ai/config.json", "mcp": {
            "github": {"type": "local", "command": ["npx", "-y", "server-github"], "environment": {"GITHUB_TOKEN": "ghp_x"}, "enabled": false, "timeout": 5000},
            "docs": {"type": "remote", "url": "https://d.example/mcp", "headers": {"X-Team": "a"}, "enabled": true, "oauth": false}}}).to_string(),
        "agy" => json!({"mcpServers": {
            "github": {"command": "npx", "args": ["-y", "server-github"], "env": {"GITHUB_TOKEN": "ghp_x"}, "disabled": true},
            "docs": {"serverUrl": "https://d.example/mcp", "headers": {"X-Team": "a"}, "disabledTools": ["search"]}}}).to_string(),
        _ => json!({"numStartups": 5, "mcpServers": {
            "github": {"command": "npx", "args": ["-y", "server-github"], "env": {"GITHUB_TOKEN": "ghp_x"}, "timeout": 30000},
            "docs": {"type": "http", "url": "https://d.example/mcp", "headers": {"X-Team": "a"}, "oauth": {"clientId": "abc"}}}}).to_string(),
    }
}

/// A config file as a value, so two texts that say the same compare equal.
fn meaning(tool: &str, text: &str) -> Value {
    fn of(i: &toml_edit::Item) -> Value {
        if let Some(t) = i.as_table_like() { return Value::Object(t.iter().map(|(k, v)| (k.to_string(), of(v))).collect()); }
        let Some(v) = i.as_value() else { return Value::Null };
        if let Some(a) = v.as_array() { return a.iter().map(|x| of(&toml_edit::Item::Value(x.clone()))).collect(); }
        v.as_str().map(Value::from).or(v.as_integer().map(Value::from)).or(v.as_bool().map(Value::from)).unwrap_or(Value::Null)
    }
    if tool == "codex" { of(text.parse::<toml_edit::DocumentMut>().unwrap().as_item()) } else { serde_json::from_str(text).unwrap() }
}

const TOOLS: [&str; 4] = ["claude", "codex", "opencode", "agy"];

#[test]
fn import_then_sync_leaves_each_tools_config_as_it_was() {
    let _s = store();
    let mut lost = vec![];
    // each tool alone, and Codex once more as Windows editors save it
    for (tool, text) in TOOLS.iter().map(|t| (*t, fixture(t))).chain([("codex", fixture("codex").replace('\n', "\r\n"))]) {
        let (_d, ctx) = ctx(&[tool]);
        let file = mcp_file(&ctx, tool);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, &text).unwrap();
        let sig = hrm_core::util::file_sig(&file);
        std::thread::sleep(std::time::Duration::from_millis(30));
        let mut reg = ctx.registry().unwrap();
        let (added, kept) = items::import_mcp(&ctx, &mut reg, tool).unwrap();
        assert_eq!((added.len(), kept.len()), (2, 0), "{tool}");
        ctx.save_registry(&reg).unwrap();
        let rep = items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
        assert!(rep.conflicts.is_empty() && rep.skipped.is_empty(), "{tool}: {rep:?}");
        let now = std::fs::read_to_string(&file).unwrap();
        if meaning(tool, &now) != meaning(tool, &text) { lost.push(format!("{tool} changed to: {now}")); }
        else if now != text || hrm_core::util::file_sig(&file) != sig { lost.push(format!("{tool}: the same content was written again")); }
    }
    assert!(lost.is_empty(), "{}", lost.join("\n\n"));
}

#[test]
fn a_changed_server_updates_only_its_own_keys_in_each_tools_entry() {
    let _s = store();
    for tool in TOOLS {
        let (_d, ctx) = ctx(&[tool]);
        let file = mcp_file(&ctx, tool);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, fixture(tool)).unwrap();
        let mut reg = ctx.registry().unwrap();
        items::import_mcp(&ctx, &mut reg, tool).unwrap();
        // edited in the app: another package and token, another address
        let g = reg.mcp.iter_mut().find(|m| m.name == "github").unwrap();
        g.args = vec!["-y".into(), "server-github@2".into()];
        g.env.insert("GITHUB_TOKEN".into(), "ghp_y".into());
        reg.mcp.iter_mut().find(|m| m.name == "docs").unwrap().url = Some("https://d2.example/mcp".into());
        ctx.save_registry(&reg).unwrap();
        let rep = items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
        assert!(rep.conflicts.is_empty() && rep.skipped.is_empty(), "{tool}: {rep:?}");
        let now = std::fs::read_to_string(&file).unwrap();
        // the edit and nothing else: the unknown keys, the switch that turns a server off and the env table are as they were
        let mut want = meaning(tool, &fixture(tool));
        let list = match tool { "codex" => "mcp_servers", "opencode" => "mcp", _ => "mcpServers" };
        let (g, d) = (json!(["-y", "server-github@2"]), json!("https://d2.example/mcp"));
        match tool {
            "opencode" => { want[list]["github"]["command"] = json!(["npx", "-y", "server-github@2"]); want[list]["github"]["environment"]["GITHUB_TOKEN"] = json!("ghp_y"); want[list]["docs"]["url"] = d; }
            "agy" => { want[list]["github"]["args"] = g; want[list]["github"]["env"]["GITHUB_TOKEN"] = json!("ghp_y"); want[list]["docs"]["serverUrl"] = d; }
            _ => { want[list]["github"]["args"] = g; want[list]["github"]["env"]["GITHUB_TOKEN"] = json!("ghp_y"); want[list]["docs"]["url"] = d; }
        }
        // Claude Code's entry had no "type"; it is one of the keys written
        if tool == "claude" { want[list]["github"]["type"] = json!("stdio"); }
        assert_eq!(meaning(tool, &now), want, "{tool}: {now}");
        if tool == "codex" { assert!(now.contains("[mcp_servers.github.env]\nGITHUB_TOKEN = \"ghp_y\"") && now.contains("model = \"gpt-6\""), "the env table keeps its form: {now}"); }
    }
}

#[test]
fn a_value_not_set_on_this_computer_never_replaces_the_one_a_tool_has() {
    let _s = store();
    let (_d, ctx) = ctx(&TOOLS);
    // the list as another computer shares it: keys without values, and this computer's store has none
    let mut reg = ctx.registry().unwrap();
    for s in [server("github", &[("GITHUB_TOKEN", "")], &[]), server("docs", &[], &[("X-Team", "")])] {
        let name = s.name.clone();
        items::add_mcp(&mut reg, s).unwrap();
        for t in TOOLS { items::set_target(&mut reg, "mcp", &name, t, None, true).unwrap(); }
    }
    ctx.save_registry(&reg).unwrap();
    // Claude Code and Codex have the servers with this computer's values; OpenCode and Antigravity have no entry yet
    for tool in ["claude", "codex"] {
        std::fs::create_dir_all(mcp_file(&ctx, tool).parent().unwrap()).unwrap();
        std::fs::write(mcp_file(&ctx, tool), fixture(tool)).unwrap();
    }
    let rep = items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
    assert!(rep.conflicts.is_empty() && rep.skipped.is_empty(), "{rep:?}");
    for tool in ["claude", "codex"] { assert_eq!(std::fs::read_to_string(mcp_file(&ctx, tool)).unwrap(), fixture(tool), "{tool} keeps its values"); }
    let oc = meaning("opencode", &std::fs::read_to_string(mcp_file(&ctx, "opencode")).unwrap());
    assert_eq!(oc["mcp"]["github"]["command"], json!(["npx", "-y", "server-github"]));
    assert!(oc["mcp"]["github"].get("environment").is_none() && oc["mcp"]["docs"].get("headers").is_none(), "a key without a value is not written: {oc}");
    // another key of the entry changes: the value the tool has still stays
    let mut reg = ctx.registry().unwrap();
    reg.mcp[0].args = vec!["-y".into(), "server-github@2".into()];
    ctx.save_registry(&reg).unwrap();
    items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
    for tool in ["claude", "codex"] {
        let now = meaning(tool, &std::fs::read_to_string(mcp_file(&ctx, tool)).unwrap());
        let e = &now[if tool == "codex" { "mcp_servers" } else { "mcpServers" }]["github"];
        assert_eq!((&e["args"], &e["env"]["GITHUB_TOKEN"]), (&json!(["-y", "server-github@2"]), &json!("ghp_x")), "{tool}");
    }
    // reading the tools: a value only the tool has is no difference, so no server is called hand-made; the value stays the tool's
    let mut reg = ctx.registry().unwrap();
    let report = items::import_mcp_all(&ctx, &mut reg);
    assert!(report.iter().all(|r| r["kept"].as_array().unwrap().is_empty()), "{report:?}");
    ctx.save_registry(&reg).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp[0].env["GITHUB_TOKEN"], "");
    // one computer saving the shared list writes "not in this file", never an empty value over another computer's
    let file = std::fs::read_to_string(ctx.registry_path()).unwrap();
    assert!(file.contains("\"GITHUB_TOKEN\": null") && file.contains("\"X-Team\": null"), "{file}");
}

#[test]
fn a_sync_that_changes_nothing_does_not_write_the_tools_files() {
    let _s = store();
    let (_d, ctx) = ctx(&TOOLS);
    let mut reg = ctx.registry().unwrap();
    for s in [server("github", &[("GITHUB_TOKEN", "ghp_x")], &[]), server("docs", &[], &[("X-Team", "a")])] {
        let name = s.name.clone();
        items::add_mcp(&mut reg, s).unwrap();
        for t in TOOLS { items::set_target(&mut reg, "mcp", &name, t, None, true).unwrap(); }
    }
    ctx.save_registry(&reg).unwrap();
    items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
    let sigs: Vec<_> = TOOLS.iter().map(|t| hrm_core::util::file_sig(&mcp_file(&ctx, t))).collect();
    assert!(sigs.iter().all(|s| s.is_some()), "the first sync wrote every file");
    std::thread::sleep(std::time::Duration::from_millis(30));
    let rep = items::sync(&ctx, &ctx.registry().unwrap()).unwrap();
    assert!(rep.conflicts.is_empty() && rep.skipped.is_empty(), "{rep:?}");
    let again: Vec<&str> = TOOLS.iter().zip(&sigs).filter(|(t, s)| &hrm_core::util::file_sig(&mcp_file(&ctx, t)) != *s).map(|(t, _)| *t).collect();
    assert!(again.is_empty(), "written again with nothing changed: {again:?}");
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
    assert!(file.contains("\"GITHUB_TOKEN\": null") && file.contains("\"DEBUG\": null") && file.contains("\"Authorization\": null") && file.contains("\"version\": 2"), "the keys stay in the file, each marked as not held there: {file}");
    // a version before this one refuses such a list: read as empty, the values would be written over the ones its tools have
    #[derive(Debug, serde::Deserialize)]
    struct OldServer { #[serde(default)] env: std::collections::BTreeMap<String, String>, #[serde(default)] headers: std::collections::BTreeMap<String, String> }
    #[derive(Debug, serde::Deserialize)]
    struct OldList { mcp: Vec<OldServer> }
    let refused = serde_json::from_str::<OldList>(&file).map(|l| l.mcp.iter().map(|s| s.env.len() + s.headers.len()).sum::<usize>()).unwrap_err();
    assert!(refused.to_string().contains("invalid type: null, expected a string"), "{refused}");
    let zip = list_in_zip(&ctx, &hrm_core::backup::create(&ctx, "Manual").unwrap().name);
    assert!(!zip.contains("ghp_secret_1") && !zip.contains("secret_2") && zip.contains("\"GITHUB_TOKEN\": null"), "a value is in the backup: {zip}");
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
fn a_value_emptied_in_an_edit_stays_empty() {
    let s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let cred = |key: &str| secrets::get_key(&format!("{}:mcp:github:env:{key}", s.prefix));
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("GITHUB_TOKEN", "ghp_1"), ("ORG", "acme"), ("HOST", "h")], &[])).unwrap();
    items::add_mcp(&mut reg, server("Github", &[("GITHUB_TOKEN", "ghp_other")], &[])).unwrap();
    ctx.save_registry(&reg).unwrap();
    // the edit the form and `hrm mcp add` make: the same server again, one value emptied, one key taken off
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("github", &[("GITHUB_TOKEN", "ghp_1"), ("ORG", "")], &[])).unwrap();
    assert_eq!((cred("ORG"), cred("HOST"), cred("GITHUB_TOKEN").as_deref()), (None, None, Some("ghp_1")), "gone before the save, and only those");
    ctx.save_registry(&reg).unwrap();
    let reg = ctx.registry().unwrap();
    let g = reg.mcp.iter().find(|m| m.name == "github").unwrap();
    assert_eq!((g.env["ORG"].as_str(), g.env.get("HOST"), g.env["GITHUB_TOKEN"].as_str()), ("", None, "ghp_1"));
    assert_eq!(cred("ORG"), None);
    // a value that is another server's by Windows' reading of the name is not taken along
    let mut reg = ctx.registry().unwrap();
    items::add_mcp(&mut reg, server("Github", &[("GITHUB_TOKEN", "")], &[])).unwrap();
    ctx.save_registry(&reg).unwrap();
    assert_eq!(ctx.registry().unwrap().mcp.iter().find(|m| m.name == "github").unwrap().env["GITHUB_TOKEN"], "ghp_1");
}

#[test]
fn a_list_written_by_an_older_version_moves_its_values_at_the_first_start() {
    let s = store();
    let (_d, ctx) = ctx(&["claude"]);
    let path = ctx.registry_path();
    // as 1.9.14 saved it: the values in the file
    let mut old: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    old["mcp"] = json!([{"name": "github", "transport": "stdio", "command": "npx", "env": {"GITHUB_TOKEN": "ghp_old", "DEBUG": ""}},
        {"name": "docs", "transport": "http", "url": "https://d.example/mcp", "headers": {"Authorization": "Bearer old"}}]);
    std::fs::write(&path, serde_json::to_string_pretty(&old).unwrap()).unwrap();
    secrets::write(&format!("{}:mcp:github:env:DEBUG", s.prefix), "hrm", b"1").unwrap();
    let reg = ctx.registry().unwrap();
    assert_eq!((reg.mcp[0].env["GITHUB_TOKEN"].as_str(), reg.mcp[0].env["DEBUG"].as_str()), ("ghp_old", ""), "a value written in the file, an empty one too, is used as it is");
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
    assert!(!zip.contains(&big) && zip.contains("\"BIG\": null"), "its key is in the backup, its value is not: {zip}");
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
