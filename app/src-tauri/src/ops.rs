//! Every operation the window can ask for through `call(op, args)`: its name, the function that runs it and its
//! flags, in one table. A new operation is one line here and one function in its area's module.

use anyhow::Result;
use hrm_core::Ctx;
use serde_json::Value;

use crate::{accounts, computers, sessions, settings, setup, usage, AppState};

/// Runs without the registry lock: it never loads, changes and saves the registry, and it may be slow (the network,
/// a terminal, a sync round), so nothing else must wait behind it. Without the flag an operation holds the lock.
pub const NO_LOCK: u8 = 1;
/// Changes nothing the pages show. Any other operation (a change, a login, a refresh) drops the kept page answers,
/// so the page drawn after it is computed fresh.
pub const NO_CHANGE: u8 = 2;
/// A call a page opens with: answered from what was kept, at once, while no new data has arrived since; one older
/// than `PAGE_FRESH` is also redone in the background for the next call (the clock-driven parts: open consoles,
/// limits). After new data the next call computes it, so the redraw that new data brings shows it.
pub const PAGE: u8 = 4;

/// An operation's function: the window's state, the context the call runs in, and the call's arguments.
pub type Handler = fn(&AppState, &Ctx, &Value) -> Result<Value>;

pub struct Op {
    pub name: &'static str,
    pub run: Handler,
    pub flags: u8,
}

const fn op(name: &'static str, run: Handler, flags: u8) -> Op { Op { name, run, flags } }

pub fn find(name: &str) -> Option<&'static Op> { OPS.iter().find(|o| o.name == name) }

/// An operation's flags; none for a name the table does not have (the call answers with an error then).
pub fn flags(name: &str) -> u8 { find(name).map_or(0, |o| o.flags) }

pub static OPS: &[Op] = &[
    // accounts
    op("state", accounts::state, NO_LOCK | NO_CHANGE | PAGE),
    op("path_status", settings::path_status, NO_LOCK | NO_CHANGE),
    op("path_set", settings::path_set, NO_LOCK),
    op("detail", accounts::detail, NO_LOCK | NO_CHANGE),
    op("add", accounts::add, 0),
    op("remove", accounts::remove, 0),
    op("update", accounts::update, 0),
    // asks Anthropic, or runs Antigravity for up to 30 s; the registry is only read
    op("quota_live", accounts::quota_live, NO_LOCK),
    op("set_cap", accounts::set_cap, 0),
    op("set_remote_control", accounts::set_remote_control, 0),
    op("set_label", accounts::set_label, 0),
    op("set_key", accounts::set_key, 0),
    op("start", accounts::start, NO_LOCK),
    op("login_start", accounts::login_start, NO_LOCK),
    op("login_submit", accounts::login_submit, NO_LOCK),
    op("login_poll", accounts::login_poll, NO_LOCK),
    op("login_cancel", accounts::login_cancel, NO_LOCK),
    op("login_terminal", accounts::login_terminal, NO_LOCK),
    op("refresh", accounts::refresh, NO_LOCK),
    op("account_stats", accounts::account_stats, NO_LOCK | NO_CHANGE),
    op("limits_extra", accounts::limits_extra, NO_LOCK | NO_CHANGE),
    // usage
    op("home", usage::home, NO_LOCK | NO_CHANGE | PAGE),
    op("requests", usage::requests, NO_LOCK | NO_CHANGE),
    op("today", usage::today, NO_LOCK | NO_CHANGE | PAGE),
    op("usage", usage::usage, NO_LOCK | NO_CHANGE),
    op("usage_csv", usage::usage_csv, NO_LOCK),
    // sessions
    op("sessions", sessions::sessions, NO_LOCK | NO_CHANGE),
    op("transcript", sessions::transcript, NO_LOCK | NO_CHANGE),
    op("file_stamp", sessions::file_stamp, NO_LOCK | NO_CHANGE),
    op("resume", sessions::resume, NO_LOCK),
    op("export_markdown", sessions::export_markdown, NO_LOCK | NO_CHANGE),
    op("show_window", sessions::show_window, NO_LOCK | NO_CHANGE),
    op("needs", sessions::needs, NO_LOCK | NO_CHANGE),
    op("answer", sessions::answer, NO_LOCK | NO_CHANGE),
    op("reply", sessions::reply, NO_LOCK | NO_CHANGE),
    op("stop", sessions::stop, NO_LOCK | NO_CHANGE),
    op("session_stats", sessions::session_stats, NO_LOCK | NO_CHANGE),
    op("session_resume_here", sessions::session_resume_here, NO_LOCK),
    op("transcripts_status", sessions::transcripts_status, NO_LOCK | NO_CHANGE),
    op("tag_set", sessions::tag_set, NO_LOCK),
    op("tag_list", sessions::tag_list, NO_LOCK | NO_CHANGE),
    op("projects", sessions::projects, NO_LOCK | NO_CHANGE | PAGE),
    op("project_alias", sessions::project_alias, 0),
    op("project_pin", sessions::project_pin, 0),
    // computers
    op("computers_status", computers::computers_status, NO_LOCK | NO_CHANGE),
    op("computers_now", computers::computers_now, NO_LOCK),
    // saves settings, never the registry, and ends with a whole sync round
    op("computers_set", computers::computers_set, NO_LOCK),
    op("computers_pair_start", computers::computers_pair_start, NO_LOCK),
    op("computers_pair_cancel", computers::computers_pair_cancel, NO_LOCK),
    op("computers_pair_join", computers::computers_pair_join, NO_LOCK),
    op("computers_forget", computers::computers_forget, NO_LOCK),
    op("computers_set_peer", computers::computers_set_peer, NO_LOCK),
    op("phones_list", computers::phones_list, NO_LOCK | NO_CHANGE),
    op("phones_set_grants", computers::phones_set_grants, NO_LOCK),
    op("phones_revoke", computers::phones_revoke, NO_LOCK),
    op("phones_pair_start", computers::phones_pair_start, NO_LOCK),
    op("phones_pair_status", computers::phones_pair_status, NO_LOCK | NO_CHANGE),
    op("phones_pair_approve", computers::phones_pair_approve, NO_LOCK),
    op("phones_pair_refuse", computers::phones_pair_refuse, NO_LOCK),
    op("phones_pair_cancel", computers::phones_pair_cancel, NO_LOCK),
    op("hub_join", computers::hub_join, NO_LOCK),
    op("hub_leave", computers::hub_leave, NO_LOCK),
    op("hub_invite", computers::hub_invite, NO_LOCK),
    op("ring_key_show", computers::ring_key_show, NO_LOCK),
    op("ring_key_enter", computers::ring_key_enter, NO_LOCK),
    // setup
    op("items", setup::items, NO_LOCK | NO_CHANGE),
    op("item_remove", setup::item_remove, 0),
    op("target", setup::target, 0),
    op("target_bulk", setup::target_bulk, 0),
    op("mcp_save", setup::mcp_save, 0),
    op("mcp_import_all", setup::mcp_import_all, 0),
    op("mcp_last_import", setup::mcp_last_import, NO_LOCK | NO_CHANGE),
    op("sync", setup::sync, 0),
    op("sync_last", setup::sync_last, NO_LOCK | NO_CHANGE),
    op("skill_install", setup::skill_install, 0),
    op("discover", setup::discover, NO_LOCK | NO_CHANGE),
    op("skill_repo_add", setup::skill_repo_add, 0),
    op("skill_repo_remove", setup::skill_repo_remove, 0),
    op("skill_update", setup::skill_update, 0),
    op("skill_adopt", setup::skill_adopt, 0),
    op("skill_takeover", setup::skill_takeover, 0),
    op("skill_import", setup::skill_import, 0),
    op("skill_check", setup::skill_check, NO_LOCK | NO_CHANGE),
    op("prompt_add", setup::prompt_add, 0),
    op("prompt_save", setup::prompt_save, 0),
    op("prompt_enable", setup::prompt_enable, 0),
    op("prompt_delete", setup::prompt_delete, 0),
    op("prompt_live", setup::prompt_live, NO_LOCK | NO_CHANGE),
    op("routing", setup::routing, NO_LOCK | NO_CHANGE),
    op("provider_add", setup::provider_add, 0),
    op("provider_remove", setup::provider_remove, 0),
    op("route_add", setup::route_add, 0),
    op("route_remove", setup::route_remove, 0),
    op("router_start", setup::router_start, NO_LOCK),
    op("router_stop", setup::router_stop, NO_LOCK),
    op("apps", setup::apps, NO_LOCK),
    op("apps_latest", setup::apps_latest, NO_LOCK),
    op("app_update", setup::app_update, NO_LOCK),
    op("app_install", setup::app_install, NO_LOCK),
    op("app_remove_copy", setup::app_remove_copy, NO_LOCK),
    op("app_uninstall", setup::app_uninstall, NO_LOCK),
    // settings
    op("settings_set", settings::settings_set, 0),
    op("live_quota_set", settings::live_quota_set, NO_LOCK),
    op("autostart_set", settings::autostart_set, NO_LOCK),
    op("import_ps", settings::import_ps, 0),
    op("detect", settings::detect, 0),
    op("backups", settings::backups, NO_LOCK | NO_CHANGE),
    op("backup_create", settings::backup_create, 0),
    op("backup_delete", settings::backup_delete, 0),
    op("backup_rename", settings::backup_rename, 0),
    op("backup_export", settings::backup_export, NO_LOCK),
    op("backup_restore", settings::backup_restore, 0),
    op("backup_import", settings::backup_import, 0),
    op("log_open", settings::log_open, NO_LOCK),
    op("restore_preview", settings::restore_preview, NO_LOCK),
    op("restore_run", settings::restore_run, 0),
    op("restore_welcome", settings::restore_welcome, NO_LOCK),
    op("archive_status", settings::archive_status, NO_LOCK | NO_CHANGE),
    op("archive_now", settings::archive_now, NO_LOCK),
    op("self_latest", settings::self_latest, NO_LOCK | NO_CHANGE),
    op("open_folder", settings::open_folder, NO_LOCK | NO_CHANGE),
    op("open_url", settings::open_url, NO_LOCK | NO_CHANGE),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Answered by `call` itself, before the table is read: they need the window's handle.
    const WINDOW: &[&str] = &["self_update_check", "self_update_install", "open_main"];

    /// The operations the interface asks for: every `call('name'` in app/ui/*.js, with its file. A call whose name is
    /// not written out there fails the test, since it could not be checked.
    fn ui_calls() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui");
        let mut out = vec![];
        for f in std::fs::read_dir(&dir).unwrap().flatten().filter(|f| f.path().extension().is_some_and(|e| e == "js")) {
            let (file, text) = (f.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(f.path()).unwrap());
            for (i, _) in text.match_indices("call(") {
                // another function whose name ends the same way, a method, or the function's own definition in core.js
                if text[..i].ends_with(|c: char| c.is_alphanumeric() || "_.$".contains(c)) || text[..i].ends_with("function ") { continue; }
                let rest = &text[i + 5..];
                let name = rest.strip_prefix('\'').and_then(|r| r.split_once('\'')).map(|(n, _)| n);
                let name = name.unwrap_or_else(|| panic!("{file}: a call whose operation is not a plain 'name': call({}", rest.chars().take(40).collect::<String>()));
                out.push((name.to_string(), file.clone()));
            }
        }
        assert!(out.len() > 100, "the interface's calls were not found in {}", dir.display());
        out
    }

    #[test]
    fn no_operation_is_in_the_table_twice() {
        let mut seen = std::collections::HashSet::new();
        for o in OPS {
            assert!(seen.insert(o.name), "{} is in the table twice", o.name);
            assert!(!WINDOW.contains(&o.name), "{} is answered before the table is read", o.name);
            // the call that reads a kept answer must not drop it, nor wait behind a change for it
            if o.flags & PAGE != 0 { assert_eq!(o.flags & (NO_LOCK | NO_CHANGE), NO_LOCK | NO_CHANGE, "{}", o.name); }
        }
    }

    #[test]
    fn every_operation_the_interface_calls_is_in_the_table() {
        let calls = ui_calls();
        for (name, file) in &calls {
            assert!(find(name).is_some() || WINDOW.contains(&name.as_str()), "{file} calls '{name}', which is not an operation");
        }
        // listed, not refused: shown with `cargo test -p hrm-app -- --nocapture`
        let unused: Vec<&str> = OPS.iter().map(|o| o.name).filter(|n| !calls.iter().any(|(c, _)| c == n)).collect();
        println!("operations no interface file calls: {unused:?}");
    }

    #[test]
    fn slow_operations_that_only_read_the_registry_run_without_its_lock() {
        for name in ["quota_live", "computers_set", "computers_now", "hub_join", "ring_key_enter", "refresh"] {
            assert!(flags(name) & NO_LOCK != 0, "{name} would hold the registry lock through a web request or a sync round");
        }
    }
}
