//! One module per coding tool, all behind the `Harness` trait. Adding a tool is one module plus one
//! line in `all()`; the app asks `caps()` instead of hard-coding what each tool supports.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::launch::LaunchSpec;
use crate::model::*;
use crate::registry::Profile;
use crate::transcripts::{FetchedTranscript, Materialized};
use crate::Ctx;

pub mod agy;
pub mod claude;
pub mod codex;
pub mod omp;
pub mod opencode;
pub mod parse;
pub mod pi;

/// Where a harness keeps the things Harness Manager can sync into it.
#[derive(Debug, Clone)]
pub struct ItemPaths {
    /// MCP config file and its format: "claude-json", "codex-toml", "opencode-json", "agy-json".
    pub mcp: Option<(PathBuf, &'static str)>,
    pub skills: Option<PathBuf>,
    pub prompt: Option<PathBuf>,
}

pub trait Harness: Sync + Send {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn caps(&self) -> Caps;
    /// Program name looked up on PATH.
    fn binary(&self) -> &'static str;
    /// The tool's own default home (used by main profiles).
    fn default_home(&self, ctx: &Ctx) -> PathBuf;
    /// Folder prefix for profile homes: `~/.<prefix>-<name>`.
    fn dir_prefix(&self) -> &'static str;

    fn home(&self, ctx: &Ctx, p: &Profile) -> PathBuf {
        if p.main { self.default_home(ctx) } else { ctx.home.join(format!(".{}-{}", self.dir_prefix(), p.name)) }
    }

    /// Create the profile home and anything it needs before first use.
    fn prepare(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        std::fs::create_dir_all(self.home(ctx, p))?;
        Ok(())
    }

    /// Spec for running the harness as this profile with extra args.
    fn launch(&self, ctx: &Ctx, p: &Profile, args: &[String]) -> Result<LaunchSpec>;
    /// Spec for logging the profile in (interactive).
    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec>;
    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity;
    fn quota(&self, _ctx: &Ctx, _p: &Profile) -> Option<Quota> { None }
    /// Store usage rows added since the last import, each file's rows before its cursor; returns rows stored.
    fn import_usage(&self, _ctx: &Ctx, _p: &Profile) -> Result<usize> { Ok(0) }
    /// The folders the tool writes sessions into for this profile (the watcher watches them; a changed file under
    /// one belongs to the profile). Only folders that exist.
    fn session_dirs(&self, _ctx: &Ctx, _p: &Profile) -> Vec<PathBuf> { vec![] }
    /// Sessions active since `since` (epoch ms); files untouched since then are skipped unread.
    fn sessions(&self, _ctx: &Ctx, _p: &Profile, _since: i64) -> Result<Vec<SessionMeta>> { Ok(vec![]) }
    fn transcript(&self, _ctx: &Ctx, _p: &Profile, _s: &SessionMeta) -> Result<Transcript> { anyhow::bail!("sessions are not readable for {}", self.label()) }
    fn resume_args(&self, _session_id: &str) -> Vec<String> { vec![] }
    fn running(&self, _ctx: &Ctx, _p: &Profile) -> Vec<RunningConsole> { vec![] }
    fn item_paths(&self, _ctx: &Ctx, _p: &Profile) -> ItemPaths { ItemPaths { mcp: None, skills: None, prompt: None } }
    /// Called before launching a profile whose harness cannot run accounts in parallel (Antigravity).
    fn activate(&self, _ctx: &Ctx, _p: &Profile) -> Result<()> { Ok(()) }
    /// Copy a transcript from another computer where this tool looks for sessions here, for the repository at
    /// `local_root`, and say how to resume it as a fork; the fetched file and the original are never changed.
    fn materialize(&self, _ctx: &Ctx, _p: &Profile, _local_root: &Path, _t: &FetchedTranscript) -> Result<Materialized> {
        anyhow::bail!("resuming elsewhere is not available for {}", self.label())
    }
    /// `materialize` is implemented: a session of this tool from another computer can be resumed here.
    fn resumes_elsewhere(&self) -> bool { false }
    /// The files that belong to a session in the tool's folder, the transcript first: the archive compresses the
    /// first and deletes them all. The default is the transcript alone.
    fn archive_paths(&self, _ctx: &Ctx, _p: &Profile, s: &SessionMeta) -> Vec<PathBuf> { vec![PathBuf::from(&s.source_path)] }
}

pub fn all() -> &'static [&'static dyn Harness] {
    static ALL: [&dyn Harness; 6] = [&claude::Claude, &codex::Codex, &opencode::OpenCode, &agy::Agy, &pi::PI, &omp::OMP];
    &ALL
}

pub fn by_id(id: &str) -> Option<&'static dyn Harness> { all().iter().copied().find(|h| h.id() == id) }

pub fn of(p: &Profile) -> &'static dyn Harness { by_id(&p.harness).expect("registry only holds known harnesses") }

/// Environment for any launch: when tests redirect profile homes (`HRM_PROFILES_HOME`), the harness
/// must see that folder as its user home too.
pub fn base_env(ctx: &Ctx, spec: &mut LaunchSpec) {
    if std::env::var_os("HRM_PROFILES_HOME").is_some() {
        let h = ctx.home.display().to_string();
        spec.env_set.push(("USERPROFILE".into(), h.clone()));
        spec.env_set.push(("HOME".into(), h));
    }
}
