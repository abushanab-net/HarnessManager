//! OpenCode. A profile gets its own config and data folders through `XDG_CONFIG_HOME` / `XDG_DATA_HOME`;
//! sessions and usage are read from its SQLite database. OpenCode 2.x keeps logins in that database and,
//! by default, hands them to one shared background service that ignores those folders, so every profile
//! but the main one runs with `--standalone` (its own private server).
//! shortcut: OpenCode 1.x does not know `--standalone`; add a version check if 1.x support is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
        Caps { parallel_accounts: true, api_key: false, quota: false, usage: true, sessions: true, mcp: true, skills: true, prompts: true, routable: false }
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
        let stored = ctx.db.insert_usage(&rows, &crate::pricing::load(ctx))?;
        ctx.db.kv_set(&mark_key, &max.to_string())?;
        Ok(stored)
    }

    fn session_dirs(&self, ctx: &Ctx, p: &Profile) -> Vec<PathBuf> { vec![self.data_dir(ctx, p)].into_iter().filter(|d| d.is_dir()).collect() }

    // sessions and usage are in the database, whose write-ahead log moves at every write; snapshots, logs and tool output are not
    fn watches(&self, path: &Path) -> bool { matches!(path.file_name().and_then(|n| n.to_str()), Some("opencode.db" | "opencode.db-wal")) }

    fn sessions(&self, ctx: &Ctx, p: &Profile, since: i64) -> Result<Vec<SessionMeta>> {
        let Some(db) = self.open_db(ctx, p) else { return Ok(vec![]) };
        let file = self.data_dir(ctx, p).join("opencode.db");
        // shortcut: the list is remembered in this process (another one asks the database again) by the size and time of the
        // database and its log, both still for two seconds (`still`); a clock set back can hide a write until the next one
        type Listed = HashMap<(PathBuf, String, i64), ([Option<(i64, u64)>; 2], Vec<SessionMeta>)>;
        static LISTED: std::sync::Mutex<Option<Listed>> = std::sync::Mutex::new(None);
        let sig = [crate::util::file_sig(&file), crate::util::file_sig(&file.with_extension("db-wal"))];
        let at = (file.clone(), p.name.clone(), since);
        if let Some((s, list)) = LISTED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).get(&at) {
            if *s == sig { return Ok(list.clone()); }
        }
        let path = file.display().to_string();
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
        let list: Vec<SessionMeta> = rows.flatten().collect();
        if sig.iter().all(|s| crate::util::still(*s)) { LISTED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(at, (sig, list.clone())); }
        Ok(list)
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
                    "text" => if let Some(tx) = x["text"].as_str() { if !tx.trim().is_empty() { t.say(tx.trim().into()) } },
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

    /// OpenCode imports its own export: 2.x `session import <file> --directory <folder>`, else 1.x `import <file>`.
    fn materialize(&self, ctx: &Ctx, p: &Profile, local_root: &Path, t: &crate::transcripts::FetchedTranscript) -> Result<crate::transcripts::Materialized> {
        let mut last = String::new();
        for v2 in [true, false] {
            let mut c = self.launch(ctx, p, &import_args(v2, &t.path, local_root))?.command();
            c.stdin(std::process::Stdio::null());
            #[cfg(windows)]
            { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
            let o = c.output()?;
            if o.status.success() {
                return Ok(crate::transcripts::Materialized { launch_args: self.resume_args(&t.id), cwd: local_root.to_path_buf(),
                    note: "OpenCode imported the session for this folder; the original stays as it is.".into() });
            }
            last = String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("").to_string();
        }
        bail!("opencode could not import the session{}", if last.is_empty() { String::new() } else { format!(": {last}") })
    }
    fn resumes_elsewhere(&self) -> bool { true }

    fn item_paths(&self, ctx: &Ctx, p: &Profile) -> ItemPaths {
        let c = self.config_dir(ctx, p);
        ItemPaths { mcp: Some((c.join("opencode.json"), "opencode-json")), skills: Some(c.join("skills")), prompt: Some(c.join("AGENTS.md")) }
    }
}

/// The import command of OpenCode 2.x (`session import`, with the folder) or 1.x (`import`).
pub fn import_args(v2: bool, file: &Path, local_root: &Path) -> Vec<String> {
    if v2 { vec!["session".into(), "import".into(), file.display().to_string(), "--directory".into(), local_root.display().to_string()] } else { vec!["import".into(), file.display().to_string()] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_list_follows_the_database_also_when_it_was_remembered() {
        let t = tempfile::tempdir().unwrap();
        let ctx = Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let p = Profile { name: "opencode-main".into(), label: None, harness: "opencode".into(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, intro: None, created_at: 0 };
        let file = OpenCode.data_dir(&ctx, &p).join("opencode.db");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let db = Connection::open(&file).unwrap();
        db.execute_batch("create table session(id text, parent_id text, title text, directory text, time_created int, time_updated int, time_archived int, tokens_input int, tokens_output int, model text);
            create table message(id text, session_id text, time_created int, time_updated int, data text);
            create table part(id text, message_id text, session_id text, time_created int, time_updated int, data text);
            insert into session values('ses_1', null, 'Fix the flaky build', 'C:\\work', 5, 9, null, 300, 40, '');
            insert into message values('m1', 'ses_1', 5, 5, '{\"role\":\"user\"}');").unwrap();
        drop(db);
        let titles = || OpenCode.sessions(&ctx, &p, 0).unwrap().into_iter().map(|s| (s.title, s.messages, s.prompts)).collect::<Vec<_>>();
        // written a while ago: the list is remembered, and asked again it is the same
        std::fs::File::options().write(true).open(&file).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60)).unwrap();
        for _ in 0..2 { assert_eq!(titles(), [("Fix the flaky build".to_string(), 1, 1)]); }
        // a write moves the file's time: the database is asked again, at once and at every look while the write is fresh
        let db = Connection::open(&file).unwrap();
        db.execute_batch("update session set title = 'Fix the build for good'; insert into message values('m2', 'ses_1', 6, 6, '{\"role\":\"assistant\"}');").unwrap();
        drop(db);
        assert_eq!(titles(), [("Fix the build for good".to_string(), 2, 1)]);
        let db = Connection::open(&file).unwrap();
        db.execute_batch("insert into message values('m3', 'ses_1', 7, 7, '{\"role\":\"user\"}');").unwrap();
        drop(db);
        assert_eq!(titles(), [("Fix the build for good".to_string(), 3, 2)]);
        // the watcher takes the database and its log, and leaves what OpenCode writes beside them
        let dir = file.parent().unwrap();
        assert!(OpenCode.watches(&file) && OpenCode.watches(&dir.join("opencode.db-wal")));
        assert!(!OpenCode.watches(&dir.join("opencode.db-shm")) && !OpenCode.watches(&dir.join("snapshot").join("pack-1.idx")) && !OpenCode.watches(&dir.join("log").join("2026-11-01.log")));
    }
}

fn model_name(raw: &str) -> String {
    serde_json::from_str::<Value>(raw).ok().and_then(|v| v["modelID"].as_str().or(v["id"].as_str()).map(String::from)).unwrap_or_else(|| raw.into())
}
