//! "Use next": the account to start next for each tool, by the rule the desktop's Overview draws with
//! (`recommendedFor` and `metricsOf` in app/ui, said again here so the desktop, the phone and the widgets agree):
//! among subscriptions that are signed in, read, and have no open window used up, the most room in the week (then
//! in the 5-hour window); with "5h first", the 5-hour window at half or more that resets soonest; else an account
//! with no plan limits to run out.

use serde_json::{Map, Value};

use crate::registry::Kind;
use crate::service::ProfileView;

/// The tools whose plans have windows to read; the others show usage only.
const PLAN_TOOLS: [&str; 3] = ["claude", "codex", "agy"];

/// One limit window as the rule reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Win {
    pub id: String,
    pub label: String,
    /// "5h", "week" or "model".
    pub kind: &'static str,
    pub left: i64,
    pub open: bool,
    pub resets_at: Option<i64>,
    pub reset_ms: i64,
    /// When this pace runs it out before its reset, ms from now.
    pub out_ms: Option<i64>,
}

/// An account as the rule reads it (`acctView`).
pub struct Acct<'a> {
    pub p: &'a ProfileView,
    pub windows: Vec<Win>,
    pub needs_login: bool,
    pub no_plan: bool,
}

/// `forecast` is `dash::forecasts`: keyed "account|limit".
pub fn acct<'a>(p: &'a ProfileView, forecast: &Map<String, Value>, now: i64) -> Acct<'a> {
    let windows = p.quota.iter().flat_map(|q| &q.tiers).map(|t| {
        let used = t.used.round().min(100.0) as i64;
        let open = t.resets_at.is_some_and(|r| r > now);
        let f = forecast.get(&format!("{}|{}", p.name, t.id));
        let out = f.and_then(|f| f["runs_out"].as_i64()).filter(|o| open && t.resets_at.is_some_and(|r| *o < r));
        let kind = match t.id.as_str() { "session" => "5h", "week" => "week", _ => "model" };
        Win { id: t.id.clone(), label: t.label.clone(), kind, left: (100 - used).max(0), open, resets_at: t.resets_at, reset_ms: if open { t.resets_at.unwrap_or(0) - now } else { 0 }, out_ms: out.map(|o| o - now) }
    }).collect();
    let sub = p.kind == Kind::Subscription;
    Acct { p, windows, needs_login: sub && (p.identity.needs_login || !p.identity.logged_in), no_plan: !PLAN_TOOLS.contains(&p.harness.as_str()) || !sub }
}

impl Acct<'_> {
    fn measured(&self) -> bool { !self.windows.is_empty() }
    pub fn w5(&self) -> Option<&Win> { self.windows.iter().find(|w| w.kind == "5h") }
    pub fn wk(&self) -> Option<&Win> { self.windows.iter().find(|w| w.kind == "week").or(self.windows.first()) }
    /// A subscription, signed in, with measured windows none of which is used up.
    fn eligible(&self) -> bool { self.p.kind == Kind::Subscription && !self.needs_login && self.measured() && self.windows.iter().all(|w| !w.open || w.left > 0) }
}

/// The account Use next names for `tool` among `accts`, with the reason; None when the tool has none.
pub fn recommended_for<'a, 'b>(accts: &'b [Acct<'a>], tool: &str, prefer_expiring: bool) -> Option<(&'b Acct<'a>, String)> {
    let pool: Vec<&Acct> = accts.iter().filter(|a| a.p.harness == tool).collect();
    let left = |w: Option<&Win>| w.map(|w| w.left).unwrap_or(0);
    // a stable sort, as the desktop's: equal accounts keep their order
    let mut by_room: Vec<&Acct> = pool.iter().copied().filter(|a| a.eligible()).collect();
    by_room.sort_by(|a, b| left(b.wk()).cmp(&left(a.wk())).then(left(b.w5()).cmp(&left(a.w5()))));
    let mut expiring: Vec<&Acct> = by_room.iter().copied().filter(|a| a.w5().is_some_and(|w| w.open && w.left >= 50)).collect();
    expiring.sort_by_key(|a| a.w5().map(|w| w.reset_ms).unwrap_or(0));
    let room = |a: &Acct| a.wk().map(|w| format!("Most room: {}% of its {} left", w.left, w.label.to_lowercase())).unwrap_or_default();
    if prefer_expiring {
        if let Some(a) = expiring.first() { return Some((a, a.w5().map(|w| format!("Its 5-hour window resets soonest, with {}% left", w.left)).unwrap_or_default())); }
    }
    if let Some(a) = by_room.first() { return Some((a, room(a))); }
    pool.into_iter().find(|a| a.no_plan && !a.needs_login).map(|a| (a, "No plan limits to run out".to_string()))
}

/// The tools whose accounts this computer lists: shown in Settings and installed (`listedTools`), in their order.
pub fn listed(ctx: &crate::Ctx) -> Vec<String> {
    crate::service::harnesses().into_iter().filter(|h| h.installed && !ctx.settings.hidden_tools.contains(&h.id)).map(|h| h.id).collect()
}

/// Where the owner's "5h first" choice is kept, so the desktop and the phone rank with the same one.
const FIVE_HOUR_FIRST: &str = "use_next:five_hour_first";

/// The owner prefers the 5-hour window that resets soonest ("5h first").
pub fn five_hour_first(ctx: &crate::Ctx) -> bool { ctx.db.kv_get(FIVE_HOUR_FIRST).ok().flatten().as_deref() == Some("1") }

/// Keep the "5h first" choice; the desktop's toggle calls this.
pub fn set_five_hour_first(ctx: &crate::Ctx, on: bool) -> anyhow::Result<()> { ctx.db.kv_set(FIVE_HOUR_FIRST, if on { "1" } else { "0" }) }

/// Use next for every tool this computer lists, in the tools' order, by the owner's "5h first" choice.
pub fn use_next(ctx: &crate::Ctx, views: &[ProfileView], now: i64) -> anyhow::Result<Vec<companion_proto::api::UseNext>> {
    let forecast = crate::dash::forecasts(ctx, now)?;
    let listed = listed(ctx);
    let accts: Vec<Acct> = views.iter().filter(|p| listed.contains(&p.harness)).map(|p| acct(p, &forecast, now)).collect();
    let five = five_hour_first(ctx);
    Ok(listed.iter().filter_map(|t| next_for(&accts, t, five, now)).collect())
}

/// Use next for `tool`, with the lines the desktop's Overview shows: why, and the runner-up.
fn next_for(accts: &[Acct], t: &str, five: bool, now: i64) -> Option<companion_proto::api::UseNext> {
    recommended_for(accts, t, five).map(|(a, reason)| {
        // the line under the name, as the desktop's Overview draws it (its "left" form)
        let soon = five && a.w5().is_some_and(|w| w.open && w.left >= 50);
        let w = if soon { a.w5() } else { a.wk() };
        let out_at = w.and_then(|w| w.out_ms).map(|m| now + m);
        let why = match w {
            Some(w) if soon => format!("{} · resets in {}", share(w), duration(w.reset_ms)),
            Some(w) => format!("{} · {}", share(w), out_at.map(|o| format!("≈ out {}", day_of(o, now))).unwrap_or_else(|| "lasts until its reset".into())),
            None if a.p.kind == Kind::Subscription => "No plan limits to run out".into(),
            None => "Pay per use · no plan limits".into(),
        };
        // the runner-up: the next account by room, as the desktop's line under the card
        let runner_up = a.measured().then(|| {
            let mut rest: Vec<&Acct> = accts.iter().filter(|b| b.p.harness == t && b.p.name != a.p.name && b.eligible()).collect();
            let left = |w: Option<&Win>| w.map(|w| w.left).unwrap_or(0);
            rest.sort_by(|x, y| left(y.wk()).cmp(&left(x.wk())).then(left(y.w5()).cmp(&left(x.w5()))));
            rest.first().and_then(|b| b.wk().map(|w| {
                let out_at = w.out_ms.map(|m| now + m);
                let when = out_at.map(|o| format!("≈ out {}", day_of(o, now).split(' ').next().unwrap_or(""))).unwrap_or_else(|| "lasts".into());
                companion_proto::api::RunnerUp { profile: b.p.name.clone(), name: b.p.label.clone(), left: w.left, out_at, line: format!("{}% left · {when}", w.left) }
            }))
        }).flatten();
        companion_proto::api::UseNext { tool: t.to_string(), profile: a.p.name.clone(), name: a.p.label.clone(), reason, why, window: w.map(|w| w.label.clone()), left: w.map(|w| w.left),
            resets_at: w.and_then(|w| w.resets_at), out_at, runner_up }
    })
}

/// "64% of the week left", as the desktop says a window's room.
fn share(w: &Win) -> String {
    let of = match w.kind { "week" => "the week".to_string(), "5h" => "the 5-hour window".to_string(), _ => w.label.clone() };
    format!("{}% of {of} left", w.left)
}

/// The desktop's `fmtDuration`: "40 m", "2 h 05 m", "3 d 4 h".
fn duration(ms: i64) -> String {
    let m = ((ms as f64) / 60_000.0).round().max(1.0) as i64;
    if m < 60 { return format!("{m} m"); }
    let (h, rm) = (m / 60, m % 60);
    if h < 24 { return if rm > 0 { format!("{h} h {rm:02} m") } else { format!("{h} h") }; }
    let (d, rh) = (h / 24, h % 24);
    if rh > 0 { format!("{d} d {rh} h") } else { format!("{d} d") }
}

/// The desktop's `dayOf`, in this computer's time: "today 15:00" within the next 18 hours, else "Sat 15:00".
fn day_of(at: i64, now: i64) -> String {
    use chrono::TimeZone;
    let Some(t) = chrono::Local.timestamp_millis_opt(at).single() else { return String::new() };
    let hhmm = t.format("%H:%M");
    if at >= now && at - now < 18 * 3_600_000 { format!("today {hhmm}") } else { format!("{} {hhmm}", t.format("%a")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Identity, Quota, QuotaTier};
    use serde_json::json;

    const NOW: i64 = 1_000_000_000_000;
    const H: i64 = 3_600_000;

    fn view(name: &str, tool: &str, kind: Kind, tiers: &[(&str, f64, Option<i64>)]) -> ProfileView {
        let tiers = tiers.iter().map(|(id, used, r)| QuotaTier { id: (*id).into(), label: if *id == "session" { "5-hour".into() } else { "Week".into() }, used: *used, resets_at: *r }).collect();
        ProfileView { name: name.into(), label: name.to_uppercase(), harness: tool.into(), harness_label: tool.into(), kind, main: false, expected_email: None,
            identity: Identity { logged_in: true, ..Default::default() }, email_mismatch: false, quota: Some(Quota { tiers, ..Default::default() }).filter(|q| !q.tiers.is_empty()),
            running: vec![], tokens_7d: 0, days_7d: vec![], sessions_7d: 0, last_used: None, home: String::new(), caps: crate::harness::all()[0].caps(), base_url: None,
            model: None, route: None, daily_cap_usd: None, intro: None, remote_control: false, created_at: 0 }
    }

    /// Use next and its lines, as the phone is given them: the desktop's words, the runner-up.
    #[test]
    fn use_next_says_what_the_desktop_overview_says() {
        let f: Map<String, Value> = [("b|week".to_string(), json!({"runs_out": NOW + 30 * H}))].into_iter().collect();
        let v = [
            view("a", "claude", Kind::Subscription, &[("session", 20.0, Some(NOW + 2 * H + 5 * 60_000)), ("week", 36.0, Some(NOW + 50 * H))]),
            view("b", "claude", Kind::Subscription, &[("session", 10.0, Some(NOW + 4 * H)), ("week", 40.0, Some(NOW + 50 * H))]),
        ];
        let accts: Vec<Acct> = v.iter().map(|p| acct(p, &f, NOW)).collect();
        let n = next_for(&accts, "claude", false, NOW).unwrap();
        assert_eq!((n.profile.as_str(), n.why.as_str(), n.left, n.out_at), ("a", "64% of the week left · lasts until its reset", Some(64), None));
        let r = n.runner_up.unwrap();
        assert_eq!((r.profile.as_str(), r.left, r.out_at), ("b", 60, Some(NOW + 30 * H)));
        assert_eq!(r.line, format!("60% left · ≈ out {}", day_of(NOW + 30 * H, NOW).split(' ').next().unwrap()));
        // "5h first": the 5-hour window that resets soonest, the soonest loss
        let n = next_for(&accts, "claude", true, NOW).unwrap();
        assert_eq!((n.profile.as_str(), n.why.as_str(), n.window.as_deref()), ("a", "80% of the 5-hour window left · resets in 2 h 05 m", Some("5-hour")));
        // with no plan limits, as the desktop says it
        let api = [view("k", "claude", Kind::ApiKey, &[])];
        let accts: Vec<Acct> = api.iter().map(|p| acct(p, &f, NOW)).collect();
        let n = next_for(&accts, "claude", false, NOW).unwrap();
        assert_eq!((n.why.as_str(), n.runner_up), ("Pay per use · no plan limits", None));
        assert_eq!((duration(40 * 60_000), duration(26 * H)), ("40 m".to_string(), "1 d 2 h".to_string()));
    }

    fn pick(views: &[ProfileView], tool: &str, five: bool) -> Option<(String, String)> {
        let f = Map::new();
        let accts: Vec<Acct> = views.iter().map(|p| acct(p, &f, NOW)).collect();
        recommended_for(&accts, tool, five).map(|(a, r)| (a.p.name.clone(), r))
    }

    #[test]
    fn the_most_room_in_the_week_wins_then_the_five_hour_window() {
        let r = Some(NOW + 5 * H);
        let v = vec![
            view("a", "claude", Kind::Subscription, &[("session", 10.0, r), ("week", 40.0, Some(NOW + 50 * H))]),
            view("b", "claude", Kind::Subscription, &[("session", 60.0, r), ("week", 20.0, Some(NOW + 50 * H))]),
            view("c", "claude", Kind::Subscription, &[("session", 0.0, r), ("week", 20.0, Some(NOW + 50 * H))]),
        ];
        let (name, reason) = pick(&v, "claude", false).unwrap();
        assert_eq!(name, "c", "the same week, more 5-hour room");
        assert_eq!(reason, "Most room: 80% of its week left");
        assert_eq!(pick(&v, "codex", false), None);
    }

    #[test]
    fn a_used_up_window_or_a_login_to_renew_rules_an_account_out() {
        let v = vec![
            view("full", "claude", Kind::Subscription, &[("session", 100.0, Some(NOW + H)), ("week", 0.0, Some(NOW + 50 * H))]),
            view("ok", "claude", Kind::Subscription, &[("session", 50.0, Some(NOW + H)), ("week", 70.0, Some(NOW + 50 * H))]),
        ];
        assert_eq!(pick(&v, "claude", false).unwrap().0, "ok");
        // a used-up window whose reset has passed is not open, so it does not count
        let v = vec![view("past", "claude", Kind::Subscription, &[("session", 100.0, Some(NOW - 1)), ("week", 10.0, Some(NOW + 50 * H))])];
        assert_eq!(pick(&v, "claude", false).unwrap().0, "past");
        let mut out = view("out", "claude", Kind::Subscription, &[("week", 0.0, Some(NOW + 50 * H))]);
        out.identity.needs_login = true;
        assert_eq!(pick(&[out], "claude", false), None);
    }

    #[test]
    fn five_hour_first_takes_the_soonest_reset_at_half_or_more() {
        let v = vec![
            view("late", "claude", Kind::Subscription, &[("session", 0.0, Some(NOW + 4 * H)), ("week", 0.0, Some(NOW + 50 * H))]),
            view("soon", "claude", Kind::Subscription, &[("session", 40.0, Some(NOW + H)), ("week", 50.0, Some(NOW + 50 * H))]),
            view("low", "claude", Kind::Subscription, &[("session", 70.0, Some(NOW + H / 2)), ("week", 50.0, Some(NOW + 50 * H))]),
        ];
        assert_eq!(pick(&v, "claude", false).unwrap().0, "late");
        let (name, reason) = pick(&v, "claude", true).unwrap();
        assert_eq!((name.as_str(), reason.as_str()), ("soon", "Its 5-hour window resets soonest, with 60% left"));
    }

    #[test]
    fn without_a_measured_subscription_an_account_with_no_plan_limits() {
        let v = vec![
            view("unread", "claude", Kind::Subscription, &[]),
            view("key", "claude", Kind::ApiKey, &[]),
        ];
        assert_eq!(pick(&v, "claude", false), Some(("key".into(), "No plan limits to run out".into())));
        assert_eq!(pick(&[view("pi", "pi", Kind::Subscription, &[])], "pi", false).unwrap().0, "pi", "a tool without plan windows");
    }

    #[test]
    fn a_forecast_run_out_before_the_reset_is_kept() {
        let p = view("a", "claude", Kind::Subscription, &[("week", 50.0, Some(NOW + 10 * H))]);
        let mut f = Map::new();
        f.insert("a|week".into(), json!({"runs_out": NOW + 2 * H}));
        assert_eq!(acct(&p, &f, NOW).windows[0].out_ms, Some(2 * H));
        f.insert("a|week".into(), json!({"runs_out": NOW + 20 * H}));
        assert_eq!(acct(&p, &f, NOW).windows[0].out_ms, None);
    }
}
