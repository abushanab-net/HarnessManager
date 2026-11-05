use serde::{Deserialize, Serialize};

/// What a harness supports; the UI reads this instead of hard-coding per tool.
#[derive(Debug, Clone, Serialize)]
pub struct Caps {
    pub parallel_accounts: bool,
    pub subscription_login: bool,
    pub api_key: bool,
    pub quota: bool,
    pub usage: bool,
    pub sessions: bool,
    pub mcp: bool,
    pub skills: bool,
    pub prompts: bool,
    pub routable: bool,
}

/// Non-secret facts about who a profile is logged in as.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Identity {
    pub logged_in: bool,
    pub email: Option<String>,
    pub name: Option<String>,
    pub org: Option<String>,
    pub role: Option<String>,
    /// Display plan: "Max 20x", "Pro", "Team Premium", "API key", "ChatGPT Pro", ...
    pub plan: Option<String>,
    /// Login needs renewing (expired token the harness could not refresh).
    pub needs_login: bool,
}

/// One limit window, e.g. the 5-hour session or the week.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTier {
    /// "session", "week", or "week:<scope>" for a per-model weekly limit.
    pub id: String,
    pub label: String,
    /// Percent used, 0-100.
    pub used: f64,
    /// Epoch milliseconds; None when no window is open.
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Quota {
    pub tiers: Vec<QuotaTier>,
    /// When the harness last measured these numbers (epoch ms).
    pub fetched_at: Option<i64>,
    pub source: String,
}

impl Quota {
    pub fn tier(&self, id: &str) -> Option<&QuotaTier> { self.tiers.iter().find(|t| t.id == id) }

    /// The limits as they stand at `now`: a window whose reset time has passed is empty again, whatever the
    /// tool measured before it reset (the cached numbers can be hours old).
    pub fn as_of(mut self, now: i64) -> Quota {
        for t in &mut self.tiers {
            if t.resets_at.map(|r| r <= now).unwrap_or(false) { t.used = 0.0; t.resets_at = None; }
        }
        self
    }
}

/// One usage record (one model response), keyed by a stable id so re-imports never double count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRow {
    pub request_id: String,
    pub profile: String,
    pub harness: String,
    pub ts: i64,
    pub model: String,
    pub project: String,
    pub session_id: String,
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    /// From the prompt (or tool result) to the reply's first logged part, when the log shows it.
    pub ttft_ms: Option<i64>,
    /// From the reply's first logged part to its last: output tokens over this is the speed.
    pub gen_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningConsole {
    pub pid: u32,
    pub started_at: Option<i64>,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMeta {
    pub harness: String,
    pub profile: String,
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub started_at: i64,
    pub last_active_at: i64,
    pub prompts: u32,
    pub tool_calls: u32,
    /// Tool calls that failed (a command's error, a non-zero exit); the session list marks such sessions.
    #[serde(default)]
    pub failed: u32,
    pub messages: u32,
    pub model: String,
    pub tokens: i64,
    pub source_path: String,
    /// Filled by the session service: project key and display name.
    #[serde(default)]
    pub project_key: String,
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub open: bool,
    /// The session's own tags, and the tags of its project (which it inherits in filters).
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_tags: Vec<String>,
    /// The computer it ran on, by id and name; `open_elsewhere`: a console on another computer is in it now.
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub open_elsewhere: bool,
    /// A session from another computer: `true`, or why it cannot be resumed here (set by the session service).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumable: Option<serde_json::Value>,
    /// A session continued here from another computer: that computer's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continued_from: Option<String>,
    /// The transcript moved to the archive (`source_path` is the archived copy): readable, not resumable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
}

/// A transcript as typed blocks, grouped into turns (one user prompt and the answer to it).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transcript {
    pub meta: SessionMeta,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Turn {
    pub prompt: String,
    pub prompt_at: i64,
    pub answer_at: i64,
    pub model: String,
    pub steps: Vec<Step>,
    /// The assistant's text replies, in order (Markdown).
    pub answer: Vec<String>,
    /// For each answer, how many steps came before it, so the reading view puts text and steps in order.
    #[serde(default)]
    pub answer_pos: Vec<usize>,
}

impl Turn {
    pub fn say(&mut self, text: String) {
        self.answer_pos.push(self.steps.len());
        self.answer.push(text);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Step {
    /// "thinking", "tool".
    pub kind: String,
    /// Tool name, e.g. Bash, Read, Edit, exec_command, apply_patch.
    pub tool: String,
    /// One-line summary of the input (command, file path).
    pub input: String,
    /// One-line summary of the result.
    pub result: String,
    /// "ok", "error" or "" when unknown.
    pub status: String,
    pub error: String,
    pub duration_ms: Option<i64>,
    /// For edits: file path and unified-ish diff text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Change>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub added: u32,
    pub removed: u32,
    pub diff: String,
}
