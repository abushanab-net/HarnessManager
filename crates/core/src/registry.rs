use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::secrets;
use crate::util::{now_ms, valid_name, write_atomic};

/// How a profile pays for its harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Vendor login (Claude, ChatGPT, Google). Never routed, aggregated or failed over.
    Subscription,
    /// An API key kept in the OS credential store, sent straight to the vendor.
    ApiKey,
    /// Sent through the local router to a route of providers.
    Routed,
}

/// One account of one harness. Its home folder is derived, so the registry can sync between computers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// The id: its folder, saved key and command line use it, so it never changes.
    pub name: String,
    /// The name shown in the app, editable; the id when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub harness: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// The harness's own default home (`~/.claude`, `~/.codex`, ...) instead of a profile folder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main: bool,
    /// Base URL for an API-key profile that talks to a compatible endpoint instead of the vendor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Model to start with, for API-key and routed profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Route id for routed profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Routed profiles: the most the router spends for it in a day (at API list prices), in US dollars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_cap_usd: Option<f64>,
    #[serde(default)]
    pub created_at: i64,
}

/// An upstream endpoint for routing: base URL, wire format and the models it serves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    /// "anthropic" (Messages API) or "openai" (Chat Completions).
    pub format: String,
    #[serde(default)]
    pub models: Vec<String>,
}

/// Endpoints whose shape is known, offered ready to add (the key is still the user's). DeepSeek, from
/// api-docs.deepseek.com (Nov 2026): Chat Completions at the root, the Anthropic Messages API under
/// `/anthropic` (for Claude Code routes), the same two models and `DEEPSEEK_API_KEY` on both.
pub fn provider_presets() -> Vec<Provider> {
    let models = || vec!["deepseek-flash".to_string(), "deepseek-v4-pro".to_string()];
    vec![
        Provider { id: "deepseek".into(), name: "DeepSeek".into(), base_url: "https://api.deepseek.com".into(), format: "openai".into(), models: models() },
        Provider { id: "deepseek-anthropic".into(), name: "DeepSeek (Anthropic format)".into(), base_url: "https://api.deepseek.com/anthropic".into(), format: "anthropic".into(), models: models() },
    ]
}

#[cfg(test)]
mod preset_tests {
    #[test]
    fn deepseek_presets_cover_both_wire_formats_with_the_same_models() {
        let p = super::provider_presets();
        let ds = p.iter().find(|x| x.id == "deepseek").unwrap();
        let an = p.iter().find(|x| x.id == "deepseek-anthropic").unwrap();
        assert_eq!((ds.format.as_str(), ds.base_url.as_str()), ("openai", "https://api.deepseek.com"));
        assert_eq!((an.format.as_str(), an.base_url.as_str()), ("anthropic", "https://api.deepseek.com/anthropic"));
        assert_eq!(ds.models, an.models);
        assert_eq!(ds.models, vec!["deepseek-flash", "deepseek-v4-pro"]);
        for m in &ds.models { assert!(crate::pricing::find(&crate::pricing::parse_catalog(&serde_json::json!({})), m).is_some(), "{m} has a price"); }
    }
}

/// A route: providers tried in order (failover), or merged into one model list (aggregate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    /// "failover" or "aggregate".
    pub mode: String,
    pub providers: Vec<String>,
    /// Aggregate mode: requests without a provider prefix go here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    /// "stdio", "http" or "sse".
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub headers: std::collections::BTreeMap<String, String>,
    /// For people only; never written into a tool's config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

impl McpServer {
    /// Each env and header value with the credential it is kept in.
    fn values(&mut self) -> impl Iterator<Item = (String, &mut String)> {
        let McpServer { name, env, headers, .. } = self;
        let name: &str = name;
        env.iter_mut().map(move |(k, v)| (secrets::mcp_target(name, "env", k), v))
            .chain(headers.iter_mut().map(move |(k, v)| (secrets::mcp_target(name, "header", k), v)))
    }

    /// An empty value is one not set on this computer (the list may be shared with another): it takes the one `from` has.
    pub fn fill_unset(&mut self, from: &McpServer) {
        for (map, src) in [(&mut self.env, &from.env), (&mut self.headers, &from.headers)] {
            for (k, v) in map.iter_mut().filter(|(_, v)| v.is_empty()) { if let Some(x) = src.get(k) { v.clone_from(x); } }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    /// Where it came from: a GitHub URL, a local folder, or "imported".
    pub source: String,
    #[serde(default)]
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillRepo {
    pub owner: String,
    pub name: String,
    /// A branch, or "HEAD" for the repository's default branch.
    #[serde(default = "head")]
    pub branch: String,
}
fn head() -> String { "HEAD".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: String,
    pub harness: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub enabled: bool,
}

/// Which harnesses (or single profiles) an MCP server or skill is enabled for. Rows, not columns,
/// so adding a harness needs no migration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    /// "mcp" or "skill".
    pub kind: String,
    pub item: String,
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// A manual name or merge for a project, keyed by the project key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAlias {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Merge this project into another key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_into: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub routes: Vec<Route>,
    #[serde(default)]
    pub mcp: Vec<McpServer>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub prompts: Vec<Prompt>,
    /// GitHub repositories the Discover tab lists skills from; empty means the defaults.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skill_repos: Vec<SkillRepo>,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub projects: Vec<ProjectAlias>,
}

impl Registry {
    /// The list with each MCP env and header value: an empty one in the file is filled from the credential store. One
    /// still in the file (an older version wrote it, or the store refused it) is used as it is.
    pub fn load(path: &Path) -> Result<Registry> {
        let mut reg = Self::read(path)?;
        for (t, v) in reg.mcp.iter_mut().flat_map(McpServer::values).filter(|(_, v)| v.is_empty()) {
            if let Some(x) = secrets::get_key(&t) { *v = x; }
        }
        Ok(reg)
    }

    /// The file as it is.
    fn read(path: &Path) -> Result<Registry> {
        // a file that exists but cannot be read is an error: treating it as empty would save an empty list over it
        let s = crate::util::read_or_empty(path)?;
        if s.trim().is_empty() { return Ok(Registry { version: 1, ..Default::default() }); }
        let reg: Registry = serde_json::from_str(&s).with_context(|| format!("{} is not valid JSON; fix or restore it", path.display()))?;
        if let Some(p) = reg.profiles.iter().find(|p| !valid_name(&p.name)) {
            bail!("{}: '{}' is not a valid account name (it becomes a folder name); fix the file", path.display(), p.name);
        }
        if let Some(p) = reg.profiles.iter().find(|p| crate::harness::by_id(&p.harness).is_none()) {
            bail!("{}: account '{}' uses tool '{}', which this version does not know; fix the file or update Harness Manager", path.display(), p.name, p.harness);
        }
        Ok(reg)
    }

    /// Each MCP env and header value goes to the credential store, and the file keeps its key with an empty value.
    pub fn save(&self, path: &Path) -> Result<()> { self.write(path, true) }

    /// `save` for a list put back from a backup: no credential is deleted, so putting the newer list back finds its values.
    pub fn save_restored(&self, path: &Path) -> Result<()> { self.write(path, false) }

    fn write(&self, path: &Path, forget: bool) -> Result<()> {
        static WARNED: std::sync::Once = std::sync::Once::new();
        let mut file = self.clone();
        let mut live = std::collections::BTreeSet::new();
        for (t, v) in file.mcp.iter_mut().flat_map(McpServer::values) {
            // of two values whose names give one credential (Windows ignores letter case there), the first takes it
            if !live.insert(t.to_lowercase()) || v.is_empty() || secrets::get_key(&t).as_deref() == Some(v.as_str()) { continue; }
            // replaced, not written over: under a name spelled with other letter case Windows keeps a second entry, which the old one hides
            let _ = secrets::delete(&t);
            // too large for a credential, or no store on this platform: the value is kept, in the file
            if let Err(e) = secrets::write(&t, "hrm", v.as_bytes()) {
                WARNED.call_once(|| crate::log::warn(&format!("an MCP server's value stays in {}: {e:#}", path.display())));
            }
        }
        // only a value that reads back leaves the file, so none is lost: not one the store refused, not the second of two that share a credential
        for (t, v) in file.mcp.iter_mut().flat_map(McpServer::values) {
            if !v.is_empty() && secrets::get_key(&t).as_deref() == Some(v.as_str()) { v.clear(); }
        }
        let text = serde_json::to_string_pretty(&file)?;
        let old = crate::util::read_or_empty(path).unwrap_or_default();
        if old != text { write_atomic(path, text.as_bytes())?; }
        // a server or key this save removed: its credentials go (a renamed server's are under its new name by now)
        // shortcut: another computer that shares the list keeps its own credentials of a server removed here; list and delete them there if that matters
        if let (true, Ok(mut was)) = (forget, serde_json::from_str::<Registry>(&old)) {
            for (t, _) in was.mcp.iter_mut().flat_map(McpServer::values).filter(|(t, _)| !live.contains(&t.to_lowercase())) { let _ = secrets::delete(&t); }
        }
        Ok(())
    }

    /// Values an older version left in the file move to the credential store: a load and a save, only when the file holds one.
    // shortcut: only Windows has a store so far (see secrets), so elsewhere a start moves nothing and warns of nothing; drop the test with the Keychain build
    pub fn move_mcp_values(path: &Path) -> Result<()> {
        if cfg!(windows) && Self::read(path)?.mcp.iter().any(|s| s.env.values().chain(s.headers.values()).any(|v| !v.is_empty())) { Self::load(path)?.save(path)?; }
        Ok(())
    }

    /// The list as a backup carries it: no env or header value, on any platform.
    pub fn without_mcp_values(&self) -> Registry {
        let mut r = self.clone();
        for (_, v) in r.mcp.iter_mut().flat_map(McpServer::values) { v.clear(); }
        r
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> { self.profiles.iter().find(|p| p.name == name) }

    pub fn get(&self, name: &str) -> Result<&Profile> {
        match self.profile(name) {
            Some(p) => Ok(p),
            None => bail!("no profile named '{name}'. Run `hrm list` to see them."),
        }
    }

    pub fn add_profile(&mut self, mut p: Profile) -> Result<()> {
        if !valid_name(&p.name) {
            bail!("'{}' is not a valid name: use letters, digits, - and _ (it becomes a folder name)", p.name);
        }
        if self.profile(&p.name).is_some() {
            bail!("a profile named '{}' already exists", p.name);
        }
        if crate::harness::by_id(&p.harness).is_none() {
            bail!("unknown harness '{}'", p.harness);
        }
        if p.main && self.profiles.iter().any(|q| q.main && q.harness == p.harness) {
            bail!("{} already has a main profile", p.harness);
        }
        if p.created_at == 0 {
            p.created_at = now_ms();
        }
        self.profiles.push(p);
        Ok(())
    }

    /// Remove a router provider; refused while a route still sends requests to it.
    pub fn remove_provider(&mut self, id: &str) -> Result<()> {
        if let Some(r) = self.routes.iter().find(|r| r.providers.iter().any(|p| p == id) || r.default_provider.as_deref() == Some(id)) { bail!("route {} still uses {id}; remove it from the route first", r.id); }
        self.providers.retain(|p| p.id != id);
        Ok(())
    }

    /// Remove a route; refused while an account is routed through it.
    pub fn remove_route(&mut self, id: &str) -> Result<()> {
        if let Some(p) = self.profiles.iter().find(|p| p.route.as_deref() == Some(id)) { bail!("account {} still uses route {id}", p.name); }
        self.routes.retain(|r| r.id != id);
        Ok(())
    }

    pub fn remove_profile(&mut self, name: &str) -> Result<Profile> {
        let i = self.profiles.iter().position(|p| p.name == name).ok_or_else(|| anyhow::anyhow!("no profile named '{name}'"))?;
        self.targets.retain(|t| t.profile.as_deref() != Some(name));
        Ok(self.profiles.remove(i))
    }
}
