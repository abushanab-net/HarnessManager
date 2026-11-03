//! OpenCode. A profile gets its own config and data folders through `XDG_CONFIG_HOME` / `XDG_DATA_HOME`;
//! sessions and usage are read from its SQLite database. OpenCode 2.x keeps logins in that database and,
//! by default, hands them to one shared background service that ignores those folders, so every profile
//! but the main one runs with `--standalone` (its own private server).
//! shortcut: OpenCode 1.x does not know `--standalone`; add a version check if 1.x support is needed.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::parse::*;
use super::{base_env, Harness, ItemPaths};
use crate::launch::{resolve_program, LaunchSpec};
use crate::model::*;
use crate::registry::{Kind, Profile};
use crate::util::{read_json, truncate};
use crate::Ctx;

pub struct OpenCode;

impl OpenCode {
    pub fn data_dir(&self, ctx: &Ctx, p: &Profile) -> PathBuf {
        if p.main { ctx.home.join(".local").join("share").join("opencode") } else { self.home(ctx, p).join("data").join("opencode") }
    }
    pub fn config_dir(&self, ctx: &Ctx, p: &Profile) -> PathBuf {
        if p.main { ctx.home.join(".config").join("opencode") } else { self.home(ctx, p).join("config").join("opencode") }
    }
    fn open_db(&self, ctx: &Ctx, p: &Profile) -> Option<Connection> {
        let path = self.data_dir(ctx, p).join("opencode.db");
        if !path.exists() { return None; }
        Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()
    }
}

impl Harness for OpenCode {
    fn id(&self) -> &'static str { "opencode" }
    fn label(&self) -> &'static str { "OpenCode" }
    fn caps(&self) -> Caps {
        Caps { parallel_accounts: true, subscription_login: true, api_key: false, quota: false, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: false }
    }
    fn binary(&self) -> &'static str { "opencode" }
    fn default_home(&self, ctx: &Ctx) -> PathBuf { ctx.home.join(".config").join("opencode") }
    fn dir_prefix(&self) -> &'static str { "opencode" }

    fn prepare(&self, ctx: &Ctx, p: &Profile) -> Result<()> {
        std::fs::create_dir_all(self.config_dir(ctx, p))?;
        std::fs::create_dir_all(self.data_dir(ctx, p))?;
        Ok(())
    }

    fn launch(&self, ctx: &Ctx, p: &Profile, args: &[String]) -> Result<LaunchSpec> {
        let mut spec = LaunchSpec { program: resolve_program(self.id(), self.binary())?, args: args.to_vec(), ..Default::default() };
        spec.env_unset = vec!["XDG_CONFIG_HOME".into(), "XDG_DATA_HOME".into(), "OPENCODE_CONFIG_DIR".into()];
        base_env(ctx, &mut spec);
        if !p.main {
            let h = self.home(ctx, p);
            spec.env_set.push(("XDG_CONFIG_HOME".into(), h.join("config").display().to_string()));
            spec.env_set.push(("XDG_DATA_HOME".into(), h.join("data").display().to_string()));
            spec.args.insert(0, "--standalone".into());
        }
        Ok(spec)
    }

    fn login(&self, ctx: &Ctx, p: &Profile) -> Result<LaunchSpec> {
        if p.kind != Kind::Subscription { bail!("OpenCode keeps its own provider keys; log in with `hrm login {}`", p.name); }
        self.launch(ctx, p, &["auth".into(), "login".into()])
    }

    fn identity(&self, ctx: &Ctx, p: &Profile) -> Identity {
        let auth = read_json(&self.data_dir(ctx, p).join("auth.json")).unwrap_or(Value::Null);
        let mut providers: Vec<String> = auth.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        // OpenCode 2.x: logins in the database
        if let Some(db) = self.open_db(ctx, p) {
            if let Ok(mut st) = db.prepare("select distinct integration_id from credential where integration_id is not null") {
                let ids = st.query_map([], |r| r.get::<_, String>(0)).map(|r| r.flatten().collect::<Vec<_>>()).unwrap_or_default();
                for id in ids { if !providers.contains(&id) { providers.push(id); } }
            }
        }
        Identity {
            logged_in: !providers.is_empty(),
            plan: (!providers.is_empty()).then(|| providers.join(", ")),
            ..Default::default()
        }
    }

    fn import_usage(&self, ctx: &Ctx, p: &Profile) -> Result<usize> {
        let Some(db) = self.open_db(ctx, p) else { return Ok(0) };
        let mark_key = format!("opencode:usage:{}", p.name);
        let since: i64 = ctx.db.kv_get(&mark_key)?.and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut st = db.prepare("select m.id, m.session_id, m.time_updated, m.data, coalesce(s.directory,'') from message m left join session s on s.id=m.session_id where m.time_updated >= ?1 order by m.time_updated")?;
        let mut rows = vec![];
        let mut max = since;
        let iter = st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)))?;
        for row in iter.flatten() {
            let (id, sid, updated, data, dir) = row;
            max = max.max(updated);
            let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
            if v["role"] != "assistant" || v["time"]["completed"].is_null() { continue; }
            let t = &v["tokens"];
            rows.push(UsageRow {
                request_id: format!("opencode:{id}"),
                profile: p.name.clone(),
                harness: "opencode".into(),
                ts: v["time"]["created"].as_i64().unwrap_or(updated),
                model: v["modelID"].as_str().unwrap_or("").into(),
                project: v["path"]["cwd"].as_str().map(String::from).unwrap_or(dir),
                session_id: sid,
                input: t["input"].as_i64().unwrap_or(0),
                output: t["output"].as_i64().unwrap_or(0) + t["reasoning"].as_i64().unwrap_or(0),
                cache_read: t["cache"]["read"].as_i64().unwrap_or(0),
                cache_write: t["cache"]["write"].as_i64().unwrap_or(0),
                // OpenCode logs when a reply started and finished, not its first token: the whole reply is timed
                ttft_ms: None,
                gen_ms: v["time"]["completed"].as_i64().zip(v["time"]["created"].as_i64()).map(|(e, s)| e - s).filter(|d| *d > 0),
            });
        }
        let stored = ctx.db.insert_usage(&rows)?;
        ctx.db.kv_set(&mark_key, &max.to_string())?;
        Ok(stored)
    }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        let Some(db) = self.open_db(ctx, p) else { return Ok(vec![]) };
        let path = self.data_dir(ctx, p).join("opencode.db").display().to_string();
        let mut st = db.prepare(
            "select s.id, s.title, s.directory, s.time_created, s.time_updated, coalesce(s.tokens_input,0)+coalesce(s.tokens_output,0), coalesce(s.model,''),
             (select count(*) from message m where m.session_id=s.id),
             (select count(*) from message m where m.session_id=s.id and json_extract(m.data,'$.role')='user'),
             (select count(*) from part x where x.session_id=s.id and json_extract(x.data,'$.type')='tool')
             from session s where s.parent_id is null and s.time_archived is null and s.time_updated >= ?1")?;
        let rows = st.query_map([since], |r| {
            Ok(SessionMeta {
                harness: "opencode".into(),
                profile: p.name.clone(),
                id: r.get(0)?,
                title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                cwd: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                started_at: r.get(3)?,
                last_active_at: r.get(4)?,
                tokens: r.get(5)?,
                model: model_name(&r.get::<_, String>(6)?),
                messages: r.get::<_, i64>(7)? as u32,
                prompts: r.get::<_, i64>(8)? as u32,
                tool_calls: r.get::<_, i64>(9)? as u32,
                source_path: path.clone(),
                ..Default::default()
            })
        })?;
        Ok(rows.flatten().collect())
    }

    fn transcript(&self, ctx: &Ctx, p: &Profile, s: &SessionMeta) -> Result<Transcript> {
        let Some(db) = self.open_db(ctx, p) else { bail!("OpenCode database not found") };
        let mut msgs = db.prepare("select id, data, time_created from message where session_id=? order by time_created")?;
        let mut parts = db.prepare("select message_id, data from part where session_id=? order by time_created, id")?;
        let mut by_msg: HashMap<String, Vec<Value>> = HashMap::new();
        for r in parts.query_map([&s.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.flatten() {
            if let Ok(v) = serde_json::from_str(&r.1) { by_msg.entry(r.0).or_default().push(v); }
        }
        let mut turns: Vec<Turn> = vec![];
        for (id, data, created) in msgs.query_map([&s.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?.flatten() {
            let Ok(m) = serde_json::from_str::<Value>(&data) else { continue };
            let ps = by_msg.remove(&id).unwrap_or_default();
            if m["role"] == "user" {
                let text = ps.iter().filter(|x| x["type"] == "text" && x["synthetic"] != true).filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("\n");
                turns.push(Turn { prompt: text.trim().into(), prompt_at: created, ..Default::default() });
                continue;
            }
            if turns.is_empty() { turns.push(Turn { prompt_at: created, ..Default::default() }); }
            let t = turns.last_mut().unwrap();
            t.answer_at = m["time"]["completed"].as_i64().unwrap_or(created);
            if t.model.is_empty() { t.model = m["modelID"].as_str().unwrap_or("").into(); }
            for x in ps {
                match x["type"].as_str().unwrap_or("") {
                    "text" => if let Some(tx) = x["text"].as_str() { if !tx.trim().is_empty() { t.answer.push(tx.trim().into()) } },
                    "reasoning" => t.steps.push(Step { kind: "thinking".into(), ..Default::default() }),
                    "tool" => {
                        let tool = x["tool"].as_str().unwrap_or("tool").to_string();
                        let st = &x["state"];
                        let out = st["output"].as_str().unwrap_or("");
                        let failed = st["status"] == "error";
                        let mut step = Step {
                            kind: "tool".into(), input: summarize_input(&tool, &st["input"]), tool: tool.clone(),
                            status: if failed { "error".into() } else if st["status"] == "completed" { "ok".into() } else { String::new() },
                            error: if failed { truncate(st["error"].as_str().unwrap_or(out), 200) } else { String::new() },
                            result: summarize_output(out),
                            duration_ms: match (st["time"]["start"].as_i64(), st["time"]["end"].as_i64()) { (Some(a), Some(b)) => Some(b - a), _ => None },
                            ..Default::default()
                        };
                        if (tool == "edit" || tool == "write") && st["status"] == "completed" {
                            let old = st["input"]["oldString"].as_str().unwrap_or("");
                            let new = st["input"]["newString"].as_str().or(st["input"]["content"].as_str()).unwrap_or("");
                            let diff: String = old.lines().map(|l| format!("-{l}\n")).chain(new.lines().map(|l| format!("+{l}\n"))).collect();
                            let (a, r) = diff_counts(&diff);
                            step.change = Some(Change { path: step.input.clone(), added: a, removed: r, diff });
                            step.result = format!("+{a} −{r}");
                        }
                        t.steps.push(step);
                    }
                    _ => {}
                }
            }
        }
        Ok(Transcript { meta: s.clone(), turns })
    }

    fn resume_args(&self, id: &str) -> Vec<String> { vec!["--session".into(), id.into()] }

    fn item_paths(&self, ctx: &Ctx, p: &Profile) -> ItemPaths {
        let c = self.config_dir(ctx, p);
        ItemPaths { mcp: Some((c.join("opencode.json"), "opencode-json")), skills: Some(c.join("skills")), prompt: Some(c.join("AGENTS.md")) }
    }
}

fn model_name(raw: &str) -> String {
    serde_json::from_str::<Value>(raw).ok().and_then(|v| v["modelID"].as_str().or(v["id"].as_str()).map(String::from)).unwrap_or_else(|| raw.into())
}
