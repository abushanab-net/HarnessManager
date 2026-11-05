//! What tokens cost at API list prices, from the public models.dev catalog (cached for a day).
//! Subscription accounts are not billed per token; for them this is what the same work would cost on the API.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use crate::Ctx;

/// US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl Price {
    pub fn cost(&self, input: i64, output: i64, cache_read: i64, cache_write: i64) -> f64 {
        (input as f64 * self.input + output as f64 * self.output + cache_read as f64 * self.cache_read + cache_write as f64 * self.cache_write) / 1e6
    }
}

pub type Prices = HashMap<String, Price>;

const DAY_MS: i64 = 86_400_000;

/// Prices the catalog may lack or lag on, from the vendor's own page. DeepSeek: api-docs.deepseek.com/quick_start/pricing,
/// read 5 Nov 2026, peak rates in USD per 1M tokens (off-peak is half; cache writes are free):
/// deepseek-flash 0.30 in, 0.006 cache hit, 1.20 out; deepseek-v4-pro 1.32 in, 0.044 cache hit, 3.96 out.
fn builtin() -> Prices {
    [("deepseek-flash", Price { input: 0.30, output: 1.20, cache_read: 0.006, cache_write: 0.0 }),
     ("deepseek-v4-pro", Price { input: 1.32, output: 3.96, cache_read: 0.044, cache_write: 0.0 })]
        .into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// Flatten the catalog to model id -> price. Anthropic and OpenAI entries win over resellers' copies, and
/// the built-in vendor prices win over the catalog.
pub fn parse_catalog(v: &Value) -> Prices {
    let mut out = builtin();
    let mut providers: Vec<(&String, &Value)> = v.as_object().map(|o| o.iter().collect()).unwrap_or_default();
    providers.sort_by_key(|(k, _)| match k.as_str() { "anthropic" => 0, "openai" => 1, _ => 2 });
    for (_, p) in providers {
        for (id, m) in p["models"].as_object().into_iter().flatten() {
            let c = &m["cost"];
            let (Some(i), Some(o)) = (c["input"].as_f64(), c["output"].as_f64()) else { continue };
            // shortcut: base prices only; long-context tiers (over 200-272k tokens) are not applied
            let price = Price { input: i, output: o, cache_read: c["cache_read"].as_f64().unwrap_or(i * 0.1), cache_write: c["cache_write"].as_f64().unwrap_or(i * 1.25) };
            out.entry(id.to_lowercase()).or_insert(price);
        }
    }
    out
}

/// Prices from the local cache, fetched again when older than a day. `HRM_PRICING_FILE` points at a
/// catalog file instead (tests, offline computers). Without a catalog every cost is unknown, never guessed.
pub fn load(ctx: &Ctx) -> Prices {
    if let Ok(f) = std::env::var("HRM_PRICING_FILE") {
        return std::fs::read_to_string(f).ok().and_then(|s| serde_json::from_str(&s).ok()).map(|v| parse_catalog(&v)).unwrap_or_default();
    }
    let fresh = ctx.db.kv_get("pricing:at").ok().flatten().and_then(|s| s.parse::<i64>().ok()).map(|t| crate::util::now_ms() - t < DAY_MS).unwrap_or(false);
    // a failed download is tried again after 15 minutes, not on every call (Home asks four times per view)
    let tried = ctx.db.kv_get("pricing:tried").ok().flatten().and_then(|s| s.parse::<i64>().ok()).map(|t| crate::util::now_ms() - t < 15 * 60_000).unwrap_or(false);
    if !fresh && !tried {
        let _ = ctx.db.kv_set("pricing:tried", &crate::util::now_ms().to_string());
        if let Ok(v) = fetch() {
            let _ = ctx.db.kv_set("pricing:catalog", &v.to_string());
            let _ = ctx.db.kv_set("pricing:at", &crate::util::now_ms().to_string());
        }
    }
    ctx.db.kv_get("pricing:catalog").ok().flatten().and_then(|s| serde_json::from_str::<Value>(&s).ok()).map(|v| parse_catalog(&v)).unwrap_or_default()
}

fn fetch() -> Result<Value> {
    let v: Value = reqwest::blocking::Client::new().get("https://models.dev/api.json").header("User-Agent", "Harness Manager")
        .timeout(std::time::Duration::from_secs(10)).send()?.error_for_status()?.json()?;
    // keep only what pricing needs
    let mut slim = serde_json::Map::new();
    for (pid, p) in v.as_object().into_iter().flatten() {
        let models: serde_json::Map<String, Value> = p["models"].as_object().into_iter().flatten()
            .filter(|(_, m)| m["cost"].is_object())
            .map(|(id, m)| (id.clone(), serde_json::json!({"cost": m["cost"]}))).collect();
        if !models.is_empty() { slim.insert(pid.clone(), serde_json::json!({"models": models})); }
    }
    Ok(Value::Object(slim))
}

/// Find a model's price: exact id, then without a provider prefix, a router prefix, a date or a context suffix.
pub fn find<'a>(prices: &'a Prices, model: &str) -> Option<&'a Price> {
    let mut m = model.trim().to_lowercase();
    if let Some((_, rest)) = m.split_once("--") { if m.starts_with("hm-") { m = rest.to_string(); } }
    if let Some(i) = m.find('[') { m.truncate(i); }
    let mut cands = vec![m.clone()];
    if let Some(last) = m.rsplit('/').next() { cands.push(last.to_string()); }
    for c in cands.clone() {
        // claude-opus-4-5-20251101 -> claude-opus-4-5
        let parts: Vec<&str> = c.split('-').collect();
        if parts.last().map(|p| p.len() == 8 && p.chars().all(|ch| ch.is_ascii_digit())).unwrap_or(false) {
            cands.push(parts[..parts.len() - 1].join("-"));
        }
    }
    cands.iter().find_map(|c| prices.get(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_prices_across_name_forms() {
        let cat = serde_json::json!({
            "openrouter": {"models": {"claude-opus-5-5": {"cost": {"input": 9, "output": 9}}}},
            "anthropic": {"models": {"claude-opus-5-5": {"cost": {"input": 4, "output": 20, "cache_read": 0.2, "cache_write": 5}}}},
            "openai": {"models": {"gpt-6.1-sol": {"cost": {"input": 2, "output": 10, "cache_read": 0.1}}}}});
        let p = parse_catalog(&cat);
        assert_eq!(find(&p, "claude-opus-5-5").unwrap().input, 4.0, "the vendor's own price wins");
        assert_eq!(find(&p, "claude-opus-5-5-20261001").unwrap().output, 20.0);
        assert_eq!(find(&p, "anthropic/claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(find(&p, "claude-opus-5-5[1m]").unwrap().input, 4.0);
        assert_eq!(find(&p, "hm-anth--claude-opus-5-5").unwrap().input, 4.0);
        assert!(find(&p, "unknown-model").is_none());
        let c = find(&p, "gpt-6.1-sol").unwrap();
        assert_eq!(c.cache_write, 2.5, "missing cache write priced at 1.25x input");
        assert!((find(&p, "claude-opus-5-5").unwrap().cost(1_000_000, 100_000, 2_000_000, 0) - 6.4).abs() < 1e-9);
    }

    #[test]
    fn deepseek_list_prices_are_built_in_and_beat_the_catalog() {
        let cat = serde_json::json!({"deepseek": {"models": {"deepseek-flash": {"cost": {"input": 9, "output": 9}}}}});
        let p = parse_catalog(&cat);
        assert_eq!(*find(&p, "deepseek-flash").unwrap(), Price { input: 0.30, output: 1.20, cache_read: 0.006, cache_write: 0.0 });
        assert_eq!(*find(&p, "deepseek/deepseek-v4-pro").unwrap(), Price { input: 1.32, output: 3.96, cache_read: 0.044, cache_write: 0.0 });
        // 1M fresh input, 1M cache hits, 100k output on v4-pro at list price
        assert!((find(&p, "deepseek-v4-pro").unwrap().cost(1_000_000, 100_000, 1_000_000, 0) - 1.76).abs() < 1e-9);
        assert!(find(&parse_catalog(&serde_json::json!({})), "deepseek-flash").is_some(), "no catalog at all still prices DeepSeek");
    }
}
