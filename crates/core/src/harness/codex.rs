//! Codex CLI. Each profile is its own `CODEX_HOME` with its own `auth.json`.
//! Limits come from the `rate_limits` Codex writes into its session logs, so no token is used.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::Value;

use super::parse::*;
use super::{base_env, Harness, ItemPaths};
use crate::launch::{resolve_program, LaunchSpec};
use crate::model::*;
use crate::registry::{Kind, Profile};
use crate::util::{read_json, truncate};
use crate::Ctx;

pub struct Codex;

impl Codex {
    fn rollouts(&self, ctx: &Ctx, p: &Profile) -> Vec<(PathBuf, i64, u64)> {
        let home = self.home(ctx, p);
        let mut out = vec![];
        crate::util::walk_files_sig(&home.join("sessions"), &is_rollout, &mut out);
        crate::util::walk_files_sig(&home.join("archived_sessions"), &is_rollout, &mut out);
        out
    }

    /// The sessions of `files` (each with its time and size), from their cached records where a file is as it was.
    fn scan(&self, ctx: &Ctx, p: &Profile, files: Vec<(PathBuf, i64, u64)>, cached: HashMap<String, (i64, i64, SessionMeta)>, since: i64) -> Result<Vec<SessionMeta>> {
        let mut out = vec![];
        let mut todo = vec![];
        for (f, mtime, len) in files {
            if mtime < since { continue; }
            let key = format!("codex|{}|{}", p.name, f.display());
            match cached.get(&key) {
                Some((m, l, meta)) if *m == mtime && *l == len as i64 => out.push(meta.clone()),
                // a rollout still being written: its last record now, the file parsed again in the background
                Some((_, _, meta)) if !parse_now() => {
                    out.push(stale(meta, mtime));
                    let name = p.name.clone();
                    refresh_session_later(ctx, key, f, mtime, len, move |f| parse_rollout_file(f, &name, false).ok().map(|t| t.meta));
                }
                _ => todo.push((f, mtime, len)),
            }
        }
        each_file_parallel_sig(&todo, |f, mtime, len| {
            Some((mtime, len, parse_rollout_file(f, &p.name, false).ok()?.meta))
        }, |r| {
            let Some((mtime, len, meta)) = r.filter(|(_, _, m)| !m.id.is_empty()) else { return Ok(()) };
            ctx.db.put_session(&format!("codex|{}|{}", p.name, meta.source_path), &meta, mtime, len as i64)?;
            out.push(meta);
            Ok(())
        })?;
        Ok(out)
    }
}

/// A rollout by its name: `rollout-….jsonl`, compressed or not.
fn is_rollout(f: &Path) -> bool {
    f.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("rollout-") && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst"))).unwrap_or(false)
}

fn window_label(minutes: i64) -> (String, String) {
    match minutes {
        300 => ("session".into(), "5-hour session".into()),
        10080 => ("week".into(), "Week".into()),
        m if m % 1440 == 0 => (format!("window:{m}"), format!("{} days", m / 1440)),
        m => (format!("window:{m}"), format!("{} hours", m / 60)),
    }
}

/// Codex `rate_limits` (from a token_count event) to tiers.
pub fn parse_rate_limits(rl: &Value, at: i64) -> Quota {
    let mut tiers = vec![];
    for key in ["primary", "secondary"] {
        let w = &rl[key];
        if !w.is_object() { continue; }
        let (id, label) = window_label(w["window_minutes"].as_i64().unwrap_or(0));
        tiers.push(QuotaTier { id, label, used: w["used_percent"].as_f64().unwrap_or(0.0), resets_at: w["resets_at"].as_i64().map(|s| s * 1000) });
    }
    Quota { tiers, fetched_at: Some(at), source: "codex".into() }
}

/// The limits a rollout line carries (a token_count event with `rate_limits`), dated by the line.
fn limits_of(line: &str) -> Option<Quota> {
    if !line.contains("\"rate_limits\"") { return None; }
    let v: Value = serde_json::from_str(line).ok()?;
    let rl = &v["payload"]["rate_limits"];
    (rl.is_object() && (rl["primary"].is_object() || rl["secondary"].is_object())).then(|| parse_rate_limits(rl, ts_ms(v["timestamp"].as_str().unwrap_or(""))))
}

/// Set once an account's rollouts were read for a reading the import had not kept (`quota`).
pub(crate) fn limits_mark(profile: &str) -> String { format!("codex:limits:{profile}") }

/// Public claims of the id_token Codex saved, for showing which ChatGPT account and plan a profile is.
/// The token itself is never sent anywhere.
fn id_claims(auth: &Value) -> Option<Value> {
    let tok = auth["tokens"]["id_token"].as_str()?;
    let part = tok.split('.').nth(1)?;
    let bytes = base64_url_decode(part)?;
    serde_json::from_slice(&bytes).ok()
}

fn base64_url_decode(s: &str) -> Option<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = vec![];
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = T.iter().position(|x| *x == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl Harness for Codex {
    fn id(&self) -> &'static str { "codex" }
    fn label(&self) -> &'static str { "Codex" }
    fn caps(&self) -> Caps {
        Caps { parallel_accounts: true, api_key: true, quota: true, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: true }
    }
    fn binary(&self) -> &'static str { "codex" }
    fn default_home(&self, ctx: &Ctx) -> PathBuf { ctx.home.join(".codex") }
    fn dir_prefix(&self) -> &'static str { "codex" }

    fn prepare(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        let home = self.home(ctx, p);
        std::fs::create_dir_all(&home)?;
        if !p.main {
            // shared instructions: the main AGENTS.md, unless the profile has its own
            let main_agents = self.default_home(ctx).join("AGENTS.md");
            let own = home.join("AGENTS.md");
            if main_agents.exists() && !own.exists() {
                std::fs::copy(&main_agents, &own)?;
            }
        }
        Ok(())
    }

    fn launch(&self, ctx: &Ctx, p: &Profile, args: &[String]) -> Result<LaunchSpec> {
        let mut spec = LaunchSpec { program: resolve_program(self.id(), self.binary())?, args: args.to_vec(), ..Default::default() };
        spec.env_unset = vec!["CODEX_HOME".into(), "OPENAI_API_KEY".into(), "HRM_CODEX_KEY".into()];
        base_env(ctx, &mut spec);
        if !p.main {
            spec.env_set.push(("CODEX_HOME".into(), self.home(ctx, p).display().to_string()));
        }
        if p.kind == Kind::Routed || p.own_route() {
            // through the router, as a provider named for this launch only: nothing is written to config.toml.
            // Codex reads a `-c` value that is not TOML as plain text, so none of these needs quoting.
            let (url, token) = crate::service::router_endpoint(ctx, p)?;
            let mut over = vec!["model_provider=hrm".to_string(), "model_providers.hrm.name=hrm".into(), format!("model_providers.hrm.base_url={url}/v1"),
                "model_providers.hrm.env_key=HRM_CODEX_KEY".into(), "model_providers.hrm.wire_api=responses".into()];
            if let Some(m) = &p.model { over.push(format!("model={m}")); }
            spec.args.splice(0..0, over.into_iter().flat_map(|o| ["-c".to_string(), o]));
            spec.env_set.push(("HRM_CODEX_KEY".into(), token));
        } else if p.kind == Kind::ApiKey {
            let key = crate::secrets::get_key(&crate::secrets::key_target(&p.name))
                .ok_or_else(|| anyhow::anyhow!("no API key saved for '{}'. Run `hrm key set {}`.", p.name, p.name))?;
            spec.env_set.push(("OPENAI_API_KEY".into(), key));
        }
        Ok(spec)
    }

    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec> {
        if p.kind != Kind::Subscription {
            bail!("'{}' uses an API key; save it with `hrm key set {}`", p.name, p.name);
        }
        let mut spec = self.launch(ctx, p, &[])?;
        spec.args = vec!["login".into()];
        Ok(spec)
    }

    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity {
        if p.kind != Kind::Subscription {
            let has = crate::secrets::get_key(&crate::secrets::key_target(&p.name)).is_some() || p.kind == Kind::Routed;
            return Identity { logged_in: has, plan: Some(if p.kind == Kind::Routed { "Routed".into() } else { "API key".into() }), ..Default::default() };
        }
        let Some(auth) = read_json(&self.home(ctx, p).join("auth.json")) else { return Identity::default() };
        if let Some(c) = id_claims(&auth) {
            let plan = c["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str().map(|s| {
                let mut ch = s.chars();
                format!("ChatGPT {}", ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default())
            });
            return Identity { logged_in: true, email: c["email"].as_str().map(String::from), name: c["name"].as_str().map(String::from), plan, ..Default::default() };
        }
        let api = auth["OPENAI_API_KEY"].as_str().map(|s| !s.is_empty()).unwrap_or(false);
        Identity { logged_in: api, plan: api.then(|| "API key".into()), ..Default::default() }
    }

    /// The reading the usage import kept (it reads those lines anyway): one row, no file.
    fn quota(&self, ctx: &Ctx, p: &Profile) -> Option<Quota> {
        if let Ok(Some(q)) = ctx.db.get_quota(&p.name) { return Some(q); }
        // an account imported before readings were kept has none until its next turn: its newest rollouts are read for it, once
        let mark = limits_mark(&p.name);
        if ctx.db.kv_get(&mark).ok().flatten().is_some() { return None; }
        let mut files: Vec<(i64, PathBuf)> = self.rollouts(ctx, p).into_iter().map(|(f, m, _)| (m, f)).collect();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        // the newest reading by when Codex took it: Codex rewrites old rollouts, so the newest file can hold an old one
        let mut best: Option<Quota> = None;
        for (_, f) in files.into_iter().take(5) {
            let Ok(text) = read_text(&f) else { continue };
            if let Some(q) = text.lines().rev().find_map(limits_of) {
                if best.as_ref().is_none_or(|b| q.fetched_at > b.fetched_at) { best = Some(q); }
            }
        }
        if let Some(q) = &best { let _ = ctx.db.put_quota(&p.name, q); }
        let _ = ctx.db.kv_set(&mark, "1");
        best
    }

    fn import_usage(&self, ctx: &Ctx, p: &Profile) -> Result<usize> {
        use rusqlite::OptionalExtension;
        let mut stored = 0;
        let prices = crate::pricing::load(ctx);
        let files = self.rollouts(ctx, p);
        let cursors = ctx.db.cursors_for(&p.name)?;
        type Read = Option<(Vec<UsageRow>, Option<Quota>, Option<crate::harness::parse::NewCursor>, (String, i64, u64))>;
        each_file_parallel_sig(&files, |f, mtime, len| -> Result<Read> {
            let key = f.display().to_string();
            let compressed = f.extension().map(|e| e == "zst").unwrap_or(false);
            // a plain rollout that grew is read from where the last import stopped, with its first line (the
            // session_meta) read again so the session id and folder are known; a compressed one, archived and
            // never growing, is read whole when its size or time changed
            let (text, cursor) = if compressed {
                if let Some((_, m, l, _)) = cursors.get(&key) { if *m == mtime && *l == len as i64 { return Ok(None); } }
                (read_text(f)?, None)
            } else {
                let mut tail = String::new();
                let Some(cur) = for_new_lines_sig(&p.name, f, mtime, len, cursors.get(&key).cloned(), |line| { tail.push_str(line); tail.push('\n'); })? else { return Ok(None) };
                let mut buf = String::new();
                {
                    use std::io::BufRead;
                    let mut first = String::new();
                    if std::io::BufReader::new(std::fs::File::open(f)?).read_line(&mut first).is_ok() { buf.push_str(&first); if !first.ends_with('\n') { buf.push('\n'); } }
                }
                buf.push_str(&tail);
                (buf, Some(cur))
            };
            let mut rows = vec![];
            let (mut model, mut cwd, mut sid) = (String::new(), String::new(), String::new());
            // reply timing: a request starts at the user's message or a tool's output; its first item done is the first reply
            let (mut start, mut first): (Option<i64>, Option<i64>) = (None, None);
            let mut limits: Option<Quota> = None;
            for line in text.lines() {
                // the last reading of these lines, as the limits read took the last of a file
                if let Some(q) = limits_of(line) { limits = Some(q); }
                if line.contains("\"type\":\"response_item\"") && (line.contains("\"role\":\"user\"") || line.contains("\"type\":\"function_call_output\"")) {
                    start = line_ts(line);
                    first = None;
                    continue;
                }
                if line.contains("\"type\":\"item_completed\"") {
                    if start.is_some() && first.is_none() { first = line_ts(line); }
                    continue;
                }
                // most of a rollout is tool output and messages: only these three kinds of line are parsed
                if !(line.contains("\"token_usage_record\"") || line.contains("\"turn_context\"") || line.contains("\"session_meta\"")) { continue; }
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                let pl = &v["payload"];
                match v["type"].as_str().unwrap_or("") {
                    "session_meta" => {
                        cwd = pl["cwd"].as_str().unwrap_or("").into();
                        sid = pl["id"].as_str().or(pl["session_id"].as_str()).unwrap_or("").into();
                    }
                    "turn_context" => if let Some(m) = pl["model"].as_str() { model = m.into() },
                    "token_usage_record" => {
                        let u = &pl["usage"];
                        let Some(id) = pl["response_id"].as_str() else { continue };
                        let cached = u["cached_input_tokens"].as_i64().unwrap_or(0);
                        let ts = ts_ms(v["timestamp"].as_str().unwrap_or(""));
                        let (ttft_ms, gen_ms) = match (start, first) {
                            (Some(s), Some(f)) if f >= s && ts >= f => (Some(f - s), Some(ts - f)),
                            _ => (None, None),
                        };
                        (start, first) = (None, None);
                        rows.push(UsageRow {
                            request_id: format!("codex:{id}"),
                            profile: p.name.clone(),
                            harness: "codex".into(),
                            ts,
                            model: if model.is_empty() { "codex".into() } else { model.clone() },
                            project: cwd.clone(),
                            session_id: sid.clone(),
                            input: (u["input_tokens"].as_i64().unwrap_or(0) - cached).max(0),
                            output: u["output_tokens"].as_i64().unwrap_or(0),
                            cache_read: cached,
                            cache_write: u["cache_write_input_tokens"].as_i64().unwrap_or(0),
                            ttft_ms,
                            gen_ms,
                        });
                    }
                    _ => {}
                }
            }
            // new lines that began mid-turn carry no turn_context: the session's last stored model stands in
            if !sid.is_empty() && rows.iter().any(|r| r.model == "codex") {
                let last: Option<String> = ctx.db.with(|c| Ok(c.query_row("select model from usage where session_id = ? order by ts desc limit 1", [&sid], |r| r.get(0)).optional()?))?;
                if let Some(m) = last { for r in rows.iter_mut().filter(|r| r.model == "codex") { r.model = m.clone(); } }
            }
            Ok(Some((rows, limits, cursor, (key, mtime, len))))
        }, |r| {
            match r {
                // rows first, then the cursor: an interrupted import reads this file again
                Ok(Some((rows, limits, cursor, (key, mtime, len)))) => {
                    stored += ctx.db.insert_usage(&rows, &prices)?;
                    // the account's reading is the newest by Codex's own time, whichever file it came from
                    if let Some(q) = limits { if ctx.db.get_quota(&p.name)?.is_none_or(|kept| q.fetched_at > kept.fetched_at) { ctx.db.put_quota(&p.name, &q)?; } }
                    match cursor { Some(c) => c.save(ctx)?, None => ctx.db.set_cursor(&key, &p.name, len as i64, mtime, len as i64, "")? }
                }
                Ok(None) => {}
                Err(e) => crate::log::warn(&format!("usage import for {}: {e:#}", p.name)),
            }
            Ok(())
        })?;
        Ok(stored)
    }

    fn session_dirs(&self, ctx: &Ctx, p: &Profile) -> Vec<PathBuf> {
        let home = self.home(ctx, p);
        [home.join("sessions"), home.join("archived_sessions")].into_iter().filter(|d| d.is_dir()).collect()
    }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        self.scan(ctx, p, self.rollouts(ctx, p), ctx.db.sessions_cached_for(&format!("codex|{}|", p.name))?, since)
    }

    fn sessions_in(&self, ctx: &Ctx, p: &Profile, files: &[PathBuf]) -> Option<Result<Vec<SessionMeta>>> {
        changed(ctx, "codex", &p.name, files, is_rollout).transpose().map(|r| r.and_then(|(files, cached)| self.scan(ctx, p, files, cached, 0)))
    }

    fn transcript(&self, _ctx: &Ctx, p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let path = PathBuf::from(&s.source_path);
        parse_rollout_file(&path, &p.name, true)
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec!["resume".into(), id.into()] }

    /// The rollout under `sessions/YYYY/MM/DD/` (the date from its name), plain even when the origin kept it
    /// compressed; Codex finds it by name and `-C` sets the folder. Its state database is never touched.
    fn materialize(&self, ctx: &Ctx, p: &Profile, local_root: &Path, t: &crate::transcripts::FetchedTranscript) -> Result<crate::transcripts::Materialized> {
        let name = Path::new(&t.source_rel).file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name = name.strip_suffix(".zst").unwrap_or(name);
        // both came from another computer: the id is an argument, the name a path part
        if !crate::util::safe_name(&t.id) || (!name.is_empty() && !crate::util::safe_name(name)) { bail!("this session's names cannot be used on this computer"); }
        // rollout-2026-11-01T10-00-00-<id>.jsonl; counted in bytes, so a name of other letters is never cut inside one
        let date = name.strip_prefix("rollout-").filter(|d| d.len() >= 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-' && d.as_bytes()[..10].iter().filter(|b| b.is_ascii_digit()).count() == 8);
        let Some(d) = date else { bail!("the rollout's name ({name}) carries no date; Codex would not find it") };
        let dest = self.home(ctx, p).join("sessions").join(&d[..4]).join(&d[5..7]).join(&d[8..10]).join(name);
        if !dest.exists() {
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::copy(&t.path, &dest)?;
        }
        Ok(crate::transcripts::Materialized { launch_args: vec!["-C".into(), local_root.display().to_string(), "resume".into(), t.id.clone()], cwd: local_root.to_path_buf(),
            note: "Codex resumes the copied rollout in this folder; the original stays as it is.".into() })
    }
    fn resumes_elsewhere(&self) -> bool { true }

    fn running(&self, ctx: &Ctx, p: &Profile) -> Vec<RunningConsole> {
        // Codex leaves no pid file; a codex process plus a rollout written in the last two minutes counts as open.
        let procs = crate::procs::by_name(&["codex"], None);
        if procs.is_empty() { return vec![]; }
        let now = crate::util::now_ms();
        let norm = |s: &str| crate::projects::normalize(s).to_lowercase();
        let mut recent: Vec<(i64, PathBuf)> = self.rollouts(ctx, p).into_iter().map(|(f, m, _)| (m, f)).filter(|(m, _)| now - m < 120_000).collect();
        recent.sort_by(|a, b| b.0.cmp(&a.0));
        if recent.is_empty() { return vec![]; }
        // the desktop app and the editor extension run one app-server for every thread, in a folder of its own: it
        // counts for the threads of the Codex home it writes to (its CODEX_HOME, else .codex in its user's home)
        let mine = norm(&self.home(ctx, p).display().to_string());
        let home_of = |pid: u32| crate::procs::env_of(pid, "CODEX_HOME")
            .or_else(|| crate::procs::env_of(pid, if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|h| Path::new(&h).join(".codex").display().to_string()));
        let server = procs.iter().find(|(pid, _, _)| home_of(*pid).map(|h| norm(&h) == mine).unwrap_or(false));
        let mut out = vec![];
        for (m, f) in recent {
            let Some(meta) = first_line(&f).and_then(|l| serde_json::from_str::<Value>(&l).ok()) else { continue };
            let cwd = meta["payload"]["cwd"].as_str().unwrap_or("");
            let id = meta["payload"]["id"].as_str().unwrap_or("").to_string();
            // a codex process working in this session's folder, else the app-server of this Codex home
            let free = |pid: &u32, c: &Option<String>| !out.iter().any(|r: &RunningConsole| r.pid == *pid) && c.as_deref().map(|c| norm(c) == norm(cwd)).unwrap_or(false);
            if let Some((pid, start, pcwd)) = procs.iter().find(|(pid, _, c)| free(pid, c)).or(server) {
                out.push(RunningConsole { pid: *pid, started_at: Some((*start).min(m)), cwd: Some(cwd.to_string()).filter(|c| !c.is_empty()).or(pcwd.clone()), session_id: Some(id) });
            }
        }
        out
    }

    fn item_paths(&self, ctx: &Ctx, p: &Profile) -> ItemPaths {
        let home = self.home(ctx, p);
        ItemPaths { mcp: Some((home.join("config.toml"), "codex-toml")), skills: Some(home.join("skills")), prompt: Some(home.join("AGENTS.md")) }
    }
}

/// Text a user typed, or None for the instructions and context Codex injects as user messages.
fn real_prompt(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.starts_with('<') || t.starts_with("# AGENTS.md") || t.starts_with("# Context from my IDE") {
        return None;
    }
    Some(t.to_string())
}

/// A line that says what the user typed as an event (`event_msg` `user_message`).
fn user_message(line: &str) -> bool {
    line.contains("\"user_message\"") && serde_json::from_str::<Value>(line).is_ok_and(|v| v["payload"]["type"] == "user_message")
}

/// Parse a Codex rollout from its file, a line at a time: one can be over a hundred megabytes, and neither its text
/// nor its parsed lines are held. `full` also builds the turns.
pub fn parse_rollout_file(path: &Path, profile: &str, full: bool) -> Result<Transcript> {
    // whether it carries what the user typed as events is known before the lines are counted: a first look, as far as the first one
    let mut has_events = false;
    for line in lines(path)? { if user_message(&line?) { has_events = true; break; } }
    let mut failed = None;
    let t = parse_lines(lines(path)?.map_while(|l| l.map_err(|e| failed = Some(e)).ok()), has_events, profile, path, full);
    match failed { Some(e) => Err(e.into()), None => Ok(t) }
}

/// Parse a Codex rollout held as text. `full` also builds the turns.
pub fn parse_rollout(text: &str, profile: &str, path: &Path, full: bool) -> Transcript {
    parse_lines(text.lines(), text.lines().any(user_message), profile, path, full)
}

/// `has_events`: newer rollouts carry what the user typed as event_msg user_message; older ones only as messages.
fn parse_lines<S: AsRef<str>>(lines: impl Iterator<Item = S>, has_events: bool, profile: &str, path: &Path, full: bool) -> Transcript {
    let mut meta = SessionMeta { harness: "codex".into(), profile: profile.into(), source_path: path.display().to_string(), ..Default::default() };
    let mut turns: Vec<Turn> = vec![];
    let mut calls: HashMap<String, (usize, usize, i64)> = HashMap::new();
    let mut first_prompt = String::new();
    let mut first_answer = String::new();
    let mut agent = String::new();
    let mut last_total = 0i64;
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(line.as_ref()) else { continue };
        let ts = v["timestamp"].as_str().map(ts_ms).unwrap_or(0);
        if ts > 0 {
            if meta.started_at == 0 { meta.started_at = ts; }
            meta.last_active_at = meta.last_active_at.max(ts);
        }
        let pl = &v["payload"];
        let ptype = pl["type"].as_str().unwrap_or("");
        let mut prompt: Option<String> = None;
        match v["type"].as_str().unwrap_or("") {
            "session_meta" => {
                meta.id = pl["id"].as_str().or(pl["session_id"].as_str()).unwrap_or("").into();
                meta.cwd = pl["cwd"].as_str().unwrap_or("").into();
                // a thread Codex spawned for a sub-agent: its task never appears as a prompt
                let sub = &pl["source"]["subagent"]["thread_spawn"];
                if sub.is_object() {
                    agent = format!("Sub-agent {}{}", sub["agent_nickname"].as_str().unwrap_or("thread"), sub["agent_role"].as_str().map(|r| format!(" · {r}")).unwrap_or_default());
                }
            }
            "turn_context" => if let Some(m) = pl["model"].as_str() { meta.model = m.into() },
            "event_msg" => match ptype {
                "user_message" => prompt = pl["message"].as_str().and_then(real_prompt),
                "token_count" => if let Some(t) = pl["info"]["total_token_usage"]["total_tokens"].as_i64() { last_total = t },
                _ => {}
            },
            "response_item" => {
                if ptype == "message" {
                    meta.messages += 1;
                    let text = pl["content"].as_array().map(|a| a.iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                    if pl["role"] == "user" && !has_events {
                        prompt = real_prompt(&text);
                    } else if pl["role"] == "assistant" && first_answer.is_empty() && !text.trim().is_empty() {
                        first_answer = text.trim().lines().next().unwrap_or("").trim_start_matches('#').trim().to_string();
                    }
                    if pl["role"] == "assistant" && full && !text.trim().is_empty() {
                        if turns.is_empty() { turns.push(Turn { prompt_at: ts, ..Default::default() }); }
                        let t = turns.last_mut().unwrap();
                        t.say(text.trim().into());
                        t.answer_at = ts;
                    }
                } else if matches!(ptype, "function_call" | "custom_tool_call" | "local_shell_call") {
                    meta.tool_calls += 1;
                    if full {
                        if turns.is_empty() { turns.push(Turn { prompt_at: ts, ..Default::default() }); }
                        let ti = turns.len() - 1;
                        let name = pl["name"].as_str().unwrap_or(if ptype == "local_shell_call" { "shell" } else { "tool" }).to_string();
                        let raw_input = pl["arguments"].as_str().map(|a| serde_json::from_str::<Value>(a).unwrap_or(Value::String(a.into())))
                            .or_else(|| pl.get("input").cloned()).or_else(|| pl.get("action").cloned()).unwrap_or(Value::Null);
                        let mut step = Step { kind: "tool".into(), tool: name.clone(), input: summarize_input(&name, &raw_input), ..Default::default() };
                        if name == "apply_patch" {
                            let patch = raw_input.as_str().map(String::from).or_else(|| raw_input["input"].as_str().map(String::from)).unwrap_or_default();
                            if let Some(c) = patch_change(&patch) {
                                step.input = c.path.clone();
                                step.result = format!("+{} −{}", c.added, c.removed);
                                step.change = Some(c);
                            }
                        }
                        turns[ti].steps.push(step);
                        if let Some(id) = pl["call_id"].as_str() { calls.insert(id.into(), (ti, turns[ti].steps.len() - 1, ts)); }
                    }
                } else if ptype.ends_with("_output") {
                    let out = match &pl["output"] {
                        Value::String(s) => serde_json::from_str::<Value>(s).ok().and_then(|o| o["output"].as_str().map(String::from)).unwrap_or_else(|| s.clone()),
                        other => content_text(other),
                    };
                    let raw = match &pl["output"] { Value::String(s) => s.clone(), other => other.to_string() };
                    let failed = exit_code(&raw).map(|c| c != 0).unwrap_or(false) || out.to_ascii_lowercase().starts_with("error");
                    if failed { meta.failed += 1; }
                    if !full { continue; }
                    let Some(&(ti, si, started)) = pl["call_id"].as_str().and_then(|id| calls.get(id)) else { continue };
                    let step = &mut turns[ti].steps[si];
                    step.status = if failed { "error".into() } else { "ok".into() };
                    if failed { step.error = truncate(&out.replace('\n', " "), 200); }
                    if step.change.is_none() { step.result = summarize_output(&out); }
                    if ts > 0 && started > 0 { step.duration_ms = Some(ts - started); }
                } else if ptype == "reasoning" && full {
                    if turns.is_empty() { turns.push(Turn { prompt_at: ts, ..Default::default() }); }
                    turns.last_mut().unwrap().steps.push(Step { kind: "thinking".into(), ..Default::default() });
                }
            }
            _ => {}
        }
        if let Some(pr) = prompt {
            meta.prompts += 1;
            if first_prompt.is_empty() { first_prompt = pr.clone(); }
            if full { turns.push(Turn { prompt: pr, prompt_at: ts, model: meta.model.clone(), ..Default::default() }); }
        }
    }
    meta.tokens = last_total;
    meta.title = truncate(&first_prompt, 80);
    if meta.title.is_empty() { meta.title = if !agent.is_empty() { agent } else if !first_answer.is_empty() { truncate(&first_answer, 80) } else { "Codex session".into() }; }
    Transcript { meta, turns }
}

/// The first file of an apply_patch body, with its +/- lines.
fn patch_change(patch: &str) -> Option<Change> {
    let mut path = None;
    let mut diff = String::new();
    for l in patch.lines() {
        for tag in ["*** Update File: ", "*** Add File: ", "*** Delete File: "] {
            if let Some(p) = l.strip_prefix(tag) {
                if path.is_none() { path = Some(p.trim().to_string()); }
            }
        }
        if (l.starts_with('+') || l.starts_with('-') || l.starts_with("@@")) && !l.starts_with("***") {
            diff.push_str(l);
            diff.push('\n');
        }
    }
    let (a, r) = diff_counts(&diff);
    path.map(|path| Change { path, added: a, removed: r, diff })
}

/// A command's exit code from a tool output: plain text "Exit code: N" or JSON metadata "exit_code":N.
fn exit_code(raw: &str) -> Option<i64> {
    for pat in ["Exit code: ", "\"exit_code\":"] {
        if let Some(i) = raw.find(pat) {
            let n: String = raw[i + pat.len()..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            if let Ok(v) = n.parse() { return Some(v); }
        }
    }
    None
}

#[cfg(test)]
mod rollout_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_rollout_read_a_line_at_a_time_is_what_its_text_parses_to() {
        let t = tempfile::tempdir().unwrap();
        let at = "2026-11-01T10:00:00.000Z";
        let line = |kind: &str, payload: Value| json!({"type": kind, "timestamp": at, "payload": payload}).to_string();
        let message = |role: &str, text: &str| line("response_item", json!({"type": "message", "role": role, "content": [{"type": "text", "text": text}]}));
        let newer = [
            line("session_meta", json!({"id": "c1", "cwd": "C:\\work"})), line("turn_context", json!({"model": "gpt-6"})),
            line("event_msg", json!({"type": "user_message", "message": "Bump ArgoCD"})), message("user", "Bump ArgoCD"),
            line("response_item", json!({"type": "function_call", "name": "exec_command", "call_id": "k1", "arguments": "{\"cmd\":\"helm upgrade\"}"})),
            line("response_item", json!({"type": "function_call_output", "call_id": "k1", "output": "Exit code: 1\nno such release"})),
            message("assistant", "The release is missing."), line("event_msg", json!({"type": "token_count", "info": {"total_token_usage": {"total_tokens": 520}}})),
        ];
        // an older rollout says what the user typed only as messages, among the ones Codex injects
        let older = [line("session_meta", json!({"id": "c0", "cwd": "C:\\work"})), message("user", "<environment_context>cwd</environment_context>"), message("user", "Rotate the certificates"), message("assistant", "Done.")];
        let mut seen = vec![];
        for (name, lines) in [("rollout-new.jsonl", &newer[..]), ("rollout-old.jsonl", &older[..])] {
            // as Codex leaves a rollout it is writing: Windows line ends, a byte that is no UTF-8 inside an answer, half a last line
            let mut bytes = lines.join("\r\n").into_bytes();
            bytes.extend_from_slice(b"\r\n");
            bytes.extend_from_slice(message("assistant", "caf#").as_bytes());
            let mark = bytes.iter().rposition(|b| *b == b'#').unwrap();
            bytes[mark] = 0xff;
            bytes.extend_from_slice(b"\n{\"type\":\"respon");
            for (file, bytes) in [(t.path().join(name), bytes.clone()), (t.path().join(format!("{name}.zst")), zstd::encode_all(&bytes[..], 3).unwrap())] {
                std::fs::write(&file, bytes).unwrap();
                for full in [false, true] {
                    let read = parse_rollout_file(&file, "codex-main", full).unwrap();
                    assert_eq!(serde_json::to_value(&read).unwrap(), serde_json::to_value(parse_rollout(&read_text(&file).unwrap(), "codex-main", &file, full)).unwrap(), "{name} full={full}");
                    seen.push((read.meta.id.clone(), read.meta.title.clone(), read.meta.prompts, read.meta.messages, read.meta.tool_calls, read.meta.failed, read.meta.tokens, read.turns.len()));
                }
            }
        }
        let new = |turns| ("c1".to_string(), "Bump ArgoCD".to_string(), 1, 3, 1, 1, 520, turns);
        let old = |turns| ("c0".to_string(), "Rotate the certificates".to_string(), 1, 4, 0, 0, 0, turns);
        assert_eq!(seen, [new(0), new(1), new(0), new(1), old(0), old(1), old(0), old(1)]);
        assert!(parse_rollout_file(&t.path().join("rollout-none.jsonl"), "codex-main", false).is_err(), "a file that cannot be read is no empty session");
    }
}

#[cfg(test)]
mod limits_tests {
    use super::*;
    use serde_json::json;

    fn reading(at: &str, week: f64) -> String {
        format!("{}\n", json!({"type": "event_msg", "timestamp": at, "payload": {"type": "token_count", "rate_limits": {"secondary": {"used_percent": week, "window_minutes": 10080, "resets_at": 4102444800i64}}}}))
    }

    #[test]
    fn the_limits_are_the_newest_reading_whether_the_import_or_the_files_gave_it() {
        let t = tempfile::tempdir().unwrap();
        // the import prices rows from a catalog: a local one, so the test never reaches the network
        std::fs::write(t.path().join("pricing.json"), "{}").unwrap();
        std::env::set_var("HRM_PRICING_FILE", t.path().join("pricing.json"));
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let p = |name: &str, main| Profile { name: name.into(), label: None, harness: "codex".into(), kind: Kind::Subscription, email: None, main, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, created_at: 0 };
        let (main, other, none) = (p("codex-main", true), p("other", false), p("none", false));
        let week = |q: Option<Quota>| q.map(|q| (q.tier("week").unwrap().used, q.fetched_at.unwrap()));
        let file = |p: &Profile, name: &str, text: &str| { let f = Codex.home(&ctx, p).join("sessions").join(name); std::fs::create_dir_all(f.parent().unwrap()).unwrap(); std::fs::write(&f, text).unwrap(); f };
        // an old rollout Codex rewrote later holds an old reading in the newest file
        let new = file(&main, "rollout-2026-11-01T10-00-00-a.jsonl", &reading("2026-11-01T10:00:00.000Z", 21.0));
        std::thread::sleep(std::time::Duration::from_millis(20));
        file(&main, "rollout-2026-10-01T10-00-00-b.jsonl", &reading("2026-10-01T10:00:00.000Z", 99.0));
        let at = ts_ms("2026-11-01T10:00:00.000Z");
        // read before any import (an account from an earlier version): the files give it, once, and the row answers after
        assert_eq!(week(Codex.quota(&ctx, &main)), Some((21.0, at)));
        assert_eq!(week(ctx.db.get_quota("codex-main").unwrap()), Some((21.0, at)));
        // the import keeps the newest reading it reads, and an older one never replaces it
        Codex.import_usage(&ctx, &main).unwrap();
        assert_eq!(week(Codex.quota(&ctx, &main)), Some((21.0, at)));
        std::fs::write(&new, format!("{}{}{}", reading("2026-11-01T10:00:00.000Z", 21.0), reading("2026-11-02T08:00:00.000Z", 30.0), reading("2026-11-02T09:00:00.000Z", 34.0))).unwrap();
        Codex.import_usage(&ctx, &main).unwrap();
        assert_eq!(week(Codex.quota(&ctx, &main)), Some((34.0, ts_ms("2026-11-02T09:00:00.000Z"))));
        // an account whose first read is its import has the same reading
        file(&other, "rollout-2026-11-01T10-00-00-c.jsonl", &reading("2026-11-01T10:00:00.000Z", 21.0));
        file(&other, "rollout-2026-10-01T10-00-00-d.jsonl", &reading("2026-10-01T10:00:00.000Z", 99.0));
        Codex.import_usage(&ctx, &other).unwrap();
        assert_eq!(week(Codex.quota(&ctx, &other)), Some((21.0, at)));
        // rollouts without a reading (an API key) are read for one once, not at every look
        let f = file(&none, "rollout-2026-11-01T10-00-00-e.jsonl", "{\"type\":\"session_meta\",\"payload\":{\"id\":\"e\"}}\n");
        assert!(Codex.quota(&ctx, &none).is_none() && ctx.db.kv_get(&limits_mark("none")).unwrap().is_some());
        std::fs::write(&f, reading("2026-11-03T10:00:00.000Z", 5.0)).unwrap();
        assert!(Codex.quota(&ctx, &none).is_none(), "the files are not read again");
        Codex.import_usage(&ctx, &none).unwrap();
        assert_eq!(week(Codex.quota(&ctx, &none)), Some((5.0, ts_ms("2026-11-03T10:00:00.000Z"))), "the import brings its first reading");
    }
}

#[cfg(test)]
mod exit_tests {
    #[test]
    fn exit_codes_in_both_forms() {
        assert_eq!(super::exit_code("Exit code: 2\nWall time: 0.1s"), Some(2));
        assert_eq!(super::exit_code(r#"{"output":"x","metadata":{"exit_code":255,"duration_seconds":0.1}}"#), Some(255));
        assert_eq!(super::exit_code("Exit code: 0"), Some(0));
        assert_eq!(super::exit_code("10 matches in 127 files"), None);
    }
}
