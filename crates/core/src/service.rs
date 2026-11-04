//! Operations shared by the CLI and the desktop app. Everything returns plain serializable views.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::harness::{self, Harness};
use crate::model::*;
use crate::projects::Resolver;
use crate::registry::{Kind, Profile, Registry};
use crate::util::now_ms;
use crate::Ctx;

#[derive(Debug, Clone, Serialize)]
pub struct HarnessView {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub caps: Caps,
}

pub fn harnesses() -> Vec<HarnessView> {
    harness::all().iter().map(|h| HarnessView {
        id: h.id().into(),
        label: h.label().into(),
        installed: crate::launch::resolve_program(h.id(), h.binary()).is_ok(),
        caps: h.caps(),
    }).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub name: String,
    /// The name to show: the label, or the id.
    pub label: String,
    pub harness: String,
    pub harness_label: String,
    pub kind: Kind,
    pub main: bool,
    pub expected_email: Option<String>,
    pub identity: Identity,
    /// The typed email does not match the account that is logged in.
    pub email_mismatch: bool,
    pub quota: Option<Quota>,
    pub running: Vec<RunningConsole>,
    pub tokens_7d: i64,
    pub days_7d: Vec<i64>,
    pub sessions_7d: i64,
    pub last_used: Option<i64>,
    pub home: String,
    pub caps: Caps,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub route: Option<String>,
    pub daily_cap_usd: Option<f64>,
    pub created_at: i64,
}

/// The last `n` calendar days (at least today), oldest first, as "YYYY-MM-DD".
pub fn last_days(n: i64) -> Vec<String> {
    let today = chrono::Local::now().date_naive();
    (0..n.max(1)).rev().map(|i| (today - chrono::Duration::days(i)).format("%Y-%m-%d").to_string()).collect()
}

pub fn profile_view(ctx: &Ctx, p: &Profile) -> ProfileView {
    let h = harness::of(p);
    let identity = h.identity(ctx, p);
    let days = last_days(7);
    let (tokens_7d, days_7d, sessions_7d, last_used) = ctx.db.with(|c| {
        let mut per = vec![0i64; 7];
        let mut st = c.prepare("select day, sum(input+output) from usage where profile=? and day>=? group by day")?;
        for r in st.query_map(rusqlite::params![p.name, days[0]], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?.flatten() {
            if let Some(i) = days.iter().position(|d| *d == r.0) { per[i] = r.1; }
        }
        let sessions: i64 = c.query_row("select count(distinct session_id) from usage where profile=? and day>=?", rusqlite::params![p.name, days[0]], |r| r.get(0))?;
        let last: Option<i64> = c.query_row("select max(ts) from usage where profile=?", [&p.name], |r| r.get(0))?;
        Ok((per.iter().sum(), per, sessions, last))
    }).unwrap_or((0, vec![0; 7], 0, None));
    let email_mismatch = match (&p.expected_email(), &identity.email) {
        (Some(want), Some(got)) => !want.eq_ignore_ascii_case(got),
        _ => false,
    };
    // a login that worked and now needs renewing is worth an event, once per change
    if p.kind == Kind::Subscription {
        let now_state = if identity.needs_login { "expired" } else if identity.logged_in { "ok" } else { "out" };
        let key = format!("login:{}", p.name);
        let before = ctx.db.kv_get(&key).ok().flatten();
        if before.as_deref() != Some(now_state) {
            if before.as_deref() == Some("ok") && now_state != "ok" {
                let _ = ctx.db.event("bad", "login", &format!("{}'s login {}. Log in again to use it.", p.label.as_deref().unwrap_or(&p.name), if now_state == "expired" { "expired" } else { "is gone" }));
            }
            let _ = ctx.db.kv_set(&key, now_state);
        }
    }
    let quota = h.quota(ctx, p);
    // every reading is kept (once), for the burn-down and the forecast on Overview
    if let Some(at) = quota.as_ref().and_then(|q| q.fetched_at) {
        for t in &quota.as_ref().unwrap().tiers { let _ = ctx.db.quota_point(&p.name, &t.id, t.used, t.resets_at, at); }
    }
    ProfileView {
        name: p.name.clone(),
        label: p.label.clone().unwrap_or_else(|| p.name.clone()),
        harness: p.harness.clone(),
        harness_label: h.label().into(),
        kind: p.kind,
        main: p.main,
        expected_email: p.email.clone(),
        email_mismatch,
        quota: quota.map(|q| q.as_of(now_ms())),
        running: h.running(ctx, p),
        identity,
        tokens_7d,
        days_7d,
        sessions_7d,
        last_used,
        home: h.home(ctx, p).display().to_string(),
        caps: h.caps(),
        base_url: p.base_url.clone(),
        model: p.model.clone(),
        route: p.route.clone(),
        daily_cap_usd: p.daily_cap_usd,
        created_at: p.created_at,
    }
}

impl Profile {
    fn expected_email(&self) -> Option<String> { self.email.clone().filter(|e| !e.is_empty()) }
}

pub fn list_views(ctx: &Ctx) -> Result<Vec<ProfileView>> {
    let reg = ctx.registry()?;
    // side by side: each account looks for its running processes and session files
    Ok(std::thread::scope(|s| {
        let views: Vec<_> = reg.profiles.iter().map(|p| s.spawn(move || profile_view(ctx, p))).collect();
        views.into_iter().map(|h| h.join().expect("profile view thread panicked")).collect()
    }))
}

/// Import new usage for every profile. Returns the number of new rows.
pub fn import_usage(ctx: &Ctx) -> Result<usize> {
    let reg = ctx.registry()?;
    // accounts side by side (each also reads its files on all cores): a first run reads gigabytes of logs
    let results: Vec<(String, Result<usize>)> = std::thread::scope(|s| {
        let running: Vec<_> = reg.profiles.iter().map(|p| s.spawn(move || (p.name.clone(), harness::of(p).import_usage(ctx, p)))).collect();
        running.into_iter().map(|h| h.join().unwrap_or_else(|_| (String::new(), Err(anyhow::anyhow!("import thread panicked"))))).collect()
    });
    let mut n = 0;
    for (name, r) in results {
        match r {
            Ok(k) => n += k,
            Err(e) => crate::log::warn(&format!("usage import for {name}: {e:#}")),
        }
    }
    Ok(n)
}

/// Refresh limits live where the user opted in (Claude) and the cached numbers are old.
pub fn refresh_quotas(ctx: &Ctx, force: bool) -> Vec<(String, String)> {
    let mut errors = vec![];
    let Ok(reg) = ctx.registry() else { return errors };
    let max_age = ctx.settings.quota_max_age_minutes as i64 * 60_000;
    for p in reg.profiles.iter().filter(|p| p.harness == "claude" && p.kind == Kind::Subscription) {
        let c = harness::claude::Claude;
        let fresh = c.quota(ctx, p).and_then(|q| q.fetched_at).map(|t| now_ms() - t < max_age).unwrap_or(false);
        if fresh && !force { continue; }
        if !force && !ctx.db.kv_get("live_quota").ok().flatten().map(|v| v == "1").unwrap_or(false) { continue; }
        if let Err(e) = c.live_quota(ctx, p) { errors.push((p.name.clone(), format!("{e:#}"))); }
    }
    errors
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRequest {
    pub name: String,
    pub harness: String,
    pub kind: Kind,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub main: bool,
}

pub fn add_profile(ctx: &Ctx, req: AddRequest) -> Result<ProfileView> {
    let h = harness::by_id(&req.harness).context("unknown harness")?;
    let caps = h.caps();
    match req.kind {
        Kind::ApiKey if !caps.api_key => bail!("{} does not take API keys", h.label()),
        Kind::Routed if !caps.routable => bail!("{} cannot be routed", h.label()),
        Kind::ApiKey if req.api_key.as_deref().unwrap_or("").trim().is_empty() => bail!("enter the API key"),
        Kind::Routed if req.route.is_none() => bail!("choose a route"),
        _ => {}
    }
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    if let Some(r) = &req.route {
        if !reg.routes.iter().any(|x| &x.id == r) { bail!("no route named '{r}'"); }
    }
    let p = Profile {
        name: req.name.trim().into(), label: None, harness: req.harness.clone(), kind: req.kind,
        email: req.email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty()),
        main: req.main, base_url: req.base_url.filter(|s| !s.is_empty()), model: req.model.filter(|s| !s.is_empty()),
        route: req.route, daily_cap_usd: None, created_at: 0,
    };
    reg.add_profile(p.clone())?;
    if let Some(k) = req.api_key.filter(|k| !k.trim().is_empty()) {
        crate::secrets::set_key(&crate::secrets::key_target(&p.name), k.trim())?;
    }
    h.prepare(ctx, &p)?;
    ctx.save_registry(&reg)?;
    Ok(profile_view(ctx, &p))
}

/// Remove a profile from the registry; `delete_home` also deletes its folder (never a main profile's).
pub fn remove_profile(ctx: &Ctx, name: &str, delete_home: bool) -> Result<()> {
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    let p = reg.remove_profile(name)?;
    // the folder first: deleting it can fail (a console still open), and then the key and login must still be there
    if delete_home && !p.main {
        let home = harness::of(&p).home(ctx, &p);
        if home.exists() {
            // links first, so the main profile's folders behind them are never touched
            for d in crate::shared::LINKED {
                let l = home.join(d);
                if crate::shared::is_link(&l) { crate::shared::remove_link(&l)?; }
            }
            std::fs::remove_dir_all(&home).with_context(|| format!("delete {} (close its consoles and try again)", home.display()))?;
        }
    }
    ctx.save_registry(&reg)?;
    if let Err(e) = crate::secrets::delete(&crate::secrets::key_target(name)) { crate::log::warn(&format!("removed {name}, but its API key stays in Credential Manager: {e:#}")); }
    // the main profile's saved login is the user's own Antigravity login: never deleted from here
    if p.harness == "agy" && !p.main {
        if let Err(e) = harness::agy::Agy.forget(ctx, name) { crate::log::warn(&format!("removed {name}, but its saved Antigravity login stays: {e:#}")); }
    }
    if let Err(e) = ctx.db.forget_profile(name) { crate::log::warn(&format!("removed {name}, but its usage history stays: {e:#}")); }
    Ok(())
}

pub fn update_profile(ctx: &Ctx, name: &str, email: Option<String>, base_url: Option<String>, model: Option<String>) -> Result<()> {
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    let p = reg.profiles.iter_mut().find(|p| p.name == name).context("no such profile")?;
    if let Some(e) = email { p.email = Some(e).filter(|e| !e.is_empty()); }
    if let Some(u) = base_url { p.base_url = Some(u).filter(|e| !e.is_empty()); }
    if let Some(m) = model { p.model = Some(m).filter(|e| !e.is_empty()); }
    ctx.save_registry(&reg)
}

/// A routed account's daily spend cap in US dollars; None or 0 removes it.
pub fn set_daily_cap(ctx: &Ctx, name: &str, cap: Option<f64>) -> Result<()> {
    if let Some(c) = cap { if !c.is_finite() || c < 0.0 { bail!("the cap is an amount in dollars, 0 or more"); } }
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    let p = reg.profiles.iter_mut().find(|p| p.name == name).context("no such profile")?;
    if p.kind != Kind::Routed { bail!("only routed accounts go through the router, so only they can have a cap"); }
    p.daily_cap_usd = cap.filter(|c| *c > 0.0);
    ctx.save_registry(&reg)
}

/// Rename an account in the app. Only the shown name changes: the folder, key and `hrm run <id>` keep the id,
/// so a registry shared with another computer still finds that computer's folders. Empty goes back to the id.
pub fn set_label(ctx: &Ctx, name: &str, label: &str) -> Result<()> {
    let label = label.trim();
    if label.chars().count() > 60 { bail!("keep the name under 60 characters"); }
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    let shown = |p: &Profile| p.label.clone().unwrap_or_else(|| p.name.clone());
    if let Some(o) = reg.profiles.iter().find(|p| p.name != name && (shown(p).eq_ignore_ascii_case(label) || p.name.eq_ignore_ascii_case(label))) {
        bail!("{} already uses the name '{label}'", o.name);
    }
    let p = reg.profiles.iter_mut().find(|p| p.name == name).context("no such profile")?;
    p.label = Some(label.to_string()).filter(|l| !l.is_empty() && l != name);
    ctx.save_registry(&reg)
}

pub fn set_api_key(ctx: &Ctx, name: &str, key: &str) -> Result<()> {
    let reg = ctx.registry()?;
    let p = reg.get(name)?;
    if p.kind != Kind::ApiKey { bail!("'{name}' is not an API-key profile"); }
    crate::secrets::set_key(&crate::secrets::key_target(name), key.trim())
}

/// Spec for running a profile, after making it the active login where the harness needs that.
pub fn launch_spec(ctx: &Ctx, name: &str, args: &[String]) -> Result<crate::launch::LaunchSpec> {
    let reg = ctx.registry()?;
    let p = reg.get(name)?;
    let h = harness::of(p);
    h.prepare(ctx, p)?;
    if !h.caps().parallel_accounts {
        h.activate(ctx, p)?;
    }
    h.launch(ctx, p, args)
}

/// Open a terminal running `hrm run <profile>` (optionally in a folder, with extra args for the harness).
pub fn start(ctx: &Ctx, name: &str, cwd: Option<PathBuf>, extra: &[String]) -> Result<()> {
    let reg = ctx.registry()?;
    let p = reg.get(name)?;
    // say so here, not in the terminal that `hrm run` would open and fail in
    if p.harness == "agy" { harness::agy::Agy.check_switch(ctx, p)?; }
    let mut args = vec!["run".to_string(), name.to_string()];
    if !extra.is_empty() {
        args.push("--".into());
        args.extend(extra.iter().cloned());
    }
    let title = format!("{} · {}", harness::of(p).label(), name);
    crate::launch::open_terminal(&ctx.settings.terminal, &title, &args, cwd.as_deref())
}

pub fn resume(ctx: &Ctx, profile: &str, session_id: &str, cwd: Option<PathBuf>) -> Result<()> {
    let reg = ctx.registry()?;
    let p = reg.get(profile)?;
    let args = harness::of(p).resume_args(session_id);
    if args.is_empty() { bail!("{} cannot resume sessions", harness::of(p).label()); }
    start(ctx, profile, cwd, &args)
}

// ---------- usage ----------

#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    pub days: Vec<String>,
    /// profile -> tokens per day (input + output)
    pub by_profile: BTreeMap<String, Vec<i64>>,
    pub by_model: Vec<(String, i64)>,
    pub by_project: Vec<(String, i64)>,
    pub total: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

pub fn usage_summary(ctx: &Ctx, days_back: i64, profile: Option<&str>) -> Result<UsageSummary> {
    let days = last_days(days_back);
    let from = days[0].clone();
    ctx.db.with(|c| {
        let pf = profile; // NULL means every profile; an exact match, so `dev_1` never counts `dev-1`
        let mut by_profile: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        let mut st = c.prepare("select profile, day, sum(input+output) from usage where day>=?1 and (?2 is null or profile = ?2) group by profile, day")?;
        for r in st.query_map(rusqlite::params![from, pf], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?.flatten() {
            let e = by_profile.entry(r.0).or_insert_with(|| vec![0; days.len()]);
            if let Some(i) = days.iter().position(|d| *d == r.1) { e[i] += r.2; }
        }
        let pairs = |sql: &str| -> rusqlite::Result<Vec<(String, i64)>> {
            let mut st = c.prepare(sql)?;
            let v = st.query_map(rusqlite::params![from, pf], |r| Ok((r.get(0)?, r.get(1)?)))?.collect();
            v
        };
        let by_model = pairs("select model, sum(input+output) t from usage where day>=?1 and (?2 is null or profile = ?2) group by model order by t desc")?;
        let by_project = pairs("select project, sum(input+output) t from usage where day>=?1 and (?2 is null or profile = ?2) group by project order by t desc limit 20")?;
        let (total, cr, cw): (i64, i64, i64) = c.query_row("select coalesce(sum(input+output),0), coalesce(sum(cache_read),0), coalesce(sum(cache_write),0) from usage where day>=?1 and (?2 is null or profile = ?2)", rusqlite::params![from, pf], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(UsageSummary { days: days.clone(), by_profile, by_model, by_project, total, cache_read: cr, cache_write: cw })
    })
}

/// Tokens per project for one profile over the last `days_back` days.
pub fn profile_projects(ctx: &Ctx, profile: &str, days_back: i64) -> Result<Vec<(String, String, i64)>> {
    let reg = ctx.registry()?;
    let s = usage_summary(ctx, days_back, Some(profile))?;
    let mut res = Resolver::new(&reg.projects);
    let mut merged: BTreeMap<String, (String, i64)> = BTreeMap::new();
    for (dir, t) in s.by_project {
        let info = res.resolve(&dir);
        let e = merged.entry(info.key).or_insert((info.name, 0));
        e.1 += t;
    }
    let mut v: Vec<(String, String, i64)> = merged.into_iter().map(|(k, (n, t))| (k, n, t)).collect();
    v.sort_by(|a, b| b.2.cmp(&a.2));
    Ok(v)
}

// ---------- sessions ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFilter {
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub query: Option<String>,
    /// Only sessions a console is working in now.
    #[serde(default)]
    pub open: Option<bool>,
    /// Only sessions of one project, by its key.
    #[serde(default)]
    pub project: Option<String>,
    /// Only sessions carrying this tag, their own or their project's.
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectGroup {
    pub key: String,
    pub name: String,
    pub root: String,
    pub remote: Option<String>,
    pub tokens: i64,
    pub last_active_at: i64,
    pub sessions: Vec<SessionMeta>,
}

pub fn sessions(ctx: &Ctx, f: &SessionFilter) -> Result<Vec<SessionMeta>> {
    let reg = ctx.registry()?;
    let since = f.days.map(|d| now_ms() - d * 86_400_000).unwrap_or(0);
    let mut res = Resolver::new(&reg.projects);
    let (stags, ptags) = (ctx.db.tags_of("session").unwrap_or_default(), ctx.db.tags_of("project").unwrap_or_default());
    let mut out = vec![];
    for p in &reg.profiles {
        if f.harness.as_deref().map(|h| h != p.harness).unwrap_or(false) { continue; }
        if f.profile.as_deref().map(|x| x != p.name).unwrap_or(false) { continue; }
        let h = harness::of(p);
        let running = h.running(ctx, p);
        let list = match h.sessions(ctx, p, since) {
            Ok(l) => l,
            Err(e) => { crate::log::warn(&format!("sessions for {}: {e:#}", p.name)); continue; }
        };
        // the most recently active sessions are the ones the open consoles are working in
        let mut recent: Vec<&SessionMeta> = list.iter().collect();
        recent.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
        let open_ids: Vec<String> = running.iter().filter_map(|r| r.session_id.clone())
            .chain(recent.iter().take(running.iter().filter(|r| r.session_id.is_none()).count()).map(|s| s.id.clone())).collect();
        for mut s in list {
            if s.last_active_at < since { continue; }
            if let Some(q) = &f.query {
                let q = q.to_lowercase();
                if !s.title.to_lowercase().contains(&q) && !s.cwd.to_lowercase().contains(&q) && !s.id.to_lowercase().contains(&q) { continue; }
            }
            let info = res.resolve(&s.cwd);
            s.project_key = info.key;
            s.project_name = info.name;
            s.open = open_ids.contains(&s.id);
            if f.open == Some(true) && !s.open { continue; }
            if f.project.as_deref().map(|k| k != s.project_key).unwrap_or(false) { continue; }
            s.tags = stags.get(&format!("{}/{}", s.profile, s.id)).cloned().unwrap_or_default();
            s.project_tags = ptags.get(&s.project_key).cloned().unwrap_or_default();
            if let Some(t) = &f.tag { if !s.tags.contains(t) && !s.project_tags.contains(t) { continue; } }
            out.push(s);
        }
    }
    out.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
    Ok(out)
}

/// Sessions grouped by "project" (default), "date", "account" or "tool".
pub fn grouped_sessions(ctx: &Ctx, f: &SessionFilter, by: &str) -> Result<Vec<ProjectGroup>> {
    let reg = ctx.registry()?;
    let list = sessions(ctx, f)?;
    let mut res = Resolver::new(&reg.projects);
    let mut groups: Vec<ProjectGroup> = vec![];
    let mut index: HashMap<String, usize> = HashMap::new();
    for s in list {
        // by tag: a session sits under each of its tags (its own and its project's); none puts it under "untagged"
        if by == "tag" {
            let mut tags: Vec<String> = s.tags.iter().chain(s.project_tags.iter()).cloned().collect();
            tags.sort();
            tags.dedup();
            if tags.is_empty() { tags.push(String::new()); }
            for t in tags {
                let name = if t.is_empty() { "untagged".to_string() } else { t.clone() };
                let gi = *index.entry(format!("tag:{t}")).or_insert_with(|| { groups.push(ProjectGroup { key: t.clone(), name, root: String::new(), remote: None, tokens: 0, last_active_at: 0, sessions: vec![] }); groups.len() - 1 });
                let g = &mut groups[gi];
                g.tokens += s.tokens;
                g.last_active_at = g.last_active_at.max(s.last_active_at);
                g.sessions.push(s.clone());
            }
            continue;
        }
        let (key, name, root, remote) = match by {
            "date" => {
                let d = chrono::DateTime::from_timestamp_millis(s.last_active_at).map(|d| d.with_timezone(&chrono::Local).format("%A %-d %B %Y").to_string()).unwrap_or_default();
                (d.clone(), d, String::new(), None)
            }
            "account" => (s.profile.clone(), s.profile.clone(), String::new(), None),
            "tool" => (s.harness.clone(), harness::by_id(&s.harness).map(|h| h.label().to_string()).unwrap_or_default(), String::new(), None),
            _ => {
                let i = res.resolve(&s.cwd);
                (i.key, i.name, i.root, i.remote)
            }
        };
        let gi = *index.entry(key.clone()).or_insert_with(|| {
            groups.push(ProjectGroup { key, name, root, remote, tokens: 0, last_active_at: 0, sessions: vec![] });
            groups.len() - 1
        });
        let g = &mut groups[gi];
        g.tokens += s.tokens;
        g.last_active_at = g.last_active_at.max(s.last_active_at);
        g.sessions.push(s);
    }
    groups.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
    Ok(groups)
}

/// A project as the Projects page shows it: its sessions, tokens, cost at list prices, accounts, activity and a day series.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectStat {
    pub key: String,
    pub name: String,
    pub root: String,
    pub remote: Option<String>,
    pub sessions: usize,
    /// Tokens of its sessions, as the Sessions page counts them.
    pub tokens: i64,
    /// API list-price cost of its requests in range; None when no model's price is known.
    pub cost: Option<f64>,
    /// Share of its request tokens that had a price.
    pub priced_share: f64,
    pub accounts: Vec<String>,
    pub last_active_at: i64,
    pub live: usize,
    /// Request tokens per calendar day, oldest first, for the range.
    pub series: Vec<i64>,
    pub pinned: bool,
    pub tags: Vec<String>,
}

/// Every project with sessions in the last `days` days (None: all time), busiest first.
pub fn projects(ctx: &Ctx, days: Option<i64>) -> Result<Vec<ProjectStat>> {
    let reg = ctx.registry()?;
    let f = SessionFilter { days, ..Default::default() };
    let groups = grouped_sessions(ctx, &f, "project")?;
    // cost and the day series come from the request log, priced per model as Overview does
    let prices = crate::pricing::load(ctx);
    let span = days.unwrap_or(30).clamp(1, 53 * 7);
    let day_list = last_days(span);
    let from = from_day(days);
    let rows: Vec<(String, String, String, i64, i64, i64, i64)> = ctx.db.with(|c| {
        let mut st = c.prepare("select project, day, model, sum(input), sum(output), sum(cache_read), sum(cache_write) from usage where day >= ?1 group by project, day, model")?;
        let r = st.query_map([&from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(r)
    })?;
    let mut res = Resolver::new(&reg.projects);
    let mut by_key: HashMap<String, (Option<f64>, i64, i64, Vec<i64>)> = HashMap::new(); // cost, priced tokens, all tokens, series
    for (dir, day, model, i, out, cr, cw) in rows {
        let key = res.resolve(&dir).key;
        let e = by_key.entry(key).or_insert_with(|| (None, 0, 0, vec![0; day_list.len()]));
        let tokens = i + out + cr + cw;
        e.2 += tokens;
        if let Some(c) = crate::pricing::find(&prices, &model).map(|p| p.cost(i, out, cr, cw)) { e.0 = Some(e.0.unwrap_or(0.0) + c); e.1 += tokens; }
        if let Some(di) = day_list.iter().position(|d| *d == day) { e.3[di] += tokens; }
    }
    let pins = &ctx.settings.pinned_projects;
    let ptags = ctx.db.tags_of("project").unwrap_or_default();
    Ok(groups.into_iter().map(|g| {
        let (cost, priced, all, series) = by_key.remove(&g.key).unwrap_or((None, 0, 0, vec![0; day_list.len()]));
        let mut accounts: Vec<String> = g.sessions.iter().map(|s| s.profile.clone()).collect();
        accounts.sort();
        accounts.dedup();
        ProjectStat { pinned: pins.contains(&g.key), tags: ptags.get(&g.key).cloned().unwrap_or_default(), key: g.key, name: g.name, root: g.root, remote: g.remote, sessions: g.sessions.len(), tokens: g.tokens, cost,
            priced_share: if all > 0 { priced as f64 / all as f64 } else { 0.0 }, accounts, last_active_at: g.last_active_at, live: g.sessions.iter().filter(|s| s.open).count(), series }
    }).collect())
}

pub fn transcript(ctx: &Ctx, profile: &str, session_id: &str) -> Result<Transcript> {
    let reg = ctx.registry()?;
    let p = reg.get(profile)?;
    let f = SessionFilter { profile: Some(profile.into()), ..Default::default() };
    let s = sessions(ctx, &f)?.into_iter().find(|s| s.id == session_id).context("session not found")?;
    let mut t = harness::of(p).transcript(ctx, p, &s)?;
    t.meta = s;
    Ok(t)
}

pub fn alias_project(ctx: &Ctx, key: &str, name: Option<String>, merge_into: Option<String>) -> Result<()> {
    let _lock = ctx.lock_registry();
    let mut reg = ctx.registry()?;
    reg.projects.retain(|a| a.key != key);
    if name.is_some() || merge_into.is_some() {
        reg.projects.push(crate::registry::ProjectAlias { key: key.into(), name, merge_into });
    }
    ctx.save_registry(&reg)
}

// ---------- in-app login ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct LoginStatus {
    pub id: String,
    pub profile: String,
    /// "waiting", "done", "failed"
    pub state: String,
    pub url: Option<String>,
    pub message: String,
    pub email: Option<String>,
    pub email_mismatch: bool,
}

struct Flow {
    child: Child,
    status: Arc<Mutex<LoginStatus>>,
}

static FLOWS: std::sync::OnceLock<Mutex<HashMap<String, Flow>>> = std::sync::OnceLock::new();

fn flows() -> &'static Mutex<HashMap<String, Flow>> { FLOWS.get_or_init(|| Mutex::new(HashMap::new())) }

/// Start a login for a profile with its output captured; the authorize link is picked out for the UI.
/// For harnesses whose login is a full-screen prompt, the caller opens a terminal instead.
pub fn login_start(ctx: &Ctx, name: &str) -> Result<LoginStatus> {
    let reg = ctx.registry()?;
    let p = reg.get(name)?.clone();
    let h = harness::of(&p);
    h.prepare(ctx, &p)?;
    if p.harness == "agy" { harness::agy::Agy.prepare_login(ctx)?; }
    let spec = h.login(ctx, &p)?;
    let mut cmd = spec.command();
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; cmd.creation_flags(0x0800_0000); } // no console window
    let mut child = cmd.spawn().with_context(|| format!("start {}", spec.program.display()))?;
    let id = format!("login-{}-{}", name, now_ms());
    let status = Arc::new(Mutex::new(LoginStatus { id: id.clone(), profile: name.into(), state: "waiting".into(), message: "Waiting for the sign-in link…".into(), ..Default::default() }));
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
        let st = status.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Some(u) = line.split_whitespace().find(|w| w.starts_with("https://")) {
                    let mut s = st.lock().unwrap();
                    if s.url.is_none() {
                        s.url = Some(u.trim_end_matches(['.', ',', ')']).to_string());
                        s.message = "Finish signing in in your browser. If it does not return here, paste the code it shows.".into();
                    }
                }
            }
        });
    }
    let snapshot = status.lock().unwrap().clone();
    flows().lock().unwrap().insert(id, Flow { child, status });
    Ok(snapshot)
}

/// Paste a code or callback URL when the browser cannot hand the login back by itself.
pub fn login_submit(id: &str, code: &str) -> Result<()> {
    let mut map = flows().lock().unwrap();
    let f = map.get_mut(id).context("this sign-in has ended")?;
    let stdin = f.child.stdin.as_mut().context("this sign-in does not accept a code")?;
    writeln!(stdin, "{}", code.trim())?;
    stdin.flush()?;
    Ok(())
}

pub fn login_poll(ctx: &Ctx, id: &str) -> Result<LoginStatus> {
    let mut map = flows().lock().unwrap();
    let f = map.get_mut(id).context("unknown sign-in")?;
    if let Some(code) = f.child.try_wait()? {
        let mut s = f.status.lock().unwrap();
        if s.state == "waiting" {
            let reg = ctx.registry()?;
            let p = reg.get(&s.profile)?.clone();
            if p.harness == "agy" { let _ = harness::agy::Agy.finish_login(ctx, &p); }
            let ident = harness::of(&p).identity(ctx, &p);
            if code.success() && ident.logged_in {
                s.state = "done".into();
                s.email = ident.email.clone();
                s.email_mismatch = matches!((&p.email, &ident.email), (Some(w), Some(g)) if !w.eq_ignore_ascii_case(g));
                s.message = match (&ident.email, s.email_mismatch) {
                    (Some(e), true) => format!("Signed in as {e}, which is not the email you entered."),
                    (Some(e), false) => format!("Signed in as {e}."),
                    _ => "Signed in.".into(),
                };
            } else {
                s.state = "failed".into();
                s.message = format!("Sign-in did not finish (exit code {}).", code.code().unwrap_or(-1));
            }
        }
        return Ok(s.clone());
    }
    let s = f.status.lock().unwrap().clone();
    Ok(s)
}

pub fn login_cancel(id: &str) {
    if let Some(mut f) = flows().lock().unwrap().remove(id) {
        let _ = f.child.kill();
    }
}

// ---------- routing ----------

/// The router address and launch token for a routed profile; starts the router if it is not running.
pub fn router_endpoint(ctx: &Ctx, p: &Profile) -> Result<(String, String)> {
    let route = p.route.clone().context("this profile has no route")?;
    let token = router_token(ctx)?;
    let port = ctx.settings.router_port;
    ensure_router(ctx)?;
    Ok((format!("http://127.0.0.1:{port}/r/{route}/{}", p.name), token))
}

pub fn router_token(ctx: &Ctx) -> Result<String> {
    if let Some(t) = ctx.db.kv_get("router:token")? { return Ok(t); }
    let t = random_token();
    ctx.db.kv_set("router:token", &t)?;
    Ok(t)
}

pub fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..4u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(now_ms() as u64 ^ (std::process::id() as u64) << 16 ^ i);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

pub fn router_running(ctx: &Ctx) -> bool { router_answer(ctx).map(|r| r.ends_with("\r\n\r\nok")).unwrap_or(false) }

/// What answers `GET /health` on the router port, if anything listens there at all.
fn router_answer(ctx: &Ctx) -> Option<String> {
    use std::io::{Read, Write};
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", ctx.settings.router_port).parse().ok()?;
    // nothing listens when the port is free; asking a closed port on Windows waits out the whole timeout
    if std::net::TcpListener::bind(addr).is_ok() { return None; }
    let t = std::time::Duration::from_millis(500);
    let mut s = std::net::TcpStream::connect_timeout(&addr, t).ok()?;
    s.set_read_timeout(Some(t)).ok()?;
    s.write_all(b"GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").ok()?;
    let mut out = String::new();
    let _ = s.take(4096).read_to_string(&mut out);
    Some(out)
}

/// Stop the background router. Returns false when it was not running. The saved process id is checked to
/// still be an `hrm` process first: after a crash or reboot Windows may have given it to another program.
pub fn router_stop(ctx: &Ctx) -> Result<bool> {
    let pid = ctx.db.kv_get("router:pid")?.and_then(|p| p.parse::<u32>().ok());
    let ours = pid.filter(|p| crate::procs::by_name(&["hrm"], None).iter().any(|(q, _, _)| q == p));
    if let Some(p) = ours { crate::procs::kill(p)?; }
    ctx.db.kv_set("router:pid", "")?;
    Ok(ours.is_some())
}

/// Start `hrm router serve` in the background when nothing listens on the router port yet.
pub fn ensure_router(ctx: &Ctx) -> Result<()> {
    if router_running(ctx) { return Ok(()); }
    // something else answers on the port: a router started now could not bind it, and the tools' requests
    // (with the router token) would go to that program
    if router_answer(ctx).is_some() {
        bail!("another program is using port {}; choose another router port in Settings", ctx.settings.router_port);
    }
    let mut c = std::process::Command::new(crate::launch::hrm_exe());
    c.args(["router", "serve"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000 | 0x0000_0008); } // no window, detached
    without_inherited_std(|| c.spawn()).context("start the router")?;
    for _ in 0..50 {
        if router_running(ctx) { return Ok(()); }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!("the router did not start on port {}", ctx.settings.router_port)
}

/// Spawn with this process's console handles marked non-inheritable: Windows hands every inheritable handle
/// to a child, and a long-lived router holding our stdout would keep whoever reads it waiting forever.
#[cfg(windows)]
fn without_inherited_std<T>(f: impl FnOnce() -> T) -> T {
    use windows_sys::Win32::Foundation::{GetHandleInformation, SetHandleInformation, HANDLE_FLAG_INHERIT};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    let hs: Vec<_> = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].iter().map(|k| unsafe { GetStdHandle(*k) }).collect();
    let mut prev = vec![];
    for h in &hs {
        let mut flags = 0u32;
        unsafe { GetHandleInformation(*h, &mut flags); SetHandleInformation(*h, HANDLE_FLAG_INHERIT, 0); }
        prev.push(flags);
    }
    let r = f();
    for (h, flags) in hs.iter().zip(prev) { unsafe { SetHandleInformation(*h, HANDLE_FLAG_INHERIT, flags & HANDLE_FLAG_INHERIT); } }
    r
}

#[cfg(not(windows))]
fn without_inherited_std<T>(f: impl FnOnce() -> T) -> T { f() }

pub fn registry_snapshot(ctx: &Ctx) -> Result<Registry> { ctx.registry() }

// ---------- home: totals, heatmap, request log ----------

#[derive(Debug, Clone, Serialize)]
pub struct HeatDay {
    pub day: String,
    pub tokens: i64,
    pub requests: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    /// API list-price cost of the tokens in range; None when no model's price is known.
    pub cost: Option<f64>,
    pub requests: i64,
    /// Every token processed: input, output, cache reads and cache writes.
    pub tokens: i64,
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// Share of prompt tokens served from cache.
    pub cache_hit_rate: f64,
    pub today_cost: Option<f64>,
    pub today_tokens: i64,
    /// Share of tokens whose model has a known price.
    pub priced_share: f64,
    /// The last 53 weeks, oldest first, ending today.
    pub heatmap: Vec<HeatDay>,
}

fn harness_clause(h: Option<&str>) -> (&'static str, String) {
    match h {
        Some(h) if !h.is_empty() => ("harness = ?2", h.to_string()),
        _ => ("?2 = ?2", String::new()),
    }
}

/// First calendar day of the last `days` days (None: all time). Every Home number uses it, so "today" is the same day everywhere.
fn from_day(days: Option<i64>) -> String {
    let today = chrono::Local::now().date_naive();
    days.map(|d| (today - chrono::Duration::days(d.max(1) - 1)).format("%Y-%m-%d").to_string()).unwrap_or_else(|| "0000".into())
}

/// Totals for the last `days` days (None: all time) and a 53-week heatmap, optionally for one tool.
pub fn overview(ctx: &Ctx, days: Option<i64>, harness: Option<&str>) -> Result<Overview> {
    let prices = crate::pricing::load(ctx);
    let today = chrono::Local::now().date_naive();
    let from = from_day(days);
    let heat_days = last_days(53 * 7);
    let (hc, hv) = harness_clause(harness);
    ctx.db.with(|c| {
        let mut st = c.prepare(&format!("select day, model, count(*), sum(input), sum(output), sum(cache_read), sum(cache_write) from usage where day >= ?1 and {hc} group by day, model"))?;
        let earliest = if from < heat_days[0] { from.clone() } else { heat_days[0].clone() };
        let rows: Vec<(String, String, i64, i64, i64, i64, i64)> = st.query_map(rusqlite::params![earliest, hv], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?.collect::<rusqlite::Result<_>>()?;
        let today_s = today.format("%Y-%m-%d").to_string();
        let mut o = Overview { cost: None, requests: 0, tokens: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, cache_hit_rate: 0.0, today_cost: None, today_tokens: 0, priced_share: 0.0,
            heatmap: heat_days.iter().map(|d| HeatDay { day: d.clone(), tokens: 0, requests: 0, cost: 0.0 }).collect() };
        let mut priced_tokens = 0i64;
        for (day, model, n, i, out, cr, cw) in rows {
            let tokens = i + out + cr + cw;
            let cost = crate::pricing::find(&prices, &model).map(|p| p.cost(i, out, cr, cw));
            if let Some(h) = o.heatmap.iter_mut().find(|h| h.day == day) {
                h.tokens += tokens;
                h.requests += n;
                h.cost += cost.unwrap_or(0.0);
            }
            if day == today_s {
                o.today_tokens += tokens;
                if let Some(x) = cost { o.today_cost = Some(o.today_cost.unwrap_or(0.0) + x); }
            }
            if day < from { continue; }
            o.requests += n;
            o.input += i;
            o.output += out;
            o.cache_read += cr;
            o.cache_write += cw;
            o.tokens += tokens;
            if let Some(x) = cost { o.cost = Some(o.cost.unwrap_or(0.0) + x); priced_tokens += tokens; }
        }
        let prompt = o.input + o.cache_read + o.cache_write;
        o.cache_hit_rate = if prompt > 0 { o.cache_read as f64 / prompt as f64 } else { 0.0 };
        o.priced_share = if o.tokens > 0 { priced_tokens as f64 / o.tokens as f64 } else { 0.0 };
        Ok(o)
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestRow {
    pub ts: i64,
    pub harness: String,
    pub profile: String,
    pub model: String,
    pub project: String,
    pub session_id: String,
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub cost: Option<f64>,
    pub ttft_ms: Option<i64>,
    pub gen_ms: Option<i64>,
}

/// Every model response, newest first: (total count, one page of rows).
pub fn request_log(ctx: &Ctx, days: Option<i64>, harness: Option<&str>, limit: i64, offset: i64) -> Result<(i64, Vec<RequestRow>)> {
    request_log_since(ctx, days, None, harness, limit, offset)
}

/// The same, from `since_ms` (epoch ms) when given instead of whole calendar days.
pub fn request_log_since(ctx: &Ctx, days: Option<i64>, since_ms: Option<i64>, harness: Option<&str>, limit: i64, offset: i64) -> Result<(i64, Vec<RequestRow>)> {
    let prices = crate::pricing::load(ctx);
    let since = from_day(days);
    let since_ms = since_ms.unwrap_or(0);
    let (hc, hv) = harness_clause(harness);
    ctx.db.with(|c| {
        let total: i64 = c.query_row(&format!("select count(*) from usage where day >= ?1 and {hc} and ts >= ?3"), rusqlite::params![since, hv, since_ms], |r| r.get(0))?;
        let mut st = c.prepare(&format!("select ts, harness, profile, model, project, session_id, input, output, cache_read, cache_write, ttft_ms, gen_ms from usage where day >= ?1 and {hc} and ts >= ?5 order by ts desc limit ?3 offset ?4"))?;
        let rows = st.query_map(rusqlite::params![since, hv, limit, offset, since_ms], |r| {
            let (input, output, cr, cw): (i64, i64, i64, i64) = (r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?);
            let model: String = r.get(3)?;
            let cost = crate::pricing::find(&prices, &model).map(|p| p.cost(input, output, cr, cw));
            Ok(RequestRow { ts: r.get(0)?, harness: r.get(1)?, profile: r.get(2)?, model, project: r.get(4)?, session_id: r.get(5)?, input, output, cache_read: cr, cache_write: cw, cost, ttft_ms: r.get(10)?, gen_ms: r.get(11)? })
        })?.collect::<rusqlite::Result<_>>()?;
        Ok((total, rows))
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Breakdown {
    pub key: String,
    pub harness: String,
    pub requests: i64,
    pub tokens: i64,
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cost: Option<f64>,
    pub cache_hit_rate: f64,
}

/// Requests, tokens and cost per account ("profile") or per model, biggest first.
pub fn breakdown(ctx: &Ctx, by: &str, days: Option<i64>, harness: Option<&str>) -> Result<Vec<Breakdown>> { breakdown_of(ctx, by, days, harness, None) }

/// `breakdown`, kept to one account when `profile` is given.
pub fn breakdown_of(ctx: &Ctx, by: &str, days: Option<i64>, harness: Option<&str>, profile: Option<&str>) -> Result<Vec<Breakdown>> {
    let prices = crate::pricing::load(ctx);
    let since = from_day(days);
    let (hc, hv) = harness_clause(harness);
    let (pc, pv) = match profile { Some(p) if !p.is_empty() => ("profile = ?3", p.to_string()), _ => ("?3 = ?3", String::new()) };
    let key = if by == "model" { "model" } else { "profile" };
    ctx.db.with(|c| {
        let mut st = c.prepare(&format!("select {key}, model, max(harness), count(*), sum(input), sum(output), sum(cache_read), sum(cache_write) from usage where day >= ?1 and {hc} and {pc} group by {key}, model"))?;
        let mut map: BTreeMap<String, Breakdown> = BTreeMap::new();
        let mut prompt: HashMap<String, (i64, i64)> = HashMap::new();
        for r in st.query_map(rusqlite::params![since, hv, pv], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?, r.get::<_, i64>(7)?)))?.flatten() {
            let (k, model, h, n, i, o, cr, cw) = r;
            let e = map.entry(k.clone()).or_insert(Breakdown { key: k.clone(), harness: h, requests: 0, tokens: 0, input: 0, output: 0, cache_read: 0, cost: None, cache_hit_rate: 0.0 });
            e.requests += n;
            e.input += i;
            e.output += o;
            e.cache_read += cr;
            e.tokens += i + o + cr + cw;
            if let Some(p) = crate::pricing::find(&prices, &model) { e.cost = Some(e.cost.unwrap_or(0.0) + p.cost(i, o, cr, cw)); }
            let pe = prompt.entry(k).or_insert((0, 0));
            pe.0 += cr;
            pe.1 += i + cr + cw;
        }
        let mut v: Vec<Breakdown> = map.into_values().map(|mut b| { let (cr, all) = prompt[&b.key]; b.cache_hit_rate = if all > 0 { cr as f64 / all as f64 } else { 0.0 }; b }).collect();
        v.sort_by(|a, b| b.tokens.cmp(&a.tokens));
        Ok(v)
    })
}
