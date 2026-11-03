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
use crate::util::{file_sig, read_json, truncate, walk_files};
use crate::Ctx;

pub struct Codex;

impl Codex {
    fn rollouts(&self, ctx: &Ctx, p: &Profile) -> Vec<PathBuf> {
        let home = self.home(ctx, p);
        let mut out = vec![];
        let keep = |f: &Path| f.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("rollout-") && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst"))).unwrap_or(false);
        walk_files(&home.join("sessions"), &keep, &mut out);
        walk_files(&home.join("archived_sessions"), &keep, &mut out);
        out
    }
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
        Caps { parallel_accounts: true, subscription_login: true, api_key: true, quota: true, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: false }
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
        spec.env_unset = vec!["CODEX_HOME".into(), "OPENAI_API_KEY".into()];
        base_env(ctx, &mut spec);
        if !p.main {
            spec.env_set.push(("CODEX_HOME".into(), self.home(ctx, p).display().to_string()));
        }
        if p.kind == Kind::ApiKey {
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
        if p.kind == Kind::ApiKey {
            let has = crate::secrets::get_key(&crate::secrets::key_target(&p.name)).is_some();
            return Identity { logged_in: has, plan: Some("API key".into()), ..Default::default() };
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

    fn quota(&self, ctx: &Ctx, p: &Profile) -> Option<Quota> {
        let mut files: Vec<(i64, PathBuf)> = self.rollouts(ctx, p).into_iter().filter_map(|f| file_sig(&f).map(|(m, _)| (m, f))).collect();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, f) in files.into_iter().take(5) {
            let Ok(text) = read_text(&f) else { continue };
            for line in text.lines().rev() {
                if !line.contains("\"rate_limits\"") { continue; }
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                let rl = &v["payload"]["rate_limits"];
                if rl.is_object() && (rl["primary"].is_object() || rl["secondary"].is_object()) {
                    return Some(parse_rate_limits(rl, ts_ms(v["timestamp"].as_str().unwrap_or(""))));
                }
            }
        }
        None
    }

    fn import_usage(&self, ctx: &Ctx, p: &Profile) -> Result<usize> {
        let mut stored = 0;
        let files = self.rollouts(ctx, p);
        each_file_parallel(&files, |f| -> Result<Option<(Vec<UsageRow>, String, i64, u64)>> {
            let mut rows = vec![];
            let Some((mtime, len)) = file_sig(f) else { return Ok(None) };
            let key = f.display().to_string();
            if let Some((_, m, l, _)) = ctx.db.cursor(&key)? {
                if m == mtime && l == len as i64 { return Ok(None); }
            }
            let text = read_text(f)?;
            let (mut model, mut cwd, mut sid) = (String::new(), String::new(), String::new());
            // reply timing: a request starts at the user's message or a tool's output; its first item done is the first reply
            let (mut start, mut first): (Option<i64>, Option<i64>) = (None, None);
            for line in text.lines() {
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
            Ok(Some((rows, key, mtime, len)))
        }, |r| {
            match r {
                // rows first, then the cursor: an interrupted import reads this file again
                Ok(Some((rows, key, mtime, len))) => {
                    stored += ctx.db.insert_usage(&rows)?;
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
        for f in self.rollouts(ctx, p) {
            let Some((mtime, len)) = file_sig(&f) else { continue };
            if mtime < since { continue; }
            let key = format!("codex|{}|{}", p.name, f.display());
            match ctx.db.session_cached(&key, mtime, len as i64)? {
                Some(m) => out.push(m),
                None => todo.push(f),
            }
        }
        each_file_parallel(&todo, |f| {
            let (mtime, len) = file_sig(f)?;
            let text = read_text(f).ok()?;
            Some((mtime, len, parse_rollout(&text, &p.name, f, false).meta))
        }, |r| {
            let Some((mtime, len, meta)) = r.filter(|(_, _, m)| !m.id.is_empty()) else { return Ok(()) };
            ctx.db.put_session(&format!("codex|{}|{}", p.name, meta.source_path), &meta, mtime, len as i64)?;
            out.push(meta);
            Ok(())
        })?;
        Ok(out)
    }

    fn transcript(&self, _ctx: &Ctx, p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let path = PathBuf::from(&s.source_path);
        Ok(parse_rollout(&read_text(&path)?, &p.name, &path, true))
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec!["resume".into(), id.into()] }

    fn running(&self, ctx: &Ctx, p: &Profile) -> Vec<RunningConsole> {
        // Codex leaves no pid file; a codex process plus a rollout written in the last two minutes counts as open.
        let procs = crate::procs::by_name(&["codex"], None);
        if procs.is_empty() { return vec![]; }
        let now = crate::util::now_ms();
        let norm = |s: &str| crate::projects::normalize(s).to_lowercase();
        let mut recent: Vec<(i64, PathBuf)> = self.rollouts(ctx, p).into_iter().filter_map(|f| file_sig(&f).map(|(m, _)| (m, f))).filter(|(m, _)| now - m < 120_000).collect();
        recent.sort_by(|a, b| b.0.cmp(&a.0));
        let mut out = vec![];
        for (m, f) in recent {
            let Some(meta) = first_line(&f).and_then(|l| serde_json::from_str::<Value>(&l).ok()) else { continue };
            let cwd = meta["payload"]["cwd"].as_str().unwrap_or("");
            let id = meta["payload"]["id"].as_str().unwrap_or("").to_string();
            // a codex process working in this session's folder; other codex processes (the desktop app) do not count
            let free = |pid: &u32, c: &Option<String>| !out.iter().any(|r: &RunningConsole| r.pid == *pid) && c.as_deref().map(|c| norm(c) == norm(cwd)).unwrap_or(false);
            if let Some((pid, start, pcwd)) = procs.iter().find(|(pid, _, c)| free(pid, c)) {
                out.push(RunningConsole { pid: *pid, started_at: Some((*start).min(m)), cwd: pcwd.clone(), session_id: Some(id) });
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

/// Parse a Codex rollout. `full` also builds the turns.
pub fn parse_rollout(text: &str, profile: &str, path: &Path, full: bool) -> Transcript {
    let mut meta = SessionMeta { harness: "codex".into(), profile: profile.into(), source_path: path.display().to_string(), ..Default::default() };
    let lines: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    // newer rollouts carry what the user typed as event_msg user_message; older ones only as messages
    let has_events = lines.iter().any(|v| v["payload"]["type"] == "user_message");
    let mut turns: Vec<Turn> = vec![];
    let mut calls: HashMap<String, (usize, usize, i64)> = HashMap::new();
    let mut first_prompt = String::new();
    let mut last_total = 0i64;
    for v in &lines {
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
                    } else if pl["role"] == "assistant" && full && !text.trim().is_empty() {
                        if turns.is_empty() { turns.push(Turn { prompt_at: ts, ..Default::default() }); }
                        let t = turns.last_mut().unwrap();
                        t.answer.push(text.trim().into());
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
    if meta.title.is_empty() { meta.title = "Codex session".into(); }
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
mod exit_tests {
    #[test]
    fn exit_codes_in_both_forms() {
        assert_eq!(super::exit_code("Exit code: 2\nWall time: 0.1s"), Some(2));
        assert_eq!(super::exit_code(r#"{"output":"x","metadata":{"exit_code":255,"duration_seconds":0.1}}"#), Some(255));
        assert_eq!(super::exit_code("Exit code: 0"), Some(0));
        assert_eq!(super::exit_code("10 matches in 127 files"), None);
    }
}
