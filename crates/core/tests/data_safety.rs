//! Files Harness Manager shares with the user and with the tools: an unreadable or busy file is never
//! taken for an empty one, parallel writers lose nothing, and enabling a prompt keeps every file it replaces.

use hrm_core::harness::{self, Harness};
use hrm_core::registry::{Kind, Profile, ProjectAlias, Registry};
use hrm_core::{items, Ctx};

fn ctx() -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::open_at(&dir.path().join("data"), &dir.path().join("home")).unwrap();
    (dir, ctx)
}

fn profile(name: &str, harness: &str, main: bool) -> Profile {
    Profile { name: name.into(), label: None, harness: harness.into(), kind: Kind::Subscription, email: None, main, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }
}

#[test]
fn an_unreadable_registry_is_an_error_not_an_empty_list() {
    let (dir, _ctx) = ctx();
    // a folder where the file should be: reading fails with something other than "not found"
    let path = dir.path().join("profiles.json");
    std::fs::create_dir_all(&path).unwrap();
    assert!(Registry::load(&path).is_err());
    assert!(Registry::load(&dir.path().join("missing.json")).unwrap().profiles.is_empty());
}

#[test]
fn parallel_registry_writers_lose_nothing() {
    let (_dir, ctx) = ctx();
    std::thread::scope(|s| {
        for i in 0..8 {
            let ctx = &ctx;
            s.spawn(move || {
                let _lock = ctx.lock_registry();
                let mut reg = ctx.registry().unwrap();
                reg.projects.push(ProjectAlias { key: format!("k{i}"), name: Some(format!("n{i}")), merge_into: None });
                ctx.save_registry(&reg).unwrap();
            });
        }
    });
    assert_eq!(ctx.registry().unwrap().projects.len(), 8);
}

#[test]
fn a_half_written_claude_json_is_left_alone() {
    let (dir, ctx) = ctx();
    let main = dir.path().join("home").join(".claude");
    std::fs::create_dir_all(&main).unwrap();
    std::fs::write(main.join("settings.json"), r#"{"model":"opus"}"#).unwrap();
    let prof = dir.path().join("home").join(".claude-work");
    std::fs::create_dir_all(&prof).unwrap();
    let broken = r#"{"oauthAccount":{"emailAddress":"me@x.com"}, "proj"#;
    std::fs::write(prof.join(".claude.json"), broken).unwrap();
    let err = hrm_core::shared::sync_claude(&ctx, &main, &dir.path().join("home").join(".claude.json"), &prof).unwrap_err();
    assert!(format!("{err:#}").contains("left unchanged"), "{err:#}");
    assert_eq!(std::fs::read_to_string(prof.join(".claude.json")).unwrap(), broken);
}

#[test]
fn enabling_a_prompt_keeps_every_accounts_own_file_and_app_edits_stick() {
    let (_dir, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("codex", "codex", true)).unwrap();
    reg.add_profile(profile("codex-work", "codex", false)).unwrap();
    let files: Vec<_> = reg.profiles.iter().map(|p| harness::of(p).item_paths(&ctx, p).prompt.unwrap()).collect();
    for (f, body) in files.iter().zip(["main rules", "work rules"]) {
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, body).unwrap();
    }
    let id = items::add_prompt(&mut reg, "codex", "Shared", "shared rules").unwrap();
    items::enable_prompt(&ctx, &mut reg, &id).unwrap();
    for f in &files { assert_eq!(std::fs::read_to_string(f).unwrap(), "shared rules"); }
    let bodies: Vec<&str> = reg.prompts.iter().map(|p| p.body.as_str()).collect();
    assert!(bodies.contains(&"main rules") && bodies.contains(&"work rules"), "both replaced files are in the library: {bodies:?}");

    // editing the enabled prompt in the app: the new text reaches the files, and no copy of the old one piles up
    let n = reg.prompts.len();
    reg.prompts.iter_mut().find(|p| p.id == id).unwrap().body = "better rules".into();
    items::enable_prompt(&ctx, &mut reg, &id).unwrap();
    for f in &files { assert_eq!(std::fs::read_to_string(f).unwrap(), "better rules"); }
    assert_eq!(reg.prompts.len(), n);
    assert_eq!(reg.prompts.iter().find(|p| p.id == id).unwrap().body, "better rules");
}

#[test]
fn an_open_claude_console_names_its_session() {
    let (dir, ctx) = ctx();
    let p = profile("claude", "claude", true);
    let sessions = dir.path().join("home").join(".claude").join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    // Claude Code 2.1 writes <pid>.json with the process start (FILETIME); our own pid is a live process
    let started = hrm_core::procs::start_ms(std::process::id()).unwrap();
    let filetime = (started + 11_644_473_600_000) * 10_000;
    let file = sessions.join(format!("{}.json", std::process::id()));
    std::fs::write(&file, format!(r#"{{"sessionId":"8a304efa","cwd":"D:\\work","startedAt":{started},"procStart":"{filetime}"}}"#)).unwrap();
    let open = harness::claude::Claude.running(&ctx, &p);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].session_id.as_deref(), Some("8a304efa"));
    assert_eq!(open[0].started_at, Some(started));
    assert_eq!(open[0].cwd.as_deref(), Some(r"D:\work"));
    // a file left by a Claude Code that crashed, whose pid Windows has since given to another program
    std::fs::write(&file, r#"{"sessionId":"old","startedAt":1793485333513,"procStart":"134379589320629346"}"#).unwrap();
    assert!(harness::claude::Claude.running(&ctx, &p).is_empty());
}

#[test]
fn an_unfinished_antigravity_sign_in_never_lands_on_the_main_login() {
    let (dir, ctx) = ctx();
    let prefix = format!("HRMTEST-core-{}", std::process::id());
    let live = format!("{prefix}-agylive");
    // this test's own credential names, and an agy program that is never running
    std::env::set_var("HRM_CRED_PREFIX", &prefix);
    std::env::set_var("HRM_AGY_CRED", &live);
    std::env::set_var("HRM_BIN_AGY", dir.path().join("agy.exe"));
    let email = |t: &str| String::from_utf8(hrm_core::secrets::read(t).unwrap().1).unwrap();
    hrm_core::secrets::write(&live, "u", br#"{"email":"main@gmail.com"}"#).unwrap();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("agy-main", "agy", true)).unwrap();
    reg.add_profile(profile("agy-new", "agy", false)).unwrap();
    ctx.save_registry(&reg).unwrap();
    let main = reg.get("agy-main").unwrap().clone();
    let agy = harness::agy::Agy;
    agy.prepare(&ctx, &main).unwrap();
    agy.prepare_login(&ctx).unwrap();
    // the sign-in wrote its login, then was cancelled before it could be filed under its account
    hrm_core::secrets::write(&live, "u", br#"{"email":"new@gmail.com"}"#).unwrap();
    agy.activate(&ctx, &main).unwrap();
    let saved = format!("{prefix}:agy:agy-main");
    let (m, l) = (email(&saved), email(&live));
    let _ = hrm_core::secrets::delete_prefix(&prefix);
    assert!(m.contains("main@gmail.com"), "main's saved login was overwritten: {m}");
    assert!(l.contains("main@gmail.com"), "{l}");
}

#[test]
fn router_stop_never_kills_a_process_that_is_not_the_router() {
    let (_dir, ctx) = ctx();
    // a saved router id that Windows has since given to another program: this test process
    ctx.db.kv_set("router:pid", &std::process::id().to_string()).unwrap();
    assert!(!hrm_core::service::router_stop(&ctx).unwrap());
    assert_eq!(ctx.db.kv_get("router:pid").unwrap().as_deref(), Some(""));
}

#[test]
fn a_broken_settings_file_is_an_error_not_silent_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings.json"), r#"{"registry_path": "D:\\OneDrive\\profiles.json",}"#).unwrap();
    let err = Ctx::open_at(&data, dir.path()).err().expect("a broken settings.json must not open");
    assert!(format!("{err:#}").contains("settings.json is not valid"), "{err:#}");
}

#[test]
fn an_account_of_an_unknown_tool_is_an_error_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.json");
    std::fs::write(&path, r#"{"version":1,"profiles":[{"name":"x","harness":"claud","kind":"subscription"}]}"#).unwrap();
    assert!(format!("{:#}", Registry::load(&path).unwrap_err()).contains("'claud'"));
}

#[test]
fn importing_mcp_never_claims_a_different_server_of_the_same_name() {
    let (dir, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude", "claude", true)).unwrap();
    items::add_mcp(&mut reg, hrm_core::registry::McpServer { name: "github".into(), transport: "stdio".into(), command: Some("library-cmd".into()), ..Default::default() }).unwrap();
    std::fs::create_dir_all(dir.path().join("home").join(".claude")).unwrap();
    std::fs::write(dir.path().join("home").join(".claude.json"), r#"{"mcpServers":{"github":{"type":"stdio","command":"my-own-cmd"},"docs":{"type":"http","url":"https://d.example/mcp"}}}"#).unwrap();
    let (added, kept) = items::import_mcp(&ctx, &mut reg, "claude").unwrap();
    assert_eq!((added, kept), (vec!["docs".to_string()], vec!["github".to_string()]));
    assert!(!reg.targets.iter().any(|t| t.item == "github"), "no target, so sync leaves the tool's own server alone");
    items::sync(&ctx, &reg).unwrap();
    let cj = std::fs::read_to_string(dir.path().join("home").join(".claude.json")).unwrap();
    assert!(cj.contains("my-own-cmd") && !cj.contains("library-cmd"), "{cj}");
}

#[test]
fn a_limit_window_whose_reset_time_passed_is_empty() {
    use hrm_core::model::{Quota, QuotaTier};
    let tier = |id: &str, used: f64, resets_at: Option<i64>| QuotaTier { id: id.into(), label: id.into(), used, resets_at };
    let q = Quota { tiers: vec![tier("session", 64.0, Some(1_000)), tier("week", 21.0, Some(9_000)), tier("week:opus", 5.0, None)], fetched_at: Some(500), source: "cache".into() };
    let q = q.as_of(2_000);
    assert_eq!((q.tiers[0].used, q.tiers[0].resets_at), (0.0, None), "the 5-hour window reset at 1000");
    assert_eq!((q.tiers[1].used, q.tiers[1].resets_at), (21.0, Some(9_000)), "the week is still open");
    assert_eq!(q.tiers[2].used, 5.0);
}

#[test]
fn a_server_removed_while_the_config_file_is_busy_is_removed_later() {
    use std::os::windows::fs::OpenOptionsExt;
    let (dir, ctx) = ctx();
    let cj = dir.path().join("home").join(".claude.json");
    std::fs::create_dir_all(dir.path().join("home").join(".claude")).unwrap();
    std::fs::write(&cj, "{}").unwrap();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude", "claude", true)).unwrap();
    items::add_mcp(&mut reg, hrm_core::registry::McpServer { name: "docs".into(), transport: "http".into(), url: Some("https://d.example/mcp".into()), ..Default::default() }).unwrap();
    items::set_target(&mut reg, "mcp", "docs", "claude", None, true).unwrap();
    items::sync(&ctx, &reg).unwrap();
    assert!(std::fs::read_to_string(&cj).unwrap().contains("docs"));
    items::remove_item(&mut reg, "mcp", "docs").unwrap();
    {
        // Claude Code holds the file open: this sync cannot write it
        let _busy = std::fs::OpenOptions::new().read(true).share_mode(0).open(&cj).unwrap();
        let rep = items::sync(&ctx, &reg).unwrap();
        assert!(!rep.skipped.is_empty(), "the failed write is reported");
    }
    items::sync(&ctx, &reg).unwrap();
    assert!(!std::fs::read_to_string(&cj).unwrap().contains("docs"), "still ours, so the next sync removes it");
}

#[test]
fn another_program_on_the_router_port_is_not_the_router() {
    use std::io::{Read, Write};
    let (_dir, mut ctx) = ctx();
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    ctx.settings.router_port = l.local_addr().unwrap().port();
    std::thread::spawn(move || for s in l.incoming().flatten().take(4) {
        let mut s = s;
        let mut buf = [0u8; 512];
        let _ = s.read(&mut buf);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello");
    });
    assert!(!hrm_core::service::router_running(&ctx));
    let err = hrm_core::service::ensure_router(&ctx).unwrap_err();
    assert!(format!("{err:#}").contains("another program is using port"), "{err:#}");
}

#[test]
fn windows_device_names_are_not_account_names() {
    use hrm_core::util::valid_name;
    for bad in ["nul", "CON", "com1", "LPT9", "aux"] { assert!(!valid_name(bad), "{bad}"); }
    for ok in ["console", "com", "com0", "nullable", "work-1"] { assert!(valid_name(ok), "{ok}"); }
}

#[test]
fn the_mirror_leaves_a_server_the_profile_changed() {
    let (dir, ctx) = ctx();
    let home = dir.path().join("home");
    let main = home.join(".claude");
    let prof = home.join(".claude-work");
    std::fs::create_dir_all(&main).unwrap();
    std::fs::create_dir_all(&prof).unwrap();
    let main_json = home.join(".claude.json");
    let cj = |p: &std::path::Path| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(p.join(".claude.json")).unwrap()).unwrap() };
    std::fs::write(&main_json, r#"{"mcpServers":{"docs":{"url":"https://a"},"wiki":{"url":"https://w"}}}"#).unwrap();
    hrm_core::shared::sync_claude(&ctx, &main, &main_json, &prof).unwrap();
    assert_eq!(cj(&prof)["mcpServers"]["docs"]["url"], "https://a");
    // the profile edits its copy of docs; main changes docs and drops wiki
    let mut pj = cj(&prof);
    pj["mcpServers"]["docs"]["url"] = "https://mine".into();
    std::fs::write(prof.join(".claude.json"), pj.to_string()).unwrap();
    std::fs::write(&main_json, r#"{"mcpServers":{"docs":{"url":"https://b"}}}"#).unwrap();
    hrm_core::shared::sync_claude(&ctx, &main, &main_json, &prof).unwrap();
    let pj = cj(&prof);
    assert_eq!(pj["mcpServers"]["docs"]["url"], "https://mine", "the profile's own edit wins");
    assert!(pj["mcpServers"].get("wiki").is_none(), "an unchanged mirrored server goes when main drops it");
}

#[test]
fn a_skill_whose_source_is_gone_is_not_reported_up_to_date() {
    let (dir, ctx) = ctx();
    let src = dir.path().join("src-skill");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("SKILL.md"), "---\nname: s\n---\n").unwrap();
    let mut reg = ctx.registry().unwrap();
    items::install_skill(&ctx, &mut reg, &src.display().to_string(), Some("s")).unwrap();
    assert_eq!(items::check_skill_updates(&ctx, &reg), vec![("s".to_string(), Some(false))]);
    std::fs::remove_dir_all(&src).unwrap();
    assert_eq!(items::check_skill_updates(&ctx, &reg), vec![("s".to_string(), None)]);
}

#[test]
fn the_program_given_to_windows_terminal_has_no_space() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("Harness Manager").join("hrm.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, b"").unwrap();
    let short = hrm_core::launch::short_path(&exe);
    assert_eq!(std::fs::canonicalize(&short).unwrap(), std::fs::canonicalize(&exe).unwrap(), "the same file");
    // this PC keeps 8.3 names (the Windows default for C:), so the short form has no space
    assert!(!short.to_string_lossy().contains(' '), "{}", short.display());
}

#[test]
fn rename_changes_only_the_shown_name() {
    let (_d, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("work", "claude", false)).unwrap();
    reg.add_profile(profile("home", "codex", false)).unwrap();
    ctx.save_registry(&reg).unwrap();
    hrm_core::service::set_label(&ctx, "work", "  Work account ").unwrap();
    let reg = ctx.registry().unwrap();
    let p = reg.get("work").unwrap();
    assert_eq!(p.label.as_deref(), Some("Work account"));
    assert_eq!(hrm_core::service::profile_view(&ctx, p).label, "Work account");
    assert!(hrm_core::service::set_label(&ctx, "home", "work account").is_err(), "another account's name is refused");
    assert!(hrm_core::service::set_label(&ctx, "home", "WORK").is_err(), "another account's id is refused");
    hrm_core::service::set_label(&ctx, "work", "").unwrap();
    assert_eq!(ctx.registry().unwrap().get("work").unwrap().label, None, "empty goes back to the id");
}

#[test]
fn default_terminal_reads_the_windows_setting() {
    // must agree with the setting Windows itself shows; "reg" is the reference
    let out = std::process::Command::new("reg").args(["query", r"HKCU\Console\%%Startup", "/v", "DelegationTerminal"]).output().unwrap();
    let txt = String::from_utf8_lossy(&out.stdout).to_ascii_uppercase();
    let want = txt.contains("{E12CFF52-A866-4C77-9A90-F570A7AA2C6B}") || txt.contains("{86633F1F-6454-40EC-89CE-DA4EBA977EE2}");
    assert_eq!(hrm_core::launch::wt_is_default_terminal(), want);
}

#[test]
fn a_reply_logged_in_parts_gets_its_first_token_time_and_speed() {
    let (_d, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude-main", "claude", true)).unwrap();
    ctx.save_registry(&reg).unwrap();
    let dir = ctx.home.join(".claude").join("projects").join("p");
    std::fs::create_dir_all(&dir).unwrap();
    // asked at :00, first part of the answer logged at :01.2 (partial count), last at :05.2
    let part = |t: &str, out: i64| format!(r#"{{"type":"assistant","timestamp":"2026-11-01T10:00:{t}Z","sessionId":"s","cwd":"C:/w","message":{{"id":"msg_1","model":"claude-opus-5-5","usage":{{"input_tokens":3,"output_tokens":{out}}},"content":[]}}}}"#);
    let log = [r#"{"type":"user","timestamp":"2026-11-01T10:00:00.000Z","sessionId":"s","message":{"role":"user","content":"hi"}}"#.to_string(),
        part("01.200", 10), part("05.200", 400)].join("\n") + "\n";
    std::fs::write(dir.join("s.jsonl"), log).unwrap();
    hrm_core::service::import_usage(&ctx).unwrap();
    let rows = hrm_core::service::request_log(&ctx, None, None, 10, 0).unwrap().1;
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].output, rows[0].ttft_ms, rows[0].gen_ms), (400, Some(1200), Some(4000)));
}

#[test]
fn skill_front_matter_gives_name_and_description() {
    let (n, d) = items::skill_front("---
name: pdf
description: \"Read and write PDFs\"
---
# PDF");
    assert_eq!((n.as_deref(), d.as_deref()), (Some("pdf"), Some("Read and write PDFs")));
    let (_, d) = items::skill_front("---
name: x
description: >
  Folded over
  two lines
other: 1
---
");
    assert_eq!(d.as_deref(), Some("Folded over two lines"));
    assert_eq!(items::skill_front("# no front matter"), (None, None));
}

#[test]
fn discover_lists_each_skill_in_a_repository_from_one_download() {
    use std::io::{Read, Write};
    let (_d, ctx) = ctx();
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        for (p, body) in [("skills/hello/SKILL.md", "---\nname: hello\ndescription: Says hello\n---\n"), ("skills/review/SKILL.md", "---\nname: review\n---\n"), ("README.md", "x")] {
            z.start_file(format!("demo-main/{p}"), zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    let body = buf.into_inner();
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h2 = hits.clone();
    std::thread::spawn(move || for s in l.incoming() {
        let Ok(mut s) = s else { continue };
        let mut b = [0u8; 4096];
        let _ = s.read(&mut b);
        h2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _ = write!(s, "HTTP/1.1 200 OK\r\ncontent-type: application/zip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
        let _ = s.write_all(&body);
    });
    std::env::set_var("HRM_GITHUB_CODELOAD", &base);
    let mut repos = items::Repos::new(ctx.data.join("discover"));
    let r = hrm_core::registry::SkillRepo { owner: "o".into(), name: "demo".into(), branch: "HEAD".into() };
    let a = items::repo_skills(&mut repos, &r).unwrap();
    let b = items::repo_skills(&mut repos, &r).unwrap();
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1, "one download for the repository");
    assert_eq!(a, b);
    let names: Vec<_> = a.iter().map(|s| (s["folder"].as_str().unwrap().to_string(), s["description"].as_str().map(String::from))).collect();
    assert_eq!(names, vec![("hello".into(), Some("Says hello".into())), ("review".into(), None)]);
    assert_eq!(a[0]["url"], "https://github.com/o/demo/tree/HEAD/skills/hello");
}

#[test]
fn store_folders_not_in_the_list_are_shown_and_adopted_with_the_ticks_their_links_show() {
    let (_d, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude-main", "claude", true)).unwrap();
    ctx.save_registry(&reg).unwrap();
    let store = items::skill_store(&ctx);
    for n in ["linked", "loose"] { std::fs::create_dir_all(store.join(n)).unwrap(); std::fs::write(store.join(n).join("SKILL.md"), "---\nname: x\n---").unwrap(); }
    let claude_skills = ctx.home.join(".claude").join("skills");
    std::fs::create_dir_all(&claude_skills).unwrap();
    hrm_core::shared::link_dir(&store.join("linked"), &claude_skills.join("linked")).unwrap();
    let found = items::unmanaged_skills(&ctx, &reg);
    assert_eq!(found, vec![("linked".to_string(), vec!["claude".to_string()]), ("loose".to_string(), vec![])]);
    let done = items::adopt_skills(&ctx, &mut reg, &[]).unwrap();
    assert_eq!(done, vec!["linked", "loose"]);
    assert!(reg.targets.iter().any(|t| t.item == "linked" && t.harness == "claude"), "the link it had becomes its tick");
    assert!(!reg.targets.iter().any(|t| t.item == "loose"));
    assert!(items::unmanaged_skills(&ctx, &reg).is_empty());
}

#[test]
fn import_from_apps_reads_every_accounts_config_and_ticks_an_accounts_own_server_for_it_alone() {
    let (_d, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude-main", "claude", true)).unwrap();
    reg.add_profile(profile("work", "claude", false)).unwrap();
    ctx.save_registry(&reg).unwrap();
    std::fs::create_dir_all(&ctx.home).unwrap();
    std::fs::write(ctx.home.join(".claude.json"), r#"{"mcpServers":{"shared":{"type":"stdio","command":"s"}}}"#).unwrap();
    let work = harness::of(reg.get("work").unwrap()).item_paths(&ctx, reg.get("work").unwrap()).mcp.unwrap().0;
    std::fs::create_dir_all(work.parent().unwrap()).unwrap();
    std::fs::write(&work, r#"{"mcpServers":{"own":{"type":"http","url":"https://own.example/mcp"}}}"#).unwrap();
    let report = items::import_mcp_all(&ctx, &mut reg);
    assert_eq!(report.len(), 2, "both files read: {report:?}");
    assert!(report[1]["label"].as_str().unwrap().contains("work"));
    assert!(reg.targets.iter().any(|t| t.item == "shared" && t.profile.is_none()), "the main profile's server is on for the tool");
    assert!(reg.targets.iter().any(|t| t.item == "own" && t.profile.as_deref() == Some("work")), "an account's own server is on for that account");
    assert!(!reg.targets.iter().any(|t| t.item == "own" && t.profile.is_none()));
}

#[test]
fn a_backup_brings_back_a_lost_skill_and_the_list_and_keeps_the_state_before() {
    use hrm_core::backup;
    let (_d, ctx) = ctx();
    let mut reg = ctx.registry().unwrap();
    reg.add_profile(profile("claude-main", "claude", true)).unwrap();
    let store = items::skill_store(&ctx);
    std::fs::create_dir_all(store.join("mine").join("scripts")).unwrap();
    std::fs::write(store.join("mine").join("SKILL.md"), "---\nname: mine\n---").unwrap();
    std::fs::write(store.join("mine").join("scripts").join("x.py"), "print(1)").unwrap();
    reg.skills.push(hrm_core::registry::Skill { name: "mine".into(), source: "store".into(), hash: String::new() });
    ctx.save_registry(&reg).unwrap();
    ctx.db.ledger_set("C:/somewhere", "skill", "mine", true).unwrap();
    let b = backup::create(&ctx, "Manual").unwrap();
    // lost: the folder, the list's entry, the ledger
    std::fs::remove_dir_all(store.join("mine")).unwrap();
    let mut reg = ctx.registry().unwrap();
    reg.skills.clear();
    ctx.save_registry(&reg).unwrap();
    ctx.db.ledger_set("C:/somewhere", "skill", "mine", false).unwrap();
    backup::restore(&ctx, &b.name).unwrap();
    assert_eq!(ctx.registry().unwrap().skills.len(), 1, "the list is back");
    assert_eq!(std::fs::read_to_string(store.join("mine").join("scripts").join("x.py")).unwrap(), "print(1)", "the folder is back");
    assert_eq!(ctx.db.ledger("C:/somewhere", "skill").unwrap(), vec!["mine"], "the ledger is back");
    let all = backup::list(&ctx);
    assert!(all.iter().any(|x| x.label == "Before restore"), "the state before was kept: {all:?}");
    backup::prune(&ctx, 1).unwrap();
    assert_eq!(backup::list(&ctx).len(), 1);
    assert!(backup::restore(&ctx, "../evil.zip").is_err());
}
