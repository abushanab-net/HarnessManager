//! Overview and Usage: totals, the request log, the breakdowns and the CSV export.

use anyhow::Result;
use hrm_core::{service, Ctx};
use serde_json::{json, Value};

use crate::{s, AppState};

// scope "all" (every computer, the default) or "here" (this one)
pub fn home(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let days = a["days"].as_i64();
    let h = a["harness"].as_str().filter(|s| !s.is_empty());
    let scope = a["scope"].as_str();
    let since = a["hours"].as_i64().map(hrm_core::util::since_hours);
    let (total, rows) = service::request_log_since(ctx, days, since, h, scope, a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
    Ok(json!({"overview": service::overview(ctx, days, h, scope)?, "requests": {"total": total, "rows": rows}, "five_hour_first": hrm_core::recommend::five_hour_first(ctx),
              "dash": hrm_core::dash::dashboard(ctx, a["hours"].as_i64().unwrap_or(24), h, scope)?, "activity": service::activity(ctx, h, scope)?}))
}

/// Use next's "5h first", kept in core so the phone and the widgets recommend the same account as the desktop.
pub fn use_next_prefer(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let on = a["on"].as_bool().unwrap_or(false);
    hrm_core::recommend::set_five_hour_first(ctx, on)?;
    Ok(json!({"five_hour_first": on}))
}

pub fn requests(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let since = a["hours"].as_i64().map(hrm_core::util::since_hours);
    let (total, rows) = service::request_log_since(ctx, a["days"].as_i64(), since, a["harness"].as_str().filter(|s| !s.is_empty()), a["scope"].as_str(), a["limit"].as_i64().unwrap_or(50), a["offset"].as_i64().unwrap_or(0))?;
    Ok(json!({"total": total, "rows": rows}))
}

pub fn today(_: &AppState, ctx: &Ctx, _: &Value) -> Result<Value> { Ok(json!(service::overview(ctx, Some(1), None, None)?.today_cost)) }

pub fn usage(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let days = a["days"].as_i64().unwrap_or(7);
    Ok(json!({"summary": service::usage_summary(ctx, days, a["profile"].as_str())?, "route_log": ctx.db.route_log(30)?,
              "accounts": service::breakdown(ctx, "profile", Some(days), None)?, "models": service::breakdown_of(ctx, "model", Some(days), None, a["profile"].as_str())?}))
}

// Usage > Export CSV: one line per account, to the file the user picked
pub fn usage_csv(_: &AppState, ctx: &Ctx, a: &Value) -> Result<Value> {
    let days = a["days"].as_i64().unwrap_or(7);
    let mut out = String::from("account,tool,tokens,input,output,cache_read,requests,list_price_usd\n");
    for b in service::breakdown(ctx, "profile", Some(days), None)? {
        out.push_str(&format!("{},{},{},{},{},{},{},{}\n", b.key, b.harness, b.tokens, b.input, b.output, b.cache_read, b.requests, b.cost.map(|c| format!("{c:.4}")).unwrap_or_default()));
    }
    // whole or not at all: a reader (or a test) never sees it half written
    hrm_core::util::write_atomic(std::path::Path::new(&s(a, "path")?), out.as_bytes())?;
    Ok(json!(true))
}
