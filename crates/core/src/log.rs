//! The diagnostic log: `<data>/logs/harness-manager.log`, at the level Settings chooses. The app has no
//! console, so warnings that the command line prints would otherwise be lost. Rolls over at 2 MB.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static SINK: Mutex<Option<(PathBuf, u8)>> = Mutex::new(None);
const MAX: u64 = 2 * 1024 * 1024;

fn rank(level: &str) -> u8 { match level { "error" => 1, "warn" => 2, _ => 3 } }

/// Where and how much to log; called whenever settings are read.
pub fn init(data: &std::path::Path, enabled: bool, level: &str) {
    *SINK.lock().unwrap_or_else(|e| e.into_inner()) = enabled.then(|| (data.join("logs").join("harness-manager.log"), rank(level)));
}

pub fn file(data: &std::path::Path) -> PathBuf { data.join("logs").join("harness-manager.log") }

fn write(level: &str, msg: &str) {
    let sink = SINK.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some((path, max)) = sink else { return };
    if rank(level) > max { return; }
    if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
    if std::fs::metadata(&path).map(|m| m.len() > MAX).unwrap_or(false) { let _ = std::fs::rename(&path, path.with_extension("log.1")); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {:5} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), level.to_uppercase(), msg.replace('\n', " | "));
    }
}

pub fn error(msg: &str) { eprintln!("{msg}"); write("error", msg); }
pub fn warn(msg: &str) { eprintln!("{msg}"); write("warn", msg); }
pub fn info(msg: &str) { write("info", msg); }

/// Panics go to the log too, with the place, then to the default hook: the app and the detached router have no
/// console, and a background thread that panics would otherwise stop without a word.
pub fn log_panics() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let p = info.payload();
        let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
        let at = info.location().map(|l| l.to_string()).unwrap_or_default();
        error(&format!("panic in thread {}: {msg} at {at}", std::thread::current().name().unwrap_or("unnamed")));
        prev(info);
    }));
}

/// One round of a background loop: a panic in it is logged (by the hook) and the loop goes on to the next round.
pub fn round(what: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        error(&format!("{what}: the round stopped on a panic; the next one runs as usual"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_at_or_above_its_level_only() {
        let d = tempfile::tempdir().unwrap();
        super::init(d.path(), true, "warn");
        super::info("not this");
        super::warn("this one");
        super::init(d.path(), false, "info");
        super::error("not when off");
        let text = std::fs::read_to_string(super::file(d.path())).unwrap();
        assert!(text.contains("WARN  this one") && !text.contains("not this") && !text.contains("not when off"));
    }

    #[test]
    fn a_round_that_panics_does_not_stop_the_loop() {
        let mut n = 0;
        for i in 0..3 { super::round("test loop", || { n += 1; if i == 1 { panic!("boom") } }); }
        assert_eq!(n, 3);
    }
}
