//! Per-computer SQLite database: usage rows, import cursors, cached quotas, session index, write ledger.

use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{Quota, SessionMeta, UsageRow};

pub struct Db {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
create table if not exists usage(
  request_id text primary key, profile text not null, harness text not null, ts integer not null,
  day text not null, model text not null, project text not null, session_id text not null,
  input integer not null, output integer not null, cache_read integer not null, cache_write integer not null);
create index if not exists usage_profile_day on usage(profile, day);
create table if not exists cursors(path text primary key, profile text not null, offset integer not null,
  mtime integer not null, len integer not null, tail text not null);
create table if not exists quota(profile text primary key, json text not null, updated_at integer not null);
create table if not exists sessions(key text primary key, profile text not null, harness text not null,
  path text not null, mtime integer not null, len integer not null, json text not null);
create table if not exists ledger(target text not null, kind text not null, item text not null,
  primary key(target, kind, item));
create table if not exists kv(k text primary key, v text not null);
create table if not exists router_log(ts integer not null, route text not null, profile text not null, provider text not null,
  model text not null, status integer not null, ms integer not null, input integer not null, output integer not null);
create table if not exists events(ts integer not null, level text not null, kind text not null, text text not null);
create index if not exists events_ts on events(ts);
create table if not exists quota_hist(ts integer not null, profile text not null, tier text not null, used real not null, resets_at integer);
create index if not exists quota_hist_p on quota_hist(profile, tier, ts);
"#;

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(10))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        // reply timings came later: add the columns, and read every log once more so older requests get them
        let has: bool = conn.prepare("select 1 from pragma_table_info('usage') where name = 'gen_ms'")?.exists([])?;
        if !has {
            conn.execute_batch("alter table usage add column ttft_ms integer; alter table usage add column gen_ms integer;
                delete from cursors; delete from kv where k like 'opencode:usage:%';")?;
        }
        // sessions learned their failed tool calls later: read each one again
        let done: bool = conn.prepare("select 1 from kv where k = 'migrated:session-failed'")?.exists([])?;
        if !done { conn.execute_batch("delete from sessions; insert or replace into kv values('migrated:session-failed', '1');")?; }
        // the router's time to the first chunk came later too
        let has: bool = conn.prepare("select 1 from pragma_table_info('router_log') where name = 'first_ms'")?.exists([])?;
        if !has { conn.execute_batch("alter table router_log add column first_ms integer;")?; }
        // Codex replies are timed since: read its logs once more so earlier replies get their timing
        let done: bool = conn.prepare("select 1 from kv where k = 'migrated:codex-timing'")?.exists([])?;
        if !done {
            conn.execute_batch("delete from cursors where path like '%rollout-%'; insert or replace into kv values('migrated:codex-timing', '1');")?;
        }
        Ok(Db { conn: Mutex::new(conn) })
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut c = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut c)
    }

    pub fn insert_usage(&self, rows: &[UsageRow]) -> Result<usize> {
        self.with(|c| {
            let tx = c.transaction()?;
            let mut n = 0;
            {
                // The same request can sit in two logs (a resumed session repeats earlier messages), sometimes with
                // a partial output count. Keep the copy with the most output, then the earliest, so the result is
                // the same whichever file is read first (files are read in parallel). An equal copy with a longer
                // timed reply (a later part of the same answer) brings its timings.
                let mut st = tx.prepare("insert into usage(request_id, profile, harness, ts, day, model, project, session_id, input, output,
                    cache_read, cache_write, ttft_ms, gen_ms) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?) on conflict(request_id) do update set
                    profile=excluded.profile, harness=excluded.harness, ts=excluded.ts, day=excluded.day, model=excluded.model,
                    project=excluded.project, session_id=excluded.session_id, input=excluded.input, output=excluded.output,
                    cache_read=excluded.cache_read, cache_write=excluded.cache_write, ttft_ms=excluded.ttft_ms, gen_ms=excluded.gen_ms
                    where (-excluded.output, excluded.ts, excluded.session_id) < (-usage.output, usage.ts, usage.session_id)
                       or (excluded.output = usage.output and excluded.ts = usage.ts and coalesce(excluded.gen_ms, -1) > coalesce(usage.gen_ms, -1))")?;
                for r in rows {
                    let day = chrono::DateTime::from_timestamp_millis(r.ts)
                        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    n += st.execute(params![r.request_id, r.profile, r.harness, r.ts, day, r.model, r.project, r.session_id,
                        r.input, r.output, r.cache_read, r.cache_write, r.ttft_ms, r.gen_ms])?;
                }
            }
            tx.commit()?;
            Ok(n)
        })
    }

    pub fn cursor(&self, path: &str) -> Result<Option<(i64, i64, i64, String)>> {
        self.with(|c| {
            Ok(c.query_row("select offset, mtime, len, tail from cursors where path=?", [path], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            }).optional()?)
        })
    }

    pub fn set_cursor(&self, path: &str, profile: &str, offset: i64, mtime: i64, len: i64, tail: &str) -> Result<()> {
        self.with(|c| {
            c.execute("insert or replace into cursors values(?,?,?,?,?,?)", params![path, profile, offset, mtime, len, tail])?;
            Ok(())
        })
    }

    pub fn put_quota(&self, profile: &str, q: &Quota) -> Result<()> {
        self.with(|c| {
            c.execute("insert or replace into quota values(?,?,?)", params![profile, serde_json::to_string(q)?, crate::util::now_ms()])?;
            Ok(())
        })
    }

    pub fn get_quota(&self, profile: &str) -> Result<Option<Quota>> {
        self.with(|c| {
            let s: Option<String> = c.query_row("select json from quota where profile=?", [profile], |r| r.get(0)).optional()?;
            Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
        })
    }

    pub fn session_cached(&self, key: &str, mtime: i64, len: i64) -> Result<Option<SessionMeta>> {
        self.with(|c| {
            let s: Option<String> = c.query_row("select json from sessions where key=? and mtime=? and len=?", params![key, mtime, len], |r| r.get(0)).optional()?;
            Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
        })
    }

    pub fn put_session(&self, key: &str, m: &SessionMeta, mtime: i64, len: i64) -> Result<()> {
        self.with(|c| {
            c.execute("insert or replace into sessions values(?,?,?,?,?,?,?)",
                params![key, m.profile, m.harness, m.source_path, mtime, len, serde_json::to_string(m)?])?;
            Ok(())
        })
    }

    pub fn ledger(&self, target: &str, kind: &str) -> Result<Vec<String>> {
        self.with(|c| {
            let mut st = c.prepare("select item from ledger where target=? and kind=? order by item")?;
            let v = st.query_map([target, kind], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(v)
        })
    }

    pub fn ledger_set(&self, target: &str, kind: &str, item: &str, owned: bool) -> Result<()> {
        self.with(|c| {
            if owned {
                c.execute("insert or ignore into ledger values(?,?,?)", [target, kind, item])?;
            } else {
                c.execute("delete from ledger where target=? and kind=? and item=?", [target, kind, item])?;
            }
            Ok(())
        })
    }

    pub fn kv_get(&self, k: &str) -> Result<Option<String>> {
        self.with(|c| Ok(c.query_row("select v from kv where k=?", [k], |r| r.get(0)).optional()?))
    }

    pub fn kv_set(&self, k: &str, v: &str) -> Result<()> {
        self.with(|c| { c.execute("insert or replace into kv values(?,?)", [k, v])?; Ok(()) })
    }

    /// One request the router handled; `first_ms` is the time to the first chunk of the answer, when one came.
    #[allow(clippy::too_many_arguments)]
    pub fn log_route(&self, route: &str, profile: &str, provider: &str, model: &str, status: i64, ms: i64, first_ms: Option<i64>, input: i64, output: i64) -> Result<()> {
        self.with(|c| {
            c.execute("insert into router_log(ts, route, profile, provider, model, status, ms, first_ms, input, output) values(?,?,?,?,?,?,?,?,?,?)",
                params![crate::util::now_ms(), route, profile, provider, model, status, ms, first_ms, input, output])?;
            Ok(())
        })
    }

    /// Something worth knowing happened (a provider paused, a backup made, a login expired). The same text again
    /// within ten minutes is one event, not two.
    pub fn event(&self, level: &str, kind: &str, text: &str) -> Result<()> {
        self.with(|c| {
            let now = crate::util::now_ms();
            let seen: bool = c.prepare("select 1 from events where text = ? and ts > ?")?.exists(params![text, now - 600_000])?;
            if !seen {
                c.execute("insert into events values(?,?,?,?)", params![now, level, kind, text])?;
                // keep a few weeks of them
                c.execute("delete from events where ts < ?", [now - 30 * 86_400_000])?;
            }
            Ok(())
        })
    }

    /// The latest events, newest first.
    pub fn events(&self, limit: i64) -> Result<Vec<serde_json::Value>> {
        self.with(|c| {
            let mut st = c.prepare("select ts, level, kind, text from events order by ts desc limit ?")?;
            let rows = st.query_map([limit], |r| Ok(serde_json::json!({"ts": r.get::<_, i64>(0)?, "level": r.get::<_, String>(1)?,
                "kind": r.get::<_, String>(2)?, "text": r.get::<_, String>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// A limit reading, kept only when it differs from the last one of that limit.
    pub fn quota_point(&self, profile: &str, tier: &str, used: f64, resets_at: Option<i64>, at: i64) -> Result<()> {
        self.with(|c| {
            let last: Option<(f64, Option<i64>)> = c.query_row("select used, resets_at from quota_hist where profile=? and tier=? order by ts desc limit 1",
                params![profile, tier], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            if last != Some((used, resets_at)) {
                c.execute("insert into quota_hist values(?,?,?,?,?)", params![at, profile, tier, used, resets_at])?;
                c.execute("delete from quota_hist where ts < ?", [at - 60 * 86_400_000])?;
            }
            Ok(())
        })
    }

    /// Readings of one limit since `since`, oldest first: (when, % used, when it resets).
    pub fn quota_points(&self, profile: &str, tier: &str, since: i64) -> Result<Vec<(i64, f64, Option<i64>)>> {
        self.with(|c| {
            let mut st = c.prepare("select ts, used, resets_at from quota_hist where profile=? and tier=? and ts >= ? order by ts")?;
            let rows = st.query_map(params![profile, tier, since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Requests the router handled, newest first.
    pub fn route_log(&self, limit: i64) -> Result<Vec<serde_json::Value>> {
        self.with(|c| {
            let mut st = c.prepare("select ts, route, profile, provider, model, status, ms, input, output, first_ms from router_log order by ts desc limit ?")?;
            let rows = st.query_map([limit], |r| Ok(serde_json::json!({"ts": r.get::<_, i64>(0)?, "route": r.get::<_, String>(1)?, "profile": r.get::<_, String>(2)?,
                "provider": r.get::<_, String>(3)?, "model": r.get::<_, String>(4)?, "status": r.get::<_, i64>(5)?, "ms": r.get::<_, i64>(6)?,
                "input": r.get::<_, i64>(7)?, "output": r.get::<_, i64>(8)?, "first_ms": r.get::<_, Option<i64>>(9)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn forget_profile(&self, profile: &str) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            for t in ["usage", "cursors", "quota", "sessions"] {
                tx.execute(&format!("delete from {t} where profile=?"), [profile])?;
            }
            tx.commit()?;
            Ok(())
        })
    }
}
