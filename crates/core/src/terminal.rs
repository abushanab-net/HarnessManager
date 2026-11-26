//! Local input for a run that owns its terminal.

use std::path::Path;

use anyhow::Result;

#[cfg(windows)]
mod control;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::run;
#[cfg(windows)]
#[doc(hidden)]
pub use windows::{run_shim, SHIM_COMMAND};

/// Submit a single line to a local run, by run ID or by the session ID reported by its hooks.
pub fn reply(data: &Path, run: &str, text: &str) -> Result<()> {
    #[cfg(windows)]
    {
        return control::send(data, run, control::Action::Reply(text));
    }
    #[cfg(not(windows))]
    {
        let _ = (data, run, text);
        anyhow::bail!("owned terminals are only available on Windows")
    }
}

/// Send Ctrl+C to a local run, by run ID or by the session ID reported by its hooks.
pub fn stop(data: &Path, run: &str) -> Result<()> {
    #[cfg(windows)]
    {
        // one Stop per turn, whoever presses it (hooks::claim_stop)
        let c = rusqlite::Connection::open(data.join("hm.db"))?;
        c.busy_timeout(std::time::Duration::from_secs(2))?;
        crate::hooks::claim_stop(&c, run)?;
        control::send(data, run, control::Action::Stop)?;
        stopped(data, run);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (data, run);
        anyhow::bail!("owned terminals are only available on Windows")
    }
}

/// After Stop the session is at its prompt, and no hook says so (Claude Code fires no Stop event after an interrupt):
/// mark it done and close its requests, so Stop is not offered again, where another Ctrl+C would exit the tool.
#[cfg(windows)]
fn stopped(data: &Path, run: &str) {
    let r = rusqlite::Connection::open(data.join("hm.db")).and_then(|c| {
        c.busy_timeout(std::time::Duration::from_secs(2))?;
        let runs = "select run from run_state where run = ?1 or session_id = ?1";
        c.execute(&format!("update run_ask set state = 'closed' where state = 'open' and run in ({runs})"), [run])?;
        c.execute("update run_state set state = 'done', reason = '', ts = ?2 where run = ?1 or session_id = ?1", rusqlite::params![run, crate::util::now_ms()])
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
            c.execute("insert into run_state(run, pid, started, profile, harness, session_id, state) values('1:2', 1, 2, 'p', 'claude', 's-1', 'waiting')", [])?;
            c.execute("insert into run_ask(id, run, kind, tool, created, expires) values('a', '1:2', 'permission', 'Bash', 0, 9999999999999)", [])?;
            Ok(())
        }).unwrap();
        super::stopped(&ctx.data, "s-1"); // by session id, as the app and `hrm stop <session>` name it
        let (state, ask): (String, String) = ctx.db.read(|c| Ok(c.query_row("select (select state from run_state), (select state from run_ask)", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
        assert_eq!((state.as_str(), ask.as_str()), ("done", "closed"));
    }
}
