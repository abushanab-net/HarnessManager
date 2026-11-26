//! Local input for a run that owns its terminal.
//!
//! A phone's input carries a `Guard` (the run version its action claimed, its deadline, its enrollment) to the relay
//! in `hrm run`, which checks it immediately before the first byte and again before the Enter of a reply typed in
//! two steps: what reaches the session is what was checked. The outcome says truthfully what happened: delivered,
//! surely not delivered (an error), typed but not submitted (`Withheld`), or not known (`MaybeDelivered`).

use std::path::Path;

use anyhow::Result;

pub use crate::hooks::Guard;

#[cfg(windows)]
mod control;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::run;
#[cfg(windows)]
#[doc(hidden)]
pub use windows::{run_shim, SHIM_COMMAND};

/// The input may have reached the session: it was sent, and what became of it is not known.
#[derive(Debug)]
pub struct MaybeDelivered(pub String);

impl std::fmt::Display for MaybeDelivered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}; it may have reached the session", self.0) }
}

impl std::error::Error for MaybeDelivered {}

/// A reply was typed into the session but its Enter withheld: the guard stopped holding between the two.
#[derive(Debug)]
pub struct Withheld(pub String);

impl std::fmt::Display for Withheld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

impl std::error::Error for Withheld {}

/// Submit a single line to a local run, by run ID or by the session ID reported by its hooks.
pub fn reply(data: &Path, run: &str, text: &str) -> Result<()> { reply_guarded(data, run, text, None) }

/// `reply` that holds only while `guard` does, checked in the terminal's own process right before the input.
#[allow(clippy::needless_return)] // the return is needed when the other platform's block is compiled in
pub fn reply_guarded(data: &Path, run: &str, text: &str, guard: Option<&Guard>) -> Result<()> {
    #[cfg(windows)]
    {
        return control::send(data, run, control::Action::Reply(text), guard);
    }
    #[cfg(not(windows))]
    {
        let _ = (data, run, text, guard);
        anyhow::bail!("owned terminals are only available on Windows")
    }
}

/// Send Ctrl+C to a local run, by run ID or by the session ID reported by its hooks.
pub fn stop(data: &Path, run: &str) -> Result<()> { stop_guarded(data, run, None) }

/// `stop`, for a phone: the run must still be at the version its action claimed (`guard.version`).
#[allow(clippy::needless_return)] // the return is needed when the other platform's block is compiled in
pub fn stop_guarded(data: &Path, run: &str, guard: Option<&Guard>) -> Result<()> {
    #[cfg(windows)]
    {
        // one Stop per turn, whoever presses it, on one resolved run (hooks::claim_stop)
        let c = rusqlite::Connection::open(data.join("hm.db"))?;
        c.busy_timeout(std::time::Duration::from_secs(2))?;
        let claimed = crate::hooks::claim_stop(&c, run, guard.map(|g| g.version))?;
        if guard.is_some() && claimed.is_none() { anyhow::bail!("that session is no longer running here"); }
        let target = claimed.as_ref().map(|(r, _)| r.as_str()).unwrap_or(run);
        let guard = guard.zip(claimed.as_ref()).map(|(g, (_, v))| Guard { version: *v, ..g.clone() });
        return match control::send(data, target, control::Action::Stop, guard.as_ref()) {
            Ok(()) => { stopped(data, target); Ok(()) }
            Err(e) => {
                // surely not delivered: the turn is given back; maybe delivered: it stays held until the session reports
                if claimed.is_some() && e.downcast_ref::<MaybeDelivered>().is_none() { let _ = crate::hooks::release_stop(&c, target); }
                Err(e)
            }
        };
    }
    #[cfg(not(windows))]
    {
        let _ = (data, run, guard);
        anyhow::bail!("owned terminals are only available on Windows")
    }
}

/// After Stop the session is at its prompt, and no hook says so (Claude Code fires no Stop event after an interrupt):
/// mark the run done and close its requests, so Stop is not offered again, where another Ctrl+C would exit the tool.
#[cfg(windows)]
fn stopped(data: &Path, run: &str) {
    let r = rusqlite::Connection::open(data.join("hm.db")).and_then(|c| {
        c.busy_timeout(std::time::Duration::from_secs(2))?;
        c.execute("update run_ask set state = 'closed' where state = 'open' and run = ?1", [run])?;
        c.execute("update run_state set state = 'done', reason = '', stopping = null, ts = ?2 where run = ?1", rusqlite::params![run, crate::util::now_ms()])
    });
    if let Err(e) = r { crate::log::info(&format!("stop: marking {run} done: {e:#}")); }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn after_stop_the_session_is_done_and_its_request_closed() {
        let t = tempfile::tempdir().unwrap();
        let ctx = crate::Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        ctx.db.with(|c| {
            c.execute("insert into run_state(run, pid, started, profile, harness, session_id, state, stopping) values('1:2', 1, 2, 'p', 'claude', 's-1', 'waiting', 5)", [])?;
            c.execute("insert into run_ask(id, run, kind, tool, created, expires) values('a', '1:2', 'permission', 'Bash', 0, 9999999999999)", [])?;
            Ok(())
        }).unwrap();
        super::stopped(&ctx.data, "1:2"); // the run claim_stop resolved
        let (state, ask, stopping): (String, String, Option<i64>) = ctx.db.read(|c| Ok(c.query_row("select (select state from run_state), (select state from run_ask), (select stopping from run_state)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
        assert_eq!((state.as_str(), ask.as_str(), stopping), ("done", "closed", None));
    }

    /// N1: a Stop that did not reach the terminal leaves the session as it was: still waiting, its prompt open, and a
    /// Stop offered again.
    #[test]
    fn a_stop_the_terminal_did_not_take_leaves_the_session_waiting_and_stoppable() {
        let t = tempfile::tempdir().unwrap();
        let ctx = crate::Ctx::open_at(&t.path().join("data"), &t.path().join("home")).unwrap();
        let pid = std::process::id();
        ctx.db.with(|c| {
            // a live run whose terminal endpoint is gone: the relay cannot be reached, so nothing was sent
            c.execute("insert into run_state(run, pid, started, profile, harness, session_id, state, terminal_port, terminal_token) values('r', ?1, ?2, 'p', 'claude', 's', 'waiting', 1, x'00')",
                rusqlite::params![pid, crate::procs::start_ms(pid).unwrap()])?;
            c.execute("insert into run_ask(id, run, kind, tool, created, expires) values('a', 'r', 'permission', 'Bash', 0, 9999999999999)", [])?;
            Ok(())
        }).unwrap();
        assert!(super::stop(&ctx.data, "r").is_err());
        let (state, ask, stopping): (String, String, Option<i64>) = ctx.db.read(|c| Ok(c.query_row("select (select state from run_state), (select state from run_ask), (select stopping from run_state)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
        assert_eq!((state.as_str(), ask.as_str(), stopping), ("waiting", "open", None));
        let c = rusqlite::Connection::open(ctx.data.join("hm.db")).unwrap();
        assert!(crate::hooks::claim_stop(&c, "r", None).unwrap().is_some(), "Stop is offered again");
    }
}
