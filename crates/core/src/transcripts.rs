//! Resume here: a session that ran on another computer is read and resumed on this one as a fork. The
//! transcripts travel through the sync folder (`sync::transcripts`); here they are fetched when needed,
//! copied where the tool looks for them (`Harness::materialize`) and launched. The original is never written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::harness;
use crate::model::{SessionMeta, Step, Transcript, Turn};
use crate::projects::Resolver;
use crate::registry::Registry;
use crate::sync::folder;
use crate::util::{now_ms, sha256_hex, write_atomic};
use crate::Ctx;

/// A transcript over this many bytes is not carried.
pub const MAX_TRANSCRIPT_BYTES: u64 = 20 * 1024 * 1024;
/// The folder holds at most this much of this computer's transcripts; the oldest go first.
pub const CAP_BYTES: i64 = 200 * 1024 * 1024;

/// The tables of this module, made on first use: what this computer carried out, and what it knows of the others'.
pub fn tables(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("create table if not exists transcripts(device text not null, key text not null, file text not null, bytes integer not null,
        sha256 text not null, mtime integer not null, source_rel text not null, fetched_at integer, primary key(device, key));
        create table if not exists transcripts_out(key text primary key, file text not null, mtime integer not null, bytes integer not null,
        sha256 text not null, skipped text, at integer not null);")
}

/// A transcript from another computer, decompressed into this computer's data folder.
#[derive(Debug, Clone)]
pub struct FetchedTranscript {
    pub device: String,
    pub harness: String,
    pub profile: String,
    pub id: String,
    pub path: PathBuf,
    /// Where the tool kept it there, relative to the tool's home.
    pub source_rel: String,
    pub bytes: u64,
}

/// What a tool needs to resume a materialized transcript: the arguments after `hrm run <profile> --`, the
/// folder to open in, the id of the new session when the tool chose one, and a line for the user.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Materialized {
    pub launch_args: Vec<String>,
    pub cwd: PathBuf,
    pub new_session_id: Option<String>,
    pub note: String,
}

/// Where a fetched copy lives: `<data>/transcripts/<origin>/<harness>/<profile>/<file without .zst>`.
fn local_path(ctx: &Ctx, device: &str, harness: &str, profile: &str, file: &str) -> PathBuf {
    let name = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or("transcript");
    ctx.data.join("transcripts").join(device).join(harness).join(profile).join(name.strip_suffix(".zst").unwrap_or(name))
}

/// The transcript of a session from another computer, fetched from the folder the first time (or again after
/// a newer entry), then kept until the entry is tombstoned.
pub fn fetch(ctx: &Ctx, device: &str, harness: &str, profile: &str, id: &str) -> Result<FetchedTranscript> {
    let key = format!("{harness}/{profile}/{id}");
    let row: Option<(String, i64, String, String, Option<i64>)> = ctx.db.with(|c| {
        tables(c)?;
        Ok(c.query_row("select file, bytes, sha256, source_rel, fetched_at from transcripts where device=? and key=?", [device, &key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional()?)
    })?;
    let name = crate::journal::device_names(ctx).get(device).cloned().unwrap_or_else(|| device.to_string());
    let Some((file, _, sha, source_rel, fetched_at)) = row else { bail!("no transcript from {name} for this session yet: it is carried on that computer's next sync round") };
    let path = local_path(ctx, device, harness, profile, &file);
    if fetched_at.is_none() || !path.exists() {
        if file.contains("..") || Path::new(&file).is_absolute() { bail!("transcript entry names a path outside the folder"); }
        let src = folder::root(ctx)?.join("devices").join(device).join(&file);
        let raw = std::fs::read(&src).with_context(|| format!("read {} (waiting for the sync service?)", src.display()))?;
        if sha256_hex(&raw) != sha { bail!("the transcript in the folder does not match its journal entry yet; try again after the next sync"); }
        let plain = zstd::decode_all(&raw[..]).context("decompress the transcript")?;
        write_atomic(&path, &plain)?;
        ctx.db.with(|c| { c.execute("update transcripts set fetched_at=? where device=? and key=?", params![now_ms(), device, key])?; Ok(()) })?;
        crate::log::info(&format!("transcripts: fetched {key} from {name} ({} bytes)", plain.len()));
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(FetchedTranscript { device: device.into(), harness: harness.into(), profile: profile.into(), id: id.into(), path, source_rel, bytes })
}

/// The repositories this computer has, by project key: the root of every folder its usage and sessions name.
pub fn roots_here(ctx: &Ctx) -> Result<HashMap<String, String>> {
    let reg = ctx.registry()?;
    let dirs: Vec<String> = ctx.db.with(|c| {
        let mut st = c.prepare("select distinct project from usage where device = ?1 and project <> '' union select distinct json_extract(json, '$.cwd') from sessions where device = ?1 and json_extract(json, '$.cwd') <> ''")?;
        let v = st.query_map([&ctx.db.device], |r| r.get::<_, Option<String>>(0))?.flatten().flatten().collect();
        Ok(v)
    })?;
    let mut res = Resolver::new(&reg.projects);
    let mut roots = HashMap::new();
    for d in dirs {
        let info = res.resolve(&d);
        if !info.root.is_empty() && Path::new(&info.root).is_dir() { roots.entry(info.key).or_insert(info.root); }
    }
    Ok(roots)
}

/// `true`, or why a session from another computer cannot be resumed here, from what this computer knows.
pub fn resumable(s: &SessionMeta, roots: &HashMap<String, String>, reg: &Registry) -> Value {
    let label = harness::by_id(&s.harness).map(|h| h.label()).unwrap_or(&s.harness);
    if !roots.contains_key(&s.project_key) { return json!(format!("not cloned here: open {} once on this computer", s.project_name)); }
    if !reg.profiles.iter().any(|p| p.name == s.profile && p.harness == s.harness) { return json!(format!("no account {} for {label} on this computer", s.profile)); }
    if s.open_elsewhere { return json!(format!("running on {}: wait until it ends", s.device_name)); }
    if !harness::by_id(&s.harness).map(|h| h.resumes_elsewhere()).unwrap_or(false) { return json!(format!("resuming elsewhere is not available for {label}")); }
    json!(true)
}

/// Sessions this computer continued from another: journal key -> the computer it came from.
pub fn forks(ctx: &Ctx) -> HashMap<String, String> {
    ctx.db.with(|c| {
        let mut st = c.prepare("select k, v from kv where k like 'forked:%'")?;
        let v = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.flatten()
            .filter_map(|(k, v)| Some((k[7..].to_string(), serde_json::from_str::<Value>(&v).ok()?["from"].as_str()?.to_string()))).collect();
        Ok(v)
    }).unwrap_or_default()
}

/// Fetch, materialize and launch a session from another computer as a fork, in this computer's clone of its
/// repository. Returns `{ok, cwd, args, note}`.
pub fn resume_here(ctx: &Ctx, device: &str, harness_id: &str, profile: &str, id: &str) -> Result<Value> {
    let (_, s) = ctx.db.remote_sessions()?.into_iter().find(|(d, m)| d == device && m.harness == harness_id && m.profile == profile && m.id == id).context("session not found")?;
    let names = crate::journal::device_names(ctx);
    let reg = ctx.registry()?;
    let roots = roots_here(ctx)?;
    let root = roots.get(&s.project_key).with_context(|| format!("not cloned here: open {} once on this computer", s.project_name))?;
    let h = harness::by_id(harness_id).context("unknown harness")?;
    let p = reg.profiles.iter().find(|p| p.name == profile && p.harness == harness_id).with_context(|| format!("no account {profile} for {} on this computer", h.label()))?;
    if crate::journal::open_elsewhere(&crate::journal::peers(ctx)?, now_ms()).contains(id) {
        bail!("running on {}: wait until it ends", names.get(device).cloned().unwrap_or_else(|| device.into()));
    }
    let t = fetch(ctx, device, harness_id, profile, id)?;
    let m = h.materialize(ctx, p, Path::new(root), &t)?;
    crate::service::start(ctx, profile, Some(m.cwd.clone()), &m.launch_args)?;
    let key = format!("forked:{harness_id}/{profile}/{}", m.new_session_id.as_deref().unwrap_or(id));
    ctx.db.kv_set(&key, &json!({"from": device, "at": now_ms()}).to_string())?;
    Ok(json!({"ok": true, "cwd": m.cwd, "args": m.launch_args, "note": m.note}))
}

/// `{carried, bytes, cap_bytes, skipped: [{key, why}], remote, fetched}`.
pub fn status(ctx: &Ctx) -> Result<Value> {
    ctx.db.with(|c| {
        tables(c)?;
        let (carried, bytes): (i64, i64) = c.query_row("select count(*), coalesce(sum(bytes), 0) from transcripts_out where skipped is null", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let skipped: Vec<Value> = c.prepare("select key, skipped from transcripts_out where skipped is not null order by key")?
            .query_map([], |r| Ok(json!({"key": r.get::<_, String>(0)?, "why": r.get::<_, String>(1)?})))?.collect::<rusqlite::Result<_>>()?;
        let (remote, fetched): (i64, i64) = c.query_row("select count(*), coalesce(sum(fetched_at is not null), 0) from transcripts", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(json!({"carried": carried, "bytes": bytes, "cap_bytes": CAP_BYTES, "skipped": skipped, "remote": remote, "fetched": fetched}))
    })
}

/// The transcript of a session from another computer, through the same parser the tool's own sessions use.
pub fn remote_transcript(ctx: &Ctx, device: &str, profile: &str, id: &str) -> Result<Transcript> {
    let f = crate::service::SessionFilter { profile: Some(profile.into()), ..Default::default() };
    let s = crate::service::sessions(ctx, &f)?.into_iter().find(|s| s.id == id && s.device == device).context("session not found")?;
    let t = fetch(ctx, device, &s.harness, profile, id)?;
    let text = || std::fs::read_to_string(&t.path).with_context(|| format!("read {}", t.path.display()));
    let mut out = match s.harness.as_str() {
        "claude" => harness::claude::parse_session(&text()?, profile, &t.path, true),
        "codex" => harness::codex::parse_rollout(&text()?, profile, &t.path, true),
        "pi" | "omp" => harness::pi::parse_session(&text()?, &s.harness, profile, &t.path, true),
        "opencode" => Transcript { meta: s.clone(), turns: opencode_export_turns(&serde_json::from_str(&text()?).context("OpenCode export")?) },
        other => bail!("sessions from another computer are not readable for {other}"),
    };
    out.meta = s;
    Ok(out)
}

/// The turns of an `opencode session export`: `{messages: [{info: {role, ...}, parts: [...]}]}`.
fn opencode_export_turns(v: &Value) -> Vec<Turn> {
    let mut turns: Vec<Turn> = vec![];
    for m in v["messages"].as_array().into_iter().flatten() {
        let (info, parts) = (&m["info"], m["parts"].as_array().cloned().unwrap_or_default());
        let text = |ps: &[Value]| ps.iter().filter(|x| x["type"] == "text" && x["synthetic"] != true).filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("\n");
        if info["role"] == "user" {
            turns.push(Turn { prompt: text(&parts).trim().into(), prompt_at: info["time"]["created"].as_i64().unwrap_or(0), ..Default::default() });
            continue;
        }
        if turns.is_empty() { turns.push(Turn::default()); }
        let t = turns.last_mut().unwrap();
        t.answer_at = info["time"]["completed"].as_i64().unwrap_or(t.answer_at);
        if t.model.is_empty() { t.model = info["modelID"].as_str().unwrap_or("").into(); }
        for x in &parts {
            match x["type"].as_str().unwrap_or("") {
                "text" => { let tx = x["text"].as_str().unwrap_or("").trim(); if !tx.is_empty() { t.say(tx.into()); } }
                "reasoning" => t.steps.push(Step { kind: "thinking".into(), ..Default::default() }),
                "tool" => {
                    let (tool, st) = (x["tool"].as_str().unwrap_or("tool").to_string(), &x["state"]);
                    let failed = st["status"] == "error";
                    t.steps.push(Step { kind: "tool".into(), input: harness::parse::summarize_input(&tool, &st["input"]), tool, status: if failed { "error".into() } else { "ok".into() },
                        error: if failed { crate::util::truncate(st["error"].as_str().unwrap_or(""), 200) } else { String::new() }, result: harness::parse::summarize_output(st["output"].as_str().unwrap_or("")), ..Default::default() });
                }
                _ => {}
            }
        }
    }
    turns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Harness;
    use crate::registry::{Kind, Profile};
    use crate::sync::transcripts as out;

    fn computer(dir: &Path, name: &str, shared: &Path, transcripts: bool) -> Ctx {
        let data = dir.join(name);
        let mut ctx = Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap();
        ctx.settings.device_name = name.into();
        ctx.settings.sync_mode = "folder".into();
        ctx.settings.sync_folder = shared.display().to_string();
        ctx.settings.sync_transcripts = transcripts;
        ctx.save_settings().unwrap();
        Ctx::open_at(&data, &dir.join(format!("home-{name}"))).unwrap()
    }

    fn profile(name: &str, harness: &str) -> Profile {
        Profile { name: name.into(), label: None, harness: harness.into(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }
    }

    fn add_profile(ctx: &Ctx, name: &str, harness: &str) {
        let mut reg = ctx.registry().unwrap();
        reg.add_profile(profile(name, harness)).unwrap();
        ctx.save_registry(&reg).unwrap();
    }

    /// A Claude Code session file of `len` bytes in A's main home, listed as A's session with the project `key`.
    fn claude_session(a: &Ctx, id: &str, cwd: &Path, key: &str, pad: usize) -> PathBuf {
        let f = a.home.join(".claude").join("projects").join(harness::claude::project_dir_name(cwd)).join(format!("{id}.jsonl"));
        let line = json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": "2026-11-05T10:00:00.000Z", "message": {"role": "user", "content": format!("fix the build {}", "x".repeat(pad))}});
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, format!("{line}\n")).unwrap();
        // the times the parser reads from the file, so a copy listed here later is the same session, not a newer one
        let ts = harness::parse::ts_ms("2026-11-05T10:00:00.000Z");
        let m = SessionMeta { harness: "claude".into(), profile: "claude-main".into(), id: id.into(), title: "fix the build".into(), cwd: cwd.display().to_string(), started_at: ts, last_active_at: ts,
            project_key: key.into(), project_name: "r".into(), source_path: f.display().to_string(), ..Default::default() };
        a.db.journal_session(&m).unwrap();
        f
    }

    fn count(ctx: &Ctx, sql: &str) -> i64 { ctx.db.with(|c| { tables(c)?; Ok(c.query_row(sql, [], |r| r.get(0))?) }).unwrap() }

    /// A repository folder with a remote, so both computers resolve it to the same key.
    fn repo(dir: &Path, name: &str) -> PathBuf {
        let r = dir.join(name);
        std::fs::create_dir_all(r.join(".git")).unwrap();
        std::fs::write(r.join(".git").join("config"), "[remote \"origin\"]\n\turl = git@github.com:example/app.git\n").unwrap();
        r
    }

    #[test]
    fn transcripts_are_carried_on_publish_and_fetched_once_on_demand() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared, true), computer(t.path(), "B", &shared, true));
        add_profile(&a, "claude-main", "claude");
        let cwd_a = repo(t.path(), "app-on-a");
        let src = claude_session(&a, "s1", &cwd_a, "github.com/example/app", 0);
        crate::journal::round(&a).unwrap();
        // the compressed copy and its entry
        let dir = shared.join("devices").join(&a.settings.device_id);
        let carried = dir.join("transcripts").join("claude").join("claude-main").join("s1.jsonl.zst");
        assert!(carried.exists(), "the transcript sits beside the journal");
        assert_eq!(zstd::decode_all(&std::fs::read(&carried).unwrap()[..]).unwrap(), std::fs::read(&src).unwrap());
        let entry: Value = a.db.with(|c| Ok(c.query_row("select data from outbox where op='transcript'", [], |r| r.get::<_, String>(0))?)).map(|s| serde_json::from_str(&s).unwrap()).unwrap();
        assert_eq!(entry["file"], "transcripts/claude/claude-main/s1.jsonl.zst");
        assert_eq!(entry["sha256"], sha256_hex(&std::fs::read(&carried).unwrap()));
        assert_eq!(entry["source_rel"], format!("projects/{}/s1.jsonl", harness::claude::project_dir_name(&cwd_a)).replace('\\', "/"));
        assert_eq!(a.db.kv_get("journal:published_seq").unwrap().as_deref(), Some("2"), "the session and its transcript went out in the same round");
        // unchanged: not carried again
        crate::journal::round(&a).unwrap();
        assert_eq!(count(&a, "select count(*) from outbox where op='transcript'"), 1);
        assert_eq!(status(&a).unwrap()["carried"], 1);
        // B stores the entry on merge and fetches the file only when asked, once
        crate::journal::round(&b).unwrap();
        assert_eq!(count(&b, "select count(*) from transcripts"), 1);
        assert_eq!(count(&b, "select count(*) from transcripts where fetched_at is not null"), 0);
        let f = fetch(&b, &a.settings.device_id, "claude", "claude-main", "s1").unwrap();
        assert!(f.path.starts_with(b.data.join("transcripts")) && f.path.ends_with("s1.jsonl"));
        assert_eq!(std::fs::read(&f.path).unwrap(), std::fs::read(&src).unwrap());
        assert_eq!(status(&b).unwrap()["fetched"], 1);
        std::fs::remove_dir_all(&shared).unwrap();
        assert!(fetch(&b, &a.settings.device_id, "claude", "claude-main", "s1").is_ok(), "the second read is from the fetched copy, not the folder");
        // the transcript read of a remote row goes through Claude Code's parser
        let tr = remote_transcript(&b, &a.settings.device_id, "claude-main", "s1").unwrap();
        assert_eq!((tr.turns.len(), tr.turns[0].prompt.as_str(), tr.meta.device_name.as_str()), (1, "fix the build", "A"));
    }

    #[test]
    fn a_transcript_over_the_cap_is_skipped_and_the_folder_is_pruned_at_the_cap_with_tombstones() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared, true), computer(t.path(), "B", &shared, true));
        add_profile(&a, "claude-main", "claude");
        let cwd = repo(t.path(), "app");
        claude_session(&a, "big", &cwd, "k", MAX_TRANSCRIPT_BYTES as usize + 1);
        claude_session(&a, "old", &cwd, "k", 100);
        claude_session(&a, "new", &cwd, "k", 100);
        out::carry_with_cap(&a, &shared, 1).unwrap();
        let st = status(&a).unwrap();
        assert_eq!(st["skipped"][0]["key"], "claude/claude-main/big", "{st}");
        assert_eq!(st["skipped"][0]["why"], "too big", "{st}");
        assert_eq!(st["carried"], 1, "a 1-byte cap keeps only the newest: {st}");
        let entries: Vec<(String, String, String)> = a.db.with(|c| Ok(c.prepare("select op, key, data from outbox order by seq")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?)).unwrap();
        let ops: Vec<&str> = entries.iter().filter(|e| e.0 != "session").map(|e| e.0.as_str()).collect();
        assert_eq!(ops, ["transcript", "transcript", "transcript", "tombstone"], "{entries:?}");
        let big = entries.iter().find(|e| e.0 == "transcript" && e.1 == "claude/claude-main/big").unwrap();
        assert_eq!(serde_json::from_str::<Value>(&big.2).unwrap()["skipped"], "too big");
        let dir = shared.join("devices").join(&a.settings.device_id).join("transcripts").join("claude").join("claude-main");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the pruned file is gone from the folder");
        // B: the skipped one is never stored, the pruned one is stored then removed by its tombstone
        crate::journal::round(&a).unwrap();
        crate::journal::round(&b).unwrap();
        let keys: Vec<String> = b.db.with(|c| Ok(c.prepare("select key from transcripts order by key")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)).unwrap();
        assert_eq!(keys.len(), 1, "{keys:?}");
    }

    #[test]
    fn materialize_claude_codex_pi_omp_opencode_and_the_antigravity_default() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let root = t.path().join("home").join("Projects").join("app");
        std::fs::create_dir_all(&root).unwrap();
        let src = t.path().join("fetched.jsonl");
        std::fs::write(&src, "{}\n").unwrap();
        let fetched = |harness: &str, id: &str, rel: &str| FetchedTranscript { device: "dev".into(), harness: harness.into(), profile: "main".into(), id: id.into(), path: src.clone(), source_rel: rel.into(), bytes: 3 };
        // Claude Code: one copy under the encoded folder, resumed by path as a fork
        assert_eq!(harness::claude::project_dir_name(Path::new("C:\\Users\\sam\\src\\my.app")), "C--Users-sam-src-my-app");
        assert_eq!(harness::claude::project_dir_name(Path::new("/home/sam/src/app")), "-home-sam-src-app");
        let f = fetched("claude", "abc", "projects/C--laptop-app/abc.jsonl");
        let m = harness::claude::Claude.materialize(&ctx, &profile("main", "claude"), &root, &f).unwrap();
        let copy = t.path().join("home").join(".claude").join("projects").join(harness::claude::project_dir_name(&root)).join("abc.jsonl");
        assert!(copy.exists());
        assert_eq!(m.launch_args, vec!["--resume".to_string(), copy.display().to_string(), "--fork-session".into()]);
        assert_eq!(m.cwd, root);
        std::fs::write(&src, "{\"changed\":1}\n").unwrap();
        harness::claude::Claude.materialize(&ctx, &profile("main", "claude"), &root, &f).unwrap();
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "{}\n", "the copy is reused, never a second one");
        // Codex: the date folder from the rollout's name, resumed with -C
        let f = fetched("codex", "cx-1", "sessions/2026/11/01/rollout-2026-11-01T10-00-00-cx-1.jsonl.zst");
        let m = harness::codex::Codex.materialize(&ctx, &profile("main", "codex"), &root, &f).unwrap();
        assert!(t.path().join("home").join(".codex").join("sessions").join("2026").join("11").join("01").join("rollout-2026-11-01T10-00-00-cx-1.jsonl").exists());
        assert_eq!(m.launch_args, vec!["-C".to_string(), root.display().to_string(), "resume".into(), "cx-1".into()]);
        // Pi and Oh My Pi: the tool's encoded folder, the original name
        let f = fetched("pi", "pi-1", "sessions/--C--laptop-app--/2026-11-05T10-00-00_pi-1.jsonl");
        let m = harness::pi::PI.materialize(&ctx, &profile("main", "pi"), &root, &f).unwrap();
        let pi_copy = harness::pi::PI.home(&ctx, &profile("main", "pi")).join("sessions").join(harness::pi::session_dir_name(&root, None)).join("2026-11-05T10-00-00_pi-1.jsonl");
        assert!(pi_copy.exists(), "{}", pi_copy.display());
        assert!(harness::pi::session_dir_name(&root, None).starts_with("--") && harness::pi::session_dir_name(&root, None).ends_with("-Projects-app--"));
        assert_eq!(m.launch_args, vec!["--session".to_string(), pi_copy.display().to_string()]);
        let m = harness::omp::OMP.materialize(&ctx, &profile("main", "omp"), &root, &f).unwrap();
        assert_eq!(harness::pi::session_dir_name(&root, Some(&ctx.home)), "-Projects-app", "Oh My Pi names a folder under home by its relative path");
        assert!(harness::omp::OMP.home(&ctx, &profile("main", "omp")).join("sessions").join("-Projects-app").join("2026-11-05T10-00-00_pi-1.jsonl").exists());
        assert_eq!(m.launch_args, vec!["--resume".to_string(), "pi-1".into()]);
        // OpenCode: the import command, version 2 first, then version 1; resumed by id
        assert_eq!(harness::opencode::import_args(true, Path::new("x.json"), &root), vec!["session", "import", "x.json", "--directory", &root.display().to_string()]);
        assert_eq!(harness::opencode::import_args(false, Path::new("x.json"), &root), vec!["import", "x.json"]);
        assert!(harness::opencode::OpenCode.resumes_elsewhere());
        // Antigravity: the default
        let err = harness::agy::Agy.materialize(&ctx, &profile("main", "agy"), &root, &fetched("agy", "c1", "")).unwrap_err().to_string();
        assert_eq!(err, "resuming elsewhere is not available for Antigravity");
        assert!(!harness::agy::Agy.resumes_elsewhere());
    }

    #[test]
    fn resume_here_names_its_three_errors_then_launches_each_tool_in_the_local_clone() {
        let t = tempfile::tempdir().unwrap();
        let shared = t.path().join("shared");
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (computer(t.path(), "A", &shared, true), computer(t.path(), "B", &shared, true));
        add_profile(&a, "claude-main", "claude");
        let cwd_a = repo(t.path(), "app-on-a");
        claude_session(&a, "s1", &cwd_a, "github.com/example/app", 0);
        crate::journal::round(&a).unwrap();
        crate::journal::round(&b).unwrap();
        let dev = a.settings.device_id.clone();
        let list = || crate::service::sessions(&b, &Default::default()).unwrap();
        // not cloned here
        let err = resume_here(&b, &dev, "claude", "claude-main", "s1").unwrap_err().to_string();
        assert_eq!(err, "not cloned here: open r once on this computer");
        assert_eq!(list()[0].resumable, Some(json!(err)));
        // cloned: B has usage in its own clone of the same repository
        let cwd_b = repo(&t.path().join("b-side"), "app");
        b.db.insert_usage(&[crate::model::UsageRow { request_id: "claude:b1".into(), profile: "claude-main".into(), harness: "claude".into(), ts: 1, model: "m".into(), project: cwd_b.display().to_string(),
            session_id: "x".into(), input: 1, output: 1, cache_read: 0, cache_write: 0, ttft_ms: None, gen_ms: None }], &Default::default()).unwrap();
        let err = resume_here(&b, &dev, "claude", "claude-main", "s1").unwrap_err().to_string();
        assert_eq!(err, "no account claude-main for Claude Code on this computer");
        assert_eq!(list()[0].resumable, Some(json!(err)));
        add_profile(&b, "claude-main", "claude");
        // running on A: A's presence says so
        b.db.with(|c| { c.execute("update peers set presence=? where device_id=?", params![json!({"at": now_ms(), "running": [{"session_id": "s1"}]}).to_string(), dev])?; Ok(()) }).unwrap();
        let err = resume_here(&b, &dev, "claude", "claude-main", "s1").unwrap_err().to_string();
        assert_eq!(err, "running on A: wait until it ends");
        assert_eq!(list()[0].resumable, Some(json!(err)));
        b.db.with(|c| { c.execute("update peers set presence=null where device_id=?", [&dev])?; Ok(()) }).unwrap();
        assert_eq!(list()[0].resumable, Some(json!(true)));
        // launched: the terminal record carries hrm run <profile> -- <the tool's args>, in the clone
        let log = t.path().join("terminal.jsonl");
        std::env::set_var("HRM_TERMINAL_LOG", &log);
        let r = resume_here(&b, &dev, "claude", "claude-main", "s1").unwrap();
        let rec: Value = serde_json::from_str(std::fs::read_to_string(&log).unwrap().lines().last().unwrap()).unwrap();
        let copy = b.home.join(".claude").join("projects").join(harness::claude::project_dir_name(&cwd_b)).join("s1.jsonl");
        assert_eq!(rec["args"], json!(["run", "claude-main", "--", "--resume", copy.display().to_string(), "--fork-session"]));
        assert_eq!(rec["cwd"], json!(cwd_b.display().to_string()));
        assert_eq!(r["cwd"], json!(cwd_b.display().to_string()));
        assert_eq!(b.db.kv_get("forked:claude/claude-main/s1").unwrap().map(|v| serde_json::from_str::<Value>(&v).unwrap()["from"].clone()), Some(json!(dev)));
        // the local copy is listed as this computer's session, continued from A; a second Resume here reuses it
        let local = list().into_iter().find(|s| s.device == b.db.device && s.id == "s1").expect("the copy is a session here");
        assert_eq!(local.continued_from.as_deref(), Some("A"));
        resume_here(&b, &dev, "claude", "claude-main", "s1").unwrap();
        assert_eq!(std::fs::read_dir(copy.parent().unwrap()).unwrap().count(), 1);
        std::env::remove_var("HRM_TERMINAL_LOG");
    }
}
