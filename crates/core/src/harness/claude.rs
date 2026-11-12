//! Claude Code. Each profile is its own `CLAUDE_CONFIG_DIR`; logins stay where Claude Code keeps them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::parse::*;
use super::{base_env, Harness, ItemPaths};
use crate::launch::{resolve_program, LaunchSpec};
use crate::model::*;
use crate::registry::{Kind, Profile};
use crate::util::{now_ms, read_json, truncate};
use crate::Ctx;

pub struct Claude;

/// Variables that would point Claude Code at another login or endpoint than the profile's.
const CONFLICTING: &[&str] = &[
    "CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL",
    "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_OAUTH_TOKEN",
];
/// Set by a running Claude Code for its own children. Inherited (Harness Manager started from a Claude Code
/// terminal), they make the account a child of that session: transcripts off, its messaging token visible.
const SESSION_MARKERS: &[&str] = &[
    "CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED", "CLAUDE_CODE_MESSAGING_SOCKET", "CLAUDE_CODE_MESSAGING_TOKEN", "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_SSE_PORT", "CLAUDE_PID", "CLAUDE_EFFORT", "CLAUDE_PLUGIN_DATA",
];

impl Claude {
    pub fn claude_json(&self, ctx: &Ctx, p: &Profile) -> PathBuf {
        if p.main { ctx.home.join(".claude.json") } else { self.home(ctx, p).join(".claude.json") }
    }

    fn creds(&self, ctx: &Ctx, p: &Profile) -> Option<Value> { read_json(&self.home(ctx, p).join(".credentials.json")) }
}

/// When Anthropic last refused an account's login (epoch ms).
fn rejected_key(p: &Profile) -> String { format!("login:refused:{}", p.name) }

impl Claude {

    /// Live limits from Anthropic, using the access token Claude Code saved on this computer. Opt-in;
    /// only while that token is valid: Harness Manager never refreshes, copies or stores it.
    pub fn live_quota(&self, ctx: &Ctx, p: &Profile) -> Result<Quota> {
        let creds = self.creds(ctx, p).ok_or_else(|| anyhow::anyhow!("not logged in"))?;
        let token = str_at(&creds, &["claudeAiOauth", "accessToken"]).ok_or_else(|| anyhow::anyhow!("not logged in"))?;
        if i64_at(&creds, &["claudeAiOauth", "expiresAt"]) < now_ms() + 60_000 {
            bail!("Claude Code's login on this computer has expired; it refreshes the next time this account runs");
        }
        let url = std::env::var("HRM_ANTHROPIC_USAGE_URL").unwrap_or_else(|_| "https://api.anthropic.com/api/oauth/usage".into());
        let r = reqwest::blocking::Client::new()
            .get(url)
            .bearer_auth(token)
            .header("anthropic-beta", "oauth-2025-04-20")
            .timeout(std::time::Duration::from_secs(15))
            .send()?;
        // a refused login is gone (signed out or revoked elsewhere) though its dates look fine: `identity` says so until Claude Code saves a new one
        if r.status() == reqwest::StatusCode::UNAUTHORIZED {
            ctx.db.kv_set(&rejected_key(p), &now_ms().to_string())?;
            bail!("Anthropic refused this account's login: log in again");
        }
        let v: Value = r.error_for_status()?.json()?;
        let q = parse_utilization(&v, Some(now_ms()), "live");
        ctx.db.put_quota(&p.name, &q)?;
        Ok(q)
    }

    fn session_files(&self, ctx: &Ctx, p: &Profile) -> Vec<(PathBuf, i64, u64)> {
        let mut out = vec![];
        let root = self.home(ctx, p).join("projects");
        let Ok(rd) = std::fs::read_dir(&root) else { return out };
        for d in rd.flatten() {
            if let Ok(files) = std::fs::read_dir(d.path()) {
                for f in files.flatten() {
                    let path = f.path();
                    if !path.extension().map(|e| e == "jsonl").unwrap_or(false) { continue; }
                    if let Ok(m) = f.metadata() {
                        let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
                        out.push((path, mtime, m.len()));
                    }
                }
            }
        }
        out
    }
}

pub fn plan_label(sub: Option<&str>, tier: Option<&str>, org_type: Option<&str>, seat: Option<&str>) -> Option<String> {
    let tier = tier.unwrap_or("");
    let sub = sub.or_else(|| org_type.map(|o| o.trim_start_matches("claude_"))).unwrap_or("");
    let seat = seat.map(|s| {
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    });
    let label = if tier.contains("20x") {
        "Max 20x".to_string()
    } else if tier.contains("5x") {
        "Max 5x".to_string()
    } else {
        match sub {
            "max" => "Max".into(),
            "pro" => "Pro".into(),
            "team" => seat.map(|s| format!("Team {s}")).unwrap_or_else(|| "Team".into()),
            "enterprise" => "Enterprise".into(),
            "" => return None,
            other => other.to_string(),
        }
    };
    Some(label)
}

/// Claude's usage document (`cachedUsageUtilization.utilization`, or the live endpoint) to tiers.
pub fn parse_utilization(v: &Value, fetched_at: Option<i64>, source: &str) -> Quota {
    let mut tiers = vec![];
    let mut add = |id: String, label: String, w: &Value| {
        if w.is_null() || w.get("utilization").is_none() || tiers.iter().any(|t: &QuotaTier| t.id == id) {
            return;
        }
        tiers.push(QuotaTier {
            id,
            label,
            used: w.get("utilization").and_then(|u| u.as_f64()).unwrap_or(0.0),
            resets_at: w.get("resets_at").and_then(|r| r.as_str()).map(ts_ms).filter(|t| *t > 0),
        });
    };
    add("session".into(), "5-hour session".into(), &v["five_hour"]);
    add("week".into(), "Week".into(), &v["seven_day"]);
    add("week:opus".into(), "Week · Opus".into(), &v["seven_day_opus"]);
    add("week:sonnet".into(), "Week · Sonnet".into(), &v["seven_day_sonnet"]);
    if let Some(limits) = v.get("limits").and_then(|l| l.as_array()) {
        for l in limits {
            if l["kind"] == "weekly_scoped" {
                if let Some(name) = str_at(l, &["scope", "model", "display_name"]) {
                    let w = serde_json::json!({"utilization": l["percent"], "resets_at": l["resets_at"]});
                    add(format!("week:{}", name.to_lowercase()), format!("Week · {name}"), &w);
                }
            }
        }
    }
    Quota { tiers, fetched_at, source: source.into() }
}

impl Harness for Claude {
    fn id(&self) -> &'static str { "claude" }
    fn label(&self) -> &'static str { "Claude Code" }
    fn caps(&self) -> Caps {
        Caps { parallel_accounts: true, subscription_login: true, api_key: true, quota: true, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: true }
    }
    fn binary(&self) -> &'static str { "claude" }
    fn default_home(&self, ctx: &Ctx) -> PathBuf { ctx.home.join(".claude") }
    fn dir_prefix(&self) -> &'static str { "claude" }

    fn prepare(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        let home = self.home(ctx, p);
        std::fs::create_dir_all(&home)?;
        if !p.main {
            crate::shared::sync_claude(ctx, &self.default_home(ctx), &ctx.home.join(".claude.json"), &home)?;
        }
        Ok(())
    }

    fn launch(&self, ctx: &Ctx, p: &Profile, args: &[String]) -> Result<LaunchSpec> {
        let mut spec = LaunchSpec { program: resolve_program(self.id(), self.binary())?, args: args.to_vec(), ..Default::default() };
        spec.env_unset = CONFLICTING.iter().chain(SESSION_MARKERS).map(|s| s.to_string()).collect();
        base_env(ctx, &mut spec);
        if !p.main {
            spec.env_set.push(("CLAUDE_CONFIG_DIR".into(), self.home(ctx, p).display().to_string()));
        }
        match p.kind {
            Kind::Subscription => {}
            Kind::ApiKey => {
                let key = crate::secrets::get_key(&crate::secrets::key_target(&p.name))
                    .ok_or_else(|| anyhow::anyhow!("no API key saved for '{}'. Run `hrm key set {}`.", p.name, p.name))?;
                spec.env_set.push(("ANTHROPIC_API_KEY".into(), key));
                if let Some(u) = &p.base_url { spec.env_set.push(("ANTHROPIC_BASE_URL".into(), u.clone())); }
                if let Some(m) = &p.model { spec.env_set.push(("ANTHROPIC_MODEL".into(), m.clone())); }
            }
            Kind::Routed => {
                let (url, token) = crate::service::router_endpoint(ctx, p)?;
                spec.env_set.push(("ANTHROPIC_BASE_URL".into(), url));
                spec.env_set.push(("ANTHROPIC_AUTH_TOKEN".into(), token));
                if let Some(m) = &p.model { spec.env_set.push(("ANTHROPIC_MODEL".into(), m.clone())); }
            }
        }
        Ok(spec)
    }

    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec> {
        if p.kind != Kind::Subscription {
            bail!("'{}' uses an API key; save it with `hrm key set {}`", p.name, p.name);
        }
        let mut args = vec!["auth".into(), "login".into(), "--claudeai".into()];
        if let Some(e) = &p.email {
            args.push("--email".into());
            args.push(e.clone());
        }
        let mut spec = self.launch(ctx, p, &args)?;
        spec.args = args;
        Ok(spec)
    }

    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity {
        if p.kind != Kind::Subscription {
            let has = crate::secrets::get_key(&crate::secrets::key_target(&p.name)).is_some() || p.kind == Kind::Routed;
            return Identity { logged_in: has, plan: Some(if p.kind == Kind::Routed { "Routed".into() } else { "API key".into() }), ..Default::default() };
        }
        let cj = read_json(&self.claude_json(ctx, p)).unwrap_or(Value::Null);
        let acc = &cj["oauthAccount"];
        let creds = self.creds(ctx, p).unwrap_or(Value::Null);
        let oauth = &creds["claudeAiOauth"];
        let logged_in = oauth.get("accessToken").is_some() || (cfg!(target_os = "macos") && acc.is_object());
        let refresh_exp = oauth.get("refreshTokenExpiresAt").and_then(|v| v.as_i64());
        // refused by Anthropic after the login was saved: a new login (or Claude Code's own refresh) rewrites the file
        let saved = std::fs::metadata(self.home(ctx, p).join(".credentials.json")).and_then(|m| m.modified()).ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
        let refused = ctx.db.kv_get(&rejected_key(p)).ok().flatten().and_then(|v| v.parse::<i64>().ok()).is_some_and(|t| t > saved);
        Identity {
            logged_in,
            email: str_at(acc, &["emailAddress"]).map(String::from),
            name: str_at(acc, &["displayName"]).map(String::from),
            org: str_at(acc, &["organizationName"]).map(String::from),
            role: str_at(acc, &["organizationRole"]).map(String::from),
            plan: plan_label(str_at(oauth, &["subscriptionType"]), str_at(oauth, &["rateLimitTier"]).or(str_at(acc, &["organizationRateLimitTier"])),
                str_at(acc, &["organizationType"]), str_at(acc, &["seatTier"])),
            needs_login: logged_in && (refused || refresh_exp.map(|t| t < now_ms()).unwrap_or(false)),
        }
    }

    fn quota(&self, ctx: &Ctx, p: &Profile) -> Option<Quota> {
        if p.kind != Kind::Subscription {
            return None;
        }
        let cj = read_json(&self.claude_json(ctx, p))?;
        let cache = &cj["cachedUsageUtilization"];
        let mut q = if cache.is_object() && cache["accountUuid"] == cj["oauthAccount"]["accountUuid"] {
            Some(parse_utilization(&cache["utilization"], cache["fetchedAtMs"].as_i64(), "claude-code"))
        } else {
            None
        };
        if let Ok(Some(live)) = ctx.db.get_quota(&p.name) {
            if live.fetched_at > q.as_ref().and_then(|q| q.fetched_at) {
                q = Some(live);
            }
        }
        q
    }

    fn import_usage(&self, ctx: &Ctx, p: &Profile) -> Result<usize> {
        let mut files = vec![];
        crate::util::walk_files_sig(&self.home(ctx, p).join("projects"), &|f: &Path| f.extension().map(|e| e == "jsonl").unwrap_or(false), &mut files);
        let mut stored = 0;
        let prices = crate::pricing::load(ctx);
        // every cursor of the account in one query, and each file's size and time from the directory listing:
        // a file that did not change is never opened (opening 8,000 files was most of a refresh)
        let cursors = ctx.db.cursors_for(&p.name)?;
        each_file_parallel_sig(&files, |f, mtime, len| {
            let mut rows = vec![];
            // reply timing: each part of an answer is logged when it completes, under one message id; the request
            // went out at the last user line (prompt or tool result). shortcut: an answer cut by a refresh mid-reply
            // has no start in the second read and shows no timing.
            let (mut sent, mut parts): (i64, std::collections::HashMap<String, (i64, i64, i64)>) = (0, Default::default());
            let cursor = for_new_lines_sig(ctx, &p.name, f, mtime, len, cursors.get(&f.display().to_string()).cloned(), |line| {
                if !line.contains("\"usage\"") {
                    if line.contains("\"type\":\"user\"") { sent = crate::harness::parse::line_ts(line).unwrap_or(sent); }
                    return;
                }
                let Ok(v) = serde_json::from_str::<Value>(line) else { return };
                if v["type"] != "assistant" { return; }
                let m = &v["message"];
                let model = m["model"].as_str().unwrap_or("");
                if model.is_empty() || model.starts_with('<') { return; }
                let Some(id) = m["id"].as_str().or(v["requestId"].as_str()) else { return };
                let ts = ts_ms(v["timestamp"].as_str().unwrap_or(""));
                let e = parts.entry(id.to_string()).or_insert((sent, ts, ts));
                e.2 = e.2.max(ts);
                let (start, first, last) = *e;
                let u = &m["usage"];
                rows.push(UsageRow {
                    request_id: format!("claude:{id}"),
                    profile: p.name.clone(),
                    harness: "claude".into(),
                    // every part carries the answer's first time, so the copies compare on output alone
                    ts: first,
                    model: model.into(),
                    project: v["cwd"].as_str().unwrap_or("").into(),
                    session_id: v["sessionId"].as_str().unwrap_or("").into(),
                    input: u["input_tokens"].as_i64().unwrap_or(0),
                    output: u["output_tokens"].as_i64().unwrap_or(0),
                    cache_read: u["cache_read_input_tokens"].as_i64().unwrap_or(0),
                    cache_write: u["cache_creation_input_tokens"].as_i64().unwrap_or(0),
                    ttft_ms: (start > 0 && start <= first).then(|| first - start),
                    gen_ms: (start > 0 && last > first).then(|| last - first),
                });
            })?;
            Ok((rows, cursor))
        }, |r: Result<(Vec<UsageRow>, Option<crate::harness::parse::NewCursor>)>| {
            match r {
                Ok((rows, cursor)) => {
                    stored += ctx.db.insert_usage(&rows, &prices)?;
                    if let Some(c) = cursor { c.save(ctx)?; }
                }
                // one unreadable file (locked by another program) does not stop the rest; it is read next time
                Err(e) => crate::log::warn(&format!("usage import for {}: {e:#}", p.name)),
            }
            Ok(())
        })?;
        Ok(stored)
    }

    fn session_dirs(&self, ctx: &Ctx, p: &Profile) -> Vec<PathBuf> { vec![self.home(ctx, p).join("projects")].into_iter().filter(|d| d.is_dir()).collect() }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        let mut out = vec![];
        let mut todo = vec![];
        let cached = ctx.db.sessions_cached_for(&format!("claude|{}|", p.name))?;
        for (f, mtime, len) in self.session_files(ctx, p) {
            if mtime < since { continue; }
            let key = format!("claude|{}|{}", p.name, f.display());
            match cached.get(&key) {
                Some((m, l, meta)) if *m == mtime && *l == len as i64 => out.push(meta.clone()),
                // a session still being written: its last record now, the file parsed again in the background
                Some((_, _, meta)) if !parse_now() => {
                    out.push(stale(meta, mtime));
                    let name = p.name.clone();
                    refresh_session_later(ctx, key, f, mtime, len, move |f| std::fs::read_to_string(f).ok().map(|t| parse_session(&t, &name, f, false).meta));
                }
                _ => todo.push((f, mtime, len)),
            }
        }
        each_file_parallel_sig(&todo, |f, mtime, len| {
            let text = std::fs::read_to_string(f).ok()?;
            Some((mtime, len, parse_session(&text, &p.name, f, false).meta))
        }, |r| {
            let Some((mtime, len, meta)) = r.filter(|(_, _, m)| m.started_at != 0) else { return Ok(()) };
            ctx.db.put_session(&format!("claude|{}|{}", p.name, meta.source_path), &meta, mtime, len as i64)?;
            out.push(meta);
            Ok(())
        })?;
        Ok(out)
    }

    fn transcript(&self, _ctx: &Ctx, p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let path = PathBuf::from(&s.source_path);
        let text = std::fs::read_to_string(&path)?;
        Ok(parse_session(&text, &p.name, &path, true))
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec!["--resume".into(), id.into()] }

    /// One copy under `projects/<this clone's folder, encoded>/<id>.jsonl` (reused when there), resumed by its
    /// path as a fork, so neither the copy nor the original is written.
    fn materialize(&self, ctx: &Ctx, p: &Profile, local_root: &Path, t: &crate::transcripts::FetchedTranscript) -> Result<crate::transcripts::Materialized> {
        let dest = self.home(ctx, p).join("projects").join(project_dir_name(local_root)).join(format!("{}.jsonl", t.id));
        if !dest.exists() {
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::copy(&t.path, &dest).with_context(|| format!("copy the transcript to {}", dest.display()))?;
        }
        Ok(crate::transcripts::Materialized { launch_args: vec!["--resume".into(), dest.display().to_string(), "--fork-session".into()], cwd: local_root.to_path_buf(), new_session_id: None,
            note: "Claude Code opens a new session that continues from the transcript; the copy and the original stay as they are.".into() })
    }
    fn resumes_elsewhere(&self) -> bool { true }

    fn running(&self, ctx: &Ctx, p: &Profile) -> Vec<RunningConsole> {
        let dir = self.home(ctx, p).join("sessions");
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        let mut out = vec![];
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().map(|x| x != "json").unwrap_or(true) { continue; }
            let Some(pid) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok()) else { continue };
            if !crate::procs::alive(pid) { continue; }
            let v = read_json(&path).unwrap_or(Value::Null);
            // Claude Code 2.1 writes startedAt (ms) and procStart; older versions procStartFt (FILETIME, 100 ns since 1601)
            let ft = v["procStart"].as_str().or(v["procStartFt"].as_str()).and_then(|s| s.parse::<i64>().ok()).map(|ft| ft / 10_000 - 11_644_473_600_000);
            if let (Some(f), Some(real)) = (ft, crate::procs::start_ms(pid)) {
                if (f - real).abs() > 2_000 { continue; } // the pid now belongs to another program
            }
            let started = v["startedAt"].as_i64().or(ft);
            let cwd = v["cwd"].as_str().map(String::from).or_else(|| crate::procs::cwd_of(pid));
            let session_id = v["sessionId"].as_str().filter(|s| !s.is_empty()).map(String::from);
            out.push(RunningConsole { pid, started_at: started, cwd, session_id });
        }
        out
    }

    fn item_paths(&self, ctx: &Ctx, p: &Profile) -> ItemPaths {
        let home = self.home(ctx, p);
        ItemPaths { mcp: Some((self.claude_json(ctx, p), "claude-json")), skills: Some(home.join("skills")), prompt: Some(home.join("CLAUDE.md")) }
    }
}

/// The folder Claude Code keeps a working directory's sessions in: every character that is not a letter or
/// digit becomes `-` (`C:\Users\sam\src\my.app` is `C--Users-sam-src-my-app`, `/home/sam` is `-home-sam`).
pub fn project_dir_name(cwd: &Path) -> String {
    cwd.display().to_string().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Parse a Claude Code session log. `full` also builds the turns (the reading view needs them; the list does not).
pub fn parse_session(text: &str, profile: &str, path: &Path, full: bool) -> Transcript {
    let mut meta = SessionMeta {
        harness: "claude".into(),
        profile: profile.into(),
        id: path.file_stem().and_then(|s| s.to_str()).unwrap_or("").into(),
        source_path: path.display().to_string(),
        ..Default::default()
    };
    let mut turns: Vec<Turn> = vec![];
    let mut tool_index: HashMap<String, (usize, usize, i64)> = HashMap::new();
    let mut seen_usage: HashSet<String> = HashSet::new();
    let mut title = String::new();
    let mut first_prompt = String::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let ts = v["timestamp"].as_str().map(ts_ms).unwrap_or(0);
        if ts > 0 {
            if meta.started_at == 0 { meta.started_at = ts; }
            meta.last_active_at = meta.last_active_at.max(ts);
        }
        if meta.cwd.is_empty() {
            if let Some(c) = v["cwd"].as_str() { meta.cwd = c.into(); }
        }
        match v["type"].as_str().unwrap_or("") {
            "ai-title" => if let Some(t) = v["aiTitle"].as_str() { title = t.into() },
            "summary" => if title.is_empty() { if let Some(t) = v["summary"].as_str() { title = t.into() } },
            "user" if v["isSidechain"] != true => {
                meta.messages += 1;
                let content = &v["message"]["content"];
                let results: Vec<&Value> = content.as_array().map(|a| a.iter().filter(|b| b["type"] == "tool_result").collect()).unwrap_or_default();
                if !results.is_empty() {
                    meta.failed += results.iter().filter(|r| r["is_error"] == true).count() as u32;
                    if !full { continue; }
                    for r in results {
                        let id = r["tool_use_id"].as_str().unwrap_or("");
                        let Some(&(ti, si, started)) = tool_index.get(id) else { continue };
                        let out = content_text(&r["content"]);
                        let step = &mut turns[ti].steps[si];
                        let is_err = r["is_error"] == true;
                        step.status = if is_err { "error".into() } else { "ok".into() };
                        if is_err { step.error = truncate(&out.replace('\n', " "), 200); }
                        if !is_err { step.result = summarize_output(&out); }
                        if ts > 0 && started > 0 { step.duration_ms = Some(ts - started); }
                        let tur = &v["toolUseResult"];
                        if let Some(hunks) = tur["structuredPatch"].as_array() {
                            let mut diff = String::new();
                            for h in hunks {
                                diff.push_str(&format!("@@ -{},{} +{},{} @@\n", h["oldStart"], h["oldLines"], h["newStart"], h["newLines"]));
                                for l in h["lines"].as_array().into_iter().flatten() {
                                    diff.push_str(l.as_str().unwrap_or(""));
                                    diff.push('\n');
                                }
                            }
                            let (a, d) = diff_counts(&diff);
                            step.change = Some(Change { path: tur["filePath"].as_str().unwrap_or(&step.input).into(), added: a, removed: d, diff });
                        } else if tur["type"] == "create" {
                            let body = tur["content"].as_str().unwrap_or("");
                            let diff: String = body.lines().map(|l| format!("+{l}\n")).collect();
                            step.change = Some(Change { path: tur["filePath"].as_str().unwrap_or(&step.input).into(), added: body.lines().count() as u32, removed: 0, diff });
                        }
                        if let Some(c) = &step.change { step.result = format!("+{} −{}", c.added, c.removed); }
                    }
                    continue;
                }
                if v["isMeta"] == true { continue; }
                let raw = content_text(content);
                let Some(prompt) = clean_prompt(&raw) else { continue };
                meta.prompts += 1;
                if first_prompt.is_empty() { first_prompt = prompt.clone(); }
                if full { turns.push(Turn { prompt, prompt_at: ts, ..Default::default() }); }
            }
            "assistant" if v["isSidechain"] != true => {
                meta.messages += 1;
                let m = &v["message"];
                if let Some(model) = m["model"].as_str() {
                    if !model.starts_with('<') { meta.model = model.into(); }
                }
                if let Some(id) = m["id"].as_str() {
                    if seen_usage.insert(id.to_string()) {
                        meta.tokens += m["usage"]["input_tokens"].as_i64().unwrap_or(0) + m["usage"]["output_tokens"].as_i64().unwrap_or(0);
                    }
                }
                for b in m["content"].as_array().into_iter().flatten() {
                    if b["type"] == "tool_use" { meta.tool_calls += 1; }
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
                        "thinking" | "redacted_thinking" => turn.steps.push(Step { kind: "thinking".into(), ..Default::default() }),
                        "tool_use" => {
                            let tool = b["name"].as_str().unwrap_or("tool").to_string();
                            let input = summarize_input(&tool, &b["input"]);
                            let mut step = Step { kind: "tool".into(), tool: tool.clone(), input, ..Default::default() };
                            if matches!(tool.as_str(), "Edit" | "MultiEdit") {
                                // the result carries the real patch; until then show the requested edit
                                let old = b["input"]["old_string"].as_str().unwrap_or("");
                                let new = b["input"]["new_string"].as_str().unwrap_or("");
                                let diff: String = old.lines().map(|l| format!("-{l}\n")).chain(new.lines().map(|l| format!("+{l}\n"))).collect();
                                let (a, d) = diff_counts(&diff);
                                step.change = Some(Change { path: step.input.clone(), added: a, removed: d, diff });
                            }
                            turn.steps.push(step);
                            if let Some(id) = b["id"].as_str() { tool_index.insert(id.into(), (ti, turn.steps.len() - 1, ts)); }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    meta.title = if !title.is_empty() { title } else { truncate(&first_prompt, 80) };
    Transcript { meta, turns }
}

/// The prompt as the user typed it: slash commands shown as `/name args`, harness-injected text skipped.
fn clean_prompt(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.starts_with("<local-command") || t.starts_with("<system-reminder>") || t.starts_with("Caveat:") {
        return None;
    }
    if t.starts_with("<command-") {
        let grab = |tag: &str| t.split(&format!("<{tag}>")).nth(1).and_then(|s| s.split(&format!("</{tag}>")).next()).map(|s| s.trim().to_string());
        let name = grab("command-name")?;
        let args = grab("command-args").unwrap_or_default();
        return Some(if args.is_empty() { name } else { format!("{name} {args}") });
    }
    if t.starts_with('[') && t.contains("Request interrupted") {
        return None;
    }
    Some(t.to_string())
}
