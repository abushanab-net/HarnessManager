//! Pi (`pi`, npm `@earendil-works/pi-coding-agent`) and the parser for its version-3 session log, which
//! Oh My Pi (`omp.rs`) shares: one `PiLike` per tool, differing in names, folders and the resume flag.
//! Each profile is its own `PI_CODING_AGENT_DIR` (both tools honour it).
//!
//! Session files: `<home>/sessions/<encoded cwd>/<timestamp>_<uuid>.jsonl`. The folder name is the cwd with
//! `/`, `\` and `:` replaced by `-` and wrapped in `--` (`D:\Projects\x` is `--D--Projects-x--`; Oh My Pi
//! writes `-<relative>` for folders under home). It is never decoded here: the header carries the cwd.
//! Line 1 is the header `{"type":"session","version":3,"id","timestamp","cwd","parentSession"?}`; Oh My Pi
//! puts a 256-byte `{"type":"title",...}` slot before it. Entries carry `id`, `parentId` and an ISO
//! `timestamp`; a `message` entry holds the user, assistant or toolResult message, an assistant one with
//! `provider`, `model`, `usage` (`input`, `output`, `cacheRead`, `cacheWrite`, `cost`) and its own ms
//! `timestamp`. `usage` (Pi) and `model_usage` (Oh My Pi) entries are model calls outside the transcript.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::parse::*;
use super::{base_env, Harness, ItemPaths};
use crate::launch::{resolve_program, LaunchSpec};
use crate::model::*;
use crate::registry::{Kind, Profile};
use crate::util::{file_sig, read_json, truncate, walk_files, write_atomic};
use crate::Ctx;

pub struct PiLike {
    pub id: &'static str,
    pub label: &'static str,
    pub binary: &'static str,
    /// The tool's own home under the user's home: `.pi/agent`, `.omp/agent`.
    pub home_dir: &'static str,
    pub resume_flag: &'static str,
    pub caps: Caps,
}

pub static PI: PiLike = PiLike {
    id: "pi",
    label: "Pi",
    binary: "pi",
    home_dir: ".pi/agent",
    resume_flag: "--session",
    // /login offers OAuth subscriptions (Anthropic, OpenAI Codex, GitHub Copilot, ...) and API keys; mcp.json
    // and models.json (compatible endpoints) are documented; no limits are written anywhere
    caps: Caps { parallel_accounts: true, subscription_login: true, api_key: true, quota: false, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: true },
};

impl PiLike {
    fn session_files(&self, ctx: &Ctx, p: &Profile) -> Vec<PathBuf> {
        let mut out = vec![];
        walk_files(&self.home(ctx, p).join("sessions"), &|f: &Path| f.extension().map(|e| e == "jsonl").unwrap_or(false), &mut out);
        out
    }

    /// The `hrm` provider in the profile's `models.json`: an endpoint Pi talks to with the key it finds in
    /// `$HRM_PI_KEY` at launch, so no key is written to disk.
    fn write_provider(&self, home: &Path, base_url: &str, api: &str, model: &str) -> Result<()> {
        let path = home.join("models.json");
        let mut v = read_json(&path).unwrap_or_else(|| json!({}));
        if !v.is_object() { v = json!({}); }
        v["providers"]["hrm"] = json!({"baseUrl": base_url, "api": api, "apiKey": "$HRM_PI_KEY", "models": [{"id": model}]});
        write_atomic(&path, serde_json::to_string_pretty(&v)?.as_bytes())
    }
}

impl Harness for PiLike {
    fn id(&self) -> &'static str { self.id }
    fn label(&self) -> &'static str { self.label }
    fn caps(&self) -> Caps { self.caps.clone() }
    fn binary(&self) -> &'static str { self.binary }
    fn default_home(&self, ctx: &Ctx) -> PathBuf { ctx.home.join(self.home_dir) }
    fn dir_prefix(&self) -> &'static str { self.id }

    fn launch(&self, ctx: &Ctx, p: &Profile, args: &[String]) -> Result<LaunchSpec> {
        let mut spec = LaunchSpec { program: resolve_program(self.id, self.binary)?, args: args.to_vec(), ..Default::default() };
        spec.env_unset = vec!["PI_CODING_AGENT_DIR".into(), "HRM_PI_KEY".into()];
        base_env(ctx, &mut spec);
        let home = self.home(ctx, p);
        if !p.main { spec.env_set.push(("PI_CODING_AGENT_DIR".into(), home.display().to_string())); }
        if p.kind == Kind::Subscription { return Ok(spec); }
        // a custom provider needs its model listed: a profile without one has nothing to run
        let Some(model) = p.model.clone() else { bail!("'{}' needs a model: hrm add {} --model <id>", p.name, p.name) };
        let (url, key, api) = if p.kind == Kind::Routed {
            let (url, token) = crate::service::router_endpoint(ctx, p)?;
            (url, token, "anthropic-messages")
        } else {
            let key = crate::secrets::get_key(&crate::secrets::key_target(&p.name)).ok_or_else(|| anyhow::anyhow!("no API key saved for '{}'. Run `hrm key set {}`.", p.name, p.name))?;
            let Some(url) = p.base_url.clone() else { bail!("'{}' needs the endpoint's base URL (--base-url)", p.name) };
            (url, key, "openai-completions")
        };
        self.write_provider(&home, &url, api, &model)?;
        spec.env_set.push(("HRM_PI_KEY".into(), key));
        spec.args.splice(0..0, ["--model".to_string(), format!("hrm/{model}")]);
        Ok(spec)
    }

    /// Pi logs in from inside its UI (`/login`), so this opens the tool for the user to do that.
    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec> {
        if p.kind != Kind::Subscription { bail!("'{}' uses an API key; save it with `hrm key set {}`", p.name, p.name); }
        self.launch(ctx, p, &[])
    }

    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity {
        if p.kind != Kind::Subscription {
            let has = crate::secrets::get_key(&crate::secrets::key_target(&p.name)).is_some() || p.kind == Kind::Routed;
            return Identity { logged_in: has, plan: Some(if p.kind == Kind::Routed { "Routed".into() } else { "API key".into() }), ..Default::default() };
        }
        let home = self.home(ctx, p);
        // Pi: one key per provider in auth.json; Oh My Pi: rows in agent.db
        let mut providers: Vec<String> = read_json(&home.join("auth.json")).and_then(|v| v.as_object().map(|o| o.keys().cloned().collect())).unwrap_or_default();
        let db = home.join("agent.db");
        if db.exists() {
            if let Ok(c) = rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX) {
                if let Ok(mut st) = c.prepare("select distinct provider from auth_credentials") {
                    for id in st.query_map([], |r| r.get::<_, String>(0)).map(|r| r.flatten().collect::<Vec<_>>()).unwrap_or_default() {
                        if !providers.contains(&id) { providers.push(id); }
                    }
                }
            }
        }
        Identity { logged_in: !providers.is_empty(), plan: (!providers.is_empty()).then(|| providers.join(", ")), ..Default::default() }
    }

    fn import_usage(&self, ctx: &Ctx, p: &Profile) -> Result<usize> {
        let mut stored = 0;
        let prices = crate::pricing::load(ctx);
        let files = self.session_files(ctx, p);
        // whole files, as Codex: the header (session id, cwd) is on line 1 and the files are small
        type Parsed = Option<(Vec<UsageRow>, String, i64, u64)>;
        each_file_parallel(&files, |f| -> Result<Parsed> {
            let Some((mtime, len)) = file_sig(f) else { return Ok(None) };
            let key = f.display().to_string();
            if let Some((_, m, l, _)) = ctx.db.cursor(&key)? {
                if m == mtime && l == len as i64 { return Ok(None); }
            }
            Ok(Some((usage_rows(&std::fs::read_to_string(f)?, self.id, &p.name), key, mtime, len)))
        }, |r| {
            match r {
                Ok(Some((rows, key, mtime, len))) => {
                    stored += ctx.db.insert_usage(&rows, &prices)?;
                    ctx.db.set_cursor(&key, &p.name, len as i64, mtime, len as i64, "")?;
                }
                Ok(None) => {}
                Err(e) => crate::log::warn(&format!("usage import for {}: {e:#}", p.name)),
            }
            Ok(())
        })?;
        Ok(stored)
    }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        let mut out = vec![];
        let mut todo = vec![];
        for f in self.session_files(ctx, p) {
            let Some((mtime, len)) = file_sig(&f) else { continue };
            if mtime < since { continue; }
            let key = format!("{}|{}|{}", self.id, p.name, f.display());
            match ctx.db.session_cached(&key, mtime, len as i64)? {
                Some(m) => out.push(m),
                None => todo.push(f),
            }
        }
        each_file_parallel(&todo, |f| {
            let (mtime, len) = file_sig(f)?;
            let text = std::fs::read_to_string(f).ok()?;
            Some((mtime, len, parse_session(&text, self.id, &p.name, f, false).meta))
        }, |r| {
            let Some((mtime, len, meta)) = r.filter(|(_, _, m)| !m.id.is_empty()) else { return Ok(()) };
            ctx.db.put_session(&format!("{}|{}|{}", self.id, p.name, meta.source_path), &meta, mtime, len as i64)?;
            out.push(meta);
            Ok(())
        })?;
        Ok(out)
    }

    fn transcript(&self, _ctx: &Ctx, p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let path = PathBuf::from(&s.source_path);
        Ok(parse_session(&std::fs::read_to_string(&path)?, self.id, &p.name, &path, true))
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec![self.resume_flag.into(), id.into()] }

    fn running(&self, ctx: &Ctx, p: &Profile) -> Vec<RunningConsole> {
        // no pid file: a process of the tool plus a session file written in the last two minutes counts as open
        let procs = crate::procs::by_name(&[self.binary], None);
        if procs.is_empty() { return vec![]; }
        let now = crate::util::now_ms();
        let norm = |s: &str| crate::projects::normalize(s).to_lowercase();
        let mut recent: Vec<(i64, PathBuf)> = self.session_files(ctx, p).into_iter().filter_map(|f| file_sig(&f).map(|(m, _)| (m, f))).filter(|(m, _)| now - m < 120_000).collect();
        recent.sort_by_key(|r| std::cmp::Reverse(r.0));
        let mut out = vec![];
        for (m, f) in recent {
            let Some(h) = header(&f) else { continue };
            let cwd = h["cwd"].as_str().unwrap_or("");
            let id = h["id"].as_str().unwrap_or("").to_string();
            let free = |pid: &u32, c: &Option<String>| !out.iter().any(|r: &RunningConsole| r.pid == *pid) && c.as_deref().map(|c| norm(c) == norm(cwd)).unwrap_or(false);
            if let Some((pid, start, pcwd)) = procs.iter().find(|(pid, _, c)| free(pid, c)) {
                out.push(RunningConsole { pid: *pid, started_at: Some((*start).min(m)), cwd: pcwd.clone(), session_id: Some(id) });
            }
        }
        out
    }

    fn item_paths(&self, ctx: &Ctx, p: &Profile) -> ItemPaths {
        let home = self.home(ctx, p);
        // mcp.json holds a `mcpServers` map in Claude Code's shape (Pi's docs say to copy that entry over)
        ItemPaths { mcp: self.caps.mcp.then(|| (home.join("mcp.json"), "claude-json")), skills: Some(home.join("skills")), prompt: Some(home.join("AGENTS.md")) }
    }
}

/// The session header: line 1, or line 2 behind Oh My Pi's title slot.
fn header(path: &Path) -> Option<Value> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).ok()?;
    std::io::BufReader::new(f).lines().take(2).filter_map(|l| serde_json::from_str::<Value>(&l.ok()?).ok()).find(|v| v["type"] == "session")
}

/// Entry time: the ISO `timestamp` of the entry.
fn entry_ts(v: &Value) -> i64 { v["timestamp"].as_str().map(ts_ms).unwrap_or(0) }

/// Every model call in a session log as usage rows: assistant messages, plus `usage` / `model_usage` entries.
pub fn usage_rows(text: &str, harness: &str, profile: &str) -> Vec<UsageRow> {
    let mut rows = vec![];
    let (mut sid, mut cwd) = (String::new(), String::new());
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let kind = v["type"].as_str().unwrap_or("");
        if kind == "session" {
            sid = v["id"].as_str().unwrap_or("").into();
            cwd = v["cwd"].as_str().unwrap_or("").into();
            continue;
        }
        let (m, usage) = match kind {
            "message" if v["message"]["role"] == "assistant" => (&v["message"], &v["message"]["usage"]),
            "usage" | "model_usage" => (&v, &v["usage"]),
            _ => continue,
        };
        if !usage.is_object() { continue; }
        let Some(id) = v["id"].as_str() else { continue };
        let ts = entry_ts(&v);
        // the message's own ms timestamp is when the reply began; the entry's is when it was written
        let began = m["timestamp"].as_i64().unwrap_or(0);
        rows.push(UsageRow {
            request_id: format!("{harness}:{sid}:{id}"),
            profile: profile.into(),
            harness: harness.into(),
            ts: if began > 0 { began } else { ts },
            model: m["model"].as_str().filter(|s| !s.is_empty()).unwrap_or(harness).into(),
            project: cwd.clone(),
            session_id: sid.clone(),
            input: usage["input"].as_i64().unwrap_or(0),
            output: usage["output"].as_i64().unwrap_or(0),
            cache_read: usage["cacheRead"].as_i64().unwrap_or(0),
            cache_write: usage["cacheWrite"].as_i64().unwrap_or(0),
            ttft_ms: None,
            gen_ms: (began > 0 && ts > began).then(|| ts - began),
        });
    }
    rows
}

/// Parse a Pi or Oh My Pi session log. `full` also builds the turns.
pub fn parse_session(text: &str, harness: &str, profile: &str, path: &Path, full: bool) -> Transcript {
    let mut meta = SessionMeta { harness: harness.into(), profile: profile.into(), source_path: path.display().to_string(), ..Default::default() };
    let mut turns: Vec<Turn> = vec![];
    let mut calls: HashMap<String, (usize, usize, i64)> = HashMap::new();
    let (mut title, mut named, mut first_prompt) = (String::new(), String::new(), String::new());
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let ts = entry_ts(&v);
        if ts > 0 {
            if meta.started_at == 0 { meta.started_at = ts; }
            meta.last_active_at = meta.last_active_at.max(ts);
        }
        match v["type"].as_str().unwrap_or("") {
            "session" => {
                meta.id = v["id"].as_str().unwrap_or("").into();
                meta.cwd = v["cwd"].as_str().unwrap_or("").into();
            }
            "title" => title = v["title"].as_str().unwrap_or("").trim().into(),
            "session_info" => named = v["name"].as_str().unwrap_or("").trim().into(),
            "model_change" => if let Some(m) = v["modelId"].as_str() { meta.model = m.into() },
            "message" => {
                let m = &v["message"];
                match m["role"].as_str().unwrap_or("") {
                    "user" => {
                        meta.messages += 1;
                        let prompt = content_text(&m["content"]).trim().to_string();
                        if prompt.is_empty() { continue; }
                        meta.prompts += 1;
                        if first_prompt.is_empty() { first_prompt = prompt.clone(); }
                        if full { turns.push(Turn { prompt, prompt_at: ts, ..Default::default() }); }
                    }
                    "assistant" => {
                        meta.messages += 1;
                        if let Some(model) = m["model"].as_str() { meta.model = model.into(); }
                        meta.tokens += m["usage"]["input"].as_i64().unwrap_or(0) + m["usage"]["output"].as_i64().unwrap_or(0);
                        for b in m["content"].as_array().into_iter().flatten() {
                            if b["type"] == "toolCall" { meta.tool_calls += 1; }
                            if !full { continue; }
                            if turns.is_empty() { turns.push(Turn { prompt_at: ts, ..Default::default() }); }
                            let ti = turns.len() - 1;
                            let turn = &mut turns[ti];
                            turn.answer_at = ts;
                            if turn.model.is_empty() { turn.model = meta.model.clone(); }
                            match b["type"].as_str().unwrap_or("") {
                                "text" => {
                                    let t = b["text"].as_str().unwrap_or("").trim();
                                    if !t.is_empty() { turn.say(t.into()); }
                                }
                                "thinking" => turn.steps.push(Step { kind: "thinking".into(), ..Default::default() }),
                                "toolCall" => {
                                    let tool = b["name"].as_str().unwrap_or("tool").to_string();
                                    let args = &b["arguments"];
                                    let mut step = Step { kind: "tool".into(), tool: tool.clone(), input: summarize_input(&tool, args), ..Default::default() };
                                    if tool == "edit" || tool == "write" {
                                        let old = args["oldText"].as_str().unwrap_or("");
                                        let new = args["newText"].as_str().or(args["content"].as_str()).unwrap_or("");
                                        let diff: String = old.lines().map(|l| format!("-{l}\n")).chain(new.lines().map(|l| format!("+{l}\n"))).collect();
                                        let (a, r) = diff_counts(&diff);
                                        step.change = Some(Change { path: step.input.clone(), added: a, removed: r, diff });
                                        step.result = format!("+{a} −{r}");
                                    }
                                    turn.steps.push(step);
                                    if let Some(id) = b["id"].as_str() { calls.insert(id.into(), (ti, turn.steps.len() - 1, ts)); }
                                }
                                _ => {}
                            }
                        }
                    }
                    "toolResult" => {
                        let failed = m["isError"] == true;
                        if failed { meta.failed += 1; }
                        if !full { continue; }
                        let Some(&(ti, si, started)) = m["toolCallId"].as_str().and_then(|id| calls.get(id)) else { continue };
                        let out = content_text(&m["content"]);
                        let step = &mut turns[ti].steps[si];
                        step.status = if failed { "error".into() } else { "ok".into() };
                        if failed { step.error = truncate(&out.replace('\n', " "), 200); }
                        if step.change.is_none() { step.result = summarize_output(&out); }
                        if ts > 0 && started > 0 { step.duration_ms = Some(ts - started); }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    meta.title = [title, named, truncate(&first_prompt, 80)].into_iter().find(|t| !t.is_empty()).unwrap_or_else(|| format!("{} session", if harness == "omp" { "Oh My Pi" } else { "Pi" }));
    Transcript { meta, turns }
}
