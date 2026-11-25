//! Harness Manager desktop app: the window is a view over hrm-core. Every action goes through one
//! command, `call(op, args)`, which runs the same core functions the `hrm` CLI uses.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use hrm_core::{items, service, Ctx};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

mod accounts;
mod computers;
mod ops;
mod sessions;
mod settings;
mod setup;
mod usage;

struct AppState {
    ctx: Mutex<Arc<Ctx>>,
    /// The pages' data, kept: see `cached`.
    pages: Mutex<HashMap<String, Page>>,
    /// Moves on with every change made through the window: an answer computed across one is never kept.
    epoch: AtomicU64,
    first_run: Mutex<Option<Value>>,
    settings_mtime: Mutex<Option<std::time::SystemTime>>,
    /// The direct connection's listener, mDNS and pairing; set once the window exists to tell.
    peers: std::sync::OnceLock<Arc<hrm_core::sync::peer::Runtime>>,
}

impl AppState {
    fn peers(&self) -> Result<&Arc<hrm_core::sync::peer::Runtime>> { self.peers.get().context("the direct connection is not up yet") }

    /// Settings were saved by a core function: take them on the next call.
    fn refresh(&self) { *self.settings_mtime.lock().unwrap_or_else(|e| e.into_inner()) = None; }

    /// The context, reopened when settings.json changed on disk (for example through `hrm settings`).
    fn ctx(&self) -> Arc<Ctx> {
        let mut cur = self.ctx.lock().unwrap_or_else(|e| e.into_inner());
        let m = std::fs::metadata(cur.data.join("settings.json")).and_then(|m| m.modified()).ok();
        let mut seen = self.settings_mtime.lock().unwrap_or_else(|e| e.into_inner());
        if m != *seen {
            if let Ok(fresh) = Ctx::open_at(&cur.data, &cur.home) { *cur = Arc::new(fresh); }
            *seen = m;
        }
        cur.clone()
    }
}

/// One page's data as last computed, and whether a fresher one is being computed now.
struct Page { value: Value, at: Instant, stamp: Stamp, busy: bool }

/// What the pages are computed from, as of now: the database (any commit, by anyone) and the account list and
/// settings files. None when it cannot be told: nothing is kept then.
type Stamp = Option<(i64, Option<std::time::SystemTime>, Option<std::time::SystemTime>)>;
fn stamp(st: &AppState) -> Stamp {
    let ctx = st.ctx();
    let mtime = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    Some((ctx.db.data_version()?, mtime(ctx.registry_path()), mtime(ctx.data.join("settings.json"))))
}

/// How old a kept page answer (see `ops::PAGE`) may be before it is also redone in the background.
const PAGE_FRESH: Duration = Duration::from_secs(10);

fn page_key(op: &str, args: &Value) -> String { format!("{op} {args}") }

/// A page call through the kept answers (see `ops::PAGE`).
fn cached(st: &Arc<AppState>, op: &str, args: &Value) -> Result<Value> {
    // the first-run report is handed over once: never kept, never computed in the background
    if op == "state" && st.first_run.lock().unwrap_or_else(|e| e.into_inner()).is_some() { return dispatch(st, op, args); }
    let key = page_key(op, args);
    let now = stamp(st);
    if now.is_none() { return dispatch(st, op, args); }
    let kept = {
        let mut pages = st.pages.lock().unwrap_or_else(|e| e.into_inner());
        match pages.get_mut(&key) {
            // anything changed since it was kept: computed now, so the redraw that follows new data shows it
            Some(p) if p.stamp != now => None,
            Some(p) => {
                let redo = p.at.elapsed() > PAGE_FRESH && !p.busy;
                if redo { p.busy = true; }
                Some((p.value.clone(), redo))
            }
            None => None,
        }
    };
    match kept {
        Some((v, redo)) => {
            if redo { let (st, op, args) = (st.clone(), op.to_string(), args.clone()); std::thread::spawn(move || { let _ = compute_page(&st, &op, &args); }); }
            Ok(v)
        }
        None => compute_page(st, op, args),
    }
}

/// Compute a page call and keep it, with the data generation it started from.
fn compute_page(st: &Arc<AppState>, op: &str, args: &Value) -> Result<Value> {
    let (stamp, epoch) = (stamp(st), st.epoch.load(Ordering::SeqCst));
    let r = dispatch(st, op, args);
    let mut pages = st.pages.lock().unwrap_or_else(|e| e.into_inner());
    match &r {
        Ok(_) if st.epoch.load(Ordering::SeqCst) != epoch => {}
        Ok(v) => {
            let first = !pages.contains_key(&page_key(op, args));
            pages.insert(page_key(op, args), Page { value: v.clone(), at: Instant::now(), stamp, busy: false });
            drop(pages);
            // a write: it must not hold the answer behind whatever is writing now
            if first { let (st, op, args) = (st.clone(), op.to_string(), args.clone()); std::thread::spawn(move || remember_page_call(&st, &op, &args)); }
        }
        Err(_) => { if let Some(p) = pages.get_mut(&page_key(op, args)) { p.busy = false; } }
    }
    r
}

/// The page calls this window makes, kept between launches (the last twelve), so they are ready at the next start.
fn remember_page_call(st: &AppState, op: &str, args: &Value) {
    // one at a time: two pages opened together must not each write back a list without the other
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = st.ctx();
    let mut list: Vec<Value> = ctx.db.kv_get("pages:calls").ok().flatten().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    let item = json!([op, args]);
    // shortcut: a listed call keeps its place, so the oldest listed goes first, not the least used; a commit here would mark every kept page stale at each launch
    if list.contains(&item) { return; }
    list.insert(0, item);
    list.truncate(12);
    let _ = ctx.db.kv_set("pages:calls", &Value::Array(list).to_string());
}

/// At launch: the page calls of the last session computed ahead, so the first page opens with its data ready.
fn prefetch_pages(st: &Arc<AppState>) {
    let list: Vec<Value> = st.ctx().db.kv_get("pages:calls").ok().flatten().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
    for c in list {
        if let (Some(op), Some(args)) = (c[0].as_str(), c.get(1)) {
            if ops::flags(op) & ops::PAGE != 0 { let _ = cached(st, op, args); }
        }
    }
}


fn s(args: &Value, k: &str) -> Result<String> {
    args[k].as_str().map(String::from).with_context(|| format!("missing {k}"))
}
fn opt(args: &Value, k: &str) -> Option<String> { args[k].as_str().map(String::from).filter(|s| !s.is_empty()) }

/// The system's file manager on `p`: a folder opened; a file shown in its folder, selected where the file manager
/// can (xdg-open would open the file itself); a file not there yet, its folder.
fn open_cmd(p: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new(if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" });
    if p.is_dir() { c.arg(p); }
    else if cfg!(windows) && p.exists() { c.arg(format!("/select,{}", p.display())); }
    else if cfg!(target_os = "macos") && p.exists() { c.arg("-R").arg(p); }
    else { c.arg(p.parent().unwrap_or(p)); }
    c
}

fn open_path(p: &std::path::Path) -> Result<()> {
    let mut c = open_cmd(p);
    c.spawn().with_context(|| format!("could not start {} to open {}", c.get_program().to_string_lossy(), p.display()))?;
    Ok(())
}

fn sync_items(ctx: &Ctx) -> Result<Value> {
    let reg = ctx.registry()?;
    Ok(serde_json::to_value(items::sync(ctx, &reg)?)?)
}

/// The MCP servers every tool already has, into the list and written back in step; the report of which files
/// were read is kept for the MCP page.
fn import_mcp_now(ctx: &Ctx) -> Result<Vec<Value>> {
    let mut reg = ctx.registry()?;
    let report = items::import_mcp_all(ctx, &mut reg);
    ctx.save_registry(&reg)?;
    sync_items(ctx)?;
    ctx.db.kv_set("mcp:last_import", &json!({"at": hrm_core::util::now_ms(), "report": report}).to_string())?;
    Ok(report)
}

/// The sync report, or `{"error": ...}`: after a change is saved, a link that fails must not undo the answer.
fn sync_soft(ctx: &Ctx) -> Value {
    sync_items(ctx).unwrap_or_else(|e| json!({"error": format!("{e:#}")}))
}

/// Run one operation of the table (`ops::OPS`), under the registry lock unless it is marked to run without.
fn dispatch(state: &AppState, op: &str, a: &Value) -> Result<Value> {
    let Some(o) = ops::find(op) else { bail!("unknown operation {op}") };
    let ctx = state.ctx();
    let ctx = ctx.as_ref();
    // an operation not marked NO_LOCK loads, changes and saves the registry: one at a time across threads and processes
    let _reg_lock = (o.flags & ops::NO_LOCK == 0).then(|| ctx.lock_registry());
    (o.run)(state, ctx, a)
}

/// Harness Manager's own newer version, from the newest release's latest.json (HRM_UPDATE_URL in tests).
/// `install` downloads and installs it, then restarts (the Windows installer closes the app itself).
async fn self_update(app: &tauri::AppHandle, install: bool) -> Result<Value> {
    use tauri_plugin_updater::UpdaterExt;
    let mut b = app.updater_builder();
    if let Some(u) = hrm_core::testenv::var("HRM_UPDATE_URL") { b = b.endpoints(vec![u.parse()?])?; }
    let current = app.package_info().version.to_string();
    let found = b.build()?.check().await?;
    let seen = json!({"at": hrm_core::util::now_ms(), "current": current, "version": found.as_ref().map(|u| u.version.clone()), "notes": found.as_ref().and_then(|u| u.body.clone())});
    let _ = app.state::<Arc<AppState>>().ctx().db.kv_set("self:latest", &seen.to_string());
    let Some(up) = found else { return Ok(json!({"current": current})) };
    if install {
        up.download_and_install(|_, _| {}, || {}).await?;
        app.restart();
    }
    Ok(json!({"current": current, "version": up.version, "notes": up.body}))
}

/// The port the tests drive the window through (HRM_DEVTOOLS_PORT): a number only, so the value can never carry
/// browser flags of its own.
fn devtools_port() -> Option<u16> { hrm_core::testenv::var("HRM_DEVTOOLS_PORT")?.parse().ok() }

/// Look for a new Harness Manager at launch and then every hour; the window offers it, or drops an old offer.
/// The tests' copy looks only when they give it a release to find.
fn watch_self_update(app: tauri::AppHandle) {
    if devtools_port().is_some() && hrm_core::testenv::var("HRM_UPDATE_URL").is_none() { return; }
    std::thread::spawn(move || loop {
        match tauri::async_runtime::block_on(self_update(&app, false)) {
            Ok(v) => { let _ = app.emit("self-update", v); }
            Err(e) => hrm_core::log::warn(&format!("update check: {e:#}")),
        }
        std::thread::sleep(std::time::Duration::from_secs(3600));
    });
}

#[tauri::command]
async fn call(app: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>, op: String, args: Value) -> Result<Value, String> {
    if op == "self_update_check" || op == "self_update_install" {
        return self_update(&app, op == "self_update_install").await.map_err(|e| format!("{e:#}"));
    }
    // the limits popup: a ring opens the window at that account
    if op == "open_main" {
        show_main(&app);
        if let Some(h) = args["hash"].as_str() { let _ = app.emit_to("main", "navigate", h); }
        return Ok(Value::Null);
    }
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let flags = ops::flags(&op);
        if flags & ops::NO_CHANGE == 0 { st.epoch.fetch_add(1, Ordering::SeqCst); st.pages.lock().unwrap_or_else(|e| e.into_inner()).clear(); }
        let r = if flags & ops::PAGE != 0 { cached(&st, &op, &args) } else { dispatch(&st, &op, &args) };
        r.map_err(|e| format!("{e:#}"))
    })
        .await
        .map_err(|e| e.to_string())?
}

/// The app has no console, so a startup error is shown in a message box instead of vanishing.
fn fatal(msg: &str) -> ! {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        unsafe { MessageBoxW(std::ptr::null_mut(), w(msg).as_ptr(), w("Harness Manager").as_ptr(), MB_OK | MB_ICONERROR) };
    }
    eprintln!("{msg}");
    std::process::exit(1)
}

/// Bring the main window back from the tray (or from behind other windows).
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The limits popup: every account's ring in a small frameless window by the tray icon. A click on the icon opens
/// it and a second one closes it; so does losing the focus. The window itself opens from the icon's menu or a double-click.
fn limits_popup(app: &tauri::AppHandle, at: tauri::PhysicalPosition<f64>, icon: tauri::Rect) {
    use std::time::{Duration, Instant};
    use tauri::{PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};
    static HID: Mutex<Option<Instant>> = Mutex::new(None);
    // the click that took its focus (and so hid it) must not open it again
    if HID.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() < Duration::from_millis(400)) { return; }
    let w = match app.get_webview_window("limits") {
        Some(w) => { if w.is_visible().unwrap_or(false) { let _ = w.hide(); return; } w }
        None => {
            let w = match WebviewWindowBuilder::new(app, "limits", WebviewUrl::App("limits.html".into()))
                .title("Limits").decorations(false).resizable(false).always_on_top(true).skip_taskbar(true).visible(false)
                .inner_size(372.0, 139.0).build() {
                Ok(w) => w,
                Err(e) => { hrm_core::log::warn(&format!("limits popup: {e:#}")); return }
            };
            let w2 = w.clone();
            w.on_window_event(move |e| if let WindowEvent::Focused(false) = e { let _ = w2.hide(); *HID.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now()); });
            w
        }
    };
    // by the icon: above a tray at the foot of the screen (Windows), under one at the top (macOS), and within the screen
    let m = app.monitor_from_point(at.x, at.y).ok().flatten();
    let scale = m.as_ref().map_or(1.0, |m| m.scale_factor());
    // sized at every open, by the accounts shown now: two rings to a row (app.css .tray-rings-grid), a row tall enough
    // for a name on two lines, all of it scaled as Text size zooms the page
    let (n, z) = { let ctx = app.state::<Arc<AppState>>().ctx(); let hidden = &ctx.settings.hidden_tools;
        let tools: Vec<String> = service::harnesses().into_iter().filter(|h| h.installed && !hidden.contains(&h.id)).map(|h| h.id).collect();
        (ctx.registry().map(|r| r.profiles.iter().filter(|p| tools.contains(&p.harness)).count()).unwrap_or(0), match ctx.settings.text_size.as_str() { "compact" => 0.92, "large" => 1.12, _ => 1.0 }) };
    let (lw, lh) = (372.0 * z, (69.0 + n.max(1).div_ceil(2) as f64 * 70.0) * z);
    let _ = w.set_size(tauri::LogicalSize::new(lw, lh));
    let (sw, sh) = (lw * scale, lh * scale);
    let (mx, my, mw, mh) = m.map(|m| (m.position().x as f64, m.position().y as f64, m.size().width as f64, m.size().height as f64)).unwrap_or((0.0, 0.0, at.x + sw, at.y + sh));
    let top = match icon.position { tauri::Position::Physical(p) => p.y as f64, tauri::Position::Logical(p) => p.y * scale };
    let bottom = top + match icon.size { tauri::Size::Physical(s) => s.height as f64, tauri::Size::Logical(s) => s.height * scale };
    let x = (at.x - sw / 2.0).clamp(mx, (mx + mw - sw).max(mx));
    let y = (if at.y > my + mh / 2.0 { top - sh - 6.0 } else { bottom + 6.0 }).clamp(my, (my + mh - sh).max(my));
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
}

/// The sessions' states as last read (hooks.rs), for the tray's entries.
static RUNS: Mutex<Vec<hrm_core::hooks::Run>> = Mutex::new(Vec::new());

/// "Needs you": the few rows `hrm hook` writes, read every second; on a change the window is told and the tray
/// says how many sessions wait, with an entry for each that brings its console to the front.
fn watch_runs(app: tauri::AppHandle, st: Arc<AppState>) {
    std::thread::spawn(move || loop {
        hrm_core::log::round("session states", || {
            let ctx = st.ctx();
            let Ok(now) = hrm_core::hooks::runs(&ctx) else { return };
            {
                let mut last = RUNS.lock().unwrap_or_else(|e| e.into_inner());
                if *last == now { return; }
                *last = now.clone();
            }
            let _ = app.emit("needs", &now);
            let Some(t) = app.tray_by_id("main") else { return };
            let waiting: Vec<&hrm_core::hooks::Run> = now.iter().filter(|r| r.state == "waiting").collect();
            let _ = t.set_tooltip(Some(match waiting.len() { 0 => "Harness Manager".to_string(), 1 => "Harness Manager · 1 session needs you".into(), n => format!("Harness Manager · {n} sessions need you") }));
            match tray_menu(&app, &ctx, &waiting) { Ok(m) => { let _ = t.set_menu(Some(m)); } Err(e) => hrm_core::log::warn(&format!("tray: {e:#}")) }
        });
        std::thread::sleep(Duration::from_secs(1));
    });
}

/// The tray's menu: a line per session waiting for you, then Open and Quit.
fn tray_menu<M: Manager<tauri::Wry>>(app: &M, ctx: &Ctx, waiting: &[&hrm_core::hooks::Run]) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
    let reg = ctx.registry().ok();
    let label = |n: &str| reg.as_ref().and_then(|r| r.get(n).ok()).and_then(|p| p.label.clone()).unwrap_or_else(|| n.to_string());
    let mut items: Vec<Box<dyn IsMenuItem<tauri::Wry>>> = vec![];
    for r in waiting {
        let text = hrm_core::util::truncate(&format!("{} needs you{}", label(&r.profile), if r.reason.is_empty() { String::new() } else { format!(": {}", r.reason) }), 60);
        items.push(Box::new(MenuItem::with_id(app, format!("run:{}", r.run), text, true, None::<&str>)?));
    }
    if !waiting.is_empty() { items.push(Box::new(PredefinedMenuItem::separator(app)?)); }
    items.push(Box::new(MenuItem::with_id(app, "open", "Open Harness Manager", true, None::<&str>)?));
    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?));
    Menu::with_items(app, &items.iter().map(|b| b.as_ref()).collect::<Vec<_>>())
}

/// The tray icon: a click shows the limits, a double-click the window; its menu opens the window or quits, and
/// brings a session that waits for you to the front.
fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let menu = tray_menu(app, &app.state::<Arc<AppState>>().ctx(), &[])?;
    let mut t = TrayIconBuilder::with_id("main").tooltip("Harness Manager").menu(&menu).show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            id => if let Some(run) = id.strip_prefix("run:") {
                let r = RUNS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r.run == run).cloned();
                if let Some(r) = r { if !sessions::focus_console(r.hwnd, r.pid) { show_main(app); } }
            },
        })
        .on_tray_icon_event(|t, e| match e {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, rect, .. } => limits_popup(t.app_handle(), position, rect),
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => show_main(t.app_handle()),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() { t = t.icon(icon.clone()); }
    t.build(app)?;
    Ok(())
}

fn main() {
    hrm_core::log::log_panics();
    // before any thread starts: set_var is not safe while others read the environment
    #[cfg(not(windows))]
    hrm_core::util::adopt_shell_path();
    let ctx = Arc::new(Ctx::open().unwrap_or_else(|e| fatal(&format!("Harness Manager could not start:\n\n{e:#}"))));
    let first = { let _l = ctx.lock_registry(); hrm_core::import::first_run(&ctx).ok().flatten().map(|r| serde_json::to_value(r).unwrap()) };
    let state = Arc::new(AppState { ctx: Mutex::new(ctx), pages: Mutex::new(HashMap::new()), epoch: AtomicU64::new(0), first_run: Mutex::new(first), settings_mtime: Mutex::new(None), peers: std::sync::OnceLock::new() });
    let mut builder = tauri::Builder::default();
    // one copy runs: opening it again (Start menu, a shortcut) shows the window the tray keeps. The tests' copy
    // skips this, so they can run while the installed app sits in the tray.
    if devtools_port().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state.clone())
        // closing hides the window; the app keeps refreshing in the tray until Quit
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if w.app_handle().state::<Arc<AppState>>().ctx().settings.close_to_tray {
                    api.prevent_close();
                    let _ = w.hide();
                } else {
                    w.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![call])
        .setup(move |app| {
            // background refresh: new usage and (opted-in) live limits, then tell the window
            let handle = app.handle().clone();
            let st = state.clone();
            let mut first = true;
            // each loop's round is guarded: a panic is logged and the next round runs, where it once ended the thread
            std::thread::spawn(move || loop {
                hrm_core::log::round("background refresh", || {
                    let ctx = st.ctx();
                    // the servers the tools already have are in the list from the first start, without an Import click
                    if std::mem::take(&mut first) {
                        let _l = ctx.lock_registry();
                        if let Err(e) = import_mcp_now(&ctx) { hrm_core::log::warn(&format!("reading the tools' MCP servers: {e:#}")); }
                        // a router left running by an older version or a dev build is replaced by this build's
                        if service::router_running(&ctx) { if let Err(e) = service::ensure_router(&ctx) { hrm_core::log::warn(&format!("router: {e:#}")); } }
                    }
                    let n = service::import_usage(&ctx).unwrap_or(0);
                    let _ = service::refresh_quotas(&ctx, false);
                    if let Err(e) = hrm_core::backup::auto(&ctx) { hrm_core::log::warn(&format!("automatic backup: {e:#}")); }
                    // what failed or interrupted operations left behind, at the first round and then hourly
                    hrm_core::sweep::hourly(&ctx);
                    if let Err(e) = hrm_core::archive::daily(&ctx) { hrm_core::log::warn(&format!("archive: {e:#}")); }
                    // the index the pages read: every account's folders walked (the watcher's safety net)
                    if let Err(e) = hrm_core::index::refresh(&ctx, hrm_core::index::Scope::All) { hrm_core::log::warn(&format!("index: {e:#}")); }
                    // across computers: out with what is new here, in with what the others published
                    if hrm_core::journal::on(&ctx) {
                        match hrm_core::journal::round(&ctx) {
                            Ok(v) => { let _ = handle.emit("computers", v); }
                            Err(e) => hrm_core::log::warn(&format!("computers: {e:#}")),
                        }
                    }
                    let _ = handle.emit("refreshed", json!({"new_rows": n}));
                });
                let mins = st.ctx().settings.refresh_minutes.max(1) as u64;
                std::thread::sleep(std::time::Duration::from_secs(mins * 60));
            });
            // what runs here, for the other computers' live views, once a minute
            let st2 = state.clone();
            std::thread::spawn(move || loop {
                hrm_core::log::round("presence", || {
                    if let Err(e) = hrm_core::journal::presence_tick(&st2.ctx()) { hrm_core::log::warn(&format!("presence: {e:#}")); }
                    if let Err(e) = hrm_core::index::mark_open(&st2.ctx()) { hrm_core::log::warn(&format!("index: open sessions: {e:#}")); }
                });
                std::thread::sleep(std::time::Duration::from_secs(60));
            });
            // the folder watcher: a changed session file is in the index within a second, and the pages redraw
            let (h6, st6) = (app.handle().clone(), state.clone());
            hrm_core::watch::start(Arc::new(move || st6.ctx()), Arc::new(move |r| { let _ = h6.emit("refreshed", json!({"new_rows": 0, "index": r})); }));
            // the pages the window opened last time, computed while it starts
            let st8 = state.clone();
            std::thread::spawn(move || prefetch_pages(&st8));
            // the direct connection: the listener while it is on, the paired computers pulled every ten seconds
            let (h3, st3) = (app.handle().clone(), state.clone());
            let on_event: hrm_core::sync::peer::OnEvent = Arc::new(move |name, v| { let _ = h3.emit(name, v); });
            match hrm_core::sync::peer::Runtime::new(std::net::Ipv4Addr::UNSPECIFIED.into(), on_event) {
                Ok(rt) => {
                    let rt = Arc::new(rt);
                    let _ = state.peers.set(rt.clone());
                    let h4 = app.handle().clone();
                    std::thread::spawn(move || loop {
                        hrm_core::log::round("direct connection", || {
                            let ctx = st3.ctx();
                            rt.tick(&ctx);
                            if ctx.settings.direct_connection {
                                if let Ok(v) = hrm_core::journal::status(&st3.ctx()) { let _ = h4.emit("computers", v); }
                            }
                        });
                        std::thread::sleep(std::time::Duration::from_secs(10));
                    });
                }
                Err(e) => hrm_core::log::warn(&format!("computers: the direct connection could not start: {e:#}")),
            }
            // the hub's events socket while the app runs (phase 5): a round on each change, the window told
            let (h5, st5) = (app.handle().clone(), state.clone());
            hrm_core::sync::hub::watch(Arc::new(move || st5.ctx()), Arc::new(move |v| { let _ = h5.emit("computers", v); }));
            let mut win = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                .title("Harness Manager")
                .inner_size(1280.0, 820.0)
                .min_inner_size(980.0, 640.0)
                // started at sign-in (--tray): the window waits in the tray
                .visible(!std::env::args().any(|a| a == "--tray"));
            // end-to-end tests drive the window over the DevTools protocol; only when they ask for it
            if let Some(port) = devtools_port() {
                // scale 1 for the tests only: on a display at 125% a 1 px line measures 0.8 px and the design tests fail
                win = win.additional_browser_args(&format!("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --force-device-scale-factor=1 --remote-debugging-port={port}"))
                    // its own browser profile: WebView2 cannot start a second, differently configured browser in the
                    // profile an open Harness Manager window is using, and the test would never get its window
                    .data_directory(state.ctx().data.join("webview"));
            }
            let w = win.build()?;
            if w.is_visible().unwrap_or(false) { w.set_focus()?; }
            tray(app)?;
            watch_runs(app.handle().clone(), state.clone());
            watch_self_update(app.handle().clone());
            // newer versions of the tools, when asked to look at launch: the Apps item shows how many
            let (handle, st) = (app.handle().clone(), state.clone());
            std::thread::spawn(move || {
                let ctx = st.ctx();
                if ctx.settings.check_updates_at_launch {
                    if let Ok(v) = hrm_core::apps::check(&ctx) { let _ = handle.emit("app-updates", v["updates"].clone()); }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run Harness Manager");
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_file_manager_opens_a_folder_and_shows_a_file_in_its_folder() {
        let args = |p: &std::path::Path| super::open_cmd(p).get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
        let file = std::env::current_exe().unwrap();
        let dir = file.parent().unwrap();
        assert_eq!(super::open_cmd(&dir).get_program(), if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" });
        assert_eq!(args(&dir), [dir.display().to_string()]);
        assert_eq!(args(&dir.join("hrm-no-such.log")), [dir.display().to_string()], "a file not there yet: its folder");
        let shown = if cfg!(windows) { vec![format!("/select,{}", file.display())] } else if cfg!(target_os = "macos") { vec!["-R".into(), file.display().to_string()] } else { vec![file.parent().unwrap().display().to_string()] };
        assert_eq!(args(&file), shown);
    }
}
