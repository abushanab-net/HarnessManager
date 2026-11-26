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

/// `stop`, for a phone: the run must still be at the version its action claimed (`guard.version`). Every Stop claims
/// its turn first (`hooks::claim_stop`) and carries that claim to the relay, which takes no other and ends the turn in
/// the same write that sends the Ctrl+C. A Stop that surely did not arrive gives its claim back; one whose arrival is
/// not known keeps it until its window ends (`hooks::STOP_WINDOW_MS`), the same for the desktop and a phone.
#[allow(clippy::needless_return)] // the return is needed when the other platform's block is compiled in
pub fn stop_guarded(data: &Path, run: &str, guard: Option<&Guard>) -> Result<()> {
    #[cfg(windows)]
    {
        let c = rusqlite::Connection::open(data.join("hm.db"))?;
        c.busy_timeout(std::time::Duration::from_secs(2))?;
        let Some(claim) = crate::hooks::claim_stop(&c, run, guard.map(|g| g.version))? else {
            if guard.is_some() { anyhow::bail!("that session is no longer running here"); }
            // no row names it: the terminal says it has none
            return control::send(data, run, control::Action::Stop, None);
        };
        let g = Guard { version: claim.version, deadline: guard.map_or(claim.deadline, |g| g.deadline.min(claim.deadline)), phone: guard.and_then(|g| g.phone.clone()), stop: Some(claim.token.clone()) };
        return match control::send(data, &claim.run, control::Action::Stop, Some(&g)) {
            Ok(()) => Ok(()),
            Err(e) => {
                if e.downcast_ref::<MaybeDelivered>().is_none() { let _ = crate::hooks::release_stop(&c, &claim.run, &claim.token); }
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

#[cfg(all(test, windows))]
mod tests {
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
