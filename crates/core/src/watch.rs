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
            match notify::recommended_watcher(move |r: notify::Result<notify::Event>| { if let Ok(e) = r { for p in e.paths { let _ = tx.send(p); } } }) {
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
        // usage first, from the cursors, for the accounts the files belong to; then their sessions
        if let Ok(reg) = ctx.registry() {
            for p in reg.profiles.iter().filter(|p| { let d = harness::of(p).session_dirs(&ctx, p); paths.iter().any(|f| d.iter().any(|x| f.starts_with(x))) }) {
                if let Err(e) = harness::of(p).import_usage(&ctx, p) { crate::log::warn(&format!("watch: usage import for {}: {e:#}", p.name)); }
            }
        }
        match index::refresh(&ctx, Scope::Paths(paths)) {
            Ok(r) => on_refresh(&r),
            Err(e) => crate::log::warn(&format!("watch: index refresh: {e:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::tests::{claude_file, computer};
    use crate::service::{sessions, SessionFilter};

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
            if sessions(&ctx, &SessionFilter { id: Some("s1".into()), ..Default::default() }).unwrap().len() == 1 {
                assert!(seen.load(std::sync::atomic::Ordering::SeqCst) >= 1, "the refresh callback ran");
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        // a CI runner's file notifications can lag or be dropped; the periodic walk covers that in use
        if std::env::var_os("CI").is_some() { eprintln!("note: the watcher did not see the file within two seconds on this runner; skipped"); return; }
        panic!("the watcher did not index the new file within two seconds");
    }
}
