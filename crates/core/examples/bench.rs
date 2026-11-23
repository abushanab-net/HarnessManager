//! A sandbox of realistic volume and the timings the performance work is judged by:
//! `cargo run --release -p hrm-core --example bench -- gen` once, then `-- run` (or one part of it: `-- run cold|state|pages|watch|walk|backup`),
//! and `-- price` on an empty sandbox.
//! Never the real data: it stops unless HRM_HOME, HRM_PROFILES_HOME and HRM_CRED_PREFIX point at a sandbox.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use hrm_core::harness;
use hrm_core::registry::{Kind, Profile};
use hrm_core::service::{self, SessionFilter};
use hrm_core::{index, Ctx, SessionMeta, UsageRow};
use serde_json::{json, Value};

const DAY: i64 = 86_400_000;
const REMOTE: &str = "0b1c2d3e-aaaa-4bbb-8ccc-1234567890ab";

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn range(&mut self, a: u64, b: u64) -> u64 { a + self.next() % (b - a) }
    fn id(&mut self) -> String { let (a, b) = (self.next(), self.next()); format!("{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}", a as u32, (a >> 32) as u16, (a >> 48) & 0xfff, b & 0xfff, b >> 16) }
}

fn iso(ms: i64) -> String { chrono::DateTime::from_timestamp_millis(ms).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true) }

fn write(path: &Path, text: &str, mtime: i64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + Duration::from_millis(mtime as u64)).unwrap();
}

/// Text of about `bytes` bytes that reads like a tool's output.
fn output(r: &mut Rng, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 80);
    while s.len() < bytes { s.push_str(&format!("   Compiling crate-{} v0.{}.{} (src/lib.rs:{}) warning: unused variable `value_{}`\n", r.range(1, 400), r.range(0, 30), r.range(0, 9), r.range(1, 2000), r.range(1, 99))); }
    s
}

fn profile(name: &str, harness: &str, main: bool) -> Profile {
    Profile { name: name.into(), label: None, harness: harness.into(), kind: Kind::Subscription, email: None, main, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 }
}

fn claude_session(r: &mut Rng, id: &str, cwd: &str, end: i64, turns: u64) -> String {
    let mut out = String::new();
    let mut t = end - turns as i64 * 90_000;
    for n in 0..turns {
        out.push_str(&json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": iso(t), "message": {"role": "user", "content": format!("Step {n}: tidy the build of module {}", r.range(1, 90))}}).to_string());
        out.push('\n');
        for k in 0..r.range(2, 4) {
            t += 20_000;
            let tool = format!("toolu_{:x}", r.next());
            out.push_str(&json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": iso(t), "message": {"id": format!("msg_{:x}", r.next()), "model": if k == 0 { "claude-opus-5-5" } else { "claude-sonnet-5" },
                "usage": {"input_tokens": r.range(5, 900), "output_tokens": r.range(20, 1500), "cache_read_input_tokens": r.range(1000, 90_000), "cache_creation_input_tokens": r.range(0, 4000)},
                "content": [{"type": "text", "text": "Looking at the build output."}, {"type": "tool_use", "id": tool, "name": "Bash", "input": {"command": "cargo build --workspace"}}]}}).to_string());
            out.push('\n');
            let size = r.range(300, 1500) as usize;
            out.push_str(&json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": iso(t + 5_000), "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": tool, "content": output(r, size)}]}}).to_string());
            out.push('\n');
        }
        t += 30_000;
    }
    out
}

/// A Codex rollout of about `bytes` bytes, its last line at `end`.
fn rollout(r: &mut Rng, id: &str, cwd: &str, end: i64, bytes: usize, out_size: (u64, u64)) -> String {
    let turn = (out_size.0 + out_size.1) as usize / 2 + 1400;
    let turns = (bytes / turn).max(3) as i64;
    let mut t = end - turns * 60_000;
    let mut out = String::with_capacity(bytes + turn);
    let mut line = |v: Value| { out.push_str(&v.to_string()); out.push('\n'); };
    line(json!({"type": "session_meta", "timestamp": iso(t), "payload": {"id": id, "cwd": cwd}}));
    line(json!({"type": "turn_context", "timestamp": iso(t), "payload": {"model": "gpt-6.1-sol"}}));
    let mut total = 0;
    for n in 0..turns {
        t += 60_000;
        let prompt = format!("Turn {n}: make the tests of package {} pass", r.range(1, 60));
        line(json!({"type": "event_msg", "timestamp": iso(t - 50_000), "payload": {"type": "user_message", "message": prompt}}));
        line(json!({"type": "response_item", "timestamp": iso(t - 50_000), "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": prompt}]}}));
        let call = format!("call_{:x}", r.next());
        line(json!({"type": "response_item", "timestamp": iso(t - 40_000), "payload": {"type": "function_call", "name": "exec_command", "call_id": call, "arguments": "{\"cmd\":\"cargo test --workspace\"}"}}));
        let size = r.range(out_size.0, out_size.1) as usize;
        line(json!({"type": "response_item", "timestamp": iso(t - 30_000), "payload": {"type": "function_call_output", "call_id": call, "output": format!("Exit code: {}\n{}", (n % 9 == 0) as u8, output(r, size))}}));
        line(json!({"type": "event_msg", "timestamp": iso(t - 20_000), "payload": {"type": "item_completed"}}));
        line(json!({"type": "response_item", "timestamp": iso(t - 10_000), "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "The tests pass now."}]}}));
        let (i, o) = (r.range(2000, 90_000), r.range(50, 2500));
        total += i + o;
        line(json!({"type": "token_usage_record", "timestamp": iso(t), "payload": {"response_id": format!("resp_{:x}", r.next()), "usage": {"input_tokens": i, "cached_input_tokens": i / 2, "output_tokens": o}}}));
        line(json!({"type": "event_msg", "timestamp": iso(t), "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": total}},
            "rate_limits": {"primary": {"used_percent": (n % 90) as f64, "window_minutes": 300, "resets_at": (t + 5 * 3_600_000) / 1000}, "secondary": {"used_percent": (n % 40) as f64, "window_minutes": 10080, "resets_at": (t + 7 * DAY) / 1000}}}}));
    }
    out
}

/// A length-delimited protobuf field holding `text`, as Antigravity keeps a step.
fn pb(text: &str) -> Vec<u8> {
    let (mut out, mut n) = (vec![0x0a], text.len());
    while n >= 0x80 { out.push((n as u8 & 0x7f) | 0x80); n >>= 7; }
    out.push(n as u8);
    out.extend_from_slice(text.as_bytes());
    out
}

fn generate(ctx: &Ctx) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let now = hrm_core::util::now_ms();
    let home = &ctx.home;
    let mut reg = ctx.registry()?;
    for (name, h, main) in [("claude-main", "claude", true), ("claude-work", "claude", false), ("codex-main", "codex", true), ("codex-work", "codex", false), ("opencode-main", "opencode", true), ("agy-main", "agy", true)] {
        reg.add_profile(profile(name, h, main))?;
    }
    ctx.save_registry(&reg)?;
    // forty clones with an origin, five plain folders
    let projects: Vec<String> = (0..45).map(|n| {
        let d = home.join("Projects").join(format!("app-{n:02}"));
        std::fs::create_dir_all(d.join("src")).unwrap();
        if n < 40 { write(&d.join(".git").join("config"), &format!("[remote \"origin\"]\n\turl = git@github.com:example/app-{n:02}.git\n"), now); }
        d.display().to_string()
    }).collect();
    let (mut files, mut bytes) = (0usize, 0u64);
    // Claude: 2,300 session files, a 3 MB .claude.json and a saved login per account
    for (name, dir, n) in [("claude-main", home.join(".claude"), 1300), ("claude-work", home.join(".claude-claude-work"), 1000)] {
        for _ in 0..n {
            let (id, cwd) = (r.id(), projects[r.range(0, 45) as usize].clone());
            let end = now - r.range(3_600_000, 60 * DAY as u64) as i64;
            let turns = r.range(18, 60);
            let text = claude_session(&mut r, &id, &cwd, end, turns);
            bytes += text.len() as u64;
            files += 1;
            write(&dir.join("projects").join(harness::claude::project_dir_name(Path::new(&cwd))).join(format!("{id}.jsonl")), &text, end);
        }
        let uuid = r.id();
        let history: serde_json::Map<String, Value> = (0..900).map(|n| (format!("{}\\work-{n}", projects[n % 45]), json!({"allowedTools": [], "history": (0..12).map(|k| json!({"display": output(&mut r, 120), "pastedContents": {}, "n": k})).collect::<Vec<_>>(), "lastCost": 1.25, "lastSessionId": r.id()}))).collect();
        let doc = json!({"numStartups": 412, "projects": history, "oauthAccount": {"accountUuid": uuid, "emailAddress": "alex@example.com", "displayName": "Alex", "organizationName": "Example", "organizationRole": "admin", "organizationType": "claude_max"},
            "cachedUsageUtilization": {"accountUuid": uuid, "fetchedAtMs": now - 120_000, "utilization": {"five_hour": {"utilization": 31.0, "resets_at": iso(now + 3 * 3_600_000)}, "seven_day": {"utilization": 12.0, "resets_at": iso(now + 4 * DAY)}}}});
        let cj = if name == "claude-main" { home.join(".claude.json") } else { dir.join(".claude.json") };
        write(&cj, &serde_json::to_string_pretty(&doc)?, now - 120_000);
        write(&dir.join(".credentials.json"), &json!({"claudeAiOauth": {"accessToken": "sandbox", "expiresAt": now + DAY, "refreshTokenExpiresAt": now + 30 * DAY, "subscriptionType": "max", "rateLimitTier": "default_claude_max_20x"}}).to_string(), now - DAY);
        println!("{name}: {n} session files, .claude.json {} KB", std::fs::metadata(&cj)?.len() / 1024);
    }
    // Codex: 700 rollouts, three of each account between 50 and 100 MB
    for (name, dir, small, big) in [("codex-main", home.join(".codex"), 350, [50usize, 75, 100]), ("codex-work", home.join(".codex-codex-work"), 344, [50, 60, 90])] {
        let sizes = (0..small).map(|_| (r.range(100, 500) as usize * 1024, (2_000, 8_000))).chain(big.iter().map(|mb| (mb << 20, (20_000, 60_000)))).collect::<Vec<_>>();
        for (k, (size, out_size)) in sizes.into_iter().enumerate() {
            // the large ones are the ones still worked in: their last line is the newest
            let end = if size > 10 << 20 { now - 600_000 - k as i64 * 1000 } else { now - r.range(3_600_000, 60 * DAY as u64) as i64 };
            let (id, cwd) = (r.id(), projects[r.range(0, 45) as usize].clone());
            let text = rollout(&mut r, &id, &cwd, end, size, out_size);
            bytes += text.len() as u64;
            files += 1;
            let d = chrono::DateTime::from_timestamp_millis(end).unwrap();
            write(&dir.join("sessions").join(d.format("%Y").to_string()).join(d.format("%m").to_string()).join(d.format("%d").to_string()).join(format!("rollout-{}-{id}.jsonl", d.format("%Y-%m-%dT%H-%M-%S"))), &text, end);
        }
        let b64 = |v: Value| { const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let b = v.to_string().into_bytes(); let mut s = String::new();
            for c in b.chunks(3) { let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
                for i in 0..=c.len() { s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char); } }
            s };
        write(&dir.join("auth.json"), &json!({"tokens": {"id_token": format!("{}.{}.sig", b64(json!({"alg": "none"})), b64(json!({"email": "alex@example.com", "https://api.openai.com/auth": {"chatgpt_plan_type": "pro"}})))}}).to_string(), now - DAY);
        println!("{name}: {} rollouts", small + 3);
    }
    // OpenCode: one database, 300 sessions of 60 messages with four parts each, and the files it writes beside it
    {
        let dir = home.join(".local").join("share").join("opencode");
        std::fs::create_dir_all(dir.join("snapshot"))?;
        std::fs::create_dir_all(home.join(".config").join("opencode"))?;
        std::fs::write(dir.join("auth.json"), json!({"anthropic": {"type": "oauth"}}).to_string())?;
        let mut db = rusqlite::Connection::open(dir.join("opencode.db"))?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch("create table session(id text primary key, parent_id text, title text, directory text, time_created int, time_updated int, time_archived int, tokens_input int, tokens_output int, model text);
            create table message(id text primary key, session_id text, time_created int, time_updated int, data text);
            create table part(id text primary key, message_id text, session_id text, time_created int, time_updated int, data text);
            create index message_session_idx on message(session_id); create index part_session_idx on part(session_id);")?;
        let tx = db.transaction()?;
        for s in 0..300 {
            let (sid, cwd) = (format!("ses_{s:04}"), projects[r.range(0, 45) as usize].clone());
            let t = now - r.range(3_600_000, 60 * DAY as u64) as i64;
            tx.execute("insert into session values(?,?,?,?,?,?,?,?,?,?)", rusqlite::params![sid, None::<String>, format!("Fix the flaky build {s}"), cwd, t, t + 3_600_000, None::<i64>, 30_000, 4_000, ""])?;
            for m in 0..60 {
                let (mid, mt) = (format!("m_{s}_{m}"), t + m * 60_000);
                let data = if m % 2 == 0 { json!({"role": "user"}) } else { json!({"role": "assistant", "modelID": "glm-5", "time": {"created": mt, "completed": mt + 9_000}, "tokens": {"input": 500, "output": 66, "reasoning": 0, "cache": {"read": 4000, "write": 0}}, "path": {"cwd": cwd}}) };
                tx.execute("insert into message values(?,?,?,?,?)", rusqlite::params![mid, sid, mt, mt, data.to_string()])?;
                for p in 0..4 {
                    let data = if p == 0 { json!({"type": "text", "text": "Fix the flaky build"}) } else { json!({"type": "tool", "tool": "bash", "state": {"status": "completed", "input": {"command": "cargo test"}, "output": output(&mut r, 900)}}) };
                    tx.execute("insert into part values(?,?,?,?,?,?)", rusqlite::params![format!("p_{s}_{m}_{p}"), mid, sid, mt, mt, data.to_string()])?;
                }
            }
        }
        tx.commit()?;
    }
    // Antigravity: 50 conversation databases and their summaries
    {
        let dir = home.join(".gemini").join("antigravity-cli");
        std::fs::create_dir_all(dir.join("conversations"))?;
        let sums = rusqlite::Connection::open(dir.join("conversation_summaries.db"))?;
        sums.execute_batch("create table conversation_summaries(conversation_id text, title text, preview text, workspace_uris text)")?;
        for n in 0..50 {
            let id = format!("conv-{n:03}");
            let db = rusqlite::Connection::open(dir.join("conversations").join(format!("{id}.db")))?;
            db.execute_batch("create table steps(idx integer, step_type integer, status integer, step_payload blob)")?;
            for k in 0..80 { db.execute("insert into steps values(?,?,?,?)", rusqlite::params![k, if k % 2 == 0 { 14 } else { 15 }, 3, pb(&output(&mut r, 2_000))])?; }
            sums.execute("insert into conversation_summaries values(?,?,?,?)", rusqlite::params![id, format!("Explain the release script {n}"), "", json!([format!("file:///{}", projects[n % 45].replace('\\', "/"))]).to_string()])?;
        }
    }
    println!("files written: {files} session files, {} MB, {} s", bytes >> 20, t0.elapsed().as_secs());
    // another computer's history, as the journal merges it: 600 sessions and 60,000 requests in folders that are not on this disk
    let t1 = Instant::now();
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        tx.execute("insert or replace into peers(device_id, name, os, last_seen) values(?, 'WORKSTATION', 'linux', ?)", rusqlite::params![REMOTE, now])?;
        for s in 0..600 {
            let (n, id) = (r.range(0, 60), r.id());
            let end = now - r.range(3_600_000, 60 * DAY as u64) as i64;
            let cwd = format!("/home/alex/src/app-{n:02}");
            let m = SessionMeta { harness: "claude".into(), profile: "claude-main".into(), id: id.clone(), title: format!("Remote work {s}"), cwd: cwd.clone(), started_at: end - 3_600_000, last_active_at: end, prompts: 12, messages: 80, tokens: 90_000,
                model: "claude-opus-5-5".into(), source_path: format!("/home/alex/.claude/projects/x/{id}.jsonl"), project_key: format!("github.com/example/app-{n:02}"), project_name: format!("app-{n:02}"), device: REMOTE.into(), ..Default::default() };
            hrm_core::db::upsert_session(&tx, &hrm_core::db::session_key(&m), &m, REMOTE)?;
            for k in 0..100 {
                let row = UsageRow { request_id: format!("claude:remote_{s}_{k}"), profile: "claude-main".into(), harness: "claude".into(), ts: end - k * 30_000, model: "claude-opus-5-5".into(), project: cwd.clone(), session_id: id.clone(),
                    input: 300, output: 700, cache_read: 40_000, cache_write: 900, ttft_ms: Some(900), gen_ms: Some(7_000) };
                hrm_core::db::upsert_usage(&tx, &row, REMOTE, Some(0.031), Some("bench"))?;
            }
        }
        // what the router handled over ninety days
        let mut st = tx.prepare("insert into router_log(ts, route, profile, provider, model, status, ms, first_ms, input, output, cache_read, cache_write) values(?,?,?,?,?,?,?,?,?,?,?,?)")?;
        for k in 0..200_000i64 {
            st.execute(rusqlite::params![now - k * (90 * DAY / 200_000), "main", "claude-routed", if k % 3 == 0 { "one" } else { "two" }, "deepseek-flash", if k % 50 == 0 { 500 } else { 200 }, 4_000 + k % 900, 700 + k % 300, 2_000, 300, 9_000, 0])?;
        }
        drop(st);
        tx.commit()?;
        Ok(())
    })?;
    println!("remote history and router log: {} ms", t1.elapsed().as_millis());
    // the first import and the first walk, as the app's first round does them
    let t2 = Instant::now();
    let n = service::import_usage(ctx)?;
    println!("first import: {} ms, {n} rows", t2.elapsed().as_millis());
    let t3 = Instant::now();
    let rep = index::refresh(ctx, index::Scope::All)?;
    println!("first walk: {} ms, {} sessions", t3.elapsed().as_millis(), rep.sessions);
    counts(ctx)
}

fn counts(ctx: &Ctx) -> anyhow::Result<()> {
    ctx.db.read(|c| {
        for t in ["usage", "usage_day", "router_log", "sessions", "cursors", "quota_hist"] { println!("{t}: {} rows", c.query_row(&format!("select count(*) from {t}"), [], |r| r.get::<_, i64>(0))?); }
        Ok(())
    })
}

/// `f` run `reps` times: the median, the fastest and the slowest (the first, where something is remembered after it), in milliseconds.
fn time<T>(label: &str, reps: usize, mut f: impl FnMut() -> T) -> T {
    let mut ms = vec![];
    let mut last = None;
    for _ in 0..reps { let t = Instant::now(); last = Some(f()); ms.push(t.elapsed().as_secs_f64() * 1000.0); }
    ms.sort_by(|a, b| a.total_cmp(b));
    println!("{label:<66} median {:>9.1} ms   min {:>9.1} ms   max {:>9.1} ms   ({reps}x)", ms[ms.len() / 2], ms[0], ms[ms.len() - 1]);
    last.unwrap()
}

/// The longest a write, and a read through the scans' cache query, waited on this context while `f` ran, in milliseconds.
fn waits<T: Send>(ctx: &Ctx, f: impl FnOnce() -> T + Send) -> (T, f64, f64, f64) {
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        let probe = s.spawn(|| {
            let (mut w, mut rd) = (0f64, 0f64);
            while !stop.load(Ordering::Relaxed) {
                let t = Instant::now();
                let _ = ctx.db.with(|_| Ok(()));
                w = w.max(t.elapsed().as_secs_f64() * 1000.0);
                let t = Instant::now();
                let _ = ctx.db.cursors_for("agy-main");
                rd = rd.max(t.elapsed().as_secs_f64() * 1000.0);
                std::thread::sleep(Duration::from_millis(2));
            }
            (w, rd)
        });
        std::thread::sleep(Duration::from_millis(20));
        let t = Instant::now();
        let out = f();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        stop.store(true, Ordering::Relaxed);
        let (w, rd) = probe.join().unwrap();
        (out, ms, w, rd)
    })
}

/// This process's memory at its highest while `f` ran, over what it was before, in MB (sampled every 5 ms).
fn peak_mb<T: Send>(f: impl FnOnce() -> T + Send) -> (T, i64) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let me = Pid::from_u32(std::process::id());
    let mem = move || { let mut sys = System::new().ok()?; sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[me]), true, ProcessRefreshKind::nothing().with_memory()); sys.process(me).map(|p| p.memory() as i64) };
    let base = mem().unwrap_or(0);
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        let probe = s.spawn(|| { let mut top = base; while !stop.load(Ordering::Relaxed) { top = top.max(mem().unwrap_or(0)); std::thread::sleep(Duration::from_millis(5)); } top });
        let out = f();
        stop.store(true, Ordering::Relaxed);
        (out, (probe.join().unwrap() - base) >> 20)
    })
}

fn files_of(dir: &Path, ext: &str) -> Vec<(PathBuf, i64, u64)> {
    let mut v = vec![];
    hrm_core::util::walk_files_sig(dir, &|f: &Path| f.extension().map(|e| e == ext).unwrap_or(false), &mut v);
    v.sort();
    v
}

/// One more turn at the end of a session file, as its tool writes it.
fn append(path: &Path, r: &mut Rng) {
    let now = iso(hrm_core::util::now_ms());
    let line = if path.file_name().unwrap().to_string_lossy().starts_with("rollout-") {
        format!("{}\n{}\n", json!({"type": "token_usage_record", "timestamp": now, "payload": {"response_id": format!("resp_{:x}", r.next()), "usage": {"input_tokens": 900, "cached_input_tokens": 400, "output_tokens": 80}}}),
            json!({"type": "event_msg", "timestamp": now, "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": 1_000_000 + r.range(0, 9999)}}, "rate_limits": {"primary": {"used_percent": 44.0, "window_minutes": 300, "resets_at": 4102444800i64}}}}))
    } else {
        format!("{}\n", json!({"type": "assistant", "sessionId": path.file_stem().unwrap().to_string_lossy(), "timestamp": now, "message": {"id": format!("msg_{:x}", r.next()), "model": "claude-opus-5-5", "usage": {"input_tokens": 10, "output_tokens": 20}, "content": [{"type": "text", "text": "Done."}]}}))
    };
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(line.as_bytes()).unwrap();
}

/// Wait until the index row of every file carries the file's signature (the background parses landed); the wait in ms.
fn settled(ctx: &Ctx, files: &[PathBuf]) -> f64 {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(120) {
        let done = files.iter().all(|f| {
            let Some((m, _)) = hrm_core::util::file_sig(f) else { return true };
            ctx.db.read(|c| Ok(c.query_row("select count(*) from sessions where path = ?1 and mtime = ?2 and instr(key, '|') > 0", rusqlite::params![f.display().to_string(), m], |r| r.get::<_, i64>(0))?)).unwrap_or(0) > 0
        });
        if done { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    t.elapsed().as_secs_f64() * 1000.0
}

/// What the watcher does with a burst of changed paths: the owners' usage import, then the index refresh of those paths.
fn watched(ctx: &Ctx, paths: Vec<PathBuf>) -> Option<index::Report> { hrm_core::watch::changed(ctx, paths) }

fn run(ctx: &Ctx, only: Option<String>) -> anyhow::Result<()> {
    let want = |part: &str| only.as_deref().is_none_or(|o| o == part);
    let mut r = Rng(hrm_core::util::now_ms() as u64 | 1);
    let reg = ctx.registry()?;
    let home = &ctx.home;
    counts(ctx)?;
    if want("cold") {
    println!("\n-- the first walk of this process (nothing changed; project lookups not yet remembered)");
    let (rep, ms, w, rd) = waits(ctx, || index::refresh(ctx, index::Scope::All).unwrap());
    println!("{:<66} {ms:>9.1} ms   longest wait: a write {w:.1} ms, a cache read {rd:.1} ms   ({} sessions)", "full walk, nothing changed, cold", rep.sessions);
    time("full walk, nothing changed", 3, || index::refresh(ctx, index::Scope::All).unwrap());
    }

    if want("state") {
    println!("\n-- state");
    time("state: list_views (6 accounts)", 5, || service::list_views(ctx).unwrap());
    time("state: harnesses (6 program lookups)", 5, service::harnesses);
    for name in ["claude-main", "claude-work"] {
        let p = reg.get(name)?;
        time(&format!("identity + limits of {name} (.claude.json)"), 5, || { let h = harness::of(p); (h.identity(ctx, p), h.quota(ctx, p)) });
        let file = harness::claude::Claude.claude_json(ctx, p);
        let text = std::fs::read_to_string(&file)?;
        time(&format!("identity + limits of {name}, the file just rewritten"), 5, || { std::fs::write(&file, &text).unwrap(); let h = harness::of(p); (h.identity(ctx, p), h.quota(ctx, p)) });
    }
    for name in ["codex-main", "codex-work"] {
        let p = reg.get(name)?;
        let q = time(&format!("limits read of {name}"), 3, || harness::of(p).quota(ctx, p));
        println!("    {}", serde_json::to_string(&q)?);
    }
    }

    if want("pages") {
    println!("\n-- pages");
    let page = SessionFilter { per: Some(50), page: Some(0), days: Some(7), ..Default::default() };
    time("sessions: a page of 50, 7 days", 5, || service::sessions(ctx, &page).unwrap());
    time("sessions: every row", 3, || service::sessions(ctx, &SessionFilter::default()).unwrap());
    time("grouped_sessions: a page of 50, 7 days", 5, || service::grouped_sessions(ctx, &page, "project").unwrap());
    let ps = time("projects: all time", 5, || service::projects(ctx, None, None).unwrap());
    let cost: f64 = ps.iter().filter_map(|p| p.cost).sum();
    println!("    {} projects, {} with a cost, cost in all {cost:.2}, of other computers' sessions only: {}", ps.len(), ps.iter().filter(|p| p.cost.is_some()).count(), ps.iter().filter(|p| !p.here).count());
    time("projects: 30 days", 5, || service::projects(ctx, Some(30), None).unwrap());
    time("projects: 30 days, this computer", 5, || service::projects(ctx, Some(30), Some("here")).unwrap());
    let sid: String = ctx.db.read(|c| Ok(c.query_row("select session_id from usage where profile = 'claude-main' and device = ?1 limit 1", [&ctx.db.device], |r| r.get(0))?))?;
    time("session_stats (usage by session id)", 5, || hrm_core::dash::session_stats(ctx, "claude-main", &sid).unwrap());
    time("codex: a session's last model (usage by session id)", 5, || ctx.db.read(|c| Ok(c.query_row("select model from usage where session_id = ? order by ts desc limit 1", [&sid], |r| r.get::<_, String>(0))?)).unwrap());
    let now = hrm_core::util::now_ms();
    time("router: stats of 24 h", 5, || hrm_core::dash::router_stats(ctx, now).unwrap());
    time("router: spent today", 5, || hrm_core::dash::spent_today(ctx, "claude-routed").unwrap());
    time("router: live strip (dash::live)", 5, || hrm_core::dash::live(ctx, now).unwrap());
    time("router: request log, newest 40", 5, || ctx.db.route_log(40).unwrap());
    time("dashboard today", 3, || hrm_core::dash::dashboard(ctx, 0, None, None).unwrap());
    time("router: the first request logged (ninety days in the log)", 1, || ctx.db.log_route("main", "claude-routed", "one", "deepseek-flash", 200, 900, Some(300), (2_000, 300, 9_000, 0)).unwrap());
    time("router: a request logged", 5, || ctx.db.log_route("main", "claude-routed", "one", "deepseek-flash", 200, 900, Some(300), (2_000, 300, 9_000, 0)).unwrap());
    println!("    router_log: {} rows", ctx.db.read(|c| Ok(c.query_row("select count(*) from router_log", [], |r| r.get::<_, i64>(0))?))?);
    }


    let claude = files_of(&home.join(".claude").join("projects"), "jsonl");
    let codex = files_of(&home.join(".codex").join("sessions"), "jsonl");
    let small = codex.iter().find(|f| f.2 < 1 << 20).unwrap().0.clone();
    let big = codex.iter().max_by_key(|f| f.2).unwrap().0.clone();
    let oc = home.join(".local").join("share").join("opencode");
    let conv = home.join(".gemini").join("antigravity-cli").join("conversations").join("conv-007.db");
    if want("pieces") {
    println!("\n-- what a refresh of one account is made of");
    for name in ["claude-main", "codex-main", "opencode-main", "agy-main"] {
        let (p, h) = (reg.get(name)?, harness::of(reg.get(name)?));
        time(&format!("{name}: its scan, nothing changed"), 5, || h.sessions(ctx, p, 0).unwrap().len());
        time(&format!("{name}: its usage import, nothing changed"), 5, || h.import_usage(ctx, p).unwrap());
        time(&format!("{name}: its open consoles"), 5, || h.running(ctx, p).len());
        time(&format!("{name}: a refresh by one path, nothing changed"), 5, || index::refresh(ctx, index::Scope::Paths(h.session_dirs(ctx, p))).unwrap());
    }
    // the 100 MB rollout read again by the scan itself, as the full walk has it (the consoles' lookup apart)
    let p = reg.get("codex-main")?;
    let (_, mb) = peak_mb(|| time("codex-main: its scan, the 100 MB rollout changed, parsed in the scan", 3, || {
        append(&big, &mut r);
        harness::parse::PARSE_NOW.store(true, Ordering::Relaxed);
        let n = harness::of(p).sessions(ctx, p, 0).unwrap().len();
        harness::parse::PARSE_NOW.store(false, Ordering::Relaxed);
        n
    }));
    println!("    memory at its highest: +{mb} MB");
    let id = harness::parse::first_line(&big).and_then(|l| serde_json::from_str::<Value>(&l).ok()).map(|v| v["payload"]["id"].as_str().unwrap_or("").to_string()).unwrap_or_default();
    index::refresh(ctx, index::Scope::All)?;
    let (t, mb) = peak_mb(|| time("codex-main: the 100 MB session's transcript", 3, || service::transcript(ctx, "codex-main", &id).unwrap()));
    println!("    memory at its highest: +{mb} MB; {} turns, {} prompts, {} tool calls, {} failed", t.turns.len(), t.meta.prompts, t.meta.tool_calls, t.meta.failed);
    }

    if want("watch") {
    println!("\n-- one changed file, as the watcher handles it");
    for (label, file) in [("a Claude session (1,300 in the account)", claude[7].0.clone()), ("a small Codex rollout (353 in the account)", small.clone()), ("the 100 MB Codex rollout", big.clone())] {
        let mut wait = 0.0;
        time(&format!("watcher: {label}"), 3, || { append(&file, &mut r); let rep = watched(ctx, vec![file.clone()]); wait = settled(ctx, &[file.clone()]); rep });
        println!("    its parse landed {wait:.0} ms after the refresh returned");
    }
    time("watcher: OpenCode wrote a snapshot file (not its database)", 3, || { let f = oc.join("snapshot").join("pack.tmp"); std::fs::write(&f, r.id()).unwrap(); watched(ctx, vec![f]) });
    time("watcher: OpenCode's database, unchanged since the last scan", 3, || watched(ctx, vec![oc.join("opencode.db-wal")]));
    time("watcher: OpenCode's database after a write", 3, || {
        let db = rusqlite::Connection::open(oc.join("opencode.db")).unwrap();
        db.execute("update session set time_updated = ?1 where id = 'ses_0007'", [hrm_core::util::now_ms()]).unwrap();
        drop(db);
        watched(ctx, vec![oc.join("opencode.db-wal")])
    });
    time("watcher: an Antigravity conversation after a write (50 in all)", 3, || {
        let db = rusqlite::Connection::open(&conv).unwrap();
        db.execute("insert into steps values(?1, 15, 3, ?2)", rusqlite::params![r.range(100, 1 << 30) as i64, pb("One more answer.")]).unwrap();
        drop(db);
        watched(ctx, vec![conv.clone()])
    });
    }

    if want("walk") {
    println!("\n-- a full walk with changed files");
    let mut wait = 0.0;
    let (_, mb) = peak_mb(|| time("full walk: only the 100 MB rollout changed", 3, || { append(&big, &mut r); let rep = index::refresh(ctx, index::Scope::All).unwrap(); wait = settled(ctx, &[big.clone()]); rep }));
    println!("    memory at its highest: +{mb} MB; parse landed {wait:.0} ms after the walk returned");
    // 18 small files of the four file-keeping accounts and the two largest rollouts
    let work = files_of(&home.join(".claude-claude-work").join("projects"), "jsonl");
    let codex2 = files_of(&home.join(".codex-codex-work").join("sessions"), "jsonl");
    let mut changed: Vec<PathBuf> = claude.iter().skip(20).take(6).chain(work.iter().skip(20).take(6)).chain(codex.iter().filter(|f| f.2 < 1 << 20).take(3)).chain(codex2.iter().filter(|f| f.2 < 1 << 20).take(3)).map(|f| f.0.clone()).collect();
    changed.push(big.clone());
    changed.push(codex2.iter().max_by_key(|f| f.2).unwrap().0.clone());
    let ((_, ms, w, rd), mb) = peak_mb(|| { for f in &changed { append(f, &mut r); } waits(ctx, || index::refresh(ctx, index::Scope::All).unwrap()) });
    let wait = settled(ctx, &changed);
    println!("{:<66} {ms:>9.1} ms   longest wait: a write {w:.1} ms, a cache read {rd:.1} ms", format!("full walk: {} changed files (two of 90 and 100 MB)", changed.len()));
    println!("    memory at its highest: +{mb} MB; every parse landed {wait:.0} ms after the walk returned");
    let mut all = vec![];
    for _ in 0..2 {
        for f in &changed { append(f, &mut r); }
        let t = Instant::now();
        index::refresh(ctx, index::Scope::All)?;
        all.push(t.elapsed().as_secs_f64() * 1000.0);
        settled(ctx, &changed);
    }
    println!("{:<66} {:>9.1} ms and {:.1} ms", "the same walk twice more", all[0], all[1]);
    // what the pages show afterwards: the same whichever way the files were parsed
    let s = service::sessions(ctx, &SessionFilter { id: Some(hrm_core::harness::parse::first_line(&big).and_then(|l| serde_json::from_str::<Value>(&l).ok()).map(|v| v["payload"]["id"].as_str().unwrap_or("").to_string()).unwrap_or_default()), ..Default::default() })?;
    println!("    the 100 MB session: {}", s.first().map(|s| format!("{} prompts, {} messages, {} tool calls, {} failed, {} tokens, model {}", s.prompts, s.messages, s.tool_calls, s.failed, s.tokens, s.model)).unwrap_or_default());
    }

    if want("backup") {
    println!("\n-- a backup beside other work");
    let (b, ms, w, rd) = waits(ctx, || hrm_core::backup::create(ctx, "Bench").unwrap());
    println!("{:<66} {ms:>9.1} ms   longest wait: a write {w:.1} ms, a cache read {rd:.1} ms   ({} MB)", "backup (history of every usage row)", b.size >> 20);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    for v in ["HRM_HOME", "HRM_PROFILES_HOME", "HRM_CRED_PREFIX"] {
        if std::env::var_os(v).is_none() { anyhow::bail!("{v} is not set: this example writes a sandbox and never runs on the real data"); }
    }
    if std::env::var_os("HRM_AGY_CRED").is_none() { std::env::set_var("HRM_AGY_CRED", format!("{}-agylive", std::env::var("HRM_CRED_PREFIX")?)); }
    let mode = std::env::args().nth(1).unwrap_or_default();
    let data = PathBuf::from(std::env::var_os("HRM_HOME").unwrap());
    if mode == "price" {
        // the first read of the price list on a computer that has none: the download from models.dev, if this build downloads on a read
        std::env::remove_var("HRM_PRICING_FILE");
        let ctx = Ctx::open()?;
        let t = Instant::now();
        let n = hrm_core::pricing::load(&ctx).len();
        println!("{:<66} {:>9.1} ms   ({n} prices)", "first read of the price list, none stored", t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        service::import_usage(&ctx)?;
        println!("{:<66} {:>9.1} ms   ({} prices after it)", "the round's usage import, no account", t.elapsed().as_secs_f64() * 1000.0, hrm_core::pricing::load(&ctx).len());
        return Ok(());
    }
    // a local price list: no measurement waits on the network
    let prices = data.join("bench-pricing.json");
    std::fs::create_dir_all(&data)?;
    std::fs::write(&prices, json!({"anthropic": {"models": {"claude-opus-5-5": {"cost": {"input": 4, "output": 20, "cache_read": 0.4, "cache_write": 5}}, "claude-sonnet-5": {"cost": {"input": 2, "output": 10, "cache_read": 0.2, "cache_write": 2.5}}}},
        "openai": {"models": {"gpt-6.1-sol": {"cost": {"input": 2, "output": 10, "cache_read": 0.2}}}}}).to_string())?;
    std::env::set_var("HRM_PRICING_FILE", &prices);
    let t = Instant::now();
    let ctx = Ctx::open()?;
    println!("{:<66} {:>9.1} ms", "open (schema and migrations)", t.elapsed().as_secs_f64() * 1000.0);
    match mode.as_str() {
        "gen" => generate(&ctx),
        "run" => run(&ctx, std::env::args().nth(2)),
        _ => anyhow::bail!("gen, run or price"),
    }
}
