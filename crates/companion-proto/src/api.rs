//! The companion API's types (`/c1`), as the computer answers and the phone reads them. Every answer carries
//! `computer` and `observed_at`; the routes and what each needs granted are in docs/plans/mobile/PLAN.md.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The API version `GET /c1/hello` answers.
pub const API: u32 = 1;

/// What a phone may see and do on one computer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Grants {
    #[serde(default)]
    pub summaries: bool,
    #[serde(default)]
    pub transcripts: bool,
    #[serde(default)]
    pub control: bool,
}

impl Default for Grants {
    /// Summaries and transcripts on, Control granted on the computer.
    fn default() -> Grants { Grants { summaries: true, transcripts: true, control: false } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Computer {
    pub id: String,
    pub name: String,
}

/// What this phone may do now, and why not when it may not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Allowed {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `GET /c1/hello`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hello {
    pub computer: Computer,
    pub observed_at: i64,
    pub app_version: String,
    pub api: u32,
    pub grants: Grants,
    pub server_time: i64,
}

/// The account to use next for one tool, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UseNext {
    pub tool: String,
    pub profile: String,
    pub name: String,
    pub reason: String,
}

/// `GET /c1/overview`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Overview {
    pub computer: Computer,
    pub observed_at: i64,
    /// Open requests, and when the first of them expires.
    pub needs_you: u32,
    pub soonest_expiry: Option<i64>,
    pub running: u32,
    /// The soonest reset of an open window, with its account.
    pub next_reset: Option<Value>,
    /// Today's tokens and requests here.
    pub today: Value,
    pub use_next: Vec<UseNext>,
}

/// One limit window of an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub id: String,
    pub label: String,
    pub left: i64,
    pub resets_at: Option<i64>,
}

/// One account, as `GET /c1/accounts` lists them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub profile: String,
    pub name: String,
    pub tool: String,
    pub plan: Option<String>,
    pub windows: Vec<Window>,
    /// When the limits were read (ms), and how long ago.
    pub read_at: Option<i64>,
    pub reading_age_ms: Option<i64>,
    /// Signed out, or a login to renew.
    pub login_attention: bool,
    pub tokens_7d: i64,
}

/// One session, as `GET /c1/sessions` lists them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub harness: String,
    pub profile: String,
    pub id: String,
    pub title: String,
    pub project: String,
    pub computer: Computer,
    /// "working", "waiting", "done", or "" when no hook reported it.
    pub state: String,
    pub open: bool,
    pub last_active_at: i64,
}

/// An open request: a permission or a question a session's hook holds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub run: String,
    pub ask_id: String,
    /// "permission" or "question".
    pub kind: String,
    pub tool: String,
    /// The command or file, or the questions in one line.
    pub what: String,
    /// The tool's input: a question's `questions` with their options.
    pub input: Value,
    pub profile: String,
    pub expires: i64,
    /// What the phone sends back in an action, so a stale screen cannot act.
    pub state_version: i64,
}

/// A WebSocket message on `GET /c1/events`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// "needs", "changed", "run-state" or "revoked".
    pub kind: String,
    pub at: i64,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Answer,
    Reply,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_id: Option<String>,
}

/// `POST /c1/actions`. The payload: `{"behavior": "allow"|"deny"}` or `{"answers": {...}}` for an answer,
/// `{"text": ...}` for a reply, `{}` for Stop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub action_id: String,
    pub kind: ActionKind,
    pub target: Target,
    pub state_version: i64,
    pub expires_at: i64,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Accepted,
    AlreadyHandled,
    Expired,
    Unavailable,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self { Status::Accepted => "accepted", Status::AlreadyHandled => "already_handled", Status::Expired => "expired", Status::Unavailable => "unavailable" }
    }

    pub fn parse(s: &str) -> Option<Status> {
        [Status::Accepted, Status::AlreadyHandled, Status::Expired, Status::Unavailable].into_iter().find(|x| x.as_str() == s)
    }
}

/// What the computer did with an action: kept 24 h, the same for a repeated `action_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Receipt {
    pub action_id: String,
    pub status: Status,
    pub detail: String,
    pub at: i64,
}

/// `PUT /c1/push`: the relay handle the phone registered (wave 2 gives it meaning).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushHandle {
    pub relay_handle: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_action_and_a_receipt_read_as_the_contract_writes_them() {
        let a: Action = serde_json::from_value(json!({"action_id": "u1", "kind": "answer", "target": {"run": "1:2", "ask_id": "a"}, "state_version": 3, "expires_at": 9, "payload": {"behavior": "allow"}})).unwrap();
        assert_eq!((a.kind, a.target.ask_id.as_deref()), (ActionKind::Answer, Some("a")));
        let r = Receipt { action_id: "u1".into(), status: Status::AlreadyHandled, detail: String::new(), at: 1 };
        assert_eq!(serde_json::to_value(&r).unwrap()["status"], "already_handled");
        for s in ["accepted", "already_handled", "expired", "unavailable"] { assert_eq!(Status::parse(s).unwrap().as_str(), s); }
        assert_eq!(Grants::default(), Grants { summaries: true, transcripts: true, control: false });
    }
}
