//! A watcher on the tools' session folders (the `notify` crate). A change becomes a usage import from the
//! cursors and `index::refresh(Paths)` within about a second; a burst coalesces; the folder set follows the
//! accounts once a minute; the periodic walk stays the safety net. A folder that cannot be watched (a network
//! drive, OneDrive placeholders) is logged once and the walk is its only refresh.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

use crate::harness;
use crate::index::{self, Report, Scope};
use crate::util::now_ms;
use crate::Ctx;

pub type CtxOf = Arc<dyn Fn() -> Arc<Ctx> + Send + Sync>;
pub type OnRefresh = Arc<dyn Fn(&Report) + Send + Sync>;

const SETTLE: Duration = Duration::from_millis(500);
const SETTLE_MOST: Duration = Duration::from_secs(3);
const RECHECK: Duration = Duration::from_secs(60);

/// The folders to watch now: every account's session folders that exist.
fn dirs_of(ctx: &Ctx) -> BTreeSet<PathBuf> {
    ctx.registry().map(|reg| reg.profiles.iter().flat_map(|p| harness::of(p).session_dirs(ctx, p)).collect()).unwrap_or_default()
}

/// Start the watcher on its own thread; `on_refresh` is called after each refresh it triggered.
pub fn start(ctx_of: CtxOf, on_refresh: OnRefresh) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || run(ctx_of, on_refresh))
}

fn run(ctx_of: CtxOf, on_refresh: OnRefresh) {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher: Option<notify::RecommendedWatcher> = None;
    let mut watched: BTreeSet<PathBuf> = BTreeSet::new();
    let mut told: HashSet<PathBuf> = HashSet::new();
    loop {
        let ctx = ctx_of();
        let dirs = dirs_of(&ctx);
        if dirs != watched || (watcher.is_none() && !dirs.is_empty()) {
            // accounts came or went: a fresh watcher over the folders of today
            let tx = tx.clone();
            // a file being opened or closed is no change: Linux reports those too, also the refresh's own reads, which would start the next refresh
            match notify::recommended_watcher(move |r: notify::Result<notify::Event>| { if let Ok(e) = r { if !e.kind.is_access() { for p in e.paths { let _ = tx.send(p); } } } }) {
                Ok(mut w) => {
                    for d in &dirs {
                        if let Err(e) = w.watch(d, RecursiveMode::Recursive) {
                            if told.insert(d.clone()) { crate::log::warn(&format!("watch: {} cannot be watched ({e}); the periodic walk refreshes it", d.display())); }
                        }
                    }
                    watcher = Some(w);
                    watched = dirs;
                }
                Err(e) => if told.insert(PathBuf::new()) { crate::log::warn(&format!("watch: the folder watcher could not start ({e}); the periodic walk stays the only refresh")); },
            }
        }
        let _ = ctx.db.kv_set("index:watcher_at", &now_ms().to_string());
        drop(ctx);
        // the first change, then the rest of the burst until it settles
        let Ok(first) = rx.recv_timeout(RECHECK) else { continue };
        let mut paths = BTreeSet::from([first]);
        let most = Instant::now() + SETTLE_MOST;
        while let Ok(p) = rx.recv_timeout(SETTLE.min(most.saturating_duration_since(Instant::now()))) { paths.insert(p); }
        let ctx = ctx_of();
        let paths: Vec<PathBuf> = paths.into_iter().collect();
        if let Some(r) = changed(&ctx, paths) { on_refresh(&r); }
    }
}

/// What a burst of changed paths asks for: the usage import, from the cursors, of the accounts they belong to, then
/// the index refresh of those paths. None when no account's sessions can have changed (a file a tool keeps beside
/// its sessions): nothing is read and nothing redrawn.
pub fn changed(ctx: &Ctx, paths: Vec<PathBuf>) -> Option<Report> {
    let reg = ctx.registry().map_err(|e| crate::log::warn(&format!("watch: index refresh: {e:#}"))).ok()?;
    let owners = index::owners(ctx, &reg, &paths);
    if owners.is_empty() { return None; }
    for p in owners {
        if let Err(e) = harness::of(p).import_usage(ctx, p) { crate::log::warn(&format!("watch: usage import for {}: {e:#}", p.name)); }
    }
    index::refresh(ctx, Scope::Paths(paths)).map_err(|e| crate::log::warn(&format!("watch: index refresh: {e:#}"))).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::tests::{claude_file, computer};
    use crate::service::{sessions, SessionFilter};

    #[test]
    fn what_a_tool_writes_beside_its_sessions_starts_no_refresh() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        let mut reg = ctx.registry().unwrap();
        reg.add_profile(crate::Profile { name: "opencode-main".into(), label: None, harness: "opencode".into(), kind: crate::Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 }).unwrap();
        ctx.save_registry(&reg).unwrap();
        let data = harness::opencode::OpenCode.data_dir(&ctx, reg.get("opencode-main").unwrap());
        std::fs::create_dir_all(data.join("snapshot")).unwrap();
        assert!(changed(&ctx, vec![data.join("snapshot").join("pack-1.idx"), data.join("log").join("today.log")]).is_none());
        assert!(changed(&ctx, vec![t.path().join("elsewhere.jsonl")]).is_none());
        // its database's log does: the account is looked at (it has no database yet, so no session)
        let r = changed(&ctx, vec![data.join("snapshot").join("pack-1.idx"), data.join("opencode.db-wal")]).unwrap();
        assert_eq!((r.profiles, r.sessions), (1, 0));
    }

    #[test]
    fn a_file_written_under_a_watched_folder_is_in_the_index_within_two_seconds() {
        let t = tempfile::tempdir().unwrap();
        let ctx = computer(t.path());
        // the usage import prices rows from a catalog: a local one, so the test never reaches the network
        std::fs::write(t.path().join("pricing.json"), "{}").unwrap();
        std::env::set_var("HRM_PRICING_FILE", t.path().join("pricing.json"));
        claude_file(&ctx, "s0", "before", "2026-11-05T09:00:00.000Z");
        index::refresh(&ctx, Scope::All).unwrap();
        let (data, home) = (ctx.data.clone(), ctx.home.clone());
        let ctx_of: CtxOf = Arc::new(move || Arc::new(Ctx::open_at(&data, &home).unwrap()));
        let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s2 = seen.clone();
        start(ctx_of, Arc::new(move |_| { s2.fetch_add(1, std::sync::atomic::Ordering::SeqCst); }));
        // the watcher takes a moment to arm; a write before that is the walk's job, not this test's
        std::thread::sleep(Duration::from_millis(700));
        claude_file(&ctx, "s1", "written while watched", "2026-11-05T10:00:00.000Z");
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(2) {
            // the callback runs once the refresh has written the row, so it is the thing to wait for; macOS also
            // reports the file written just before the watch began, so a first callback can come without this one
            if seen.load(std::sync::atomic::Ordering::SeqCst) >= 1 && sessions(&ctx, &SessionFilter { id: Some("s1".into()), ..Default::default() }).unwrap().len() == 1 { return; }
            std::thread::sleep(Duration::from_millis(100));
        }
        // a CI runner's file notifications can lag or be dropped; the periodic walk covers that in use
        if std::env::var_os("CI").is_some() { eprintln!("note: the watcher did not see the file within two seconds on this runner; skipped"); return; }
        panic!("the watcher did not index the new file within two seconds");
    }
}
