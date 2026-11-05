//! Oh My Pi (`omp`, npm `@oh-my-pi/pi-coding-agent`): Pi's fork, the same session format with a title slot
//! and `model_usage` entries, read by `pi.rs`. Its providers live in `models.yml` and its logins in
//! `agent.db`, so API-key and routed accounts are not offered (no YAML writer here).
//! shortcut: sessions are read from `<home>/sessions`; Oh My Pi's XDG mode moves them to `$XDG_DATA_HOME/omp/sessions`, add that root if someone uses it.

use super::pi::PiLike;
use crate::model::Caps;

pub static OMP: PiLike = PiLike {
    id: "omp",
    label: "Oh My Pi",
    binary: "omp",
    home_dir: ".omp/agent",
    resume_flag: "--resume",
    caps: Caps { parallel_accounts: true, subscription_login: true, api_key: false, quota: false, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: false },
};
