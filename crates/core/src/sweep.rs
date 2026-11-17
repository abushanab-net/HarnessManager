//! The safeguard against what a failed or interrupted operation leaves behind. Harness Manager names its own
//! temporary files: `write_atomic`'s `<file>.hrm-tmp-<pid>-<n>`, a backup's `<name>.zip.part`, a skill's
//! `.<name>.new` and `.<name>.old` while it is replaced, the staging folder of a skill download, the `.command`
//! file a macOS terminal was opened with. So a sweep can tell them from everything else. It runs at start and then
//! hourly, takes only those names, only once they are an hour old (nothing is written for that long), only in
//! folders Harness Manager writes to, and never goes through a link.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};

use crate::util::now_ms;
use crate::Ctx;

/// Older than this, a temporary file is no longer being written.
const OLD_MS: i64 = 3_600_000;

/// What a sweep removed (or, on a dry run, would remove).
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Swept {
    pub files: usize,
    pub bytes: u64,
    /// A skill whose replacement stopped halfway and was put right.
    pub repaired: Vec<String>,
    /// The first fifty paths, for the log and `hrm cleanup`.
    pub what: Vec<String>,
    /// Two of the folders looked at can hold the same file (the account list sits in the data folder): it counts once.
    #[serde(skip)]
    seen: std::collections::HashSet<PathBuf>,
}

impl Swept {
    fn took(&mut self, p: &Path, bytes: u64) {
        if !self.seen.insert(p.to_path_buf()) { return; }
        self.files += 1;
        self.bytes += bytes;
        if self.what.len() < 50 { self.what.push(p.display().to_string()); }
    }
}

fn age_ms(p: &Path) -> i64 {
    std::fs::symlink_metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|a| a.as_millis() as i64).unwrap_or(0)
}

/// `write_atomic`'s temporary name: `<stem>.hrm-tmp-<pid>-<n>`, both numbers.
fn is_tmp(name: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    name.rsplit_once(".hrm-tmp-").and_then(|(stem, t)| t.split_once('-').map(|(pid, n)| !stem.is_empty() && digits(pid) && digits(n))).unwrap_or(false)
}

/// Temporary files in `dir` and `depth` levels below it. `parts`: a backup's part files too (only where backups
/// are written: a browser's unfinished download is also called `.zip.part`).
fn files(dir: &Path, depth: usize, parts: bool, older_ms: i64, dry: bool, s: &mut Swept) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let (p, name) = (e.path(), e.file_name().to_string_lossy().to_string());
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() { continue; }
        if ft.is_dir() {
            // not the browser engine's own folder, and never through a link: its target is someone else's
            if depth > 0 && name != "webview" && !crate::shared::is_link(&p) { files(&p, depth - 1, parts, older_ms, dry, s); }
            continue;
        }
        if !(is_tmp(&name) || parts && name.ends_with(".zip.part")) || age_ms(&p) < older_ms { continue; }
        let len = e.metadata().map(|m| m.len()).unwrap_or(0);
        if dry || std::fs::remove_file(&p).is_ok() { s.took(&p, len); }
    }
}

fn dir_bytes(d: &Path) -> u64 {
    let mut v = vec![];
    crate::util::walk_files(d, &|_| true, &mut v);
    v.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum()
}

fn remove_dir(d: &Path, dry: bool, s: &mut Swept) {
    let bytes = dir_bytes(d);
    if dry || std::fs::remove_dir_all(d).is_ok() { s.took(d, bytes); }
}

/// A skill is replaced by copying to `.<name>.new`, moving the installed one to `.<name>.old`, then moving the new
/// one in. Stopped before the swap, the copy may be half made and the installed skill is whole: the copy goes.
/// Stopped inside the swap, the skill's folder is missing and the copy is complete: it moves in, else the old one
/// moves back. What is left over then goes.
fn skills(store: &Path, older_ms: i64, dry: bool, s: &mut Swept) {
    let Ok(rd) = std::fs::read_dir(store) else { return };
    let mut left: Vec<(String, &'static str, PathBuf)> = rd.flatten().filter_map(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        let (skill, kind) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".new").map(|x| (x, "new")).or_else(|| n.strip_suffix(".old").map(|x| (x, "old"))))?;
        let p = e.path();
        (crate::util::valid_name(skill) && e.file_type().ok()?.is_dir() && !crate::shared::is_link(&p) && age_ms(&p) >= older_ms).then(|| (skill.to_string(), kind, p))
    }).collect();
    // the new copy is looked at first: when the skill's folder is missing it is the one that moves in
    left.sort_by_key(|(skill, kind, _)| (skill.clone(), *kind));
    for (skill, kind, p) in left {
        let live = store.join(&skill);
        if live.exists() { remove_dir(&p, dry, s); continue; }
        if dry || std::fs::rename(&p, &live).is_ok() { s.repaired.push(format!("{skill} (its {} copy moved back in)", if kind == "new" { "new" } else { "previous" })); }
    }
}

/// Remove what failed operations left; `dry` only reports it.
pub fn sweep(ctx: &Ctx, dry: bool) -> Swept { sweep_older(ctx, OLD_MS, dry) }

fn sweep_older(ctx: &Ctx, older_ms: i64, dry: bool) -> Swept {
    let mut s = Swept::default();
    let st = &ctx.settings;
    // (folder, levels below it, backups are written here)
    let mut dirs: Vec<(PathBuf, usize, bool)> = vec![(ctx.data.clone(), 6, false), (crate::backup::dir(ctx), 0, true), (ctx.home.clone(), 0, false)];
    if st.sync_mode == "folder" && !st.sync_folder.is_empty() {
        // this computer's own part of the shared folder; the other computers sweep theirs
        let f = Path::new(&st.sync_folder);
        dirs.push((f.join("devices").join(&st.device_id), 4, false));
        dirs.push((f.join("backups").join(&st.device_id), 0, true));
    }
    if !st.archive_dir.is_empty() { dirs.push((PathBuf::from(&st.archive_dir), 5, false)); }
    if let Some(d) = ctx.registry_path().parent() { dirs.push((d.to_path_buf(), 0, false)); }
    // beside each file Harness Manager writes for a tool: the account's folder, its MCP config, its prompt file
    if let Ok(reg) = ctx.registry() {
        for p in &reg.profiles {
            let Some(h) = crate::harness::by_id(&p.harness) else { continue };
            dirs.push((h.home(ctx, p), 0, false));
            let ip = h.item_paths(ctx, p);
            for f in [ip.mcp.map(|m| m.0), ip.prompt].into_iter().flatten() {
                if let Some(d) = f.parent() { dirs.push((d.to_path_buf(), 0, false)); }
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    for (d, depth, parts) in dirs { files(&d, depth, parts, older_ms, dry, &mut s); }
    skills(&crate::items::skill_store(ctx), older_ms, dry, &mut s);
    let staging = ctx.data.join("skill-staging");
    if staging.is_dir() && !crate::shared::is_link(&staging) && age_ms(&staging) >= older_ms { remove_dir(&staging, dry, &mut s); }
    // macOS: the `.command` file each terminal window was opened with (`hrm-<pid>-<n>.command`)
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let ours = name.strip_prefix("hrm-").and_then(|n| n.strip_suffix(".command")).map(|n| is_tmp(&format!("x.hrm-tmp-{n}"))).unwrap_or(false);
            if !ours || !e.file_type().map(|t| t.is_file()).unwrap_or(false) || age_ms(&e.path()) < older_ms { continue; }
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            if dry || std::fs::remove_file(e.path()).is_ok() { s.took(&e.path(), len); }
        }
    }
    if !dry && (s.files > 0 || !s.repaired.is_empty()) {
        crate::log::info(&format!("cleanup: removed {} leftovers of interrupted operations ({:.1} MB){}", s.files, s.bytes as f64 / 1_048_576.0,
            if s.repaired.is_empty() { String::new() } else { format!("; put right: {}", s.repaired.join(", ")) }));
    }
    s
}

/// The sweep for a long-running process: at its first call, then at most once an hour.
pub fn hourly(ctx: &Ctx) {
    static LAST: AtomicI64 = AtomicI64::new(0);
    let now = now_ms();
    if now - LAST.load(Ordering::Relaxed) < OLD_MS { return; }
    LAST.store(now, Ordering::Relaxed);
    sweep(ctx, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> (tempfile::TempDir, Ctx) {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        (t, ctx)
    }

    fn put(p: &Path) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, b"x").unwrap(); }

    #[test]
    fn only_harness_managers_own_temporary_names_are_taken() {
        for (name, ours) in [("settings.hrm-tmp-4242-7", true), (".claude.hrm-tmp-1-0", true), ("s1.jsonl.hrm-tmp-88-12", true),
            ("settings.hrm-tmp-", false), ("settings.hrm-tmp-abc-1", false), ("settings.hrm-tmp-12", false), (".hrm-tmp-1-2", false), ("notes.tmp", false), ("settings.json", false)] {
            assert_eq!(is_tmp(name), ours, "{name}");
        }
    }

    #[test]
    fn leftovers_go_from_the_folders_it_writes_to_and_nothing_else_does() {
        let (t, ctx) = ctx();
        let gone = [ctx.data.join("settings.hrm-tmp-1-1"), ctx.data.join("transcripts").join("d").join("claude").join("work").join("s1.jsonl.hrm-tmp-1-2"),
            crate::backup::dir(&ctx).join("2026-11-12 08.41.00 Automatic.zip.part"), ctx.home.join(".claude.hrm-tmp-9-9")];
        let stay = [ctx.data.join("settings.json"), ctx.data.join("notes.zip.part"), ctx.home.join("download.zip.part"), ctx.home.join("deep").join("x.hrm-tmp-1-1"),
            ctx.data.join("webview").join("y.hrm-tmp-1-1"), ctx.data.join("odd.hrm-tmp-abc-1")];
        for p in gone.iter().chain(&stay) { put(p); }
        // a folder reached through a link belongs to someone else
        let outside = t.path().join("outside");
        put(&outside.join("z.hrm-tmp-1-1"));
        crate::shared::link_dir(&outside, &ctx.data.join("linked")).unwrap();

        let dry = sweep_older(&ctx, 0, true);
        assert_eq!(dry.files, gone.len(), "{:?}", dry.what);
        assert!(gone.iter().all(|p| p.exists()), "a dry run removes nothing");
        assert_eq!(sweep_older(&ctx, OLD_MS, false).files, 0, "what may still be written stays");

        let s = sweep_older(&ctx, 0, false);
        assert_eq!((s.files, s.bytes), (gone.len(), gone.len() as u64), "{:?}", s.what);
        for p in &gone { assert!(!p.exists(), "{}", p.display()); }
        for p in &stay { assert!(p.exists(), "{}", p.display()); }
        assert!(outside.join("z.hrm-tmp-1-1").exists(), "not through a link");
    }

    #[test]
    fn a_skill_replacement_that_stopped_halfway_is_put_right() {
        let (_t, ctx) = ctx();
        let store = crate::items::skill_store(&ctx);
        let skill = |dir: &str, text: &str| { let p = store.join(dir).join("SKILL.md"); put(&p); std::fs::write(&p, text).unwrap(); };
        // stopped while copying: the installed one is whole, the half copy goes
        skill("a", "installed"); skill(".a.new", "half");
        // stopped inside the swap: the folder is missing, the complete new copy moves in and the old one goes
        skill(".b.new", "new"); skill(".b.old", "previous");
        // the new copy is gone too: the previous one moves back
        skill(".c.old", "previous");
        let s = sweep_older(&ctx, 0, false);
        let read = |d: &str| std::fs::read_to_string(store.join(d).join("SKILL.md")).unwrap();
        assert_eq!((read("a"), read("b"), read("c")), ("installed".to_string(), "new".to_string(), "previous".to_string()));
        for left in [".a.new", ".b.new", ".b.old", ".c.old"] { assert!(!store.join(left).exists(), "{left}"); }
        assert_eq!(s.repaired.len(), 2, "{:?}", s.repaired);
    }

    #[test]
    fn the_staging_folder_of_a_skill_download_goes_when_it_is_old() {
        let (_t, ctx) = ctx();
        put(&ctx.data.join("skill-staging").join("repo").join("SKILL.md"));
        assert_eq!(sweep_older(&ctx, OLD_MS, false).files, 0);
        assert_eq!(sweep_older(&ctx, 0, false).files, 1);
        assert!(!ctx.data.join("skill-staging").exists());
    }
}
