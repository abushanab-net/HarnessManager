//! The archive: a session of this computer older than `archive_days`, not open anywhere, whose transcript the
//! tool keeps as a file (Claude Code, Codex, Pi, Oh My Pi), has that file compressed into every enabled archive
//! location (`<sync_folder>/archive/<device_id>/` and a folder of the owner's choice), each copy checked by its
//! sha256, and only then deleted from the tool's folder. The session stays listed with its numbers, readable from
//! the archive; it cannot be resumed from there. Usage rows are never touched.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::harness::{self, Harness};
use crate::model::{SessionMeta, Transcript};
use crate::projects::Resolver;
use crate::registry::Profile;
use crate::util::{now_ms, sha256_hex, walk_files, write_atomic};
use crate::Ctx;

/// What Resume says on an archived session.
pub const REFUSAL: &str = "archived: it can be read, not resumed";

/// The archived sessions, made on first use: the journal key, the session's meta (`source_path` = the first copy),
/// where that copy is and how big.
pub fn tables(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("create table if not exists archived(key text primary key, harness text not null, profile text not null, id text not null,
        device text not null, json text not null, path text not null, bytes integer not null, at integer not null);")
}

/// The enabled locations: the sync folder's `archive/<device_id>`, then the chosen folder.
pub fn roots(ctx: &Ctx) -> Vec<PathBuf> {
    let s = &ctx.settings;
    let mut v = vec![];
    if s.archive_in_sync_folder && s.sync_mode == "folder" && !s.sync_folder.is_empty() && Path::new(&s.sync_folder).is_dir() {
        v.push(Path::new(&s.sync_folder).join("archive").join(&s.device_id));
    }
    if !s.archive_dir.is_empty() { v.push(PathBuf::from(&s.archive_dir)); }
    v
}

/// A folder or file name from a project name, account or id: nothing a file system refuses.
fn safe(s: &str) -> String {
    let t: String = s.trim().chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '-' } else { c }).collect();
    let t = t.trim_matches(['.', ' ', '-']).to_string();
    if t.is_empty() { "no-folder".into() } else { t }
}

/// `<project name or no-folder>/<YYYY-MM>/<harness>-<profile>-<id>.jsonl.zst` under a root; the month is the
/// session's last activity in this computer's zone.
fn rel_path(s: &SessionMeta) -> PathBuf {
    let month = chrono::DateTime::from_timestamp_millis(s.last_active_at).unwrap_or_default().with_timezone(&chrono::Local).format("%Y-%m").to_string();
    let project = if s.cwd.trim().is_empty() { "no-folder".to_string() } else { safe(&s.project_name) };
    PathBuf::from(project).join(month).join(format!("{}-{}-{}.jsonl.zst", s.harness, safe(&s.profile), safe(&s.id)))
}

/// Tools that keep a transcript as a file the archive can move.
fn archivable(harness: &str) -> bool { matches!(harness, "claude" | "codex" | "pi" | "omp") }

/// One session into every root, verified, then out of the tool's folder and into the table. Returns the bytes of a copy.
fn archive_one(ctx: &Ctx, h: &dyn Harness, p: &Profile, s: &SessionMeta, roots: &[PathBuf]) -> Result<i64> {
    let files = h.archive_paths(ctx, p, s);
    let src = files.first().context("no transcript file")?;
    // Codex keeps older rollouts compressed already: every archive copy is zstd of the log itself, as the folder carries it
    let packed = zstd::encode_all(harness::parse::read_text(src)?.as_bytes(), 3).context("compress the transcript")?;
    let sha = sha256_hex(&packed);
    let rel = rel_path(s);
    let mut meta = s.clone();
    meta.archived = true;
    meta.open = false;
    meta.open_elsewhere = false;
    let mut first: Option<PathBuf> = None;
    for root in roots {
        let out = root.join(&rel);
        write_atomic(&out, &packed)?;
        // read back before anything is deleted: a copy that does not match is not a copy
        if sha256_hex(&std::fs::read(&out)?) != sha { bail!("the copy at {} does not read back as written", out.display()); }
        let side = out.with_file_name(format!("{}-{}-{}.json", s.harness, safe(&s.profile), safe(&s.id)));
        let info = json!({"session": meta, "sha256": sha, "bytes": packed.len(), "archived_at": now_ms(), "device": ctx.db.device, "device_name": ctx.settings.device_name, "original": s.source_path});
        write_atomic(&side, serde_json::to_string_pretty(&info)?.as_bytes())?;
        first.get_or_insert(out);
    }
    let first = first.context("no archive location")?;
    for f in &files {
        if f.is_file() { std::fs::remove_file(f).with_context(|| format!("remove {}", f.display()))?; }
    }
    meta.source_path = first.display().to_string();
    let key = crate::db::session_key(s);
    ctx.db.with(|c| {
        tables(c)?;
        c.execute("insert or replace into archived(key, harness, profile, id, device, json, path, bytes, at) values(?,?,?,?,?,?,?,?,?)",
            params![key, s.harness, s.profile, s.id, ctx.db.device, serde_json::to_string(&meta)?, meta.source_path, packed.len() as i64, now_ms()])?;
        // the scan's cache row of the file that is gone
        c.execute("delete from sessions where key=?", [format!("{}|{}|{}", s.harness, s.profile, s.source_path)])?;
        Ok(())
    })?;
    Ok(packed.len() as i64)
}

/// The step: every session of this computer older than `days` by its last activity, not open here or elsewhere,
/// of a tool that keeps transcripts as files. Returns `{archived, bytes}` (and `why` when nothing could run).
pub fn run(ctx: &Ctx, days: u32) -> Result<Value> {
    if days == 0 { return Ok(json!({"archived": 0, "bytes": 0, "why": "archiving is off"})); }
    let roots = roots(ctx);
    if roots.is_empty() { return Ok(json!({"archived": 0, "bytes": 0, "why": "no archive location: turn on the sync folder or choose a folder"})); }
    let reg = ctx.registry()?;
    let mut res = Resolver::new(&reg.projects);
    let cutoff = now_ms() - days as i64 * 86_400_000;
    let elsewhere = crate::journal::open_elsewhere(&crate::journal::peers(ctx).unwrap_or_default(), now_ms());
    let (mut n, mut bytes) = (0usize, 0i64);
    for p in reg.profiles.iter().filter(|p| archivable(&p.harness)) {
        let h = harness::of(p);
        let list = match h.sessions(ctx, p, 0) {
            Ok(l) => l,
            Err(e) => { crate::log::warn(&format!("archive: sessions for {}: {e:#}", p.name)); continue; }
        };
        // open: a console names it, or it is among the newest for consoles that name none (as the session list reckons)
        let running = h.running(ctx, p);
        let mut recent: Vec<&SessionMeta> = list.iter().collect();
        recent.sort_by_key(|s| std::cmp::Reverse(s.last_active_at));
        let open: Vec<String> = running.iter().filter_map(|r| r.session_id.clone())
            .chain(recent.iter().take(running.iter().filter(|r| r.session_id.is_none()).count()).map(|s| s.id.clone())).collect();
        for mut s in list {
            if s.last_active_at >= cutoff || s.id.is_empty() || open.contains(&s.id) || elsewhere.contains(&s.id) { continue; }
            let info = res.resolve(&s.cwd);
            s.project_key = info.key;
            s.project_name = info.name;
            match archive_one(ctx, h, p, &s, &roots) {
                Ok(b) => { n += 1; bytes += b; }
                Err(e) => crate::log::warn(&format!("archive: {}/{}/{}: {e:#}", s.harness, s.profile, s.id)),
            }
        }
    }
    ctx.db.kv_set("archive:last", &json!({"at": now_ms(), "archived": n, "bytes": bytes}).to_string())?;
    if n > 0 {
        crate::log::info(&format!("archive: {n} sessions moved ({:.1} MB)", bytes as f64 / 1_048_576.0));
        let _ = ctx.db.event("ok", "archive", &format!("{n} {} older than {days} days moved to the archive.", if n == 1 { "session" } else { "sessions" }));
    }
    Ok(json!({"archived": n, "bytes": bytes}))
}

/// Archive now, with the setting's age.
pub fn archive_now(ctx: &Ctx) -> Result<Value> { run(ctx, ctx.settings.archive_days) }

/// The daily step: once a day while the app runs, with the setting's age; 0 means never.
pub fn daily(ctx: &Ctx) -> Result<()> {
    if ctx.settings.archive_days == 0 { return Ok(()); }
    let last = ctx.db.kv_get("archive:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).and_then(|v| v["at"].as_i64()).unwrap_or(0);
    if now_ms() - last < 86_400_000 { return Ok(()); }
    run(ctx, ctx.settings.archive_days).map(|_| ())
}

/// The archived sessions, every computer's, as session rows: `archived`, the computer, and (from another computer)
/// why they cannot be resumed.
pub fn sessions(ctx: &Ctx) -> Vec<SessionMeta> {
    let names = crate::journal::device_names(ctx);
    let rows: Vec<(String, String)> = ctx.db.with(|c| {
        tables(c)?;
        let mut st = c.prepare("select device, json from archived")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }).unwrap_or_default();
    rows.into_iter().filter_map(|(dev, j)| {
        let mut m: SessionMeta = serde_json::from_str(&j).ok()?;
        m.archived = true;
        m.open = false;
        m.open_elsewhere = false;
        m.device_name = names.get(&dev).cloned().unwrap_or_else(|| dev.clone());
        m.resumable = (dev != ctx.db.device).then(|| json!(REFUSAL));
        m.device = dev;
        Some(m)
    }).collect()
}

/// A session of this account and id is in the archive.
pub fn is_archived(ctx: &Ctx, profile: &str, id: &str) -> bool {
    ctx.db.with(|c| { tables(c)?; Ok(c.prepare("select 1 from archived where profile=? and id=?")?.exists([profile, id])?) }).unwrap_or(false)
}

/// The archived copy, through the same parser the tool's own sessions use.
pub fn transcript(ctx: &Ctx, s: &SessionMeta) -> Result<Transcript> {
    let _ = ctx;
    let path = Path::new(&s.source_path);
    let text = harness::parse::read_text(path).with_context(|| format!("read the archived copy {}", path.display()))?;
    let mut t = match s.harness.as_str() {
        "claude" => harness::claude::parse_session(&text, &s.profile, path, true),
        "codex" => harness::codex::parse_rollout(&text, &s.profile, path, true),
        "pi" | "omp" => harness::pi::parse_session(&text, &s.harness, &s.profile, path, true),
        other => bail!("archived sessions are not readable for {other}"),
    };
    t.meta = s.clone();
    Ok(t)
}

/// `{days, archived, bytes, last: {at, archived, bytes} | null, locations: [{path, kind, sessions, bytes}]}`:
/// each location counted from its files, so both say what they hold.
pub fn status(ctx: &Ctx) -> Result<Value> {
    let last = ctx.db.kv_get("archive:last")?.and_then(|v| serde_json::from_str::<Value>(&v).ok()).unwrap_or(Value::Null);
    let (n, bytes): (i64, i64) = ctx.db.with(|c| { tables(c)?; Ok(c.query_row("select count(*), coalesce(sum(bytes), 0) from archived", [], |r| Ok((r.get(0)?, r.get(1)?)))?) })?;
    let s = &ctx.settings;
    let sync = (s.sync_mode == "folder" && !s.sync_folder.is_empty()).then(|| Path::new(&s.sync_folder).join("archive").join(&s.device_id));
    let locations: Vec<Value> = [(sync, "sync_folder", s.archive_in_sync_folder), ((!s.archive_dir.is_empty()).then(|| PathBuf::from(&s.archive_dir)), "folder", true)].into_iter()
        .filter_map(|(p, kind, on)| {
            let p = p?;
            let mut files = vec![];
            walk_files(&p, &|f| f.extension().map(|e| e == "zst").unwrap_or(false), &mut files);
            let bytes: u64 = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum();
            Some(json!({"path": p, "kind": kind, "on": on, "sessions": files.len(), "bytes": bytes}))
        }).collect();
    Ok(json!({"days": s.archive_days, "archived": n, "bytes": bytes, "last": last, "locations": locations}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Kind;

    fn profile(name: &str, harness: &str) -> Profile {
        Profile { name: name.into(), label: None, harness: harness.into(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }
    }

    /// A Claude Code session file in the main home, last active at `stamp`.
    fn claude_session(ctx: &Ctx, id: &str, cwd: &Path, stamp: &str) -> PathBuf {
        let f = ctx.home.join(".claude").join("projects").join(harness::claude::project_dir_name(cwd)).join(format!("{id}.jsonl"));
        let lines = [json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": stamp, "message": {"role": "user", "content": format!("fix the build in {id}")}}),
            json!({"type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": stamp, "message": {"id": format!("m-{id}"), "model": "claude-opus-5-5", "usage": {"input_tokens": 10, "output_tokens": 5}, "content": [{"type": "text", "text": "done"}]}})];
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        f
    }

    #[test]
    fn the_archive_moves_only_old_closed_sessions_into_both_locations_keeps_them_listed_and_readable_and_respects_never() {
        let t = tempfile::tempdir().unwrap();
        let (shared, chosen) = (t.path().join("shared"), t.path().join("chosen"));
        std::fs::create_dir_all(&shared).unwrap();
        let mut ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        ctx.settings.sync_mode = "folder".into();
        ctx.settings.sync_folder = shared.display().to_string();
        ctx.settings.archive_dir = chosen.display().to_string();
        ctx.settings.archive_days = 0;
        ctx.save_settings().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let mut reg = ctx.registry().unwrap();
        reg.add_profile(profile("claude-main", "claude")).unwrap();
        ctx.save_registry(&reg).unwrap();
        let cwd = t.path().join("app");
        std::fs::create_dir_all(&cwd).unwrap();
        let old = claude_session(&ctx, "old", &cwd, "2026-01-10T10:00:00.000Z");
        let fresh = claude_session(&ctx, "fresh", &cwd, &chrono::Utc::now().to_rfc3339());
        // never: nothing moves
        assert_eq!(archive_now(&ctx).unwrap()["archived"], 0);
        assert!(old.exists() && fresh.exists());
        // 30 days: the old one goes, the fresh one stays
        let r = run(&ctx, 30).unwrap();
        assert_eq!(r["archived"], 1, "{r}");
        assert!(!old.exists() && fresh.exists());
        let rel = Path::new("app").join("2026-01").join("claude-claude-main-old.jsonl.zst");
        let (a, b) = (shared.join("archive").join(&ctx.settings.device_id).join(&rel), chosen.join(&rel));
        assert!(a.is_file() && b.is_file(), "both locations hold the copy");
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        assert!(a.with_file_name("claude-claude-main-old.json").is_file(), "the meta sits beside it");
        assert!(zstd::decode_all(&std::fs::read(&a).unwrap()[..]).unwrap().starts_with(b"{\"type\":\"user\""));
        // listed, archived, with its numbers; readable; not resumable
        let list = crate::service::sessions(&ctx, &Default::default()).unwrap();
        let s = list.iter().find(|s| s.id == "old").expect("still listed");
        assert!(s.archived && s.prompts == 1 && s.source_path == a.display().to_string(), "{s:?}");
        assert!(!list.iter().find(|s| s.id == "fresh").unwrap().archived);
        let tr = crate::service::transcript(&ctx, "claude-main", "old").unwrap();
        assert_eq!((tr.turns.len(), tr.turns[0].prompt.as_str(), tr.meta.archived), (1, "fix the build in old", true));
        assert_eq!(crate::service::resume(&ctx, "claude-main", "old", None).unwrap_err().to_string(), REFUSAL);
        let st = status(&ctx).unwrap();
        assert_eq!((st["archived"].as_i64(), st["locations"][0]["sessions"].as_i64(), st["locations"][1]["sessions"].as_i64()), (Some(1), Some(1), Some(1)), "{st}");
        // again: nothing more to move
        assert_eq!(run(&ctx, 30).unwrap()["archived"], 0);
    }
}
