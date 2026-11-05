//! The session index: the `sessions` table as the pages read it. This is the only code that walks the tools'
//! folders. A refresh runs the harness scans (which keep their per-file cache rows), resolves each session's
//! project, marks what is open here, journals this computer's sessions (the phase 1 rules), keeps the promoted
//! columns and the project table in step, and records `index:refreshed_at`. A page open is then one query.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};

use crate::harness::{self, Harness};
use crate::model::SessionMeta;
use crate::projects::{ProjectInfo, Resolver};
use crate::registry::{Profile, Registry};
use crate::util::now_ms;
use crate::Ctx;

/// What to refresh: every account's folders (the periodic walk), or the accounts the changed files belong to
/// (what the watcher saw; the scans re-read only files whose size or time moved).
pub enum Scope { All, Paths(Vec<PathBuf>) }

#[derive(Debug, Default, Clone, Serialize)]
pub struct Report {
    pub profiles: usize,
    pub sessions: usize,
    /// Index rows written (new or changed) and removed (their file is gone).
    pub changed: usize,
    pub removed: usize,
    pub ms: u128,
}

// one refresh at a time in this process: the watcher, the walk and a first page open may ask together
static ONE: Mutex<()> = Mutex::new(());

/// The index key of this computer's session: the scan's cache key (its file) for tools that keep files, the id
/// for OpenCode, whose sessions share one database.
pub fn local_key(m: &SessionMeta) -> String {
    if m.harness == "opencode" { format!("{}|{}|{}", m.harness, m.profile, m.id) } else { format!("{}|{}|{}", m.harness, m.profile, m.source_path) }
}

/// The accounts a changed file belongs to: those with a session folder above it.
fn owners<'a>(ctx: &Ctx, reg: &'a Registry, paths: &[PathBuf]) -> Vec<&'a Profile> {
    reg.profiles.iter().filter(|p| { let dirs = harness::of(p).session_dirs(ctx, p); paths.iter().any(|f| dirs.iter().any(|d| f.starts_with(d))) }).collect()
}

pub fn refresh(ctx: &Ctx, scope: Scope) -> Result<Report> {
    let reg = ctx.registry()?;
    let names: Vec<String> = match &scope {
        Scope::All => reg.profiles.iter().map(|p| p.name.clone()).collect(),
        Scope::Paths(paths) => owners(ctx, &reg, paths).into_iter().map(|p| p.name.clone()).collect(),
    };
    refresh_profiles(ctx, &reg, &names, matches!(scope, Scope::All))
}

/// The refresh of some accounts; `all` records `index:refreshed_at` (a partial refresh leaves it).
pub(crate) fn refresh_profiles(ctx: &Ctx, reg: &Registry, names: &[String], all: bool) -> Result<Report> {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let t = Instant::now();
    struct Now;
    impl Drop for Now { fn drop(&mut self) { harness::parse::PARSE_NOW.store(false, std::sync::atomic::Ordering::Relaxed); } }
    // the walk parses a changed file now; a partial refresh keeps 1.6.2's way (the cached record with the file's time,
    // the parse in the background), so a rollout being written is not re-read whole on every burst of writes
    harness::parse::PARSE_NOW.store(all, std::sync::atomic::Ordering::Relaxed);
    let _now = Now;
    let mut res = Resolver::new(&reg.projects);
    let mut rep = Report::default();
    let mut to_journal: Vec<SessionMeta> = vec![];
    let mut infos: HashMap<String, ProjectInfo> = HashMap::new();
    for p in reg.profiles.iter().filter(|p| names.contains(&p.name)) {
        let h = harness::of(p);
        let list = match h.sessions(ctx, p, 0) {
            Ok(l) => l,
            Err(e) => { crate::log::warn(&format!("index: sessions for {}: {e:#}", p.name)); continue; }
        };
        let open = open_ids(h, ctx, p, &list);
        rep.profiles += 1;
        rep.sessions += list.len();
        reconcile(ctx, h, p, list, &open, &mut res, &mut infos, &mut to_journal, &mut rep)?;
    }
    // the other computers learn of this one's sessions here, once their projects are known
    if let Err(e) = ctx.db.journal_sessions(&to_journal) { crate::log::warn(&format!("journal: sessions: {e:#}")); }
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        for i in infos.values() { tx.execute("insert or replace into project_index(key, name, root, remote) values(?,?,?,?)", params![i.key, i.name, i.root, i.remote])?; }
        if all { tx.execute("insert or replace into kv values('index:refreshed_at', ?)", [now_ms().to_string()])?; }
        tx.commit()?;
        Ok(())
    })?;
    rep.ms = t.elapsed().as_millis();
    if rep.changed + rep.removed > 0 { crate::log::info(&format!("index: {} accounts, {} sessions, {} written, {} removed, {} ms", rep.profiles, rep.sessions, rep.changed, rep.removed, rep.ms)); }
    Ok(rep)
}

/// The sessions the open consoles are working in: the ones they name, and for consoles that name none, the most
/// recently active ones.
fn open_ids(h: &dyn Harness, ctx: &Ctx, p: &Profile, list: &[SessionMeta]) -> HashSet<String> {
    let running = h.running(ctx, p);
    if running.is_empty() { return HashSet::new(); }
    let mut recent: Vec<&SessionMeta> = list.iter().collect();
    recent.sort_by_key(|s| std::cmp::Reverse(s.last_active_at));
    let unnamed = running.iter().filter(|r| r.session_id.is_none()).count();
    running.iter().filter_map(|r| r.session_id.clone()).chain(recent.iter().take(unnamed).map(|s| s.id.clone())).collect()
}

/// One account's index rows against what its scan listed: rows written only where something differs (the JSON,
/// the project, `open`), rows whose session is gone removed, and the scan's cache signature left alone.
#[allow(clippy::too_many_arguments)]
fn reconcile(ctx: &Ctx, h: &dyn Harness, p: &Profile, list: Vec<SessionMeta>, open: &HashSet<String>, res: &mut Resolver, infos: &mut HashMap<String, ProjectInfo>, to_journal: &mut Vec<SessionMeta>, rep: &mut Report) -> Result<()> {
    let prefix = format!("{}|{}|", h.id(), p.name);
    let existing: HashMap<String, (String, String, String, bool)> = ctx.db.with(|c| {
        let mut st = c.prepare("select key, json, project_key, project_name, open from sessions where key >= ?1 and key < ?2 and device = ?3")?;
        let rows = st.query_map(params![prefix, crate::db::prefix_end(&prefix), ctx.db.device], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0))))?;
        Ok(rows.flatten().collect())
    })?;
    let mut seen: HashSet<String> = HashSet::new();
    let (device, device_name) = (ctx.db.device.clone(), ctx.settings.device_name.clone());
    ctx.db.with(|c| {
        let tx = c.transaction()?;
        for m in list {
            let key = local_key(&m);
            let info = res.resolve(&m.cwd);
            infos.entry(info.key.clone()).or_insert_with(|| info.clone());
            let is_open = open.contains(&m.id);
            let json = serde_json::to_string(&m)?;
            match existing.get(&key) {
                Some((j, pk, pn, op)) if *j == json && *pk == info.key && *pn == info.name && *op == is_open => {}
                Some(_) => {
                    tx.execute("update sessions set json=?, id=?, title=?, cwd=?, started_at=?, last_active_at=?, tokens=?, project_key=?, project_name=?, open=? where key=?",
                        params![json, m.id, m.title, m.cwd, m.started_at, m.last_active_at, m.tokens, info.key, info.name, is_open as i64, key])?;
                    rep.changed += 1;
                }
                None => {
                    tx.execute("insert into sessions(key, profile, harness, path, mtime, len, json, device, id, title, cwd, started_at, last_active_at, tokens, project_key, project_name, open) values(?,?,?,?,0,0,?,?,?,?,?,?,?,?,?,?,?)",
                        params![key, m.profile, m.harness, m.source_path, json, device, m.id, m.title, m.cwd, m.started_at, m.last_active_at, m.tokens, info.key, info.name, is_open as i64])?;
                    rep.changed += 1;
                }
            }
            seen.insert(key);
            let mut j = m;
            j.project_key = info.key;
            j.project_name = info.name;
            j.open = is_open;
            j.device = device.clone();
            j.device_name = device_name.clone();
            to_journal.push(j);
        }
        for k in existing.keys().filter(|k| !seen.contains(*k)) {
            tx.execute("delete from sessions where key=?", [k])?;
            rep.removed += 1;
        }
        tx.commit()?;
        Ok(())
    })
}

/// The `open` column of this computer's rows from the consoles running now (the presence tick calls this once a
/// minute, so a session whose console closed stops showing as live without a file change).
pub fn mark_open(ctx: &Ctx) -> Result<()> {
    let reg = ctx.registry()?;
    for p in &reg.profiles {
        let h = harness::of(p);
        let running = h.running(ctx, p);
        let unnamed = running.iter().filter(|r| r.session_id.is_none()).count();
        let mut ids: Vec<String> = running.iter().filter_map(|r| r.session_id.clone()).collect();
        ctx.db.with(|c| {
            if unnamed > 0 {
                let mut st = c.prepare("select id from sessions where device=?1 and harness=?2 and profile=?3 and instr(key, '|') > 0 order by last_active_at desc limit ?4")?;
                ids.extend(st.query_map(params![ctx.db.device, h.id(), p.name, unnamed as i64], |r| r.get::<_, String>(0))?.flatten());
            }
            let set = json!(ids).to_string();
            c.execute("update sessions set open = (id in (select value from json_each(?1))) where device=?2 and harness=?3 and profile=?4 and instr(key, '|') > 0 and open <> (id in (select value from json_each(?1)))",
                params![set, ctx.db.device, h.id(), p.name])?;
            Ok(())
        })?;
    }
    Ok(())
}

/// A refresh of every account when the last one is older than `max_age_ms` (the CLI, which has no background walk).
pub fn refresh_if_older(ctx: &Ctx, max_age_ms: i64) -> Result<()> {
    let at: i64 = ctx.db.kv_get("index:refreshed_at")?.and_then(|s| s.parse().ok()).unwrap_or(0);
    if now_ms() - at > max_age_ms { refresh(ctx, Scope::All)?; }
    Ok(())
}

/// Every session read again: this computer's index rows go (the scans' cache with them), the search table is
/// rebuilt, and a full refresh follows.
pub fn rebuild(ctx: &Ctx) -> Result<Report> {
    ctx.db.with(|c| {
        c.execute("delete from sessions where device = ?1 and instr(key, '|') > 0", [&ctx.db.device])?;
        c.execute("delete from project_index", [])?;
        c.execute_batch("insert into sessions_fts(sessions_fts) values('rebuild')")?;
        Ok(())
    })?;
    refresh(ctx, Scope::All)
}

/// `hrm index status`: rows per tool, the other computers' and archived rows, the last full refresh, the watcher.
pub fn status(ctx: &Ctx) -> Result<Value> {
    let refreshed_at: Option<i64> = ctx.db.kv_get("index:refreshed_at")?.and_then(|s| s.parse().ok());
    let watcher_at: Option<i64> = ctx.db.kv_get("index:watcher_at")?.and_then(|s| s.parse().ok());
    let (tools, remote, archived, projects) = ctx.db.with(|c| {
        let mut st = c.prepare("select harness, count(*), max(last_active_at) from sessions where device = ?1 and instr(key, '|') > 0 group by harness order by harness")?;
        let tools: Vec<Value> = st.query_map([&ctx.db.device], |r| Ok(json!({"harness": r.get::<_, String>(0)?, "sessions": r.get::<_, i64>(1)?, "last_active_at": r.get::<_, i64>(2)?})))?.collect::<rusqlite::Result<_>>()?;
        let remote: i64 = c.query_row("select count(*) from sessions where device <> ?1", [&ctx.db.device], |r| r.get(0))?;
        let archived: i64 = c.query_row("select count(*) from archived", [], |r| r.get(0))?;
        let projects: i64 = c.query_row("select count(*) from project_index", [], |r| r.get(0))?;
        Ok((tools, remote, archived, projects))
    })?;
    Ok(json!({
        "sessions": tools.iter().map(|t| t["sessions"].as_i64().unwrap_or(0)).sum::<i64>(),
        "tools": tools, "remote": remote, "archived": archived, "projects": projects, "refreshed_at": refreshed_at,
        // the watcher runs in the app; it writes its heartbeat once a minute
        "watcher": watcher_at.map(|t| now_ms() - t < 180_000).unwrap_or(false), "watcher_at": watcher_at,
    }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::service::{sessions, sessions_total, SessionFilter};
    use std::path::Path;

    pub(crate) fn computer(dir: &Path) -> Ctx {
        let ctx = Ctx::open_at(&dir.join("data"), &dir.join("home")).unwrap();
        let mut reg = ctx.registry().unwrap();
        reg.add_profile(Profile { name: "claude-main".into(), label: None, harness: "claude".into(), kind: crate::registry::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 }).unwrap();
        ctx.save_registry(&reg).unwrap();
        std::fs::create_dir_all(ctx.home.join(".claude").join("projects").join("p")).unwrap();
        ctx
    }

    /// A Claude session file with one prompt (its title), active at `at` (an RFC 3339 instant).
    pub(crate) fn claude_file(ctx: &Ctx, id: &str, title: &str, at: &str) -> PathBuf {
        let f = ctx.home.join(".claude").join("projects").join("p").join(format!("{id}.jsonl"));
        let cwd = ctx.home.join("work");
        let line = json!({"type": "user", "sessionId": id, "cwd": cwd, "timestamp": at, "message": {"role": "user", "content": title}});
        std::fs::write(&f, format!("{line}\n")).unwrap();
        f
    }

    fn ids(ctx: &Ctx, f: &SessionFilter) -> Vec<String> { sessions(ctx, f).unwrap().into_iter().map(|s| s.id).collect() }

    fn rows(ctx: &Ctx) -> Vec<(String, String)> {
        ctx.db.with(|c| Ok(c.prepare("select key, json from sessions where device = ?1 and instr(key, '|') > 0 order by key")?.query_map([&ctx.db.device], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?)).unwrap()
    }

    #[test]
    fn the_index_lists_what_the_scan_lists_pages_with_totals_and_searches_a_word_of_the_title() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        claude_file(&ctx, "s1", "Add a Longhorn backup target", "2026-11-05T10:00:00.000Z");
        claude_file(&ctx, "s2", "Rotate the staging certificates", "2026-11-05T11:00:00.000Z");
        claude_file(&ctx, "s3", "Bump ArgoCD", "2026-11-05T12:00:00.000Z");
        let r = refresh(&ctx, Scope::All).unwrap();
        assert_eq!((r.profiles, r.sessions), (1, 3));
        let reg = ctx.registry().unwrap();
        let mut scanned: Vec<String> = harness::of(&reg.profiles[0]).sessions(&ctx, &reg.profiles[0], 0).unwrap().into_iter().map(|s| s.id).collect();
        scanned.sort();
        let mut listed = ids(&ctx, &SessionFilter::default());
        assert_eq!(listed, vec!["s3", "s2", "s1"], "newest first");
        listed.sort();
        assert_eq!(listed, scanned, "the index answers the scan's list");
        // a second refresh writes nothing
        let r = refresh(&ctx, Scope::All).unwrap();
        assert_eq!((r.changed, r.removed), (0, 0));
        // paging and totals
        let page = SessionFilter { per: Some(2), page: Some(0), ..Default::default() };
        assert_eq!(ids(&ctx, &page), vec!["s3", "s2"]);
        assert_eq!(sessions_total(&ctx, &page).unwrap(), 3);
        assert_eq!(ids(&ctx, &SessionFilter { page: Some(1), ..page.clone() }), vec!["s1"]);
        // the search: a word of the title (FTS5), case apart, and a prefix of one
        assert_eq!(ids(&ctx, &SessionFilter { query: Some("longhorn".into()), ..Default::default() }), vec!["s1"]);
        assert_eq!(ids(&ctx, &SessionFilter { query: Some("certif".into()), ..Default::default() }), vec!["s2"]);
        assert_eq!(ids(&ctx, &SessionFilter { query: Some("nothing-of-the-kind".into()), ..Default::default() }), Vec::<String>::new());
        // the project was resolved into the index and the project table
        let s = &sessions(&ctx, &SessionFilter::default()).unwrap()[0];
        assert!(!s.project_key.is_empty() && s.project_name == "work" && s.device == ctx.db.device, "{s:?}");
        assert_eq!(status(&ctx).unwrap()["projects"], 1);
    }

    #[test]
    fn a_changed_file_is_re_read_by_paths_and_nothing_else_moves() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        claude_file(&ctx, "s1", "one", "2026-11-05T10:00:00.000Z");
        let f2 = claude_file(&ctx, "s2", "two", "2026-11-05T11:00:00.000Z");
        refresh(&ctx, Scope::All).unwrap();
        let before = rows(&ctx);
        // a later turn appended: the file's size and time move
        let line = json!({"type": "user", "sessionId": "s2", "cwd": ctx.home.join("work"), "timestamp": "2026-11-06T09:00:00.000Z", "message": {"role": "user", "content": "and then"}});
        std::fs::write(&f2, format!("{}{line}\n", std::fs::read_to_string(&f2).unwrap())).unwrap();
        let r = refresh(&ctx, Scope::Paths(vec![f2.clone()])).unwrap();
        assert_eq!((r.profiles, r.sessions, r.removed), (1, 2, 0), "{r:?}");
        let after = rows(&ctx);
        assert_eq!(before.len(), after.len());
        for (b, a) in before.iter().zip(after.iter()) {
            if b.0.ends_with("s2.jsonl") { assert_ne!(b.1, a.1, "the changed file's row was re-read"); } else { assert_eq!(b, a, "an untouched row is as it was"); }
        }
        // the row carries the file's new time at once; the full parse lands from the background within a moment
        let s2 = sessions(&ctx, &SessionFilter { id: Some("s2".into()), ..Default::default() }).unwrap().remove(0);
        assert!(s2.last_active_at > harness::parse::ts_ms("2026-11-05T11:00:00.000Z"), "{s2:?}");
        let t0 = Instant::now();
        let parsed = loop {
            let s2 = sessions(&ctx, &SessionFilter { id: Some("s2".into()), ..Default::default() }).unwrap().remove(0);
            if s2.prompts == 2 || t0.elapsed().as_secs() > 5 { break s2; }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert_eq!((parsed.prompts, parsed.last_active_at), (2, harness::parse::ts_ms("2026-11-06T09:00:00.000Z")));
        // a path under no account's folder refreshes nothing
        assert_eq!(refresh(&ctx, Scope::Paths(vec![t.path().join("elsewhere.jsonl")])).unwrap().profiles, 0);
    }

    #[test]
    fn a_removed_file_drops_its_row_and_swapped_fixtures_converge() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        let f1 = claude_file(&ctx, "s1", "one", "2026-11-05T10:00:00.000Z");
        let f2 = claude_file(&ctx, "s2", "two", "2026-11-05T11:00:00.000Z");
        refresh(&ctx, Scope::All).unwrap();
        std::fs::remove_file(&f1).unwrap();
        let r = refresh(&ctx, Scope::All).unwrap();
        assert_eq!((r.changed, r.removed), (0, 1));
        assert_eq!(ids(&ctx, &SessionFilter::default()), vec!["s2"]);
        std::fs::remove_file(&f2).unwrap();
        claude_file(&ctx, "n1", "new one", "2026-11-07T10:00:00.000Z");
        claude_file(&ctx, "n2", "new two", "2026-11-07T11:00:00.000Z");
        refresh(&ctx, Scope::All).unwrap();
        assert_eq!(ids(&ctx, &SessionFilter::default()), vec!["n2", "n1"]);
        assert_eq!(sessions_total(&ctx, &SessionFilter { query: Some("two".into()), ..Default::default() }).unwrap(), 1);
        // a rebuild reads everything again and lands on the same list
        let r = rebuild(&ctx).unwrap();
        assert_eq!((r.changed, r.removed), (2, 0));
        assert_eq!(ids(&ctx, &SessionFilter::default()), vec!["n2", "n1"]);
    }

    #[test]
    fn the_first_read_of_an_empty_index_refreshes_and_a_page_marks_open_sessions() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        claude_file(&ctx, "s1", "one", "2026-11-05T10:00:00.000Z");
        assert_eq!(ids(&ctx, &SessionFilter::default()), vec!["s1"], "a page open before any refresh reads the folders once");
        assert!(ctx.db.kv_get("index:refreshed_at").unwrap().is_some());
        mark_open(&ctx).unwrap();
        assert!(ids(&ctx, &SessionFilter { open: Some(true), ..Default::default() }).is_empty(), "no console runs in the sandbox");
    }
}
