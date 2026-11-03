//! Test stand-in for claude, codex, opencode and agy: copied under each tool's name, it performs the
//! tool's login (writing the same files the real tool writes) and records how it was launched.
//!
//! FAKE_EMAIL      email the login signs in as (default fake@example.com)
//! FAKE_RUN_LOG    file that receives one JSON line per launch: tool, args, cwd and relevant variables
//! FAKE_WAIT_CODE  when set, `auth login` waits for a pasted code on stdin before finishing
//! FAKE_HOLD_MS    keep running this long first, like a console the user left open

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::json;

fn home() -> PathBuf { PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap()) }

fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }

fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        let k = c.len() + 1;
        for i in 0..k { out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char); }
    }
    out
}

fn record(tool: &str, args: &[String]) {
    let Ok(log) = std::env::var("FAKE_RUN_LOG") else { return };
    let vars = ["CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL", "CODEX_HOME", "OPENAI_API_KEY", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_CODE_MESSAGING_TOKEN"];
    let env: serde_json::Map<String, serde_json::Value> = vars.iter().filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), json!(v)))).collect();
    let line = json!({"tool": tool, "args": args, "cwd": std::env::current_dir().unwrap().display().to_string(), "env": env});
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap();
    f.write_all(format!("{line}\n").as_bytes()).unwrap();
}

fn email(args: &[String]) -> String {
    args.windows(2).find(|w| w[0] == "--email").map(|w| w[1].clone()).or_else(|| std::env::var("FAKE_EMAIL").ok()).unwrap_or_else(|| "fake@example.com".into())
}

fn wait_for_code() {
    if std::env::var("FAKE_WAIT_CODE").is_ok() {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).unwrap();
        if line.trim() != "good-code" {
            eprintln!("invalid code");
            std::process::exit(2);
        }
    }
}

fn claude(args: &[String]) {
    if args.len() >= 2 && args[0] == "auth" && args[1] == "login" {
        println!("Opening browser to sign in…");
        println!("If the browser didn't open, visit: https://claude.ai/oauth/authorize?code=true&client_id=fake&state=xyz");
        std::io::stdout().flush().unwrap();
        wait_for_code();
        let dir = std::env::var("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|_| home().join(".claude"));
        let json_path = if std::env::var("CLAUDE_CONFIG_DIR").is_ok() { dir.join(".claude.json") } else { home().join(".claude.json") };
        std::fs::create_dir_all(&dir).unwrap();
        let mut cj: serde_json::Value = std::fs::read_to_string(&json_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        let uuid = format!("acct-{}", email(args));
        cj["oauthAccount"] = json!({"accountUuid": uuid, "emailAddress": email(args), "displayName": "Fake User", "organizationName": "Fake Org", "organizationType": "claude_max", "organizationRole": "admin"});
        cj["cachedUsageUtilization"] = json!({"fetchedAtMs": now_ms(), "accountUuid": uuid, "utilization": {
            "five_hour": {"utilization": 64, "resets_at": "2099-01-01T10:00:00+00:00"},
            "seven_day": {"utilization": 41, "resets_at": "2099-01-05T10:00:00+00:00"},
            "limits": [{"kind": "weekly_scoped", "percent": 12, "resets_at": "2099-01-05T10:00:00+00:00", "scope": {"model": {"display_name": "Fable"}}}]}});
        std::fs::write(&json_path, serde_json::to_string_pretty(&cj).unwrap()).unwrap();
        let creds = json!({"claudeAiOauth": {"accessToken": "fake-access", "refreshToken": "fake-refresh", "expiresAt": now_ms() + 3_600_000,
            "subscriptionType": "max", "rateLimitTier": "default_claude_max_20x"}});
        std::fs::write(dir.join(".credentials.json"), creds.to_string()).unwrap();
        println!("Login successful.");
        return;
    }
    record("claude", args);
}

fn codex(args: &[String]) {
    if args.first().map(|a| a == "login").unwrap_or(false) {
        println!("Starting local login server on http://localhost:1455.");
        println!("If your browser did not open, navigate to this URL to authenticate: https://auth.openai.com/oauth/authorize?client_id=fake");
        std::io::stdout().flush().unwrap();
        let dir = std::env::var("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|_| home().join(".codex"));
        std::fs::create_dir_all(&dir).unwrap();
        let claims = json!({"email": email(args), "name": "Fake User", "https://api.openai.com/auth": {"chatgpt_plan_type": "pro"}});
        let tok = format!("{}.{}.sig", b64url(br#"{"alg":"none"}"#), b64url(claims.to_string().as_bytes()));
        std::fs::write(dir.join("auth.json"), json!({"auth_mode": "chatgpt", "tokens": {"id_token": tok, "access_token": "x", "refresh_token": "y"}}).to_string()).unwrap();
        println!("Successfully logged in");
        return;
    }
    record("codex", args);
}

fn opencode(args: &[String]) {
    // like OpenCode 2.x: without --standalone the shared background service stores the login in its own
    // (the main) folder, whatever XDG_DATA_HOME says
    let standalone = args.iter().any(|a| a == "--standalone");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--standalone").collect();
    if rest.len() >= 2 && rest[0] == "auth" && rest[1] == "login" {
        let base = match std::env::var("XDG_DATA_HOME") {
            Ok(x) if standalone => PathBuf::from(x),
            _ => home().join(".local").join("share"),
        };
        let dir = base.join("opencode");
        std::fs::create_dir_all(&dir).unwrap();
        let c = rusqlite::Connection::open(dir.join("opencode.db")).unwrap();
        c.execute_batch("create table if not exists credential(id text, integration_id text, label text, value text, connector_id text, method_id text, active int, time_created int, time_updated int);
            insert into credential values('c1', 'opencode-go', 'OpenCode Go', 'x', null, null, 1, 0, 0);").unwrap();
        println!("Login successful");
        return;
    }
    record("opencode", args);
}

#[cfg(windows)]
fn write_cred(target: &str, blob: &[u8]) {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Security::Credentials::{CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC};
    let t: Vec<u16> = target.encode_utf16().chain([0]).collect();
    let u: Vec<u16> = "antigravity".encode_utf16().chain([0]).collect();
    let c = CREDENTIALW {
        Flags: 0, Type: CRED_TYPE_GENERIC, TargetName: t.as_ptr() as *mut u16, Comment: std::ptr::null_mut(),
        LastWritten: FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }, CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8, Persist: CRED_PERSIST_LOCAL_MACHINE, AttributeCount: 0,
        Attributes: std::ptr::null_mut(), TargetAlias: std::ptr::null_mut(), UserName: u.as_ptr() as *mut u16,
    };
    // like Harness Manager: serialize with its mutex and confirm the value stuck
    use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
    let name: Vec<u16> = r"Local\HarnessManager-credentials".encode_utf16().chain([0]).collect();
    let m = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    unsafe { WaitForSingleObject(m, 10_000) };
    for _ in 0..5 {
        assert!(unsafe { CredWriteW(&c, 0) } != 0, "CredWriteW failed");
        if has_cred(target) { break; }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    unsafe { ReleaseMutex(m); windows_sys::Win32::Foundation::CloseHandle(m); }
}

#[cfg(windows)]
fn has_cred(target: &str) -> bool {
    use windows_sys::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};
    let t: Vec<u16> = target.encode_utf16().chain([0]).collect();
    let mut p: *mut CREDENTIALW = std::ptr::null_mut();
    let ok = unsafe { CredReadW(t.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) } != 0;
    if ok { unsafe { CredFree(p as *const _) }; }
    ok
}

fn agy(args: &[String]) {
    // a launch without a saved login is Antigravity's sign-in
    #[cfg(windows)]
    {
        // never the real Antigravity login: a test that forgets the variable fails instead
        let target = std::env::var("HRM_AGY_CRED").expect("HRM_AGY_CRED must name the test's login slot");
        if !has_cred(&target) {
            println!("Sign in with Google: https://accounts.google.com/o/oauth2/auth?client_id=fake");
            write_cred(&target, json!({"email": email(args), "token": "fake"}).to_string().as_bytes());
            println!("Signed in");
            return;
        }
    }
    record("agy", args);
}

fn main() {
    let me = std::env::current_exe().unwrap();
    let name = me.file_stem().unwrap().to_string_lossy().to_lowercase();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(ms) = std::env::var("FAKE_HOLD_MS").ok().and_then(|v| v.parse().ok()) { std::thread::sleep(std::time::Duration::from_millis(ms)); }
    // every real tool answers this, and it is not a run
    if args == ["--version"] { println!("1.0.0 (fake {name})"); return; }
    match name.as_str() {
        "claude" => claude(&args),
        "codex" => codex(&args),
        "opencode" => opencode(&args),
        "agy" => agy(&args),
        other => { eprintln!("fakeharness: copy me as claude, codex, opencode or agy (got {other})"); std::process::exit(2); }
    }
}
