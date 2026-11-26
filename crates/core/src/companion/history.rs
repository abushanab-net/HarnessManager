use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use companion_proto::api::{DayTokens, ModelUsage, PastReset, ProjectUsage, RequestDay, RunOut};
use rusqlite::params;

use crate::{db::local_day, projects::Resolver, service::last_days, util::now_ms, Ctx};

pub(super) fn quota(ctx: &Ctx, profile: &str, login: Option<&str>, now: i64) -> Result<(Vec<PastReset>, Vec<RunOut>)> {
    let since = now - 56 * 86_400_000;
    ctx.db.read(|c| {
        // Include the retained predecessor so an already-empty window does not run out again at the cutoff.
        let mut stmt = c.prepare("select tier, ts, used, resets_at from quota_hist where profile=?1 and ts<=?2 and
            ((account = ?3 collate nocase) or ((device=?4 or device is null) and (coalesce(account,'')='' or ?3 is null)))
            order by tier, ts, rowid")?;
        let mut rows = stmt.query(params![profile, now, login.filter(|s| !s.is_empty()), ctx.db.device])?;
        let mut resets = BTreeMap::new();
        let mut run_outs = Vec::new();
        let mut empty_window = None;
        while let Some(row) = rows.next()? {
            let window: String = row.get(0)?;
            let at: i64 = row.get(1)?;
            let used: f64 = row.get(2)?;
            let reset: Option<i64> = row.get(3)?;
            if let Some(end) = reset.filter(|t| (since..=now).contains(t)) {
                let reading = resets.entry((end, window.clone())).or_insert(PastReset {
                    window: window.clone(), resets_at: end, read_at: None, left: None,
                });
                if at < end {
                    reading.read_at = Some(at);
                    reading.left = (at >= end - 6 * 3_600_000).then(|| (100.0 - used).clamp(0.0, 100.0));
                }
            }
            if reset.is_some_and(|end| end <= at) { continue; }
            if used >= 100.0 {
                let cycle = (window.clone(), reset);
                if empty_window.as_ref() != Some(&cycle) && at >= since {
                    run_outs.push(RunOut { window, at });
                }
                empty_window = Some(cycle);
            } else {
                empty_window = None;
            }
        }
        run_outs.sort_by_key(|e| e.at);
        Ok((resets.into_values().collect(), run_outs))
    })
}

pub(super) fn request_days(ctx: &Ctx) -> Result<Vec<RequestDay>> {
    let days = last_days(60);
    ctx.db.read(|c| {
        // Sequence holes have no dates. A peer with a hole, a backlog or no manifest cannot rule out
        // activity on any empty day. Otherwise use the oldest synchronized peer's calendar day.
        let through: Option<i64> = c.query_row("select min(case
            when gap is not null or applied_seq is null or last_seq is null or applied_seq < last_seq then 0
            else min(coalesce(last_pull,0), coalesce(last_seen,0)) end) from peers",
            [], |r| r.get(0))?;
        let through = through.map(local_day);
        let mut stmt = c.prepare("select day, sum(requests) from usage_day where day between ?1 and ?2 group by day")?;
        let counts = stmt.query_map(params![days[0], days.last()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<BTreeMap<String, i64>>>()?;
        Ok(days.into_iter().map(|day| {
            let requests = counts.get(&day).copied().unwrap_or(0);
            let known = requests > 0 || through.as_ref().is_none_or(|end| &day < end);
            RequestDay { day, requests, known }
        }).collect())
    })
}

/// A desktop project key can contain a private folder or repository URL.
pub(super) fn project_key(key: &str) -> String { crate::util::sha256_hex(key.as_bytes()) }

pub(super) fn project(ctx: &Ctx, key: &str) -> Result<ProjectUsage> {
    let days = last_days(30);
    let reg = ctx.registry()?;
    let mut resolver = Resolver::new(&reg.projects);
    ctx.db.read(|c| {
        let mut keys = c.prepare("select key from project_index union select distinct project_key from sessions
            union select json_extract(json,'$.project_key') from archived")?;
        let mut canonical = None;
        for row in keys.query_map([], |r| r.get::<_, Option<String>>(0))? {
            if let Some(k) = row?.filter(|k| !k.is_empty() && (k == key || project_key(k) == key)) { canonical = Some(k); break; }
        }
        let mut dirs = Vec::new();
        let mut folders = c.prepare("select distinct project from usage_day where device=?1 and day between ?2 and ?3")?;
        for dir in folders.query_map(params![ctx.db.device, days[0], days.last()], |r| r.get::<_, String>(0))? {
            let dir = dir?;
            let resolved = resolver.resolve(&dir).key;
            if canonical.as_ref().map_or_else(|| resolved == key || project_key(&resolved) == key, |k| k == &resolved) {
                canonical = Some(resolved);
                dirs.push(dir);
            }
        }
        let canonical = canonical.ok_or_else(|| anyhow!("unknown project"))?;
        // Local requests follow folder aliases. A peer's folder can mean something else here, so use
        // the project carried by that computer's merged session, as on the desktop Projects page.
        let mut usage = c.prepare("select day, model, sum(input+output+cache_read+cache_write), sum(cost), sum(requests)
            from usage_day where device=?1 and day between ?2 and ?3 and project in (select value from json_each(?4))
            group by day, model union all
            select u.day, u.model, sum(u.input+u.output+u.cache_read+u.cache_write), sum(u.cost_usd), count(*)
            from sessions s join usage u on u.session_id=s.id and u.device=s.device and s.key=u.harness||'/'||u.profile||'/'||u.session_id
            where s.project_key=?5 and u.device<>?1 and u.day between ?2 and ?3 group by u.day, u.model")?;
        let mut rows = usage.query(params![ctx.db.device, days[0], days.last(), serde_json::to_string(&dirs)?, canonical])?;
        let mut daily_tokens: Vec<_> = days.iter().map(|day| DayTokens { day: day.clone(), tokens: 0 }).collect();
        let mut by_model = BTreeMap::<String, ModelUsage>::new();
        let mut total = ModelUsage::default();
        while let Some(row) = rows.next()? {
            let (day, model, tokens, cost, requests): (String, String, i64, Option<f64>, i64) =
                (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
            if let Ok(i) = days.binary_search(&day) { daily_tokens[i].tokens += tokens; }
            for stats in [&mut total, by_model.entry(model).or_default()] {
                stats.tokens += tokens;
                stats.requests += requests;
                if let Some(cost) = cost { *stats.cost_usd.get_or_insert(0.0) += cost; }
            }
        }
        Ok(ProjectUsage { computer: super::computer(ctx), observed_at: now_ms(), key: project_key(&canonical), tokens: total.tokens,
            cost_usd: total.cost_usd, requests: total.requests, by_model, daily_tokens })
    })
}
