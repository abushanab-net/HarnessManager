//! Overview: a period's numbers beside the period before, small trends, reply timing, the usage mix, limit
//! forecasts from the reading history, live rates on each route, and the score of the game layer.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde_json::{json, Value};

use crate::registry::Kind;
use crate::util::now_ms;
use crate::Ctx;

/// Points in each small trend line.
const BUCKETS: usize = 12;
/// How far back the live rates look.
const LIVE_MS: i64 = 5 * 60_000;

#[derive(Default, Clone)]
struct Agg { tokens: i64, requests: i64, cost: f64, priced: bool, billed: f64, prompt: i64, cache_read: i64, ttft: Vec<i64> }

impl Agg {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, cost: Option<f64>, billed: bool, i: i64, o: i64, cr: i64, cw: i64, ttft: Option<i64>) {
        self.tokens += i + o + cr + cw;
        self.requests += 1;
        self.prompt += i + cr + cw;
        self.cache_read += cr;
        if let Some(c) = cost {
            self.cost += c;
            self.priced = true;
            if billed { self.billed += c; }
        }
        if let Some(t) = ttft { self.ttft.push(t); }
    }
    fn hit(&self) -> f64 { if self.prompt > 0 { self.cache_read as f64 / self.prompt as f64 } else { 0.0 } }
    fn json(&mut self) -> Value {
        self.ttft.sort_unstable();
        json!({"tokens": self.tokens, "requests": self.requests, "cost": self.priced.then_some(self.cost), "billed": self.billed,
               "hit": self.hit(), "p50": pct(&self.ttft, 0.5), "p95": pct(&self.ttft, 0.95), "replies": self.ttft.len()})
    }
}

/// The value below which `p` of the sorted values fall.
fn pct(sorted: &[i64], p: f64) -> Option<i64> {
    if sorted.is_empty() { return None; }
    Some(sorted[((sorted.len() - 1) as f64 * p).round() as usize])
}

/// A folder path's last part: the name a project goes by.
fn project_name(p: &str) -> String {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or("(no folder)").to_string()
}

/// What the router spent for one account since local midnight, at API list prices (models without a price count 0).
pub fn spent_today(ctx: &Ctx, profile: &str) -> Result<f64> {
    let prices = crate::pricing::load(ctx);
    let midnight = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|t| t.and_local_timezone(chrono::Local).earliest()).map(|t| t.timestamp_millis()).unwrap_or(0);
    let rows: Vec<(String, i64, i64)> = ctx.db.with(|c| {
        let mut st = c.prepare("select model, sum(input), sum(output) from router_log where profile = ? and ts >= ? group by model")?;
        let v = st.query_map(rusqlite::params![profile, midnight], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    // + 0.0: an empty sum of floats is -0.0, which would read "$-0.00"
    Ok(rows.iter().map(|(m, i, o)| crate::pricing::find(&prices, m).map(|p| p.cost(*i, *o, 0, 0)).unwrap_or(0.0)).sum::<f64>() + 0.0)
}

/// Everything the Overview shows for the last `hours`, optionally for one tool; `scope` "here" keeps to this computer.
pub fn dashboard(ctx: &Ctx, hours: i64, harness: Option<&str>, scope: Option<&str>) -> Result<Value> {
    let now = now_ms();
    let span = hours.max(1) * 3_600_000;
    let since = now - span;
    let reg = ctx.registry()?;
    let billed: HashSet<&str> = reg.profiles.iter().filter(|p| p.kind != Kind::Subscription).map(|p| p.name.as_str()).collect();
    let caps = caps(ctx, &reg);
    let h = harness.unwrap_or("");
    let dev = if scope == Some("here") { ctx.db.device.as_str() } else { "" };
    type Row = (i64, String, String, String, i64, i64, i64, i64, Option<i64>, Option<f64>, String);
    let rows: Vec<Row> = ctx.db.with(|c| {
        let mut st = c.prepare("select ts, model, profile, project, input, output, cache_read, cache_write, ttft_ms, cost_usd, device from usage where ts >= ?1 and (?2 = '' or harness = ?2) and (?3 = '' or device = ?3)")?;
        let v = st.query_map(rusqlite::params![since - span, h, dev], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let (mut cur, mut prev) = (Agg::default(), Agg::default());
    let mut spark = vec![Agg::default(); BUCKETS];
    let mut mix: HashMap<(&str, String), (i64, f64)> = HashMap::new();
    let mut by_device: HashMap<&str, Agg> = HashMap::new();
    for (ts, model, profile, project, i, o, cr, cw, ttft, cost, device) in &rows {
        let b = billed.contains(profile.as_str());
        if *ts < since { prev.add(*cost, b, *i, *o, *cr, *cw, *ttft); continue; }
        cur.add(*cost, b, *i, *o, *cr, *cw, *ttft);
        by_device.entry(device.as_str()).or_default().add(*cost, b, *i, *o, *cr, *cw, *ttft);
        let k = (((ts - since) * BUCKETS as i64) / span).clamp(0, BUCKETS as i64 - 1) as usize;
        spark[k].add(*cost, b, *i, *o, *cr, *cw, *ttft);
        for key in [("account", profile.clone()), ("model", model.clone()), ("project", project_name(project))] {
            let e = mix.entry(key).or_default();
            e.0 += i + o + cr + cw;
            e.1 += cost.unwrap_or(0.0);
        }
    }
    // the four tiles split by computer: [{id, name, value}] each, biggest first
    let names = crate::journal::device_names(ctx);
    let mut devs: Vec<(&str, Agg)> = by_device.into_iter().collect();
    devs.sort_by_key(|(_, a)| std::cmp::Reverse(a.tokens));
    let tile = |f: &dyn Fn(&Agg) -> Value| -> Value {
        json!({"by_device": devs.iter().map(|(id, a)| json!({"id": id, "name": names.get(*id).cloned().unwrap_or_else(|| id.to_string()), "value": f(a)})).collect::<Vec<_>>()})
    };
    let tiles = json!({"tokens": tile(&|a| json!(a.tokens)), "requests": tile(&|a| json!(a.requests)), "cost": tile(&|a| json!(a.priced.then_some(a.cost))), "hit": tile(&|a| json!(a.hit()))});

    // time to first reply: 16 bars from 0 to a little past the goal or the p95, whichever is later
    let goal = ctx.settings.first_reply_goal_s.max(1) as i64 * 1000;
    let cur_json = cur.json();
    let p95 = cur_json["p95"].as_i64().unwrap_or(0);
    let top = ((goal * 3 / 2).max(p95 * 5 / 4).max(2000) + 999) / 1000 * 1000;
    let mut hist = vec![0i64; 16];
    for t in &cur.ttft { hist[((t * 16) / top).clamp(0, 15) as usize] += 1; }
    let within = cur.ttft.iter().filter(|t| **t <= goal).count();

    let mixes: serde_json::Map<String, Value> = ["account", "model", "project"].iter().map(|kind| {
        let mut v: Vec<(&String, &(i64, f64))> = mix.iter().filter(|((k, _), _)| k == kind).map(|((_, n), x)| (n, x)).collect();
        v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        (kind.to_string(), json!(v.iter().take(6).map(|(n, (t, c))| json!({"key": n, "tokens": t, "cost": c})).collect::<Vec<_>>()))
    }).collect();

    let sparks: Vec<Value> = spark.iter_mut().map(|a| a.json()).collect();
    Ok(json!({
        "now": now, "hours": hours, "cur": cur_json, "prev": prev.json(),
        "spark": {
            "tokens": sparks.iter().map(|s| s["tokens"].clone()).collect::<Vec<_>>(),
            "requests": sparks.iter().map(|s| s["requests"].clone()).collect::<Vec<_>>(),
            "cost": sparks.iter().map(|s| s["cost"].clone()).collect::<Vec<_>>(),
            "hit": sparks.iter().map(|s| s["hit"].clone()).collect::<Vec<_>>(),
            "p95": sparks.iter().map(|s| s["p95"].clone()).collect::<Vec<_>>(),
        },
        "latency": {"hist": hist, "top_ms": top, "goal_ms": goal, "within": within},
        "mix": mixes,
        "tiles": tiles,
        "forecast": forecasts(ctx, now)?,
        "live": live(ctx, now)?,
        "events": ctx.db.events(8)?,
        "game": game(ctx, now)?,
        "fleet": fleet(ctx),
        "caps": caps,
    }))
}

/// The router's last 24 hours: requests, errors, failovers, and per provider its requests and p95 time to first chunk.
pub fn router_stats(ctx: &Ctx, now: i64) -> Result<Value> {
    let since = now - 86_400_000;
    ctx.db.with(|c| {
        let (total, errors): (i64, i64) = c.query_row("select count(*), coalesce(sum(status <> 200), 0) from router_log where ts >= ?", [since], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let failovers: i64 = c.query_row("select count(*) from events where kind = 'router' and text like '% failed over %' and ts >= ?", [since], |r| r.get(0))?;
        let mut st = c.prepare("select provider, first_ms from router_log where ts >= ?")?;
        let mut per: HashMap<String, (i64, Vec<i64>)> = HashMap::new();
        for r in st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)))? {
            let (p, f) = r?;
            let e = per.entry(p).or_default();
            e.0 += 1;
            if let Some(f) = f { e.1.push(f); }
        }
        let providers: serde_json::Map<String, Value> = per.into_iter().map(|(p, (n, mut f))| { f.sort_unstable(); (p, json!({"requests": n, "p95_first": pct(&f, 0.95)})) }).collect();
        Ok(json!({"requests": total, "errors": errors, "failovers": failovers, "providers": providers}))
    })
}

/// One account's last 7 days (tokens, cache share, cost at list prices, reply timing, busiest project) and its
/// tokens per day over 30 days, for its own page.
pub fn account_stats(ctx: &Ctx, profile: &str) -> Result<Value> {
    let days = crate::service::last_days(30);
    let week = &days[days.len() - 7];
    type Row = (String, String, i64, i64, i64, i64, Option<i64>, Option<f64>);
    let rows: Vec<Row> = ctx.db.with(|c| {
        let mut st = c.prepare("select day, project, input, output, cache_read, cache_write, ttft_ms, cost_usd from usage where profile = ?1 and day >= ?2")?;
        let v = st.query_map(rusqlite::params![profile, days[0]], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let mut a = Agg::default();
    let (mut per_day, mut cost30, mut projects) = (vec![0i64; days.len()], 0.0, HashMap::<String, i64>::new());
    for (day, project, i, o, cr, cw, ttft, cost) in &rows {
        let t = i + o + cr + cw;
        let cost = *cost;
        cost30 += cost.unwrap_or(0.0);
        if let Some(k) = days.iter().position(|d| d == day) { per_day[k] += t; }
        if day >= week {
            a.add(cost, false, *i, *o, *cr, *cw, *ttft);
            *projects.entry(project_name(project)).or_default() += t;
        }
    }
    let top = projects.iter().max_by_key(|(_, t)| **t).map(|(n, t)| json!({"name": n, "share": if a.tokens > 0 { *t as f64 / a.tokens as f64 } else { 0.0 }}));
    let mut w = a.json();
    w["top_project"] = json!(top);
    Ok(json!({"week": w, "days": days, "per_day": per_day, "cost30": cost30}))
}

/// One session's numbers from the usage log: share read from cache, first-reply p50, cost at list prices.
pub fn session_stats(ctx: &Ctx, profile: &str, session: &str) -> Result<Value> {
    type Row = (Option<f64>, i64, i64, i64, i64, Option<i64>);
    let rows: Vec<Row> = ctx.db.with(|c| {
        let mut st = c.prepare("select cost_usd, input, output, cache_read, cache_write, ttft_ms from usage where profile = ?1 and session_id = ?2")?;
        let v = st.query_map(rusqlite::params![profile, session], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let mut a = Agg::default();
    for (cost, i, o, cr, cw, t) in &rows { a.add(*cost, false, *i, *o, *cr, *cw, *t); }
    Ok(a.json())
}

/// Each capped account's cap and what the router spent for it today.
pub fn caps(ctx: &Ctx, reg: &crate::Registry) -> serde_json::Map<String, Value> {
    reg.profiles.iter().filter_map(|p| p.daily_cap_usd.map(|c| (p.name.clone(), c)))
        .map(|(n, c)| { let s = spent_today(ctx, &n).unwrap_or(0.0); (n, json!({"cap": c, "spent": s})) }).collect()
}

/// For each limit with two or more readings in its current window: the level it reaches at the reset at this pace,
/// and when it runs out first, if it does. Keyed "account|limit".
pub fn forecasts(ctx: &Ctx, now: i64) -> Result<serde_json::Map<String, Value>> {
    let keys: Vec<(String, String)> = ctx.db.with(|c| {
        let mut st = c.prepare("select distinct profile, tier from quota_hist where ts >= ?")?;
        let v = st.query_map([now - 8 * 86_400_000], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })?;
    let mut out = serde_json::Map::new();
    for (p, t) in keys {
        let pts = ctx.db.quota_points(&p, &t, now - 8 * 86_400_000)?;
        if let Some(f) = forecast(&pts, now) { out.insert(format!("{p}|{t}"), f); }
    }
    Ok(out)
}

/// The pace of one window, from its readings (oldest first: when, % used, reset time).
fn forecast(pts: &[(i64, f64, Option<i64>)], now: i64) -> Option<Value> {
    let &(t1, u1, r1) = pts.last()?;
    let reset = r1.filter(|r| *r > now)?;
    // the same window: readings whose reset time is the last one's, give or take the minutes the tools round to
    let first = pts.iter().find(|(_, _, r)| r.map(|r| (r - reset).abs() < 5 * 60_000).unwrap_or(false))?;
    let (t0, u0) = (first.0, first.1);
    if t1 - t0 < 10 * 60_000 || u1 <= u0 { return None; }
    let rate = (u1 - u0) / (t1 - t0) as f64; // % per ms
    let at_reset = u1 + rate * (reset - t1) as f64;
    let runs_out = (at_reset > 100.0).then(|| t1 + ((100.0 - u1) / rate) as i64);
    Some(json!({"at_reset": at_reset, "runs_out": runs_out, "per_hour": rate * 3_600_000.0}))
}

/// The last five minutes: tokens per account (here and on the other computers, from their presence) and per
/// router provider, the circuit breakers, and what runs on the other computers.
pub fn live(ctx: &Ctx, now: i64) -> Result<Value> {
    let peers: Vec<crate::journal::Peer> = crate::journal::peers(ctx).unwrap_or_default().into_iter()
        .filter(|p| p.presence["at"].as_i64().map(|t| now - t < LIVE_MS).unwrap_or(false)).collect();
    let running: Vec<Value> = peers.iter().flat_map(|p| p.presence["running"].as_array().cloned().unwrap_or_default().into_iter().map(|mut r| {
        r["device"] = json!(p.id);
        r["device_name"] = json!(p.name);
        r
    })).collect();
    ctx.db.with(|c| {
        let since = now - LIVE_MS;
        let mut st = c.prepare("select profile, sum(input + output + cache_read + cache_write), count(*) from usage where ts >= ? and device = ? group by profile")?;
        let mut accounts: serde_json::Map<String, Value> = st.query_map(rusqlite::params![since, ctx.db.device], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))?
            .filter_map(|r| r.ok()).map(|(p, t, n)| (p, json!({"tokens": t, "requests": n}))).collect();
        for p in &peers {
            for (name, a) in p.presence["accounts"].as_object().into_iter().flatten() {
                let e = accounts.entry(name.clone()).or_insert(json!({"tokens": 0, "requests": 0}));
                e["tokens"] = json!(e["tokens"].as_i64().unwrap_or(0) + a["tokens_5m"].as_i64().unwrap_or(0));
                e["requests"] = json!(e["requests"].as_i64().unwrap_or(0) + a["requests_5m"].as_i64().unwrap_or(0));
            }
        }
        let mut st = c.prepare("select provider, sum(input + output), count(*), sum(status <> 200), avg(first_ms) from router_log where ts >= ? group by provider")?;
        let providers: serde_json::Map<String, Value> = st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<f64>>(4)?)))?
            .filter_map(|r| r.ok()).map(|(p, t, n, e, f)| (p, json!({"tokens": t, "requests": n, "errors": e, "first_ms": f.map(|x| x.round() as i64)}))).collect();
        let mut st = c.prepare("select substr(k, 9), v from kv where k like 'breaker:%'")?;
        let breakers: serde_json::Map<String, Value> = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok()).filter_map(|(p, v)| serde_json::from_str(&v).ok().map(|v| (p, v))).collect();
        Ok(json!({"window_ms": LIVE_MS, "accounts": accounts, "providers": providers, "breakers": breakers, "running": running}))
    })
}

/// Experience from use (a point a reply, ten a day used), and the days since a limit last ran out.
fn game(ctx: &Ctx, now: i64) -> Result<Value> {
    ctx.db.with(|c| {
        let (replies, days): (i64, i64) = c.query_row("select count(*), count(distinct day) from usage", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let first: Option<i64> = c.query_row("select min(ts) from quota_hist", [], |r| r.get(0))?;
        let last_hit: Option<i64> = c.query_row("select max(ts) from quota_hist where used >= 99.5", [], |r| r.get(0))?;
        let streak = last_hit.or(first).map(|t| (now - t) / 86_400_000);
        Ok(json!({"xp": replies + 10 * days, "streak_days": streak, "since_hit": last_hit.is_some()}))
    })
}

/// Health of the parts around the tools: tool updates, the last backup, and errors in today's log.
fn fleet(ctx: &Ctx) -> Value {
    let apps = ctx.db.kv_get("apps:latest").ok().flatten().and_then(|v| serde_json::from_str::<Value>(&v).ok());
    let backup = crate::backup::list(ctx).into_iter().next();
    // shortcut: reads the whole log; it stays small (warnings only by default), page it if that changes
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log = std::fs::read_to_string(crate::log::file(&ctx.data)).unwrap_or_default();
    let errors = log.lines().filter(|l| l.starts_with(&today) && l.contains(" ERROR ")).count();
    json!({
        "apps": apps,
        "backup": backup.map(|b| json!({"at": b.at, "size": b.size})),
        "backups": crate::backup::list(ctx).len(),
        "log_errors_today": errors,
        "log_enabled": ctx.settings.log_enabled,
        "sync": ctx.db.kv_get("sync:last").ok().flatten().and_then(|v| serde_json::from_str::<Value>(&v).ok()),
        "self": ctx.db.kv_get("self:latest").ok().flatten().and_then(|v| serde_json::from_str::<Value>(&v).ok()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_pick_the_value_below_which_that_share_falls() {
        let v: Vec<i64> = (1..=100).collect();
        assert_eq!(pct(&v, 0.5), Some(51));
        assert_eq!(pct(&v, 0.95), Some(95));
        assert_eq!(pct(&[], 0.5), None);
    }

    #[test]
    fn a_window_at_a_steady_pace_is_forecast_to_its_reset() {
        let h = 3_600_000;
        let reset = 10 * h;
        // 10% an hour from 20% at hour 0 to 40% at hour 2: 120% by the reset at hour 10, out at hour 8
        let pts = [(0, 20.0, Some(reset)), (h, 30.0, Some(reset + 30_000)), (2 * h, 40.0, Some(reset))];
        let f = forecast(&pts, 2 * h).unwrap();
        assert!((f["at_reset"].as_f64().unwrap() - 120.0).abs() < 0.01);
        assert_eq!(f["runs_out"].as_i64(), Some(8 * h));
        // a reading from the window before does not count
        let pts = [(0, 90.0, Some(h)), (2 * h, 40.0, Some(reset))];
        assert!(forecast(&pts, 2 * h).is_none());
        // no faster than flat: no forecast
        let pts = [(0, 40.0, Some(reset)), (2 * h, 40.0, Some(reset))];
        assert!(forecast(&pts, 2 * h).is_none());
    }

    #[test]
    fn projects_go_by_their_folder_name() {
        assert_eq!(project_name("C:\\Projects\\platform-gitops\\"), "platform-gitops");
        assert_eq!(project_name("/home/a/site"), "site");
        assert_eq!(project_name(""), "(no folder)");
    }
}
