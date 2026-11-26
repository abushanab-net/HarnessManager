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
    /// Open requests a phone may answer, and when the first of them stops being a phone's to answer.
    pub needs_you: u32,
    pub soonest_expiry: Option<i64>,
    /// Open requests past a phone's window: they are answered at the computer.
    #[serde(default)]
    pub at_computer: u32,
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
    /// What the phone is shown of the tool's input: a question's `questions` with their text, `multiSelect` and
    /// options (label, description); `{}` for a permission, whose command or file is `what`.
    pub input: Value,
    pub profile: String,
    /// When it stops being a phone's to answer: its own expiry, or 300 s after it opened, whichever comes first.
    pub expires: i64,
    /// What the phone sends back in an action, so a stale screen cannot act.
    pub state_version: i64,
    /// Set once the request is past a phone's window: why it is answered at the computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_computer: Option<String>,
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
    /// The computer reserved the action and then stopped before it knew the outcome: it may or may not have reached
    /// the session, and it is never sent again. The phone shows "delivery unknown".
    Unknown,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self { Status::Accepted => "accepted", Status::AlreadyHandled => "already_handled", Status::Expired => "expired", Status::Unavailable => "unavailable", Status::Unknown => "unknown" }
    }

    pub fn parse(s: &str) -> Option<Status> {
        [Status::Accepted, Status::AlreadyHandled, Status::Expired, Status::Unavailable, Status::Unknown].into_iter().find(|x| x.as_str() == s)
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

/// `GET /c1/accounts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Accounts {
    pub computer: Computer,
    pub observed_at: i64,
    pub accounts: Vec<Account>,
}

/// `GET /c1/accounts/{profile}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountOne {
    pub computer: Computer,
    pub observed_at: i64,
    pub account: Account,
}

/// Reply and Stop for a run, each with the reason when this phone may not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunActions {
    pub reply: Allowed,
    pub stop: Allowed,
}

/// A session in `GET /c1/sessions`; one `hrm run` started also names its run, state version and actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionItem {
    #[serde(flatten)]
    pub session: Session,
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub state_version: Option<i64>,
    #[serde(default)]
    pub actions: Option<RunActions>,
}

/// `GET /c1/sessions?cursor=`: a page, and the cursor of the next one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionPage {
    pub computer: Computer,
    pub observed_at: i64,
    pub sessions: Vec<SessionItem>,
    pub next_cursor: Option<i64>,
}

/// `GET /c1/sessions/{harness}/{profile}/{id}/transcript?after=`: turns `after..` up to a bounded page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptPage {
    pub computer: Computer,
    pub observed_at: i64,
    pub harness: String,
    pub profile: String,
    pub id: String,
    pub title: String,
    pub after: usize,
    /// The turns as the desktop draws them: prompt, steps, answer.
    pub turns: Vec<Value>,
    pub next: Option<usize>,
    pub total: usize,
}

/// What this phone may do with the open requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestActions {
    pub answer: Allowed,
}

/// `GET /c1/requests`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Requests {
    pub computer: Computer,
    pub observed_at: i64,
    pub requests: Vec<Request>,
    pub actions: RequestActions,
}

/// An open request as the `needs` event names it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Need {
    pub run: String,
    pub ask_id: String,
    /// As `Request::expires`: the phone's window.
    pub expires: i64,
    pub state_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_computer: Option<String>,
}

/// An event read: what it says.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    /// The open requests now.
    Needs(Vec<Need>),
    /// A run's state moved; "ended" once the run is gone.
    RunState { run: String, state: String, state_version: i64 },
    /// Usage or limits changed: read the overview again.
    Changed,
    /// This phone was revoked or lost Summaries; the socket closes.
    Revoked,
    /// A kind this version does not know.
    Other(String),
}

impl Event {
    pub fn update(&self) -> serde_json::Result<Update> {
        Ok(match self.kind.as_str() {
            "needs" => Update::Needs(serde_json::from_value(self.data["requests"].clone())?),
            "run-state" => Update::RunState { run: serde_json::from_value(self.data["run"].clone())?, state: serde_json::from_value(self.data["state"].clone())?,
                state_version: serde_json::from_value(self.data["state_version"].clone())? },
            "changed" => Update::Changed,
            "revoked" => Update::Revoked,
            k => Update::Other(k.to_string()),
        })
    }
}

/// An answer the computer refused, with its HTTP status: 403 outside the grants, 404 for nothing there.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub status: u16,
    pub error: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "the computer answered {}: {}", self.status, self.error) }
}

impl std::error::Error for Refused {}

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
        for s in ["accepted", "already_handled", "expired", "unavailable", "unknown"] { assert_eq!(Status::parse(s).unwrap().as_str(), s); }
        assert_eq!(Grants::default(), Grants { summaries: true, transcripts: true, control: false });
    }
}
