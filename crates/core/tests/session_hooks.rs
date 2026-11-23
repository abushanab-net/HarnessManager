//! The session hooks (hooks.rs) at launch: Claude Code started by `hrm run` gets them as `--settings <file>` with a
//! file in the data folder, ahead of the user's arguments; nothing in the account's folder carries them.

use hrm_core::registry::Kind;
use hrm_core::service::{self, AddRequest};
use hrm_core::{hooks, Ctx};

fn ctx() -> (tempfile::TempDir, Ctx) {
    // never the real credential store or tools
    std::env::set_var("HRM_CRED_PREFIX", format!("HRMTEST-hooks-{}", std::process::id()));
    std::env::set_var("HRM_BIN_CLAUDE", "claude-fake.exe");
    std::env::set_var("HRM_BIN_CODEX", "codex-fake.exe");
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
    (dir, ctx)
}

fn add(ctx: &Ctx, name: &str, harness: &str, kind: Kind, key: Option<&str>) -> String {
    service::add_profile(ctx, AddRequest { name: name.into(), harness: harness.into(), kind, email: None, api_key: key.map(String::from),
        base_url: None, model: None, route: None, intro: None, main: false }).unwrap().home
}

/// Every file under `dir`, read as text.
fn texts(dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if hrm_core::shared::is_link(&p) { continue; }
        if p.is_dir() { out.extend(texts(&p)); } else { out.push((p.clone(), String::from_utf8_lossy(&std::fs::read(&p).unwrap()).into_owned())); }
    }
    out
}

#[test]
fn claude_code_gets_the_hooks_as_settings_from_the_data_folder_and_the_account_folder_is_not_touched() {
    let (_dir, ctx) = ctx();
    let home = add(&ctx, "work", "claude", Kind::Subscription, None);
    let user = vec!["--resume".to_string(), "s-1".into()];
    let args = service::launch_spec(&ctx, "work", &user).unwrap().args;
    let file = ctx.data.join("claude-hooks.json").display().to_string();
    assert_eq!(args, ["--settings", file.as_str(), "--resume", "s-1"], "ours first, the user's after");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for e in hooks::EVENTS { assert_eq!(v["hooks"][e][0]["hooks"][0]["command"], serde_json::json!(hooks::command()), "{e}"); }
    // nothing in the account's folder names the hook
    let home = std::path::Path::new(&home);
    assert!(!home.join("settings.json").exists());
    for (p, t) in texts(home) { assert!(!t.contains(" hook"), "{} mentions the hook", p.display()); }

    // the user's own --settings wins: that session goes untracked
    for own in [vec!["--settings".to_string(), "mine.json".into()], vec!["--settings=mine.json".into()]] {
        assert_eq!(service::launch_spec(&ctx, "work", &own).unwrap().args, own);
    }
    // another tool: nothing
    add(&ctx, "cx", "codex", Kind::Subscription, None);
    assert!(!service::launch_spec(&ctx, "cx", &[]).unwrap().args.iter().any(|a| a == "--settings"));
    // the setting off: the next start goes without it
    let off = service::settings_set(&ctx, "session_hooks", serde_json::json!(false)).unwrap();
    assert_eq!(service::launch_spec(&off, "work", &user).unwrap().args, user);
}

#[test]
fn remote_control_is_added_last_for_a_claude_subscription_account_only() {
    let (_dir, ctx) = ctx();
    add(&ctx, "sub", "claude", Kind::Subscription, None);
    let file = ctx.data.join("claude-hooks.json").display().to_string();
    let args = |name: &str| service::launch_spec(&ctx, name, &["fix the bug".to_string()]).unwrap().args;
    assert_eq!(args("sub"), ["--settings", file.as_str(), "fix the bug"]);
    service::set_remote_control(&ctx, "sub", true).unwrap();
    assert!(service::profile_view(&ctx, ctx.registry().unwrap().get("sub").unwrap()).remote_control);
    // after the prompt: `--remote-control [name]` would take it as the session's name
    assert_eq!(args("sub"), ["--settings", file.as_str(), "fix the bug", "--remote-control"]);
    service::set_remote_control(&ctx, "sub", false).unwrap();
    assert_eq!(args("sub"), ["--settings", file.as_str(), "fix the bug"]);
    add(&ctx, "keyed", "claude", Kind::ApiKey, Some("sk-ant-test"));
    assert!(service::set_remote_control(&ctx, "keyed", true).is_err());
    let _ = hrm_core::secrets::delete_prefix(&format!("HRMTEST-hooks-{}", std::process::id()));
}
