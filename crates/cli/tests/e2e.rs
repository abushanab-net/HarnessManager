//! End-to-end tests of the `hrm` binary against a sandbox: temporary homes, the fake harness under each
//! tool's name, a private credential prefix, and a recorded terminal.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

struct Env {
    _dir: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    bin: PathBuf,
    prefix: String,
    run_log: PathBuf,
    term_log: PathBuf,
    extra: Vec<(String, String)>,
}

fn target_dir() -> PathBuf {
    // tests run from target/debug/deps
    std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

impl Env {
    fn new() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let data = dir.path().join("data");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let fake = target_dir().join("fakeharness.exe");
        assert!(fake.exists(), "build the workspace first: cargo build --workspace");
        for t in ["claude", "codex", "opencode", "agy"] {
            std::fs::copy(&fake, bin.join(format!("{t}.exe"))).unwrap();
        }
        let id = format!("{}{}", std::process::id(), dir.path().file_name().unwrap().to_string_lossy().replace('.', ""));
        Env {
            run_log: dir.path().join("runs.jsonl"),
            term_log: dir.path().join("terminal.jsonl"),
            prefix: format!("HRMTEST-{id}"),
            home, data, bin, _dir: dir, extra: vec![],
        }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(target_dir().join("hrm.exe"));
        c.args(args)
            .env("HRM_HOME", &self.data)
            .env("HRM_PROFILES_HOME", &self.home)
            .env("HRM_CRED_PREFIX", &self.prefix)
            .env("HRM_AGY_CRED", format!("{}-agylive", self.prefix))
            .env("HRM_TERMINAL_LOG", &self.term_log)
            .env("FAKE_RUN_LOG", &self.run_log)
            .env("HRM_EXE", target_dir().join("hrm.exe"))
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME");
        for t in ["claude", "codex", "opencode", "agy"] {
            c.env(format!("HRM_BIN_{}", t.to_uppercase()), self.bin.join(format!("{t}.exe")));
        }
        for (k, v) in &self.extra { c.env(k, v); }
        c
    }

    /// Run and expect success; returns stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        let so = String::from_utf8_lossy(&out.stdout).to_string();
        let se = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(out.status.success(), "hrm {args:?} failed\nstdout: {so}\nstderr: {se}");
        so
    }

    fn fail(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(!out.status.success(), "hrm {args:?} should have failed");
        String::from_utf8_lossy(&out.stderr).to_string()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut a = args.to_vec();
        a.push("--json");
        serde_json::from_str(&self.ok(&a)).unwrap()
    }

    fn runs(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.run_log).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn terminals(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.term_log).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn profile(&self, name: &str) -> Value {
        self.json(&["list"]).as_array().unwrap().iter().find(|p| p["name"] == name).cloned().unwrap_or_else(|| panic!("no profile {name}"))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.cmd(&["router", "stop"]).output();
        let _ = hrm_core::secrets::delete_prefix(&self.prefix);
    }
}

/// A child process that is killed when the test ends, also when an assertion fails first.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn jsonl(lines: &[Value]) -> String { lines.iter().map(|l| l.to_string() + "\n").collect() }

/// A Claude session log with a prompt, an Edit that returns a patch, a failing Bash and an answer.
fn claude_session(id: &str, cwd: &str, ts: &str) -> String {
    jsonl(&[
        json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": ts, "message": {"role": "user", "content": "Add a backup target"}}),
        json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": ts, "requestId": "r1", "message": {"id": "msg_1", "model": "claude-opus-5-5", "role": "assistant",
            "usage": {"input_tokens": 100, "output_tokens": 50, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 10},
            "content": [{"type": "thinking", "thinking": ""}]}}),
        // the same message streamed again: must not be counted twice
        json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": ts, "requestId": "r1", "message": {"id": "msg_1", "model": "claude-opus-5-5", "role": "assistant",
            "usage": {"input_tokens": 100, "output_tokens": 50, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 10},
            "content": [{"type": "tool_use", "id": "t1", "name": "Edit", "input": {"file_path": "values.yaml", "old_string": "a", "new_string": "b"}}]}}),
        json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": ts, "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]},
            "toolUseResult": {"filePath": "values.yaml", "structuredPatch": [{"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 2, "lines": ["-a", "+b", "+c"]}]}}),
        json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": ts, "message": {"id": "msg_2", "model": "claude-opus-5-5", "role": "assistant",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "tool_use", "id": "t2", "name": "Bash", "input": {"command": "argocd app sync x"}}]}}),
        json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": ts, "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t2", "is_error": true, "content": "rpc error: connection closed"}]}}),
        json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": ts, "message": {"id": "msg_3", "model": "claude-opus-5-5", "role": "assistant",
            "usage": {"input_tokens": 1, "output_tokens": 1}, "content": [{"type": "text", "text": "The backup target is set."}]}}),
        json!({"type": "ai-title", "aiTitle": "Longhorn backup target", "sessionId": id}),
    ])
}

fn now_iso() -> String { chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true) }

#[test]
fn harnesses_and_first_run_detect_main_profiles() {
    let e = Env::new();
    std::fs::create_dir_all(e.home.join(".claude")).unwrap();
    std::fs::create_dir_all(e.home.join(".codex")).unwrap();
    let hs = e.json(&["harnesses"]);
    assert_eq!(hs.as_array().unwrap().len(), 6);
    // the four fake tools are on PATH; Pi and Oh My Pi are whatever this computer has
    assert!(hs.as_array().unwrap().iter().filter(|h| !matches!(h["id"].as_str(), Some("pi" | "omp"))).all(|h| h["installed"] == true));
    let agy = hs.as_array().unwrap().iter().find(|h| h["id"] == "agy").unwrap();
    assert_eq!(agy["caps"]["parallel_accounts"], false);
    let list = e.json(&["list"]);
    let names: Vec<&str> = list.as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["claude-main", "codex-main"]);
}

#[test]
fn claude_subscription_profile_login_identity_quota_and_launch() {
    let mut e = Env::new();
    e.extra.push(("FAKE_EMAIL".into(), "alex@example.net".into()));
    e.ok(&["add", "team-alex", "--email", "alex@example.net"]);
    let home = e.home.join(".claude-team-alex");
    assert!(home.join(".credentials.json").exists(), "login wrote credentials into the profile home");
    let p = e.profile("team-alex");
    assert_eq!(p["identity"]["logged_in"], true);
    assert_eq!(p["identity"]["email"], "alex@example.net");
    assert_eq!(p["identity"]["plan"], "Max 20x");
    assert_eq!(p["email_mismatch"], false);
    let tiers = p["quota"]["tiers"].as_array().unwrap();
    assert_eq!(tiers.iter().find(|t| t["id"] == "session").unwrap()["used"], 64.0);
    assert_eq!(tiers.iter().find(|t| t["id"] == "week").unwrap()["used"], 41.0);
    assert!(tiers.iter().any(|t| t["id"] == "week:fable"));

    // run: profile home set, conflicting variables removed, args passed through
    let mut c = e.cmd(&["run", "team-alex", "--", "--resume", "abc"]);
    c.env("ANTHROPIC_API_KEY", "leaked-from-parent");
    assert!(c.status().unwrap().success());
    let run = e.runs().pop().unwrap();
    assert_eq!(run["tool"], "claude");
    assert_eq!(run["args"], json!(["--resume", "abc"]));
    assert_eq!(run["env"]["CLAUDE_CONFIG_DIR"], home.display().to_string());
    assert!(run["env"].get("ANTHROPIC_API_KEY").is_none(), "a subscription launch must not inherit an API key");

    // start opens a terminal that runs `hrm run`
    e.ok(&["start", "team-alex"]);
    let t = e.terminals().pop().unwrap();
    assert_eq!(t["args"], json!(["run", "team-alex"]));
}

#[test]
fn login_with_wrong_email_is_flagged() {
    let mut e = Env::new();
    e.extra.push(("FAKE_EMAIL".into(), "someone-else@example.com".into()));
    // the fake signs in as --email when given, so log in without it and set the expectation afterwards
    e.ok(&["add", "acct", "--no-login"]);
    e.ok(&["login", "acct"]);
    let reg_path = e.data.join("profiles.json");
    let mut reg: Value = serde_json::from_str(&std::fs::read_to_string(&reg_path).unwrap()).unwrap();
    reg["profiles"].as_array_mut().unwrap().iter_mut().find(|p| p["name"] == "acct").unwrap()["email"] = json!("expected@example.com");
    std::fs::write(&reg_path, reg.to_string()).unwrap();
    assert_eq!(e.profile("acct")["email_mismatch"], true);
}

#[test]
fn api_key_profile_keeps_the_key_out_of_argv_and_files() {
    let mut e = Env::new();
    e.extra.push(("HRM_API_KEY".into(), "sk-ant-test-123".into()));
    // started from inside a Claude Code session: its markers must not reach the account
    e.extra.push(("CLAUDE_CODE_CHILD_SESSION".into(), "1".into()));
    e.extra.push(("CLAUDE_CODE_MESSAGING_TOKEN".into(), "parent-secret".into()));
    e.ok(&["add", "team-ci", "--api-key", "--model", "claude-sonnet-5-5"]);
    let p = e.profile("team-ci");
    assert_eq!(p["identity"]["plan"], "API key");
    assert_eq!(p["identity"]["logged_in"], true);
    e.ok(&["run", "team-ci", "--", "-p", "hi"]);
    let run = e.runs().pop().unwrap();
    assert_eq!(run["env"]["ANTHROPIC_API_KEY"], "sk-ant-test-123");
    assert_eq!(run["env"]["ANTHROPIC_MODEL"], "claude-sonnet-5-5");
    assert!(run["env"].get("CLAUDE_CODE_CHILD_SESSION").is_none() && run["env"].get("CLAUDE_CODE_MESSAGING_TOKEN").is_none(), "no parent session markers");
    assert!(!run["args"].to_string().contains("sk-ant"));
    let reg = std::fs::read_to_string(e.data.join("profiles.json")).unwrap();
    assert!(!reg.contains("sk-ant"), "keys never go into the registry");
    e.ok(&["start", "team-ci"]);
    assert!(!e.terminals().pop().unwrap().to_string().contains("sk-ant"), "keys never go on a terminal command line");
    // replace and delete
    e.extra.retain(|(k, _)| k != "HRM_API_KEY");
    e.extra.push(("HRM_API_KEY".into(), "sk-ant-new".into()));
    e.ok(&["key", "set", "team-ci"]);
    e.ok(&["run", "team-ci"]);
    assert_eq!(e.runs().pop().unwrap()["env"]["ANTHROPIC_API_KEY"], "sk-ant-new");
    e.ok(&["key", "delete", "team-ci"]);
    assert!(e.fail(&["run", "team-ci"]).contains("no API key saved"));
}

#[test]
fn invalid_and_duplicate_names_are_rejected() {
    let e = Env::new();
    assert!(e.fail(&["add", "bad name", "--no-login"]).contains("not a valid name"));
    e.ok(&["add", "one", "--no-login"]);
    assert!(e.fail(&["add", "one", "--no-login"]).contains("already exists"));
    assert!(e.fail(&["add", "x", "--harness", "nope", "--no-login"]).contains("unknown harness"));
    assert!(e.fail(&["run", "missing"]).contains("no profile named"));
}

#[test]
#[cfg(windows)]
fn an_interrupted_usage_import_loses_nothing() {
    use std::os::windows::fs::OpenOptionsExt;
    let e = Env::new();
    e.ok(&["add", "a1", "--no-login"]);
    let projects = e.home.join(".claude-a1").join("projects");
    write(&projects.join("a").join("s1.jsonl"), &claude_session("s1", r"D:\work", &now_iso()));
    let second = projects.join("b").join("s2.jsonl");
    write(&second, &jsonl(&[json!({"type": "assistant", "sessionId": "s2", "cwd": r"D:\work", "timestamp": now_iso(),
        "message": {"id": "msg_77", "model": "claude-opus-5-5", "usage": {"input_tokens": 500, "output_tokens": 0}, "content": []}})]));
    // another program holds the second file open, so this import fails partway
    let lock = std::fs::OpenOptions::new().read(true).share_mode(0).open(&second).unwrap();
    assert_eq!(e.json(&["usage"])["total"], 167, "the first file's rows are stored");
    drop(lock);
    assert_eq!(e.json(&["usage"])["total"], 667, "nothing was skipped");
    assert_eq!(e.json(&["usage", "--days", "0"])["total"], 667, "--days 0 means today, not a crash");
}

#[test]
fn usage_is_imported_once_per_message_and_incrementally() {
    let e = Env::new();
    e.ok(&["add", "a1", "--no-login"]);
    let log = e.home.join(".claude-a1").join("projects").join("D--work").join("s1.jsonl");
    let ts = now_iso();
    write(&log, &claude_session("s1", "D:\\work", &ts));
    let u = e.json(&["usage"]);
    let days = u["by_profile"]["a1"].as_array().unwrap();
    assert_eq!(days.iter().map(|d| d.as_i64().unwrap()).sum::<i64>(), 150 + 15 + 2, "msg_1 counted once");
    assert_eq!(u["cache_read"], 1000);
    // append a new message; re-import adds only it
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    use std::io::Write;
    writeln!(f, "{}", json!({"type": "assistant", "sessionId": "s1", "cwd": "D:\\work", "timestamp": ts, "message": {"id": "msg_9", "model": "claude-opus-5-5", "usage": {"input_tokens": 1000, "output_tokens": 0}, "content": []}})).unwrap();
    drop(f);
    let u = e.json(&["usage"]);
    assert_eq!(u["total"], 1167);
    let u = e.json(&["usage"]);
    assert_eq!(u["total"], 1167, "a second import adds nothing");
    assert_eq!(e.profile("a1")["tokens_7d"], 1167);
    // Claude Code logs a reply's first parts with the output so far and the real total on its last line,
    // which may only be written after an import: the full count wins
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    writeln!(f, "{}", json!({"type": "assistant", "sessionId": "s1", "cwd": "D:\\work", "timestamp": ts, "message": {"id": "msg_10", "model": "claude-opus-5-5", "usage": {"input_tokens": 0, "output_tokens": 8}, "content": [{"type": "text", "text": "Sure"}]}})).unwrap();
    drop(f);
    assert_eq!(e.json(&["usage"])["total"], 1175);
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    writeln!(f, "{}", json!({"type": "assistant", "sessionId": "s1", "cwd": "D:\\work", "timestamp": ts, "message": {"id": "msg_10", "model": "claude-opus-5-5", "stop_reason": "end_turn", "usage": {"input_tokens": 0, "output_tokens": 5000}, "content": [{"type": "tool_use", "id": "t9", "name": "Edit", "input": {}}]}})).unwrap();
    drop(f);
    assert_eq!(e.json(&["usage"])["total"], 1167 + 5000, "the reply's final output count replaces the partial one");
}

#[test]
fn sessions_group_by_repository_across_claude_and_codex() {
    let e = Env::new();
    std::fs::create_dir_all(e.home.join(".codex")).unwrap();
    e.ok(&["add", "a1", "--no-login"]);
    // a git repository with an origin remote, used from two folders
    let repo = e.home.join("Projects").join("platform-gitops");
    write(&repo.join(".git").join("config"), "[remote \"origin\"]\n\turl = git@github.com:example/platform-gitops.git\n");
    std::fs::create_dir_all(repo.join("apps")).unwrap();
    let ts = now_iso();
    write(&e.home.join(".claude-a1").join("projects").join("x").join("s1.jsonl"), &claude_session("s1", &repo.display().to_string(), &ts));
    // a Codex rollout started in apps\, compressed like Codex writes them
    let rollout = jsonl(&[
        json!({"type": "session_meta", "timestamp": ts, "payload": {"id": "c1", "cwd": repo.join("apps").display().to_string()}}),
        json!({"type": "turn_context", "timestamp": ts, "payload": {"model": "gpt-6"}}),
        json!({"type": "event_msg", "timestamp": ts, "payload": {"type": "user_message", "message": "Bump ArgoCD"}}),
        json!({"type": "response_item", "timestamp": ts, "payload": {"type": "custom_tool_call", "name": "apply_patch", "call_id": "k1", "input": "*** Begin Patch\n*** Update File: apps/argocd.yaml\n@@\n-v1\n+v2\n*** End Patch"}}),
        json!({"type": "response_item", "timestamp": ts, "payload": {"type": "custom_tool_call_output", "call_id": "k1", "output": "Success"}}),
        json!({"type": "response_item", "timestamp": ts, "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Bumped."}]}}),
        json!({"type": "token_usage_record", "timestamp": ts, "payload": {"response_id": "resp_1", "usage": {"input_tokens": 500, "cached_input_tokens": 400, "output_tokens": 20}}}),
        json!({"type": "event_msg", "timestamp": ts, "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": 520}},
            "rate_limits": {"primary": {"used_percent": 21, "window_minutes": 10080, "resets_at": 4102444800i64}, "secondary": {"used_percent": 7, "window_minutes": 300, "resets_at": 4102444800i64}}}}),
    ]);
    let dir = e.home.join(".codex").join("sessions").join("2026").join("11").join("01");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rollout-2026-11-01T10-00-00-c1.jsonl.zst"), zstd::encode_all(rollout.as_bytes(), 3).unwrap()).unwrap();

    let groups = e.json(&["sessions"]);
    let g = groups.as_array().unwrap().iter().find(|g| g["name"] == "platform-gitops").expect("one platform-gitops group");
    assert_eq!(g["key"], "github.com/example/platform-gitops");
    let tools: Vec<&str> = g["sessions"].as_array().unwrap().iter().map(|s| s["harness"].as_str().unwrap()).collect();
    assert!(tools.contains(&"claude") && tools.contains(&"codex"), "{tools:?}");
    assert_eq!(groups.as_array().unwrap().iter().filter(|g| g["name"] == "platform-gitops").count(), 1);
    // other groupings
    let by_tool = e.json(&["sessions", "--group", "tool"]);
    assert_eq!(by_tool.as_array().unwrap().len(), 2);
    let by_account = e.json(&["sessions", "--group", "account"]);
    assert!(by_account.as_array().unwrap().iter().any(|g| g["key"] == "codex-main"));
    // search
    let found = e.json(&["sessions", "--search", "longhorn"]);
    assert_eq!(found.as_array().unwrap().iter().map(|g| g["sessions"].as_array().unwrap().len()).sum::<usize>(), 1);

    // transcripts
    let t = e.json(&["session", "a1", "s1"]);
    assert_eq!(t["meta"]["title"], "Longhorn backup target");
    let turn = &t["turns"][0];
    assert_eq!(turn["prompt"], "Add a backup target");
    let steps = turn["steps"].as_array().unwrap();
    assert_eq!(steps[0]["kind"], "thinking");
    let edit = steps.iter().find(|s| s["tool"] == "Edit").unwrap();
    assert_eq!(edit["change"]["added"], 2);
    assert_eq!(edit["change"]["removed"], 1);
    let bash = steps.iter().find(|s| s["tool"] == "Bash").unwrap();
    assert_eq!(bash["status"], "error");
    assert!(bash["error"].as_str().unwrap().contains("rpc error"));
    assert_eq!(turn["answer"][0], "The backup target is set.");
    let ct = e.json(&["session", "codex-main", "c1"]);
    let st = &ct["turns"][0]["steps"][0];
    assert_eq!(st["tool"], "apply_patch");
    assert_eq!(st["change"]["path"], "apps/argocd.yaml");
    assert_eq!(st["status"], "ok");

    // an old rollout Codex rewrote later: the newest file, an old reading; the newest reading still wins
    let old = e.home.join(".codex").join("sessions").join("2026").join("10").join("01");
    std::fs::create_dir_all(&old).unwrap();
    write(&old.join("rollout-2026-10-01T10-00-00-old.jsonl"), &jsonl(&[json!({"type": "event_msg", "timestamp": "2026-10-01T10:00:00.000Z", "payload": {"type": "token_count",
        "rate_limits": {"primary": {"used_percent": 99, "window_minutes": 10080, "resets_at": 4102444800i64}}}})]));
    // codex limits and usage from the rollout, without any token
    let q = e.json(&["quota", "codex-main"]);
    assert_eq!(q["tiers"].as_array().unwrap().iter().find(|t| t["id"] == "week").unwrap()["used"], 21.0);
    assert_eq!(q["tiers"].as_array().unwrap().iter().find(|t| t["id"] == "session").unwrap()["used"], 7.0);
    let u = e.json(&["usage", "--profile", "codex-main"]);
    assert_eq!(u["total"], 120, "input minus cached plus output");

    // resume opens a terminal in the session's own account
    e.ok(&["resume", "codex-main", "c1"]);
    assert_eq!(e.terminals().pop().unwrap()["args"], json!(["run", "codex-main", "--", "resume", "c1"]));

    // projects can be renamed
    e.ok(&["projects", "rename", "github.com/example/platform-gitops", "Platform"]);
    let groups = e.json(&["sessions"]);
    assert!(groups.as_array().unwrap().iter().any(|g| g["name"] == "Platform"));
}

#[test]
fn shared_layer_links_folders_and_mirrors_settings_and_mcp() {
    let e = Env::new();
    let main = e.home.join(".claude");
    write(&main.join("skills").join("pdf").join("SKILL.md"), "pdf");
    write(&main.join("rules").join("a.md"), "rule");
    write(&main.join("CLAUDE.md"), "# main instructions");
    write(&main.join("settings.json"), &json!({"theme": "dark", "model": "opus", "env": {"FOO": "1", "CLAUDE_CONFIG_DIR": "x"}, "apiKeyHelper": "secret"}).to_string());
    write(&e.home.join(".claude.json"), &json!({"mcpServers": {"github": {"type": "http", "url": "https://x"}}}).to_string());
    e.ok(&["add", "p1", "--no-login"]);
    let prof = e.home.join(".claude-p1");
    assert!(hrm_core::shared::is_link(&prof.join("skills")), "skills linked to main");
    assert!(prof.join("skills").join("pdf").join("SKILL.md").exists());
    assert!(std::fs::read_to_string(prof.join("CLAUDE.md")).unwrap().starts_with('@'));
    let s: Value = serde_json::from_str(&std::fs::read_to_string(prof.join("settings.json")).unwrap()).unwrap();
    assert_eq!(s["model"], "opus");
    assert!(s.get("apiKeyHelper").is_none() && s["env"].get("CLAUDE_CONFIG_DIR").is_none(), "login settings are never mirrored");
    let pj: Value = serde_json::from_str(&std::fs::read_to_string(prof.join(".claude.json")).unwrap()).unwrap();
    assert!(pj["mcpServers"]["github"].is_object());

    // the profile changes one setting and adds its own MCP server; main changes another
    let mut s2 = s.clone();
    s2["model"] = json!("sonnet");
    write(&prof.join("settings.json"), &s2.to_string());
    let mut pj2 = pj.clone();
    pj2["mcpServers"]["mine"] = json!({"type": "stdio", "command": "mine"});
    write(&prof.join(".claude.json"), &pj2.to_string());
    write(&main.join("settings.json"), &json!({"theme": "light", "model": "opus"}).to_string());
    write(&e.home.join(".claude.json"), &json!({"mcpServers": {}}).to_string());
    e.ok(&["sync"]);
    let s: Value = serde_json::from_str(&std::fs::read_to_string(prof.join("settings.json")).unwrap()).unwrap();
    assert_eq!(s["model"], "sonnet", "the profile's own change wins");
    assert_eq!(s["theme"], "light", "main's change flows through");
    let pj: Value = serde_json::from_str(&std::fs::read_to_string(prof.join(".claude.json")).unwrap()).unwrap();
    assert!(pj["mcpServers"].get("github").is_none(), "removed from main, removed from the profile");
    assert!(pj["mcpServers"]["mine"].is_object(), "the profile's own server is kept");

    // removing the profile with its folder never touches main's linked folders
    e.ok(&["remove", "p1", "--delete-home"]);
    assert!(!prof.exists());
    assert!(main.join("skills").join("pdf").join("SKILL.md").exists());
}

#[test]
fn mcp_servers_are_written_per_tool_and_only_ours_are_removed() {
    let e = Env::new();
    for d in [".claude", ".codex", ".gemini"] { std::fs::create_dir_all(e.home.join(d)).unwrap(); }
    std::fs::create_dir_all(e.home.join(".config").join("opencode")).unwrap();
    write(&e.home.join(".codex").join("config.toml"), "model = \"gpt-6\"\n\n[mcp_servers.handmade]\ncommand = \"hm\"\n");
    write(&e.home.join(".claude.json"), &json!({"mcpServers": {"fs": {"type": "stdio", "command": "different"}}}).to_string());
    e.ok(&["list"]);
    e.ok(&["mcp", "add", "fs", "--command", "npx", "--args", "-y", "server-fs", "--env", "ROOT=C:\\"]);
    e.ok(&["mcp", "add", "docs", "--url", "https://mcp.example.com/mcp", "--header", "Authorization: Bearer t"]);
    for h in ["claude", "codex", "opencode", "agy"] {
        e.ok(&["mcp", "enable", "docs", h]);
    }
    let out = e.cmd(&["mcp", "enable", "fs", "claude"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("conflict"), "a hand-made fs entry is not overwritten");
    e.ok(&["mcp", "enable", "fs", "codex"]);

    let claude: Value = serde_json::from_str(&std::fs::read_to_string(e.home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(claude["mcpServers"]["docs"]["type"], "http");
    assert_eq!(claude["mcpServers"]["fs"]["command"], "different");
    let codex = std::fs::read_to_string(e.home.join(".codex").join("config.toml")).unwrap();
    assert!(codex.contains("[mcp_servers.fs]") && codex.contains("command = \"npx\"") && codex.contains("http_headers"));
    assert!(codex.contains("model = \"gpt-6\"") && codex.contains("[mcp_servers.handmade]"), "the rest of config.toml is kept");
    let oc: Value = serde_json::from_str(&std::fs::read_to_string(e.home.join(".config").join("opencode").join("opencode.json")).unwrap()).unwrap();
    assert_eq!(oc["mcp"]["docs"]["type"], "remote");
    let agy: Value = serde_json::from_str(&std::fs::read_to_string(e.home.join(".gemini").join("config").join("mcp_config.json")).unwrap()).unwrap();
    assert_eq!(agy["mcpServers"]["docs"]["serverUrl"], "https://mcp.example.com/mcp");

    e.ok(&["mcp", "disable", "docs", "codex"]);
    e.ok(&["mcp", "remove", "fs"]);
    let codex = std::fs::read_to_string(e.home.join(".codex").join("config.toml")).unwrap();
    assert!(!codex.contains("[mcp_servers.fs]") && !codex.contains("docs"));
    assert!(codex.contains("[mcp_servers.handmade]"), "never removes what it did not write");
    let claude: Value = serde_json::from_str(&std::fs::read_to_string(e.home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(claude["mcpServers"]["fs"]["command"], "different");

    // import brings a tool's own servers under management
    e.ok(&["mcp", "import", "codex"]);
    let l = e.json(&["mcp", "list"]);
    assert!(l["servers"].as_array().unwrap().iter().any(|s| s["name"] == "handmade"));
}

#[test]
fn skills_link_into_tools_and_prompts_keep_the_live_file() {
    let e = Env::new();
    for d in [".claude", ".codex"] { std::fs::create_dir_all(e.home.join(d)).unwrap(); }
    e.ok(&["list"]);
    let src = e.home.join("my-skill");
    write(&src.join("SKILL.md"), "---\nname: my-skill\n---\nDo things");
    e.ok(&["skills", "install", &src.display().to_string()]);
    e.ok(&["skills", "enable", "my-skill", "codex"]);
    let linked = e.home.join(".codex").join("skills").join("my-skill");
    assert!(hrm_core::shared::is_link(&linked) && linked.join("SKILL.md").exists());
    assert!(e.ok(&["skills", "check"]).contains("up to date"));
    write(&src.join("SKILL.md"), "changed");
    assert!(e.ok(&["skills", "check"]).contains("update available"));
    // an update whose copy fails halfway (a file another program holds open) keeps the installed version
    {
        use std::os::windows::fs::OpenOptionsExt;
        write(&src.join("notes.txt"), "busy");
        let lock = std::fs::OpenOptions::new().read(true).share_mode(0).open(src.join("notes.txt")).unwrap();
        assert!(e.fail(&["skills", "install", &src.display().to_string()]).contains("is unchanged"));
        drop(lock);
        let installed = e.home.join(".agents").join("skills").join("my-skill");
        assert_eq!(std::fs::read_to_string(installed.join("SKILL.md")).unwrap(), "---\nname: my-skill\n---\nDo things");
        assert!(linked.join("SKILL.md").exists(), "Codex still sees the skill");
        std::fs::remove_file(src.join("notes.txt")).unwrap();
    }
    e.ok(&["skills", "disable", "my-skill", "codex"]);
    assert!(!linked.exists());

    // installing the installed copy over itself is refused and loses nothing
    let store = e.home.join(".agents").join("skills").join("my-skill");
    assert!(e.fail(&["skills", "install", &store.display().to_string()]).contains("installed copy"));
    assert!(store.join("SKILL.md").exists());
    // removing never reaches outside the store: unknown names and paths are refused, the folders stay
    for bad in ["..", "..\\..", "missing", &e.home.display().to_string()] {
        assert!(e.fail(&["skills", "remove", bad]).contains("no skill named"), "{bad}");
    }
    assert!(e.home.join(".agents").exists() && store.exists());
    // importing Claude's skills keeps Claude's own version when the store has a different one by that name
    write(&e.home.join(".claude").join("skills").join("my-skill").join("SKILL.md"), "my own edited version");
    write(&e.home.join(".claude").join("skills").join("other").join("SKILL.md"), "---\nname: other\n---\n");
    let out = e.ok(&["skills", "import", "claude"]);
    assert!(out.contains("Imported 1 skill(s): other") && out.contains("Left alone") && out.contains("my-skill"), "{out}");
    let own = e.home.join(".claude").join("skills").join("my-skill");
    assert!(!hrm_core::shared::is_link(&own));
    assert_eq!(std::fs::read_to_string(own.join("SKILL.md")).unwrap(), "my own edited version");

    write(&e.home.join(".codex").join("AGENTS.md"), "hand-written instructions");
    let pf = e.home.join("p.md");
    write(&pf, "Be brief.");
    let id = e.ok(&["prompts", "add", "codex", "Brief", "--file", &pf.display().to_string()]).trim().trim_start_matches("Added ").to_string();
    e.ok(&["prompts", "enable", &id]);
    assert_eq!(std::fs::read_to_string(e.home.join(".codex").join("AGENTS.md")).unwrap(), "Be brief.");
    let ps = e.json(&["prompts", "list"]);
    assert!(ps.as_array().unwrap().iter().any(|p| p["body"] == "hand-written instructions"), "the live file was saved first");
}

#[test]
fn codex_and_opencode_profiles_are_isolated() {
    let mut e = Env::new();
    e.extra.push(("FAKE_EMAIL".into(), "alex@example.org".into()));
    e.ok(&["add", "codex-personal", "--harness", "codex"]);
    let p = e.profile("codex-personal");
    assert_eq!(p["identity"]["email"], "alex@example.org");
    assert_eq!(p["identity"]["plan"], "ChatGPT Pro");
    e.ok(&["run", "codex-personal"]);
    assert_eq!(e.runs().pop().unwrap()["env"]["CODEX_HOME"], e.home.join(".codex-codex-personal").display().to_string());

    e.ok(&["add", "oc-work", "--harness", "opencode"]);
    assert_eq!(e.profile("oc-work")["identity"]["plan"], "opencode-go", "the login is read from its database");
    assert!(!e.home.join(".local").join("share").join("opencode").join("opencode.db").exists(), "the login stayed out of the main profile");
    e.ok(&["run", "oc-work"]);
    let run = e.runs().pop().unwrap();
    assert_eq!(run["args"][0], "--standalone", "a private server, not the shared background service");
    assert_eq!(run["env"]["XDG_DATA_HOME"], e.home.join(".opencode-oc-work").join("data").display().to_string());

    // OpenCode sessions and usage come from its database
    let db = e.home.join(".opencode-oc-work").join("data").join("opencode").join("opencode.db");
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch("create table session(id text, parent_id text, title text, directory text, time_created int, time_updated int, time_archived int, tokens_input int, tokens_output int, model text);
        create table message(id text, session_id text, time_created int, time_updated int, data text);
        create table part(id text, message_id text, session_id text, time_created int, time_updated int, data text);").unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    c.execute("insert into session values('ses1',null,'Fix the build','D:\\proj',?1,?1,null,300,40,'')", [now]).unwrap();
    c.execute("insert into message values('m1','ses1',?1,?1,?2)", rusqlite::params![now, json!({"role": "user"}).to_string()]).unwrap();
    c.execute("insert into message values('m2','ses1',?1,?1,?2)", rusqlite::params![now + 1, json!({"role": "assistant", "modelID": "glm-5", "time": {"created": now, "completed": now + 5}, "tokens": {"input": 300, "output": 40, "reasoning": 0, "cache": {"read": 7, "write": 0}}, "path": {"cwd": "D:\\proj"}}).to_string()]).unwrap();
    c.execute("insert into part values('p1','m1','ses1',?1,?1,?2)", rusqlite::params![now, json!({"type": "text", "text": "Fix the build"}).to_string()]).unwrap();
    c.execute("insert into part values('p2','m2','ses1',?1,?1,?2)", rusqlite::params![now + 1, json!({"type": "tool", "tool": "bash", "state": {"status": "completed", "input": {"command": "cargo build"}, "output": "ok"}}).to_string()]).unwrap();
    c.execute("insert into part values('p3','m2','ses1',?1,?1,?2)", rusqlite::params![now + 2, json!({"type": "text", "text": "Fixed."}).to_string()]).unwrap();
    drop(c);
    let u = e.json(&["usage", "--profile", "oc-work"]);
    assert_eq!(u["total"], 340);
    let t = e.json(&["session", "oc-work", "ses1"]);
    assert_eq!(t["turns"][0]["prompt"], "Fix the build");
    assert_eq!(t["turns"][0]["steps"][0]["input"], "cargo build");
    assert_eq!(t["turns"][0]["answer"][0], "Fixed.");
}

#[test]
fn antigravity_profiles_swap_the_single_login() {
    let mut e = Env::new();
    e.extra.push(("FAKE_EMAIL".into(), "first@gmail.com".into()));
    e.ok(&["add", "agy-one", "--harness", "agy"]);
    e.extra.retain(|(k, _)| k != "FAKE_EMAIL");
    e.extra.push(("FAKE_EMAIL".into(), "second@gmail.com".into()));
    e.ok(&["add", "agy-two", "--harness", "agy"]);
    let live = format!("{}-agylive", e.prefix);
    let email_of = |t: &str| -> String {
        let (_, b) = hrm_core::secrets::read(t).unwrap();
        serde_json::from_slice::<Value>(&b).unwrap()["email"].as_str().unwrap().to_string()
    };
    assert_eq!(email_of(&live), "second@gmail.com");
    e.ok(&["run", "agy-one"]);
    assert_eq!(email_of(&live), "first@gmail.com", "starting agy-one swapped its login in");
    assert_eq!(e.runs().pop().unwrap()["tool"], "agy");
    e.ok(&["run", "agy-two"]);
    assert_eq!(email_of(&live), "second@gmail.com");
    assert_eq!(e.profile("agy-one")["identity"]["email"], "first@gmail.com");

    // while agy-two's console is open, neither starting agy-one nor a sign-in may swap the login under it
    let open = KillOnDrop(std::process::Command::new(e.bin.join("agy.exe")).env("FAKE_HOLD_MS", "60000")
        .env("HRM_AGY_CRED", &live).env("USERPROFILE", &e.home).spawn().unwrap());
    // wait until hrm sees the console (a busy machine starts it late), not a fixed time
    let t0 = std::time::Instant::now();
    while e.profile("agy-two")["running"].as_array().map(|r| r.is_empty()).unwrap_or(true) {
        assert!(t0.elapsed() < std::time::Duration::from_secs(10), "the Antigravity console never showed as open");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let err = e.fail(&["run", "agy-one"]);
    assert!(err.contains("Antigravity is open for 'agy-two'"), "{err}");
    assert!(e.fail(&["start", "agy-one"]).contains("Antigravity is open"));
    assert!(e.fail(&["login", "agy-one"]).contains("Close Antigravity"));
    assert_eq!(email_of(&live), "second@gmail.com", "the open console kept its login");
    e.ok(&["run", "agy-two"]); // its own account is fine
    drop(open);
    e.ok(&["run", "agy-one"]);
    assert_eq!(email_of(&live), "first@gmail.com");
}

#[test]
fn a_folder_that_cannot_be_deleted_keeps_the_account_and_its_key() {
    use std::os::windows::fs::OpenOptionsExt;
    let mut e = Env::new();
    e.extra.push(("HRM_API_KEY".into(), "sk-keep".into()));
    e.ok(&["add", "k1", "--api-key"]);
    let key = format!("{}:key:k1", e.prefix);
    let busy = e.home.join(".claude-k1").join("busy.txt");
    write(&busy, "open in another program");
    let lock = std::fs::OpenOptions::new().read(true).share_mode(0).open(&busy).unwrap();
    assert!(e.fail(&["remove", "k1", "--delete-home"]).contains("close its consoles"));
    e.profile("k1");
    assert!(hrm_core::secrets::read(&key).is_some(), "the key is still there");
    drop(lock);
    e.ok(&["remove", "k1", "--delete-home"]);
    assert!(hrm_core::secrets::read(&key).is_none());
    assert!(!busy.exists());
}

#[test]
fn adding_an_antigravity_account_keeps_the_main_login() {
    // Antigravity already signed in on this computer, and Harness Manager started fresh
    let e = Env::new();
    std::fs::create_dir_all(e.home.join(".gemini")).unwrap();
    let live = format!("{}-agylive", e.prefix);
    hrm_core::secrets::write(&live, "antigravity", br#"{"email":"main@gmail.com"}"#).unwrap();
    e.ok(&["init"]);
    e.ok(&["add", "agy-new", "--harness", "agy"]);
    let email_of = |t: &str| -> String {
        let (_, b) = hrm_core::secrets::read(t).unwrap();
        serde_json::from_slice::<Value>(&b).unwrap()["email"].as_str().unwrap().to_string()
    };
    e.ok(&["run", "agy-main"]);
    assert_eq!(email_of(&live), "main@gmail.com", "the main login was saved before the new sign-in and comes back");
    e.ok(&["run", "agy-new"]);
    assert_ne!(email_of(&live), "main@gmail.com");
    // removing the account deletes its saved login
    let saved = format!("{}:agy:agy-new", e.prefix);
    assert!(hrm_core::secrets::read(&saved).is_some());
    e.ok(&["remove", "agy-new", "--delete-home"]);
    assert!(hrm_core::secrets::read(&saved).is_none(), "no login left behind in Credential Manager");
    // agy-new was active: the slot goes back to the main login, which a later sign-in must not overwrite
    assert_eq!(email_of(&live), "main@gmail.com");
    e.ok(&["add", "agy-other", "--harness", "agy"]);
    e.ok(&["run", "agy-main"]);
    assert_eq!(email_of(&live), "main@gmail.com", "the removed account's login was never saved as main's");
    // removing the main profile keeps the user's own login
    e.ok(&["run", "agy-other"]);
    e.ok(&["remove", "agy-main"]);
    assert_eq!(email_of(&format!("{}:agy:agy-main", e.prefix)), "main@gmail.com");
}

#[test]
fn powershell_launcher_accounts_and_keys_are_imported() {
    let e = Env::new();
    let blob = Command::new("pwsh").args(["-NoProfile", "-Command", "ConvertFrom-SecureString (ConvertTo-SecureString 'sk-ant-imported' -AsPlainText -Force)"]).output().unwrap();
    let hex = String::from_utf8_lossy(&blob.stdout).trim().to_string();
    let computer = std::env::var("COMPUTERNAME").unwrap();
    let ps = e.home.join("claude-accounts.json");
    write(&ps, &json!({"accounts": [{"name": "work-max", "email": "alex@example.com"}, {"name": "personal-pro", "email": "alex@example.org"}],
        "apiKeys": [{"name": "my key", "computer": computer, "key": hex}, {"name": "other", "computer": "ELSEWHERE", "key": "00"}]}).to_string());
    let rep = e.json(&["import", &ps.display().to_string()]);
    assert_eq!(rep["accounts"], json!(["work-max", "personal-pro"]));
    assert_eq!(rep["keys"], json!(["my-key"]));
    assert!(e.home.join(".claude-work-max").exists());
    e.ok(&["run", "my-key"]);
    assert_eq!(e.runs().pop().unwrap()["env"]["ANTHROPIC_API_KEY"], "sk-ant-imported");
}

#[test]
fn settings_can_be_read_and_changed() {
    let e = Env::new();
    assert!(e.ok(&["settings", "terminal"]).contains("wt"));
    e.ok(&["settings", "terminal", "console"]);
    assert!(e.ok(&["settings", "terminal"]).contains("console"));
    e.ok(&["settings", "router_port", "16000"]);
    assert!(e.ok(&["router", "status"]).contains("16000"));
}

#[test]
fn routed_profile_starts_the_router_and_gets_its_address_and_token() {
    let mut e = Env::new();
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
    e.ok(&["settings", "router_port", &port]);
    e.extra.push(("HRM_API_KEY".into(), "sk-provider".into()));
    e.ok(&["provider", "add", "openrouter", "--base-url", "https://openrouter.ai/api/v1", "--format", "openai", "--models", "deepseek/deepseek-v4,z-ai/glm-5"]);
    e.ok(&["route", "add", "cheap", "--mode", "aggregate", "--providers", "openrouter"]);
    assert!(e.fail(&["route", "add", "x", "--providers", "missing"]).contains("no provider"));
    e.ok(&["add", "routed-1", "--route", "cheap", "--model", "hm-openrouter--z-ai/glm-5"]);
    assert_eq!(e.profile("routed-1")["identity"]["plan"], "Routed");
    let models = e.ok(&["router", "models", "cheap"]);
    assert!(models.contains("hm-openrouter--deepseek/deepseek-v4"));
    e.ok(&["run", "routed-1"]);
    let run = e.runs().pop().unwrap();
    assert_eq!(run["env"]["ANTHROPIC_BASE_URL"], format!("http://127.0.0.1:{port}/r/cheap/routed-1"));
    let token = run["env"]["ANTHROPIC_AUTH_TOKEN"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    assert_eq!(run["env"]["ANTHROPIC_MODEL"], "hm-openrouter--z-ai/glm-5");
    assert!(run["env"].get("ANTHROPIC_API_KEY").is_none(), "the provider key stays in the router");
    // the CLI refuses the same removals the app refuses
    assert!(e.fail(&["route", "remove", "cheap"]).contains("routed-1"));
    assert!(e.fail(&["provider", "remove", "openrouter"]).contains("cheap"));
    assert!(e.ok(&["router", "status"]).contains("running"));
    // the router answers its health check and rejects requests without the token
    let health = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok();
    assert!(health);
    e.ok(&["router", "stop"]);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(e.ok(&["router", "status"]).contains("stopped"));
}

/// A one-shot HTTP server: answers the first request with `body` and returns the request it saw.
fn mock_once(body: &'static str) -> (String, std::thread::JoinHandle<String>) { mock_reply("200 OK", body) }

fn mock_reply(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/oauth/usage", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = [0u8; 8192];
        let n = s.read(&mut buf).unwrap();
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        write!(s, "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).unwrap();
        req
    });
    (url, h)
}

#[test]
fn live_claude_limits_use_the_saved_login_while_valid() {
    let mut e = Env::new();
    e.ok(&["add", "live1"]);
    let (url, h) = mock_once(r#"{"five_hour":{"utilization":12,"resets_at":"2099-01-01T10:00:00+00:00"},"seven_day":{"utilization":3,"resets_at":"2099-01-05T10:00:00+00:00"}}"#);
    e.extra.push(("HRM_ANTHROPIC_USAGE_URL".into(), url));
    let q = e.json(&["quota", "live1", "--live"]);
    let req = h.join().unwrap().to_lowercase();
    assert!(req.contains("authorization: bearer fake-access"), "uses the access token Claude Code saved");
    assert!(req.contains("anthropic-beta: oauth-2025-04-20"));
    assert_eq!(q["source"], "live");
    assert_eq!(q["tiers"][0]["used"], 12.0);
    // the live reading is newer than Claude Code's cache, so it is what the list shows
    let p = e.profile("live1");
    assert_eq!(p["quota"]["source"], "live");
    assert_eq!(p["identity"]["needs_login"], false);
    // Anthropic refuses the saved login (signed out or revoked elsewhere): the account says it needs a new one
    let (url, h) = mock_reply("401 Unauthorized", r#"{"error":"invalid token"}"#);
    e.extra.retain(|(k, _)| k != "HRM_ANTHROPIC_USAGE_URL");
    e.extra.push(("HRM_ANTHROPIC_USAGE_URL".into(), url));
    assert!(e.fail(&["quota", "live1", "--live"]).contains("log in again"));
    h.join().unwrap();
    assert_eq!(e.profile("live1")["identity"]["needs_login"], true, "a refused login shows as expired");
    // a new login rewrites the file: no longer refused
    std::thread::sleep(std::time::Duration::from_millis(20));
    let creds = e.home.join(".claude-live1").join(".credentials.json");
    let saved = std::fs::read_to_string(&creds).unwrap();
    std::fs::write(&creds, saved).unwrap();
    assert_eq!(e.profile("live1")["identity"]["needs_login"], false);
    // an expired login is never refreshed by Harness Manager
    let mut c: Value = serde_json::from_str(&std::fs::read_to_string(&creds).unwrap()).unwrap();
    c["claudeAiOauth"]["expiresAt"] = json!(1);
    std::fs::write(&creds, c.to_string()).unwrap();
    assert!(e.fail(&["quota", "live1", "--live"]).contains("expired"));
}

#[test]
fn codex_api_key_profile_passes_the_key_in_the_environment() {
    let mut e = Env::new();
    e.extra.push(("HRM_API_KEY".into(), "sk-openai-test".into()));
    e.ok(&["add", "cx-key", "--harness", "codex", "--api-key"]);
    e.ok(&["run", "cx-key", "--", "exec", "hi"]);
    let run = e.runs().pop().unwrap();
    assert_eq!(run["tool"], "codex");
    assert_eq!(run["env"]["OPENAI_API_KEY"], "sk-openai-test");
    assert_eq!(run["args"], json!(["exec", "hi"]));
    assert!(e.fail(&["add", "oc-key", "--harness", "opencode", "--api-key"]).contains("does not take API keys"));
}

#[test]
fn overview_and_request_log_price_tokens_from_the_catalog() {
    let mut e = Env::new();
    let cat = e.home.join("pricing.json");
    write(&cat, &json!({"anthropic": {"models": {"claude-opus-5-5": {"cost": {"input": 4, "output": 20, "cache_read": 0.2, "cache_write": 5}}}}}).to_string());
    e.extra.push(("HRM_PRICING_FILE".into(), cat.display().to_string()));
    e.ok(&["add", "a1", "--no-login"]);
    write(&e.home.join(".claude-a1").join("projects").join("x").join("s1.jsonl"), &claude_session("s1", r"D:\work", &now_iso()));
    let o = e.json(&["overview", "--days", "7"]);
    assert_eq!(o["requests"], 3);
    assert_eq!(o["tokens"], 100 + 50 + 1000 + 10 + 15 + 2);
    let cost = (100.0 * 4.0 + 50.0 * 20.0 + 1000.0 * 0.2 + 10.0 * 5.0 + 10.0 * 4.0 + 5.0 * 20.0 + 4.0 + 20.0) / 1e6;
    assert!((o["cost"].as_f64().unwrap() - cost).abs() < 1e-12, "{} vs {cost}", o["cost"]);
    assert_eq!(o["heatmap"].as_array().unwrap().len(), 371);
    assert!(o["cache_hit_rate"].as_f64().unwrap() > 0.8);
    let r = e.json(&["requests"]);
    assert_eq!(r["total"], 3);
    assert_eq!(r["rows"][0]["model"], "claude-opus-5-5");
    assert!(r["rows"][0]["cost"].as_f64().unwrap() > 0.0);
    // a tool with no usage is counted as zero and not priced
    let o2 = e.json(&["overview", "--harness", "codex"]);
    assert_eq!(o2["requests"], 0);
    assert!(o2["cost"].is_null());
}

/// A zip like GitHub's codeload serves: one top folder `<repo>-<branch>/` with the repository inside.
fn repo_zip(files: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut buf);
    let opts = zip::write::SimpleFileOptions::default();
    for (p, body) in files {
        z.start_file(format!("skills-repo-main/{p}"), opts).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap();
    buf.into_inner()
}

/// Serves whatever bytes are in `body` to every request, and records the request paths.
fn mock_codeload(body: std::sync::Arc<std::sync::Mutex<Vec<u8>>>) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            seen2.lock().unwrap().push(req.split_whitespace().nth(1).unwrap_or("").to_string());
            let b = body.lock().unwrap().clone();
            let _ = write!(s, "HTTP/1.1 200 OK\r\ncontent-type: application/zip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", b.len());
            let _ = s.write_all(&b);
        }
    });
    (base, seen)
}

#[test]
fn skills_install_from_github_and_detect_updates() {
    let mut e = Env::new();
    std::fs::create_dir_all(e.home.join(".codex")).unwrap();
    let body = std::sync::Arc::new(std::sync::Mutex::new(repo_zip(&[("skills/pdf/SKILL.md", "---\nname: pdf\n---\nRead PDFs"), ("skills/pdf/scripts/x.py", "print(1)"), ("README.md", "repo")])));
    let (base, seen) = mock_codeload(body.clone());
    e.extra.push(("HRM_GITHUB_CODELOAD".into(), base));
    e.ok(&["list"]);
    e.ok(&["skills", "install", "https://github.com/anthropics/skills-repo/tree/main/skills/pdf"]);
    assert_eq!(seen.lock().unwrap()[0], "/anthropics/skills-repo/zip/refs/heads/main", "the branch archive is downloaded");
    let store = e.home.join(".agents").join("skills").join("pdf");
    assert_eq!(std::fs::read_to_string(store.join("SKILL.md")).unwrap(), "---\nname: pdf\n---\nRead PDFs");
    assert!(store.join("scripts").join("x.py").exists(), "the whole skill folder is installed");
    assert!(!store.join("README.md").exists(), "only the skill's folder, not the repository");
    e.ok(&["skills", "enable", "pdf", "codex"]);
    assert!(e.home.join(".codex").join("skills").join("pdf").join("SKILL.md").exists());
    assert!(e.ok(&["skills", "check"]).contains("up to date"));
    *body.lock().unwrap() = repo_zip(&[("skills/pdf/SKILL.md", "---\nname: pdf\n---\nRead PDFs, now with tables")]);
    assert!(e.ok(&["skills", "check"]).contains("update available"));
    // a path without SKILL.md is refused
    assert!(e.fail(&["skills", "install", "https://github.com/anthropics/skills-repo/tree/main/nothing"]).contains("no SKILL.md"));
    // a branch name with '/' in it: each split is tried until one holds the skill
    seen.lock().unwrap().clear();
    e.ok(&["skills", "install", "https://github.com/o/r/tree/release/v2/skills/pdf", "--name", "pdf2"]);
    assert!(seen.lock().unwrap().iter().any(|p| p == "/o/r/zip/refs/heads/release/v2"), "{:?}", seen.lock().unwrap());
}

#[test]
fn codex_replies_are_timed_from_the_message_to_the_first_item_done() {
    let e = Env::new();
    // one clock reading, so the gaps between lines are exact
    let base = chrono::Utc::now() - chrono::Duration::seconds(60);
    let at = |s: i64| (base + chrono::Duration::seconds(s)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rollout = jsonl(&[
        json!({"type": "session_meta", "timestamp": at(0), "payload": {"id": "t1", "cwd": "C:/work"}}),
        json!({"type": "response_item", "timestamp": at(0), "payload": {"type": "message", "role": "user", "content": []}}),
        json!({"type": "event_msg", "timestamp": at(2), "payload": {"type": "item_completed"}}),
        json!({"type": "event_msg", "timestamp": at(3), "payload": {"type": "item_completed"}}),
        json!({"type": "token_usage_record", "timestamp": at(3), "payload": {"response_id": "rt1", "usage": {"input_tokens": 10, "output_tokens": 5}}}),
        // no new message or tool output before this one: no timing rather than a wrong one
        json!({"type": "token_usage_record", "timestamp": at(9), "payload": {"response_id": "rt2", "usage": {"input_tokens": 10, "output_tokens": 5}}}),
    ]);
    let dir = e.home.join(".codex").join("sessions").join("2026").join("11").join("02");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rollout-2026-11-02T10-00-00-t1.jsonl.zst"), zstd::encode_all(rollout.as_bytes(), 3).unwrap()).unwrap();
    let rows = e.json(&["requests", "--harness", "codex"])["rows"].as_array().unwrap().clone();
    let get = |id: i64| rows.iter().find(|r| r["output"] == 5 && r["ts"].as_i64().is_some() && (id == 1) == r["ttft_ms"].is_i64()).cloned();
    let timed = get(1).expect("the first reply is timed");
    assert_eq!(timed["ttft_ms"], 2000);
    assert_eq!(timed["gen_ms"], 1000);
    assert!(get(2).is_some(), "the second one has no timing");
}

#[test]
fn a_login_that_expires_is_an_event_once() {
    let mut e = Env::new();
    e.extra.push(("FAKE_EMAIL".into(), "alex@example.net".into()));
    e.ok(&["add", "team-alex", "--email", "alex@example.net"]);
    e.ok(&["list"]);
    // the saved login's refresh token runs out
    let creds = e.home.join(".claude-team-alex").join(".credentials.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&creds).unwrap()).unwrap();
    v["claudeAiOauth"]["refreshTokenExpiresAt"] = json!(1000);
    std::fs::write(&creds, v.to_string()).unwrap();
    e.ok(&["list"]);
    e.ok(&["list"]);
    let ctx = hrm_core::Ctx::open_at(&e.data, &e.home).unwrap();
    let ev: Vec<Value> = ctx.db.events(10).unwrap().into_iter().filter(|x| x["kind"] == "login").collect();
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert!(ev[0]["text"].as_str().unwrap().contains("team-alex's login expired"));
}
