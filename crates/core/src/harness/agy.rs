//! Google Antigravity CLI (`agy`). It keeps one login in the OS credential store, so profiles cannot run
//! side by side: starting a profile swaps the saved login in, after saving the current one back.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::{base_env, Harness, ItemPaths};
use crate::launch::{resolve_program, LaunchSpec};
use crate::model::*;
use crate::registry::{Kind, Profile};
use crate::util::{file_sig, truncate};
use crate::Ctx;

pub struct Agy;

/// Credential Antigravity reads its login from. `HRM_AGY_CRED` lets tests use their own.
pub fn live_target() -> String { std::env::var("HRM_AGY_CRED").unwrap_or_else(|_| "gemini:antigravity".into()) }

fn snapshot_target(profile: &str) -> String { format!("{}:agy:{profile}", crate::secrets::prefix()) }

const ACTIVE_KEY: &str = "agy:active";
/// Stored while a sign-in runs: the slot holds a login nobody owns yet. If the sign-in never finishes
/// (cancelled, app closed), that login must not be filed under the main profile.
const UNOWNED: &str = "-";

impl Agy {
    fn root(&self, ctx: &Ctx) -> PathBuf { ctx.home.join(".gemini") }

    pub fn active(&self, ctx: &Ctx) -> Option<String> { ctx.db.kv_get(ACTIVE_KEY).ok().flatten().filter(|s| !s.is_empty() && s != UNOWNED) }

    fn unowned(&self, ctx: &Ctx) -> bool { ctx.db.kv_get(ACTIVE_KEY).ok().flatten().as_deref() == Some(UNOWNED) }

    /// Antigravity has one login slot. Swapping it while Antigravity runs would hand the running console the
    /// other account's login, and its token refreshes would be saved under the wrong account.
    pub fn check_switch(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        let owner = self.owner(ctx);
        if owner.as_deref() == Some(p.name.as_str()) || self.consoles().is_empty() { return Ok(()); }
        bail!("Antigravity is open for '{}'. Close it before starting '{}': both use the same sign-in, so switching while it runs mixes the two accounts.",
            owner.unwrap_or_else(|| "another account".into()), p.name)
    }

    /// Running Antigravity consoles: processes of the agy program Harness Manager launches.
    fn consoles(&self) -> Vec<(u32, i64, Option<String>)> {
        crate::procs::by_name(&["agy"], resolve_program("agy", "agy").ok().as_deref())
    }

    /// Who the login in the slot belongs to: the active profile, or with nothing recorded yet the main one
    /// (the same rule `identity` uses).
    fn owner(&self, ctx: &Ctx) -> Option<String> {
        if self.unowned(ctx) { return None; }
        self.active(ctx).or_else(|| ctx.registry().ok()?.profiles.into_iter().find(|p| p.harness == "agy" && p.main).map(|p| p.name))
    }

    /// Save the login currently in Antigravity's slot to the profile that owns it.
    fn save_current(&self, ctx: &Ctx) -> Result<()> {
        if let (Some(owner), Some((user, blob))) = (self.owner(ctx), crate::secrets::read(&live_target())) {
            crate::secrets::write(&snapshot_target(&owner), &user, &blob)?;
        }
        Ok(())
    }

    /// A removed profile: its saved login goes, and it no longer owns the slot.
    pub fn forget(&self, ctx: &Ctx, name: &str) -> Result<()> {
        crate::secrets::delete(&snapshot_target(name))?;
        if self.active(ctx).as_deref() != Some(name) { return Ok(()); }
        // its login is still in the slot: hand the slot back to the main profile, or empty it, so the
        // removed login is never saved under another profile
        let main = ctx.registry()?.profiles.into_iter().find(|p| p.harness == "agy" && p.main && p.name != name).map(|p| p.name);
        match main.as_ref().and_then(|m| crate::secrets::read(&snapshot_target(m))) {
            Some((user, blob)) => {
                crate::secrets::write(&live_target(), &user, &blob)?;
                ctx.db.kv_set(ACTIVE_KEY, main.as_deref().unwrap_or(""))
            }
            None => {
                crate::secrets::delete(&live_target())?;
                ctx.db.kv_set(ACTIVE_KEY, "")
            }
        }
    }

    /// Before a fresh login: park the current login with its owner and empty the slot.
    pub fn prepare_login(&self, ctx: &Ctx) -> Result<()> {
        if !self.consoles().is_empty() { bail!("Close Antigravity before signing in: the sign-in replaces the login it is using."); }
        self.save_current(ctx)?;
        crate::secrets::delete(&live_target())?;
        ctx.db.kv_set(ACTIVE_KEY, UNOWNED)
    }

    /// After a login finished: the slot now belongs to `p`.
    pub fn finish_login(&self, ctx: &Ctx, p: &Profile) -> Result<bool> {
        match crate::secrets::read(&live_target()) {
            Some((user, blob)) => {
                crate::secrets::write(&snapshot_target(&p.name), &user, &blob)?;
                ctx.db.kv_set(ACTIVE_KEY, &p.name)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn conversations(&self, ctx: &Ctx) -> Vec<PathBuf> {
        let dir = self.root(ctx).join("antigravity-cli").join("conversations");
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e == "db").unwrap_or(false)).collect()).unwrap_or_default()
    }
}

impl Harness for Agy {
    fn id(&self) -> &'static str { "agy" }
    fn label(&self) -> &'static str { "Antigravity" }
    fn caps(&self) -> Caps {
        Caps { parallel_accounts: false, subscription_login: true, api_key: false, quota: false, usage: false, sessions: true, mcp: true, skills: true, prompts: true, routable: false }
    }
    fn binary(&self) -> &'static str { "agy" }
    fn default_home(&self, ctx: &Ctx) -> PathBuf { self.root(ctx) }
    fn dir_prefix(&self) -> &'static str { "agy" }

    fn prepare(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        if p.main && self.active(ctx).is_none() && !self.unowned(ctx) && crate::secrets::read(&live_target()).is_some() {
            ctx.db.kv_set(ACTIVE_KEY, &p.name)?;
        }
        Ok(())
    }

    fn activate(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        if self.active(ctx).as_deref() == Some(p.name.as_str()) { return Ok(()); }
        self.check_switch(ctx, p)?;
        let Some((user, blob)) = crate::secrets::read(&snapshot_target(&p.name)) else {
            bail!("'{}' has no saved Antigravity login yet; run `hrm login {}`", p.name, p.name);
        };
        self.save_current(ctx)?;
        let previous = crate::secrets::read(&live_target());
        crate::secrets::write(&live_target(), &user, &blob)?;
        // the slot and its owner change together: if the owner cannot be recorded, the old login goes back,
        // so it is never later saved under the wrong account
        if let Err(e) = ctx.db.kv_set(ACTIVE_KEY, &p.name) {
            let _ = match previous {
                Some((u, b)) => crate::secrets::write(&live_target(), &u, &b),
                None => crate::secrets::delete(&live_target()),
            };
            return Err(e);
        }
        Ok(())
    }

    fn launch(&self, ctx: &Ctx, _p: &Profile, args: &[String]) -> Result<LaunchSpec> {
        let mut spec = LaunchSpec { program: resolve_program(self.id(), self.binary())?, args: args.to_vec(), ..Default::default() };
        base_env(ctx, &mut spec);
        Ok(spec)
    }

    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec> {
        if p.kind != Kind::Subscription { bail!("Antigravity only supports Google sign-in"); }
        self.launch(ctx, p, &[])
    }

    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity {
        let saved = crate::secrets::read(&snapshot_target(&p.name));
        // with nothing recorded yet, the login in the slot belongs to the main profile
        let active = self.active(ctx).as_deref() == Some(p.name.as_str()) || (p.main && self.active(ctx).is_none() && !self.unowned(ctx));
        let blob = if active { crate::secrets::read(&live_target()) } else { saved };
        match blob {
            Some((_, b)) => {
                let v: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
                let email = v["email"].as_str().or(v["account"]["email"].as_str()).map(String::from);
                Identity { logged_in: true, email, plan: Some(if active { "Google · active".into() } else { "Google".into() }), ..Default::default() }
            }
            None => Identity::default(),
        }
    }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        // conversations are shared by every Antigravity profile; they are listed under the main one
        if !p.main { return Ok(vec![]); }
        let mut sums = summaries(&self.root(ctx).join("antigravity-cli").join("conversation_summaries.db"));
        sums.extend(summaries(&self.root(ctx).join("antigravity").join("conversation_summaries.db")));
        let mut out = vec![];
        for f in self.conversations(ctx) {
            let id = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let (mtime, _) = file_sig(&f).unwrap_or((0, 0));
            if mtime < since { continue; }
            let s = sums.iter().find(|s| s.0 == id);
            let steps = read_steps(&f);
            let first_user = steps.iter().find(|(t, _)| *t == 14).and_then(|(_, txt)| txt.first().cloned()).unwrap_or_default();
            let title = s.map(|s| if s.1.is_empty() { s.2.clone() } else { s.1.clone() }).filter(|t| !t.is_empty()).unwrap_or(first_user);
            out.push(SessionMeta {
                harness: "agy".into(), profile: p.name.clone(), id,
                title: truncate(&title, 80),
                cwd: s.map(|s| s.3.clone()).unwrap_or_default(),
                started_at: mtime, last_active_at: mtime,
                prompts: steps.iter().filter(|(t, _)| *t == 14).count() as u32,
                messages: steps.len() as u32,
                source_path: f.display().to_string(),
                ..Default::default()
            });
        }
        Ok(out)
    }

    fn transcript(&self, _ctx: &Ctx, _p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let mut turns: Vec<Turn> = vec![];
        for (ty, texts) in read_steps(Path::new(&s.source_path)) {
            if ty == 14 {
                turns.push(Turn { prompt: texts.first().cloned().unwrap_or_default(), prompt_at: s.started_at, ..Default::default() });
            } else if let Some(t) = turns.last_mut() {
                let body: Vec<String> = texts.into_iter().filter(|x| x.len() > 3).collect();
                if !body.is_empty() { t.say(body.join("\n\n")); }
            }
        }
        Ok(Transcript { meta: s.clone(), turns })
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec!["--conversation".into(), id.into()] }

    fn running(&self, ctx: &Ctx, p: &Profile) -> Vec<RunningConsole> {
        if self.active(ctx).as_deref() != Some(p.name.as_str()) { return vec![]; }
        self.consoles().into_iter().map(|(pid, start, cwd)| RunningConsole { pid, started_at: Some(start), cwd, session_id: None }).collect()
    }

    fn item_paths(&self, ctx: &Ctx, _p: &Profile) -> ItemPaths {
        let c = self.root(ctx).join("config");
        ItemPaths { mcp: Some((c.join("mcp_config.json"), "agy-json")), skills: Some(c.join("skills")), prompt: Some(c.join("AGENTS.md")) }
    }
}

/// (id, title, preview, first workspace folder) from Antigravity's summaries database.
fn summaries(path: &Path) -> Vec<(String, String, String, String)> {
    let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return vec![] };
    let Ok(mut st) = db.prepare("select conversation_id, coalesce(title,''), coalesce(preview,''), coalesce(workspace_uris,'[]') from conversation_summaries") else { return vec![] };
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, String>(3)?)))
        .map(|it| it.flatten().map(|(a, b, c, w): (String, String, String, String)| {
            let ws = serde_json::from_str::<Vec<String>>(&w).ok().and_then(|v| v.into_iter().next()).unwrap_or_default();
            let ws = reqwest::Url::parse(&ws).ok().and_then(|u| u.to_file_path().ok()).map(|p| p.display().to_string())
                .unwrap_or_else(|| ws.trim_start_matches("file:///").replace('/', "\\"));
            (a, b, c, ws)
        }).collect())
        .unwrap_or_default()
}

/// Steps of a conversation as (step type, readable strings). Payloads are protobuf without a published
/// schema, so text is recovered by walking the wire format and keeping UTF-8 strings.
fn read_steps(path: &Path) -> Vec<(i64, Vec<String>)> {
    let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return vec![] };
    let Ok(mut st) = db.prepare("select step_type, step_payload from steps order by idx") else { return vec![] };
    st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<Vec<u8>>>(1)?)))
        .map(|it| it.flatten().map(|(t, b)| {
            let mut out = vec![];
            proto_strings(&b.unwrap_or_default(), 0, &mut out);
            out.retain(|s| !looks_like_id(s));
            out.dedup();
            (t, out)
        }).collect())
        .unwrap_or_default()
}

/// Ids and references, not text: hex runs and anything carrying a UUID.
fn looks_like_id(s: &str) -> bool {
    (s.len() >= 8 && s.chars().all(|c| c.is_ascii_hexdigit())) || has_uuid(s)
}

fn has_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(36).any(|w| w.iter().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { *c == b'-' } else { c.is_ascii_hexdigit() }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_not_text() {
        assert!(super::looks_like_id("$520e7190-198c-47f4-b9f1-988213c43b61\"$c75c1e05"));
        assert!(super::looks_like_id("c75c1e05994c48bf"));
        assert!(!super::looks_like_id("exit"));
        assert!(!super::looks_like_id("Explain the release script"));
    }

    #[test]
    fn corrupt_files_do_not_crash_the_parser() {
        // a length field of 2^64-1 and one of 2^63, then a truncated one
        let huge = [0x0a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0x41];
        let half = [0x12, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        for b in [&huge[..], &half[..], &[0x0a, 0x05, 0x41][..]] {
            let mut out = vec![];
            super::proto_strings(b, 0, &mut out);
            assert!(out.is_empty());
            let mut wrapped = vec![0x0a, b.len() as u8];
            wrapped.extend_from_slice(b);
            super::proto_strings(&wrapped, 0, &mut out);
        }
    }
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 { return Some(v); }
    }
    None
}

pub fn proto_strings(b: &[u8], depth: u32, out: &mut Vec<String>) {
    let mut i = 0;
    while i < b.len() {
        let Some(key) = varint(b, &mut i) else { return };
        match key & 7 {
            0 => { if varint(b, &mut i).is_none() { return } }
            1 => i = i.saturating_add(8),
            5 => i = i.saturating_add(4),
            2 => {
                // a corrupt length must not overflow or run past the end
                let Some(end) = varint(b, &mut i).and_then(|l| usize::try_from(l).ok()).and_then(|l| i.checked_add(l)).filter(|e| *e <= b.len()) else { return };
                let chunk = &b[i..end];
                // a nested message parses cleanly; otherwise keep it as text if it is readable UTF-8
                let mut nested = vec![];
                if depth < 12 && looks_nested(chunk) {
                    proto_strings(chunk, depth + 1, &mut nested);
                    out.extend(nested);
                } else if let Ok(s) = std::str::from_utf8(chunk) {
                    if s.chars().count() >= 2 && s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t' || c == '\r') {
                        out.push(s.trim().to_string());
                    }
                }
                i = end;
            }
            _ => return,
        }
    }
}

fn looks_nested(b: &[u8]) -> bool {
    if b.is_empty() { return false; }
    if std::str::from_utf8(b).map(|s| s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t' || c == '\r')).unwrap_or(false) { return false; }
    let mut i = 0;
    while i < b.len() {
        let Some(key) = varint(b, &mut i) else { return false };
        if key >> 3 == 0 { return false; }
        match key & 7 {
            0 => { if varint(b, &mut i).is_none() { return false } }
            1 => i = i.saturating_add(8),
            5 => i = i.saturating_add(4),
            2 => { let Some(n) = varint(b, &mut i).and_then(|l| usize::try_from(l).ok()).and_then(|l| i.checked_add(l)) else { return false }; i = n; }
            _ => return false,
        }
    }
    i == b.len()
}
