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
    /// The other computers this one knows (paired, or seen through the hub or a shared folder), for the Computers
    /// screen; empty without Summaries.
    #[serde(default)]
    pub computers: Vec<KnownComputer>,
}

/// Another computer this one knows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownComputer {
    pub id: String,
    pub name: String,
    pub os: String,
    pub last_seen: Option<i64>,
}

/// The account to use next for one tool, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UseNext {
    pub tool: String,
    pub profile: String,
    pub name: String,
    /// Why, in a word: the rule that chose it ("Most room: 80% of its week left").
    pub reason: String,
    /// The line the desktop's Overview shows under the name: "80% of the week left · ≈ out Sat 15:00", or with "5h
    /// first", "60% of the 5-hour window left · resets in 2 h 10 m" (the soonest loss).
    #[serde(default)]
    pub why: String,
    /// The window that line speaks of: its label, room left, reset and the time this pace runs it out first, if it does.
    #[serde(default)]
    pub window: Option<String>,
    #[serde(default)]
    pub left: Option<i64>,
    #[serde(default)]
    pub resets_at: Option<i64>,
    #[serde(default)]
    pub out_at: Option<i64>,
    /// The next best account, as the desktop's Runner-up line: "teksa 75% left · lasts".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_up: Option<RunnerUp>,
}

/// The runner-up under Use next.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunnerUp {
    pub profile: String,
    pub name: String,
    /// Room left in its week (or first window), and when this pace runs it out, if it does.
    pub left: i64,
    pub out_at: Option<i64>,
    /// "75% left · ≈ out Sat", or "75% left · lasts".
    pub line: String,
}

/// One day's tokens on this computer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayTokens {
    /// "YYYY-MM-DD", this computer's calendar.
    pub day: String,
    pub tokens: i64,
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
    /// Today's estimated cost here at API list prices (USD); None when no model's price is known.
    #[serde(default)]
    pub today_cost: Option<f64>,
    /// The last 30 days' tokens here, oldest first, ending today: the desktop Overview's series.
    #[serde(default)]
    pub daily_tokens: Vec<DayTokens>,
}

/// One limit window of an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Window {
    pub id: String,
    pub label: String,
    pub left: i64,
    pub resets_at: Option<i64>,
    /// How long the window is, in hours (5 for the 5-hour window, 168 for a week); None when not known.
    #[serde(default)]
    pub hours: Option<f64>,
    /// The desktop's forecast from the readings in this window: the room left at the reset at this pace, and when it
    /// runs out before the reset, if it does. None without two readings far enough apart.
    #[serde(default)]
    pub left_at_reset: Option<i64>,
    #[serde(default)]
    pub runs_out_at: Option<i64>,
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
    /// "open", or "expired": a question whose window ended, listed for ten minutes after `ended_at`.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub ended_at: Option<i64>,
    /// When the request opened (ms).
    #[serde(default)]
    pub opened_at: i64,
    /// The session it belongs to (its id, the folder it runs in); the title and Claude's last line before the request
    /// come from its transcript, and are None without one. The last line needs Transcripts.
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub last_line: Option<String>,
    /// A permission's own description of the command, when the tool gave one.
    #[serde(default)]
    pub description: Option<String>,
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

/// Handle-only enrollment from earlier phones. A signed enrollment also supplies the relay secret.
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
    /// For `revoked`: when the computer removed this phone (ms); None when it lost Summaries instead.
    pub fn revoked_at(&self) -> Option<i64> { self.data["revoked_at"].as_i64() }

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

/// Why a call reached no answer, so the phone tells a computer that removed it from one it cannot reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// The computer refused this phone at TLS (a fatal alert it sent): the phone was removed there, or never paired.
    Revoked(String),
    /// Nothing answered, the connection dropped or was refused, the answer did not come in time, or the computer is
    /// not the one paired with.
    Unreachable(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Revoked(e) => write!(f, "the computer refused this phone: {e}"),
            ConnectError::Unreachable(e) => write!(f, "the computer was not reached: {e}"),
        }
    }
}

impl std::error::Error for ConnectError {}

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
