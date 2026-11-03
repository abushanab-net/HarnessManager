use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

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
    pub fn load(path: &Path) -> Result<Registry> {
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

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, serde_json::to_string_pretty(self)?.as_bytes())
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
