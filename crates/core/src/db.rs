//! Per-computer SQLite database: usage rows, import cursors, cached quotas, session index, write ledger.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{Quota, SessionMeta, UsageRow};

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
    /// Read-only connections, opened on first use: the pages read side by side, and never wait behind a write
    /// or the watcher's import (WAL lets readers run while one connection writes).
    readers: [OnceLock<Option<Mutex<Connection>>>; 3],
    /// This computer's id: every row written here carries it.
    pub device: String,
    /// Sync is on: every change is also written to the outbox for the other computers.
    pub journal: bool,
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
create table if not exists tags(kind text not null, key text not null, tag text not null, primary key(kind, key, tag));
create table if not exists outbox(seq integer primary key autoincrement, ts integer not null, op text not null, key text not null, data text not null);
create index if not exists outbox_key on outbox(op, key);
create table if not exists peers(device_id text primary key, name text, os text, app_version text, incarnation text, last_seen integer,
  last_seq integer, applied_seq integer, roots text, transport text, presence text);
create table if not exists cursors2(device_id text not null, incarnation text not null, seq integer not null, primary key(device_id, incarnation));
"#;

/// A usage row's insert: the same request from two logs (a resumed session repeats earlier messages) keeps the
/// copy with the most output, then the earliest, so the result is the same whichever file is read first. An
/// equal copy with a longer timed reply (a later part of the same answer) brings its timings.
const USAGE_UPSERT: &str = "insert into usage(request_id, profile, harness, ts, day, model, project, session_id, input, output,
    cache_read, cache_write, ttft_ms, gen_ms, device, cost_usd, price_rev) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) on conflict(request_id) do update set
    profile=excluded.profile, harness=excluded.harness, ts=excluded.ts, day=excluded.day, model=excluded.model,
    project=excluded.project, session_id=excluded.session_id, input=excluded.input, output=excluded.output,
    cache_read=excluded.cache_read, cache_write=excluded.cache_write, ttft_ms=excluded.ttft_ms, gen_ms=excluded.gen_ms,
    device=excluded.device, cost_usd=excluded.cost_usd, price_rev=excluded.price_rev
    where (-excluded.output, excluded.ts, excluded.session_id) < (-usage.output, usage.ts, usage.session_id)
       or (excluded.output = usage.output and excluded.ts = usage.ts and coalesce(excluded.gen_ms, -1) > coalesce(usage.gen_ms, -1))";

/// The calendar day of an instant in this computer's zone: every computer buckets a row by its own clock.
pub fn local_day(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts).map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// Store one usage row (ours or a merged one) with its frozen cost; 1 when it was written, 0 when an equal or better copy was there.
pub fn upsert_usage(c: &Connection, r: &UsageRow, device: &str, cost: Option<f64>, rev: Option<&str>) -> rusqlite::Result<usize> {
    c.execute(USAGE_UPSERT, params![r.request_id, r.profile, r.harness, r.ts, local_day(r.ts), r.model, r.project, r.session_id,
        r.input, r.output, r.cache_read, r.cache_write, r.ttft_ms, r.gen_ms, device, cost, rev])
}

/// A limit reading into the history, when it differs from the last one of that limit; true when it was new.
pub fn quota_point_in(c: &Connection, profile: &str, tier: &str, used: f64, resets_at: Option<i64>, at: i64) -> rusqlite::Result<bool> {
    let last: Option<(f64, Option<i64>)> = c.query_row("select used, resets_at from quota_hist where profile=? and tier=? order by ts desc limit 1",
        params![profile, tier], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    if last == Some((used, resets_at)) { return Ok(false); }
    c.execute("insert into quota_hist values(?,?,?,?,?)", params![at, profile, tier, used, resets_at])?;
    c.execute("delete from quota_hist where ts < ?", [at - 60 * 86_400_000])?;
    Ok(true)
}

/// The upper bound of the keys starting with `prefix` (its last byte, always an ASCII `|` here, stepped up): a
/// range the primary key answers, where `like` would scan every row.
pub fn prefix_end(prefix: &str) -> String {
    let mut b = prefix.as_bytes().to_vec();
    if let Some(l) = b.last_mut() { *l += 1; }
    String::from_utf8_lossy(&b).into_owned()
}

/// The journal key of a session: `<harness>/<profile>/<id>`.
pub fn session_key(m: &SessionMeta) -> String { format!("{}/{}/{}", m.harness, m.profile, m.id) }

/// A session as the other computers know it: by its journal key, newest `last_active_at` wins.
pub fn upsert_session(c: &Connection, key: &str, m: &SessionMeta, device: &str) -> rusqlite::Result<usize> {
    c.execute(&format!("insert into sessions(key, profile, harness, path, mtime, len, json, device, {COLS}) values(?,?,?,?,?,0,?,?,?,?,?,?,?,?,?,?)
        on conflict(key) do update set profile=excluded.profile, harness=excluded.harness, path=excluded.path, mtime=excluded.mtime, json=excluded.json, device=excluded.device,
        id=excluded.id, title=excluded.title, cwd=excluded.cwd, started_at=excluded.started_at, last_active_at=excluded.last_active_at, tokens=excluded.tokens,
        project_key=excluded.project_key, project_name=excluded.project_name
        where excluded.mtime > sessions.mtime"),
        params![key, m.profile, m.harness, m.source_path, m.last_active_at, serde_json::to_string(m).unwrap_or_default(), device,
            m.id, m.title, m.cwd, m.started_at, m.last_active_at, m.tokens, m.project_key, m.project_name])
}

/// The columns of `sessions` the pages filter and sort on, promoted out of the JSON (the index, WP-10).
pub const COLS: &str = "id, title, cwd, started_at, last_active_at, tokens, project_key, project_name";

/// The promoted columns of every row from its JSON: once at the migration, and for rows written before it.
pub fn promote_columns(c: &Connection) -> rusqlite::Result<usize> {
    c.execute("update sessions set id = coalesce(json_extract(json, '$.id'), ''), title = coalesce(json_extract(json, '$.title'), ''), cwd = coalesce(json_extract(json, '$.cwd'), ''),
        started_at = coalesce(json_extract(json, '$.started_at'), 0), last_active_at = coalesce(json_extract(json, '$.last_active_at'), 0), tokens = coalesce(json_extract(json, '$.tokens'), 0),
        project_key = coalesce(json_extract(json, '$.project_key'), ''), project_name = coalesce(json_extract(json, '$.project_name'), '') where last_active_at = 0", [])
}

/// Usage added up per day, computer, account, tool, model and project: the Overview, its heatmap and the account
/// and project totals read these few thousand rows instead of every response. Triggers keep it in step with
/// `usage` whoever writes it (import, merge from other computers, restore, pricing); it is never synced, each
/// computer adds up the rows it has. `cost` is null while none of the rows is priced, as `sum(cost_usd)` was.
const USAGE_DAY: &str = r#"
create table if not exists usage_day(day text not null, device text not null, profile text not null, harness text not null,
  model text not null, project text not null, requests integer not null, input integer not null, output integer not null,
  cache_read integer not null, cache_write integer not null, cost real, priced_n integer not null, priced_tokens integer not null,
  primary key(day, device, profile, harness, model, project));
create trigger if not exists usage_day_ins after insert on usage begin
  insert into usage_day values(new.day, new.device, new.profile, new.harness, new.model, new.project, 1, new.input, new.output,
    new.cache_read, new.cache_write, new.cost_usd, new.cost_usd is not null,
    case when new.cost_usd is null then 0 else new.input + new.output + new.cache_read + new.cache_write end)
  on conflict(day, device, profile, harness, model, project) do update set requests = requests + 1, input = input + excluded.input,
    output = output + excluded.output, cache_read = cache_read + excluded.cache_read, cache_write = cache_write + excluded.cache_write,
    cost = case when excluded.cost is null then cost else coalesce(cost, 0) + excluded.cost end,
    priced_n = priced_n + excluded.priced_n, priced_tokens = priced_tokens + excluded.priced_tokens;
end;
create trigger if not exists usage_day_del after delete on usage begin
  update usage_day set requests = requests - 1, input = input - old.input, output = output - old.output, cache_read = cache_read - old.cache_read,
    cache_write = cache_write - old.cache_write,
    cost = case when old.cost_usd is null then cost when priced_n <= 1 then null else cost - old.cost_usd end,
    priced_n = priced_n - (old.cost_usd is not null),
    priced_tokens = priced_tokens - (case when old.cost_usd is null then 0 else old.input + old.output + old.cache_read + old.cache_write end)
  where day = old.day and device = old.device and profile = old.profile and harness = old.harness and model = old.model and project = old.project;
  delete from usage_day where day = old.day and device = old.device and profile = old.profile and harness = old.harness and model = old.model
    and project = old.project and requests <= 0;
end;
create trigger if not exists usage_day_upd after update of day, device, profile, harness, model, project, input, output, cache_read, cache_write, cost_usd on usage begin
  update usage_day set requests = requests - 1, input = input - old.input, output = output - old.output, cache_read = cache_read - old.cache_read,
    cache_write = cache_write - old.cache_write,
    cost = case when old.cost_usd is null then cost when priced_n <= 1 then null else cost - old.cost_usd end,
    priced_n = priced_n - (old.cost_usd is not null),
    priced_tokens = priced_tokens - (case when old.cost_usd is null then 0 else old.input + old.output + old.cache_read + old.cache_write end)
  where day = old.day and device = old.device and profile = old.profile and harness = old.harness and model = old.model and project = old.project;
  delete from usage_day where day = old.day and device = old.device and profile = old.profile and harness = old.harness and model = old.model
    and project = old.project and requests <= 0;
  insert into usage_day values(new.day, new.device, new.profile, new.harness, new.model, new.project, 1, new.input, new.output,
    new.cache_read, new.cache_write, new.cost_usd, new.cost_usd is not null,
    case when new.cost_usd is null then 0 else new.input + new.output + new.cache_read + new.cache_write end)
  on conflict(day, device, profile, harness, model, project) do update set requests = requests + 1, input = input + excluded.input,
    output = output + excluded.output, cache_read = cache_read + excluded.cache_read, cache_write = cache_write + excluded.cache_write,
    cost = case when excluded.cost is null then cost else coalesce(cost, 0) + excluded.cost end,
    priced_n = priced_n + excluded.priced_n, priced_tokens = priced_tokens + excluded.priced_tokens;
end;
"#;

/// The totals from every row, once (and again if they are ever found missing): in one write transaction, so
/// a row another process adds meanwhile is counted by its trigger or by this, never both.
const USAGE_DAY_FILL: &str = "begin immediate;
delete from usage_day;
insert into usage_day select day, device, profile, harness, model, project, count(*), sum(input), sum(output), sum(cache_read), sum(cache_write),
  sum(cost_usd), count(cost_usd), coalesce(sum(case when cost_usd is null then 0 else input + output + cache_read + cache_write end), 0)
  from usage group by day, device, profile, harness, model, project;
insert or replace into kv values('migrated:usage_day', '1');
commit;";

/// The index's search table and the triggers that keep it in step with `sessions` (external content: it holds
/// only the words, by rowid; `insert or replace` elsewhere is safe because `recursive_triggers` is on).
const INDEX_SCHEMA: &str = r#"
create index if not exists sessions_dev_active on sessions(device, last_active_at);
create index if not exists sessions_proj_active on sessions(project_key, last_active_at);
create index if not exists usage_device_project on usage(device, project);
create index if not exists usage_day_project on usage(day, project);
-- the dashboard and the live strip read by time: without it each read scanned every usage row
create index if not exists usage_ts on usage(ts);
-- an account's last use (max(ts) per profile, on every account list): one index step, not every row of the account
create index if not exists usage_profile_ts on usage(profile, ts);
-- the live strip (this computer's last five minutes): device alone matched nearly every row
create index if not exists usage_device_ts on usage(device, ts);
create table if not exists project_index(key text primary key, name text not null, root text not null, remote text);
create virtual table if not exists sessions_fts using fts5(title, cwd, profile, content='sessions', content_rowid='rowid');
"#;

/// The triggers that keep the search table in step; made after the first build of the table.
const INDEX_TRIGGERS: &str = r#"
create trigger if not exists sessions_fts_ai after insert on sessions begin
  insert into sessions_fts(rowid, title, cwd, profile) values (new.rowid, new.title, new.cwd, new.profile); end;
create trigger if not exists sessions_fts_ad after delete on sessions begin
  insert into sessions_fts(sessions_fts, rowid, title, cwd, profile) values ('delete', old.rowid, old.title, old.cwd, old.profile); end;
create trigger if not exists sessions_fts_au after update of title, cwd, profile on sessions begin
  insert into sessions_fts(sessions_fts, rowid, title, cwd, profile) values ('delete', old.rowid, old.title, old.cwd, old.profile);
  insert into sessions_fts(rowid, title, cwd, profile) values (new.rowid, new.title, new.cwd, new.profile); end;
"#;

impl Db {
    pub fn open(path: &Path, device: &str, journal: bool) -> Result<Db> {
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(10))?;
        // a write transaction takes the write lock when it begins: a deferred one that read first fails at once
        // (SQLITE_BUSY_SNAPSHOT, no busy wait) when the router or the CLI wrote in between
        conn.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // pages read from the mapped file, not one ReadFile each: the year of usage the Overview adds up is ~3x faster
        conn.pragma_update(None, "mmap_size", 1i64 << 30)?;
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
        // across computers: every row says which computer it ran on, and its cost is frozen at the price of its day
        let has: bool = conn.prepare("select 1 from pragma_table_info('usage') where name = 'device'")?.exists([])?;
        if !has {
            conn.execute_batch("alter table usage add column device text not null default ''; alter table usage add column cost_usd real;
                alter table usage add column price_rev text; alter table sessions add column device text not null default '';")?;
        }
        conn.execute_batch("create index if not exists usage_unpriced on usage(model) where cost_usd is null;")?;
        // the direct connection: how a peer is reached and what it signs with
        for (col, ty) in [("address", "text"), ("last_error", "text"), ("port", "integer"), ("cert_pem", "text"), ("pubkey", "text"), ("paired_at", "integer"), ("last_pull", "integer"), ("path", "text"), ("connected", "integer"), ("relayed_by", "text")] {
            let has: bool = conn.prepare("select 1 from pragma_table_info('peers') where name = ?")?.exists([col])?;
            if !has { conn.execute_batch(&format!("alter table peers add column {col} {ty};"))?; }
        }
        let done: bool = conn.prepare("select 1 from kv where k = 'migrated:device'")?.exists([])?;
        if !done {
            let n = conn.execute("update usage set device = ?1 where device = ''", [device])? + conn.execute("update sessions set device = ?1 where device = ''", [device])?;
            conn.execute("insert or replace into kv values('migrated:device', '1')", [])?;
            if n > 0 { crate::log::info(&format!("{n} rows stamped as this computer's ({device})")); }
        }
        // a row written without a computer (seen once after an update) belongs to this one
        if !device.is_empty() {
            conn.execute("update usage set device=?1 where device=''", [device])?;
            conn.execute("update sessions set device=?1 where device=''", [device])?;
        }
        // the index (WP-10): the pages read the sessions table, so its filter and sort columns leave the JSON
        for (col, ty) in [("id", "text not null default ''"), ("title", "text not null default ''"), ("cwd", "text not null default ''"), ("started_at", "integer not null default 0"),
            ("last_active_at", "integer not null default 0"), ("tokens", "integer not null default 0"), ("project_key", "text not null default ''"), ("project_name", "text not null default ''"), ("open", "integer not null default 0")] {
            let has: bool = conn.prepare("select 1 from pragma_table_info('sessions') where name = ?")?.exists([col])?;
            if !has { conn.execute_batch(&format!("alter table sessions add column {col} {ty};"))?; }
        }
        conn.pragma_update(None, "recursive_triggers", "on")?;
        crate::archive::tables(&conn)?;
        conn.execute_batch(INDEX_SCHEMA)?;
        let done: bool = conn.prepare("select 1 from kv where k = 'migrated:index'")?.exists([])?;
        if !done {
            promote_columns(&conn)?;
            conn.execute_batch("insert into sessions_fts(sessions_fts) values('rebuild'); insert or replace into kv values('migrated:index', '1');")?;
        }
        conn.execute_batch(INDEX_TRIGGERS)?;
        conn.execute_batch(USAGE_DAY)?;
        if !conn.prepare("select 1 from kv where k = 'migrated:usage_day'")?.exists([])? {
            if let Err(e) = conn.execute_batch(USAGE_DAY_FILL) { let _ = conn.execute_batch("rollback"); return Err(e.into()); }
        }
        Ok(Db { conn: Mutex::new(conn), path: path.to_path_buf(), readers: Default::default(), device: device.into(), journal })
    }

    /// Rows without a cost (from before costs were frozen, or a catalog that lacked their model) priced now, by model.
    pub fn price_unpriced(&self, prices: &crate::pricing::Prices) -> Result<usize> {
        if prices.is_empty() { return Ok(0); }
        let rev = crate::pricing::rev(prices);
        self.with(|c| {
            let models: Vec<String> = c.prepare("select distinct model from usage where cost_usd is null")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            let mut n = 0;
            for m in models {
                let Some(p) = crate::pricing::find(prices, &m) else { continue };
                n += c.execute("update usage set cost_usd = (input * ?1 + output * ?2 + cache_read * ?3 + cache_write * ?4) / 1e6, price_rev = ?5 where model = ?6 and cost_usd is null",
                    params![p.input, p.output, p.cache_read, p.cache_write, rev, m])?;
            }
            if n > 0 { crate::log::info(&format!("{n} usage rows priced at list prices ({rev})")); }
            Ok(n)
        })
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut c = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut c)
    }

    fn reader(&self, i: usize) -> Option<&Mutex<Connection>> {
        self.readers[i].get_or_init(|| {
            let c = Connection::open(&self.path).ok()?;
            c.busy_timeout(std::time::Duration::from_secs(10)).ok()?;
            c.pragma_update(None, "query_only", true).ok()?;
            c.pragma_update(None, "mmap_size", 1i64 << 30).ok()?;
            Some(Mutex::new(c))
        }).as_ref()
    }

    /// A number that changes whenever any connection (this process's writer, the CLI, another process) commits a
    /// change to the database; always read on the same connection, as SQLite requires. None without a reader.
    pub fn data_version(&self) -> Option<i64> {
        let m = self.reader(0)?;
        let c = m.lock().unwrap_or_else(|e| e.into_inner());
        c.query_row("pragma data_version", [], |r| r.get(0)).ok()
    }

    /// A query on a read-only connection: a free one if there is, else the first one's turn. It sees what was
    /// committed when it starts; a write in it fails. Without a reader (it could not open) it runs as `with`.
    pub fn read<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        for i in 0..self.readers.len() {
            let Some(m) = self.reader(i) else { return self.with(f) };
            if let Ok(mut c) = m.try_lock() { return f(&mut c); }
        }
        match self.reader(0) {
            Some(m) => f(&mut m.lock().unwrap_or_else(|e| e.into_inner())),
            None => self.with(f),
        }
    }

    /// Store usage rows read here, each with its cost at today's list prices (frozen; `price_rev` names the table).
    pub fn insert_usage(&self, rows: &[UsageRow], prices: &crate::pricing::Prices) -> Result<usize> {
        // an unchanged file brings nothing: no transaction, no disk sync (this was most of a refresh's time)
        if rows.is_empty() { return Ok(0); }
        let rev = (!prices.is_empty()).then(|| crate::pricing::rev(prices));
        self.with(|c| {
            let tx = c.transaction()?;
            let mut n = 0;
            for r in rows {
                let cost = crate::pricing::find(prices, &r.model).map(|p| p.cost(r.input, r.output, r.cache_read, r.cache_write));
                let rev = cost.and(rev.as_deref());
                if upsert_usage(&tx, r, &self.device, cost, rev)? == 0 { continue; }
                n += 1;
                if self.journal {
                    let mut data = serde_json::to_value(r)?;
                    data["cost_usd"] = serde_json::json!(cost);
                    data["price_rev"] = serde_json::json!(rev);
                    crate::journal::record(&tx, "usage", &r.request_id, &data)?;
                }
            }
            tx.commit()?;
            Ok(n)
        })
    }

    /// A session of this computer as the journal carries it (project resolved); recorded when new or more recent.
    pub fn journal_session(&self, m: &SessionMeta) -> Result<()> {
        if !self.journal { return Ok(()); }
        let key = session_key(m);
        self.with(|c| {
            if upsert_session(c, &key, m, &self.device)? == 0 { return Ok(()); }
            crate::journal::record(c, "session", &key, &serde_json::to_value(m)?)?;
            Ok(())
        })
    }

    /// Sessions merged from the other computers, with the computer each ran on.
    pub fn remote_sessions(&self) -> Result<Vec<(String, SessionMeta)>> {
        self.with(|c| {
            let mut st = c.prepare("select device, json from sessions where device <> '' and device <> ?")?;
            let v = st.query_map([&self.device], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.flatten()
                .filter_map(|(d, j)| serde_json::from_str(&j).ok().map(|m| (d, m))).collect();
            Ok(v)
        })
    }

    /// Every cursor of one account, by file path: one query per import instead of one per file.
    pub fn cursors_for(&self, profile: &str) -> Result<std::collections::HashMap<String, (i64, i64, i64, String)>> {
        self.with(|c| {
            let mut st = c.prepare("select path, offset, mtime, len, tail from cursors where profile = ?")?;
            let rows = st.query_map([profile], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))))?;
            Ok(rows.flatten().collect())
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

    /// The cached records of one account's session files, by key: one query instead of one per file.
    pub fn sessions_cached_for(&self, prefix: &str) -> Result<std::collections::HashMap<String, (i64, i64, SessionMeta)>> {
        self.with(|c| {
            let mut st = c.prepare("select key, mtime, len, json from sessions where key >= ?1 and key < ?2")?;
            let rows = st.query_map([prefix, &prefix_end(prefix)], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)))?;
            let mut out = std::collections::HashMap::new();
            for (key, mtime, len, json) in rows.flatten() {
                if let Ok(m) = serde_json::from_str::<SessionMeta>(&json) { out.insert(key, (mtime, len, m)); }
            }
            Ok(out)
        })
    }

    /// This computer's sessions into the journal, in one transaction, only those whose activity moved since
    /// their last record (one statement per session was a second on every page open).
    pub fn journal_sessions(&self, list: &[SessionMeta]) -> Result<()> {
        if !self.journal || list.is_empty() { return Ok(()); }
        self.with(|c| {
            let known: std::collections::HashMap<String, i64> = c.prepare("select key, mtime from sessions where device = ? and key like '%/%'")?
                .query_map([&self.device], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<_>>()?;
            let tx = c.transaction()?;
            for m in list {
                let key = session_key(m);
                if known.get(&key).map(|t| *t >= m.last_active_at).unwrap_or(false) { continue; }
                if upsert_session(&tx, &key, m, &self.device)? == 0 { continue; }
                crate::journal::record(&tx, "session", &key, &serde_json::to_value(m)?)?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    /// A scan's cache row (keyed `<harness>|<profile>|<file>`), with the file's signature; the project and `open`
    /// columns the index resolved stay as they are.
    pub fn put_session(&self, key: &str, m: &SessionMeta, mtime: i64, len: i64) -> Result<()> {
        self.with(|c| {
            c.execute(&format!("insert into sessions(key, profile, harness, path, mtime, len, json, device, {COLS}) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,'','')
                on conflict(key) do update set profile=excluded.profile, harness=excluded.harness, path=excluded.path, mtime=excluded.mtime, len=excluded.len, json=excluded.json, device=excluded.device,
                id=excluded.id, title=excluded.title, cwd=excluded.cwd, started_at=excluded.started_at, last_active_at=excluded.last_active_at, tokens=excluded.tokens"),
                params![key, m.profile, m.harness, m.source_path, mtime, len, serde_json::to_string(m)?, self.device, m.id, m.title, m.cwd, m.started_at, m.last_active_at, m.tokens])?;
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

    /// Every tag of one kind ("session": profile/id, "project": the project key), by key.
    pub fn tags_of(&self, kind: &str) -> Result<std::collections::HashMap<String, Vec<String>>> {
        self.with(|c| {
            let mut st = c.prepare("select key, tag from tags where kind=? order by rowid")?;
            let mut m: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for r in st.query_map([kind], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.flatten() { m.entry(r.0).or_default().push(r.1); }
            Ok(m)
        })
    }

    /// Replace the tags of one session or project.
    pub fn tag_set(&self, kind: &str, key: &str, tags: &[String]) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("delete from tags where kind=? and key=?", [kind, key])?;
            for t in tags { tx.execute("insert or ignore into tags values(?,?,?)", [kind, key, t.as_str()])?; }
            if self.journal { crate::journal::record(&tx, "tag", &format!("{kind}/{key}"), &serde_json::json!({"tags": tags}))?; }
            tx.commit()?;
            Ok(())
        })
    }


    /// Every tag with how many sessions and projects carry it, oldest first (so each keeps its colour).
    pub fn tag_list(&self) -> Result<Vec<(String, i64, i64)>> {
        self.with(|c| {
            let mut st = c.prepare("select tag, sum(kind='session'), sum(kind='project') from tags group by tag order by min(rowid)")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })
    }

    /// Rename a tag everywhere; an empty new name deletes it.
    pub fn tag_rename(&self, from: &str, to: &str) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            // the journal carries whole tag lists per key: note which keys change before the old tag is gone
            let keys: Vec<(String, String)> = if self.journal { tx.prepare("select distinct kind, key from tags where tag=?")?.query_map([from], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()? } else { vec![] };
            if to.is_empty() { tx.execute("delete from tags where tag=?", [from])?; }
            else { tx.execute("insert or ignore into tags select kind, key, ?2 from tags where tag=?1", [from, to])?; tx.execute("delete from tags where tag=?", [from])?; }
            for (kind, key) in keys {
                let tags: Vec<String> = tx.prepare("select tag from tags where kind=? and key=? order by rowid")?.query_map([&kind, &key], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
                crate::journal::record(&tx, "tag", &format!("{kind}/{key}"), &serde_json::json!({"tags": tags}))?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn kv_get(&self, k: &str) -> Result<Option<String>> {
        self.read(|c| Ok(c.query_row("select v from kv where k=?", [k], |r| r.get(0)).optional()?))
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
            if quota_point_in(c, profile, tier, used, resets_at, at)? && self.journal {
                crate::journal::record(c, "quota", &format!("{profile}/{tier}"), &serde_json::json!({"used": used, "resets_at": resets_at, "at": at}))?;
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

    /// Forget an account's history read here; what the other computers read of it stays theirs.
    pub fn forget_profile(&self, profile: &str) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            if self.journal {
                let keys: Vec<String> = tx.prepare("select key from sessions where profile=? and device=? and key like '%/%'")?.query_map([profile, &self.device], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
                for k in keys { crate::journal::record(&tx, "tombstone", &format!("session/{k}"), &serde_json::json!({}))?; }
            }
            for t in ["cursors", "quota"] { tx.execute(&format!("delete from {t} where profile=?"), [profile])?; }
            for t in ["usage", "sessions"] { tx.execute(&format!("delete from {t} where profile=? and device=?"), [profile, &self.device])?; }
            tx.commit()?;
            Ok(())
        })
    }
}
