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
        return control::send(data, run, control::Action::Stop);
    }
    #[cfg(not(windows))]
    {
        let _ = (data, run);
        anyhow::bail!("owned terminals are only available on Windows")
    }
}
