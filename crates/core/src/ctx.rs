use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::registry::Registry;
use crate::util::write_atomic;

/// Per-computer settings (not synced): where the registry lives, which terminal to open, refresh pace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Absolute path of profiles.json; empty means `<data>/profiles.json`.
    pub registry_path: String,
    /// "wt" (Windows Terminal) or "console" (the classic console window).
    pub terminal: String,
    pub refresh_minutes: u32,
    /// With live limits on, read a Claude account's limits from Anthropic when its cached ones are older than this.
    pub quota_max_age_minutes: u32,
    pub router_port: u16,
    /// The window's X hides it to the tray; off: the X quits.
    pub close_to_tray: bool,
    /// Ask each vendor for the tools' newest versions when the app starts.
    pub check_updates_at_launch: bool,
    /// "system" (the default: as Windows or macOS is), "light" or "dark".
    pub theme: String,
    /// The look: "hm" (Harness, the default) or "vg" (VektraGrid).
    pub brand: String,
    /// Limits as what is "left" or what is "used".
    pub quota_display: String,
    /// How large the window draws everything: "compact", "comfortable" (the default) or "large".
    pub text_size: String,
    /// Tools left out of every page (their accounts too).
    pub hidden_tools: Vec<String>,
    /// Hours between automatic backups; 0: none.
    pub backup_hours: u32,
    /// Backups kept; the oldest go first.
    pub backup_keep: u32,
    pub log_enabled: bool,
    /// "error", "warn" or "info".
    pub log_level: String,
    /// Overview: particles moving along the live routes; off: a still picture.
    pub moving_routes: bool,
    /// Projects shown in the sidebar, by key, in this order.
    pub pinned_projects: Vec<String>,
    /// This computer, across computers: made once; empty means "make one now".
    pub device_id: String,
    /// The name other computers see; the computer name unless changed.
    pub device_name: String,
    /// Made once per fresh database: a restore keeps `device_id` and starts a new incarnation.
    pub incarnation: String,
    /// "off", "folder" (a shared folder) or "hub" (a hub you host).
    pub sync_mode: String,
    /// The shared folder, absolute, when `sync_mode` is "folder".
    pub sync_folder: String,
    /// The hub's address when `sync_mode` is "hub"; its token lives in the credential store.
    pub hub_url: String,
    /// Origin of this computer's push relay. An empty address disables wakes.
    pub push_relay_url: String,
    pub sync_transcripts: bool,
    /// Tell the other computers what runs here, once a minute.
    pub sync_presence: bool,
    /// Talk to paired computers straight over the network (mDNS or an address), besides the folder.
    pub direct_connection: bool,
    /// Experimental, off: phones paired and served through the hub's mailbox. Off, this computer makes no phone call
    /// to the hub (no invite, no mailbox pass, no snapshot); only a deletion already owed there is still sent.
    pub phones_via_hub: bool,
    /// The direct connection's port; 0: a free one is picked once and saved here.
    pub listen_port: u16,
    /// Sessions older than this many days have their transcript moved to the archive; 0: never.
    pub archive_days: u32,
    /// A folder of the owner's choice for the archive; "" means none.
    pub archive_dir: String,
    /// Archive copies also go to `<sync_folder>/archive/<device_id>/` when a folder is set.
    pub archive_in_sync_folder: bool,
    /// Every backup zip is also copied to `<sync_folder>/backups/<device_id>/`, keeping `backup_keep` of them.
    pub backup_in_sync_folder: bool,
    /// The zip also holds this computer's transcripts, compressed.
    pub backup_transcripts: bool,
    /// Every account's prompt file (CLAUDE.md, AGENTS.md) ends with a short block, between Harness Manager's own
    /// marks, that tells an agent how to read `hrm list` and when it may use another account. Off: no file is touched.
    pub agent_hint: bool,
    /// Claude Code started by `hrm run` gets `--settings <data>/claude-hooks.json`, whose hooks run `hrm hook` on a few
    /// events, so the app shows which session is working, waiting for you or done. No account's files change.
    pub session_hooks: bool,
    /// How long a permission request or question from such a session waits for an answer in the app; after that the
    /// hook steps aside and only the terminal prompt answers it. 0: the app does not offer answers.
    pub request_seconds: u32,
    /// Give `hrm run` its own terminal on Windows. Ignored on other platforms.
    pub own_terminal: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { registry_path: String::new(), terminal: "wt".into(), refresh_minutes: 5, quota_max_age_minutes: 10, router_port: 15731,
            close_to_tray: true, check_updates_at_launch: false, theme: "system".into(), brand: "hm".into(), quota_display: "left".into(), text_size: "comfortable".into(), hidden_tools: vec![],
            backup_hours: 24, backup_keep: 10, log_enabled: true, log_level: "warn".into(),
            moving_routes: true, pinned_projects: vec![],
            device_id: String::new(), device_name: String::new(), incarnation: String::new(), sync_mode: "off".into(), sync_folder: String::new(),
            hub_url: String::new(), push_relay_url: String::new(), sync_transcripts: true, sync_presence: true, direct_connection: false, phones_via_hub: false, listen_port: 0, archive_days: 30, archive_dir: String::new(),
            archive_in_sync_folder: true, backup_in_sync_folder: true, backup_transcripts: false, agent_hint: false, session_hooks: true, request_seconds: 300, own_terminal: false }
    }
}

/// The data folder: `HRM_HOME`, else `HarnessManager` in the platform's data folder. `secrets` asks too, without a
/// `Ctx`: a computer with no system credential store keeps its credential file here.
pub fn data_dir() -> PathBuf {
    std::env::var_os("HRM_HOME").map(PathBuf::from).unwrap_or_else(|| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("HarnessManager"))
}

/// Everything a command needs: where data lives, the user's home, settings, registry and database.
pub struct Ctx {
    pub data: PathBuf,
    pub home: PathBuf,
    pub settings: Settings,
    pub db: Db,
}

impl Ctx {
    /// `HRM_HOME` overrides the data folder and `HRM_PROFILES_HOME` the folder profile homes are created in;
    /// tests set both to temp folders.
    pub fn open() -> Result<Ctx> {
        let data = data_dir();
        let home = std::env::var_os("HRM_PROFILES_HOME").map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().expect("home dir"));
        Ctx::open_at(&data, &home)
    }

    pub fn open_at(data: &Path, home: &Path) -> Result<Ctx> {
        std::fs::create_dir_all(data).with_context(|| format!("create {}", data.display()))?;
        let settings_path = data.join("settings.json");
        // defaults only when there is no file: a broken one would otherwise be saved over with defaults later
        let text = crate::util::read_or_empty(&settings_path)?;
        let mut settings: Settings = if text.trim().is_empty() { Settings::default() } else {
            serde_json::from_str(&text).with_context(|| format!("{} is not valid; fix it or delete it to use the defaults", settings_path.display()))?
        };
        crate::log::init(data, settings.log_enabled, &settings.log_level);
        // this computer's ids are made once and kept; the name defaults to the computer's
        let mut made = false;
        for id in [&mut settings.device_id, &mut settings.incarnation] {
            if id.is_empty() { *id = crate::util::uuid4(); made = true; }
        }
        if settings.device_name.is_empty() { settings.device_name = crate::util::hostname(); made = true; }
        if made { write_atomic(&settings_path, serde_json::to_string_pretty(&settings)?.as_bytes())?; }
        let db = Db::open(&data.join("hm.db"), &settings.device_id, settings.sync_mode != "off" || settings.direct_connection)?;
        Ok(Ctx { data: data.to_path_buf(), home: home.to_path_buf(), settings, db })
    }

    pub fn registry_path(&self) -> PathBuf {
        if self.settings.registry_path.is_empty() { self.data.join("profiles.json") } else { PathBuf::from(&self.settings.registry_path) }
    }

    pub fn registry(&self) -> Result<Registry> { Registry::load(&self.registry_path()) }

    pub fn save_registry(&self, reg: &Registry) -> Result<()> { reg.save(&self.registry_path()) }

    /// Hold this across load, change and save of the registry, so two writers never lose each other's change.
    pub fn lock_registry(&self) -> crate::util::NamedLock { crate::util::NamedLock::take("registry") }

    pub fn save_settings(&self) -> Result<()> {
        write_atomic(&self.data.join("settings.json"), serde_json::to_string_pretty(&self.settings)?.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settings_file_with_keys_no_longer_known_still_loads() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        // written by 1.9.16 and before: the two Overview settings that changed nothing
        std::fs::write(data.join("settings.json"), r#"{"terminal": "console", "first_reply_goal_s": 20, "game_layer": true, "moving_routes": false}"#).unwrap();
        let ctx = Ctx::open_at(&data, &t.path().join("home")).unwrap();
        assert_eq!(ctx.settings.terminal, "console");
        assert!(!ctx.settings.moving_routes);
        // the ids made on this first open are saved: the file is written again without the old keys, and loads again
        let saved = std::fs::read_to_string(data.join("settings.json")).unwrap();
        assert!(!saved.contains("game_layer") && !saved.contains("first_reply_goal_s"), "{saved}");
        assert_eq!(Ctx::open_at(&data, &t.path().join("home")).unwrap().settings.terminal, "console");
    }
}
