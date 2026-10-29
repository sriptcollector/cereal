//! Cereal: free, local, open-source AI meeting notes. No bot joins your call.

mod appicon;
mod account;
#[cfg(windows)]
mod audio_route;
mod calendar;
mod capture;
#[cfg(target_os = "macos")]
mod capture_mac;
mod cli;
mod commands;
mod connectors;
mod db;
mod detect;
mod engine;
mod hub;
mod library;
mod library_cmds;
mod logging;
mod mcp;
mod notes;
mod process;
mod recorder;
mod quick;
mod screens;
#[cfg(windows)]
mod jumplist;
mod settings;
mod sounds;
mod state;
mod sync;
mod updates;

use state::St;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

pub static UI_READY: AtomicBool = AtomicBool::new(false);
pub static WANT_VISIBLE: AtomicBool = AtomicBool::new(false);

pub fn show_hub(app: &AppHandle, page: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        if UI_READY.load(Ordering::SeqCst) {
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            // Shown by `ui_ready` once the page has painted (no white flash).
            WANT_VISIBLE.store(true, Ordering::SeqCst);
        }
        if let Some(p) = page {
            let _ = app.emit("navigate", p.to_string());
        }
    }
    hub::write_state(app);
}

pub fn ui_ready(app: &AppHandle) {
    UI_READY.store(true, Ordering::SeqCst);
    if WANT_VISIBLE.swap(false, Ordering::SeqCst) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    hub::write_state(app);
}

fn create_main(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Cereal")
        .inner_size(1180.0, 800.0)
        .min_inner_size(860.0, 600.0)
        .center()
        .visible(false)
        .background_color(tauri::window::Color(10, 10, 10, 255))
        .build()?;
    WANT_VISIBLE.store(visible, Ordering::SeqCst);
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        ui_ready(&app2);
    });
    let w2 = w.clone();
    w.on_window_event(move |ev| match ev {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Nothing runs in the background: closing the main window quits Cereal
            // (a recording is finished properly first), popup, pill and tray included.
            // The page saves a note or title still waiting to be saved first.
            api.prevent_close();
            quit_after_flush(w2.app_handle());
        }
        tauri::WindowEvent::Focused(_) => update_pill(w2.app_handle()),
        _ => {}
    });
    Ok(())
}

/// Asks the page to save what's still waiting (notes and titles are saved a
/// moment after typing stops), then quits. Quits anyway after 2 s.
pub fn quit_after_flush(app: &AppHandle) {
    static ASKED: AtomicBool = AtomicBool::new(false);
    if !UI_READY.load(Ordering::SeqCst) || app.emit_to("main", "flush-and-quit", ()).is_err() {
        hub::quit(app);
        return;
    }
    if ASKED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        let a = app.clone();
        let _ = app.run_on_main_thread(move || hub::quit(&a));
    });
}

// ---------------- meeting-detected popup ----------------

const POPUP_W: f64 = 360.0;
const POPUP_H: f64 = 112.0;

pub fn show_popup(st: &St, call: &detect::Call) {
    let Some(app) = st.app.clone() else { return };
    let call = call.clone();
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let w = match app2.get_webview_window("popup") {
            Some(w) => w,
            None => match WebviewWindowBuilder::new(&app2, "popup", WebviewUrl::App("index.html#popup".into()))
                .title("Cereal")
                .inner_size(POPUP_W, POPUP_H)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
            {
                Ok(w) => w,
                Err(e) => {
                    log::warn!("popup: {e}");
                    return;
                }
            },
        };
        if let Ok(Some(mon)) = app2.primary_monitor() {
            let wa = mon.work_area();
            let s = mon.scale_factor();
            let size = PhysicalSize::new((POPUP_W * s) as u32, (POPUP_H * s) as u32);
            let _ = w.set_size(size);
            let _ = w.set_position(PhysicalPosition::new(wa.position.x + wa.size.width as i32 - size.width as i32 - (16.0 * s) as i32, wa.position.y + (16.0 * s) as i32));
        }
        let _ = app2.emit_to("popup", "detected", call);
        let _ = w.show();
    });
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AutoStarted {
    app: String,
    meeting_id: String,
    consent: bool,
}

/// Recording started by itself: say so right away (never record silently). The first
/// time, the card also carries the consent note.
pub fn show_auto_started(st: &St, call: &detect::Call, meeting_id: &str, consent: bool) {
    show_popup(st, call);
    if let Some(app) = &st.app {
        let _ = app.emit_to("popup", "auto-started", AutoStarted { app: call.app.clone(), meeting_id: meeting_id.to_string(), consent });
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EventPopup {
    event: calendar::Event,
}

/// "Meeting starting" card for a calendar event (a minute before it starts).
pub fn show_event_popup(st: &St, e: &calendar::Event) {
    let call = detect::Call { key: format!("event:{}", e.id), app: e.title.clone() };
    show_popup(st, &call);
    if let Some(app) = &st.app {
        let _ = app.emit_to("popup", "event-starting", EventPopup { event: e.clone() });
    }
}

/// Granola-style: put the note beside the call (right third of the screen).
pub fn dock_beside_call(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let Ok(Some(mon)) = w.current_monitor().or_else(|_| app.primary_monitor()) else { return };
    let wa = mon.work_area();
    let width = (wa.size.width as f64 * 0.34).max(520.0 * mon.scale_factor()) as u32;
    let _ = w.unmaximize();
    let _ = w.set_size(PhysicalSize::new(width, wa.size.height));
    let _ = w.set_position(PhysicalPosition::new(wa.position.x + wa.size.width as i32 - width as i32, wa.position.y));
}

/// Hot-corner card: start taking notes.
pub fn show_corner_card(st: &St) {
    let call = detect::Call { key: "corner".into(), app: String::new() };
    show_popup(st, &call);
    if let Some(app) = &st.app {
        let _ = app.emit_to("popup", "hot-corner", ());
    }
}

/// 14 minutes of silence: ask "Still there?" before stopping at 15.
pub fn show_still_there(st: &St) {
    let call = detect::Call { key: "still-there".into(), app: String::new() };
    show_popup(st, &call);
    if let Some(app) = &st.app {
        let _ = app.emit_to("popup", "still-there", ());
    }
}

/// Granola's mini window: a small always-on-top note in the top-right corner; again
/// restores the normal window.
pub fn mini_window(app: &AppHandle) -> bool {
    static NORMAL: std::sync::Mutex<Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>> = std::sync::Mutex::new(None);
    let Some(w) = app.get_webview_window("main") else { return false };
    let mut normal = NORMAL.lock().unwrap();
    if let Some((pos, size)) = normal.take() {
        let _ = w.set_always_on_top(false);
        let _ = w.set_size(size);
        let _ = w.set_position(pos);
        return false;
    }
    let Ok(Some(mon)) = w.current_monitor() else { return false };
    if let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) {
        *normal = Some((pos, size));
    }
    let wa = mon.work_area();
    let s = mon.scale_factor();
    let size = PhysicalSize::new((360.0 * s) as u32, (520.0 * s) as u32);
    let _ = w.unmaximize();
    let _ = w.set_size(size);
    let _ = w.set_position(PhysicalPosition::new(wa.position.x + wa.size.width as i32 - size.width as i32 - (24.0 * s) as i32, wa.position.y + (24.0 * s) as i32));
    let _ = w.set_always_on_top(true);
    true
}

pub fn hide_popup(st: &St) {
    if let Some(app) = &st.app {
        if let Some(w) = app.get_webview_window("popup") {
            let _ = w.hide();
        }
    }
}

// ---------------- floating recording pill ----------------

const PILL_W: f64 = 64.0;
const PILL_H: f64 = 268.0;
/// With live captions open the pill grows to the left.
const PILL_WIDE: f64 = 400.0;

pub fn pill_wide(app: &AppHandle, wide: bool) {
    let Some(p) = app.get_webview_window("pill") else { return };
    let (Ok(pos), Ok(size), Ok(Some(mon))) = (p.outer_position(), p.outer_size(), p.current_monitor()) else { return };
    let s = mon.scale_factor();
    let w = ((if wide { PILL_WIDE } else { PILL_W }) * s) as u32;
    let right = pos.x + size.width as i32;
    let _ = p.set_size(PhysicalSize::new(w, size.height));
    let _ = p.set_position(PhysicalPosition::new(right - w as i32, pos.y));
}

/// While recording and the main window isn't in front, a small pill on the right edge
/// shows that notes are being taken; click it to come back.
pub fn update_pill(app: &AppHandle) {
    let (recording, auto) = app.try_state::<St>().map(|s| s.session.lock().as_ref().map(|x| (true, x.auto)).unwrap_or((false, false))).unwrap_or((false, false));
    let main_front = app.get_webview_window("main").map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false)).unwrap_or(false);
    // Always visible for automatic recordings; otherwise when the main window isn't in front.
    let indicator = app.try_state::<St>().map(|s| s.settings.read().show_indicator).unwrap_or(true);
    let want = recording && (auto || (!main_front && indicator));
    let pill = app.get_webview_window("pill");
    if !want {
        if let Some(p) = pill {
            let _ = p.hide();
        }
        return;
    }
    let p = match pill {
        Some(p) => p,
        None => match WebviewWindowBuilder::new(app, "pill", WebviewUrl::App("index.html#pill".into()))
            .title("Cereal recording")
            .inner_size(PILL_W, PILL_H)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build()
        {
            Ok(p) => {
                if let Ok(Some(mon)) = app.primary_monitor() {
                    let wa = mon.work_area();
                    let s = mon.scale_factor();
                    let size = PhysicalSize::new((PILL_W * s) as u32, (PILL_H * s) as u32);
                    let _ = p.set_size(size);
                    let _ = p.set_position(PhysicalPosition::new(wa.position.x + wa.size.width as i32 - size.width as i32 - (8.0 * s) as i32, wa.position.y + (wa.size.height as i32 - size.height as i32) / 2));
                }
                p
            }
            Err(e) => {
                log::warn!("pill: {e}");
                return;
            }
        },
    };
    if !p.is_visible().unwrap_or(false) {
        let _ = p.show();
    }
}

// ---------------- tray ----------------

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let st = app.state::<St>();
    let rec = st.session.lock().is_some();
    let open = MenuItem::with_id(app, "open", "Open Cereal", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", if rec { "Stop taking notes" } else { "Start taking notes" }, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "Restart Cereal", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Cereal", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &toggle, &PredefinedMenuItem::separator(app)?, &settings, &PredefinedMenuItem::separator(app)?, &restart, &quit])?;
    // Today's meetings that haven't ended (up to 8, like Granola); click opens the note.
    let today = chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339();
    let todays: Vec<calendar::Event> = calendar::events()
        .into_iter()
        .filter(|e| !e.all_day && !e.declined && e.end > now)
        .filter(|e| chrono::DateTime::parse_from_rfc3339(&e.start).map(|d| d.with_timezone(&chrono::Local).date_naive() == today).unwrap_or(false))
        .take(8)
        .collect();
    if !todays.is_empty() {
        menu.insert(&MenuItem::with_id(app, "today", "Today", false, None::<&str>)?, 0)?;
        for (i, e) in todays.iter().enumerate() {
            let t = chrono::DateTime::parse_from_rfc3339(&e.start).map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p").to_string()).unwrap_or_default();
            let label = format!("{t}   {}", e.title.chars().take(40).collect::<String>());
            menu.insert(&MenuItem::with_id(app, format!("ev:{}", e.id), label, true, None::<&str>)?, i + 1)?;
        }
        menu.insert(&PredefinedMenuItem::separator(app)?, todays.len() + 1)?;
    }
    Ok(menu)
}

pub fn refresh_tray(st: &St) {
    let Some(app) = st.app.clone() else { return };
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let (Some(tray), Ok(menu)) = (app2.tray_by_id("notes"), tray_menu(&app2)) {
            let rec = app2.state::<St>().session.lock().is_some();
            let _ = tray.set_menu(Some(menu));
            // Like Granola's menu-bar title: the next meeting and how soon.
            let tip = if rec {
                "Cereal: taking notes".to_string()
            } else {
                calendar::next_event()
                    .and_then(|e| {
                        let mins = (chrono::DateTime::parse_from_rfc3339(&e.start).ok()?.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_minutes();
                        (mins < 24 * 60).then(|| format!("Cereal · {} in {}", e.title.chars().take(40).collect::<String>(), if mins < 60 { format!("{} min", mins.max(1)) } else { format!("{} h", mins / 60) }))
                    })
                    .unwrap_or_else(|| "Cereal".to_string())
            };
            let _ = tray.set_tooltip(Some(tip));
        }
        update_pill(&app2);
        hub::write_state(&app2);
    });
}

fn on_menu(app: &AppHandle, id: &str) {
    let st = app.state::<St>().inner().clone();
    match id {
        "open" => show_hub(app, None),
        "settings" => show_hub(app, Some("settings")),
        "quit" => quit_after_flush(app),
        "restart" => hub::restart(app),
        id if id.starts_with("ev:") => {
            let ev = id.trim_start_matches("ev:").to_string();
            if let Ok(m) = library_cmds::note_for_event(app.state::<St>(), ev) {
                show_hub(app, Some(&format!("meeting:{}", m.id)));
            }
        }
        "toggle" => {
            if st.session.lock().is_some() {
                recorder::stop(&st);
            } else {
                match recorder::start(&st, None, "", "") {
                    Ok(m) => show_hub(app, Some(&format!("meeting:{}", m.id))),
                    Err(e) => log::warn!("start from tray: {e:#}"),
                }
            }
        }
        _ => {}
    }
}

/// Meetings left half-done by a crash or a forced quit: finish them now.
fn recover(st: &St) {
    for m in st.db.meetings(None).unwrap_or_default() {
        if matches!(m.status.as_str(), "recording" | "paused" | "processing") {
            log::info!("recovering meeting {} ({})", m.id, m.status);
            let _ = st.db.set(&m.id, "status", "processing");
            let st2 = st.clone();
            let id = m.id.clone();
            tauri::async_runtime::spawn(async move { process::run(&st2, &id, None).await });
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    // --quit / --stop / --hide with no copy running: nothing to do, don't start the app.
    if args.iter().any(|a| a == "--quit" || a == "--stop" || a == "--hide") && !hub::already_running() {
        return;
    }
    if args.iter().any(|a| a == "--mcp") {
        std::process::exit(mcp::serve());
    }
    if let Some(code) = mcp::cli(&args) {
        std::process::exit(code);
    }
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| hub::handle_args(app, &args)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            logging::init(&data_dir);
            let st = St::new(Some(handle.clone()), config_dir, data_dir, St::bundled_dir(&handle))?;
            app.manage(st.clone());

            // No launch at login: remove any login item / Run key an older version made.
            {
                use tauri_plugin_autostart::ManagerExt;
                let _ = handle.autolaunch().disable();
            }

            // Always show the main window at launch (an old `--hidden` flag is ignored).
            create_main(&handle, true)?;

            TrayIconBuilder::with_id("notes")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Cereal")
                .menu(&tray_menu(&handle)?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_hub(tray.app_handle(), None);
                    }
                })
                .build(app)?;

            hub::write_state(&handle);
            // cereal:// links (Windows registers the scheme for this user).
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                #[cfg(windows)]
                let _ = handle.deep_link().register_all();
                let h = handle.clone();
                handle.deep_link().on_open_url(move |ev| {
                    for u in ev.urls() {
                        hub::open_url(&h, u.as_str());
                    }
                });
            }
            #[cfg(windows)]
            jumplist::set();
            quick::register(&handle, &st);
            quick::hot_corner(handle.clone(), st.clone());
            engine::auto_setup(&st);
            detect::start(st.clone());
            calendar::start(st.clone());
            sync::start(st.clone());
            recover(&st);
            // `--start` / `--open <page>` on first launch too (not only when forwarded).
            let first: Vec<String> = std::env::args().collect();
            if first.iter().any(|a| a == "--start" || a == "--open" || a.starts_with("cereal://") || a.starts_with("--task=")) {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(1500));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || hub::handle_args(&h2, &first));
                });
            }
            // Silent updates when running on its own (the Toolbay hub updates Notes itself).
            if !hub::from_hub() {
                updates::start(&st);
            }
            let st2 = st.clone();
            std::thread::spawn(move || loop {
                process::retention_pass(&st2);
                process::trash_pass(&st2);
                std::thread::sleep(Duration::from_secs(6 * 3600));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            appicon::set_app_icon,
            commands::get_settings,
            commands::save_settings,
            commands::app_info,
            commands::ui_ready,
            commands::meetings_list,
            commands::meeting_get,
            commands::meeting_create,
            commands::meeting_update,
            commands::rename_speaker,
            commands::segment_update,
            commands::segment_delete,
            commands::record_start,
            commands::record_stop,
            commands::record_pause,
            commands::rec_state,
            commands::regenerate,
            commands::reprocess,
            commands::templates_list,
            commands::template_save,
            commands::template_delete,
            commands::search,
            commands::list_mics,
            commands::audio_check,
            commands::models_info,
            commands::rerun_auto,
            commands::download_model,
            commands::detected,
            commands::popup_action,
            commands::open_hub,
            commands::export_markdown,
            commands::open_data_dir,
            commands::reveal_audio,
            commands::quit,
            commands::toolbay_account,
            commands::sync_status,
            commands::sync_delete_remote,
            commands::pill_click,
            commands::still_here,
            commands::screens_list,
            commands::screen_to_note,
            commands::screen_remove,
            commands::send_to_notion,
            commands::send_to_hubspot,
            commands::post_text,
            commands::calendar_draft,
            commands::mini_window,
            commands::exe_path,
            commands::use_version,
            commands::quick_hide,
            commands::pill_wide,
            commands::screenshot,
            commands::current_event,
            commands::set_app_rule,
            library_cmds::folders_list,
            library_cmds::folder_save,
            library_cmds::folder_delete,
            library_cmds::files_list,
            library_cmds::file_add,
            library_cmds::file_delete,
            library_cmds::meeting_trash,
            library_cmds::meeting_restore,
            library_cmds::meeting_purge,
            library_cmds::trash_list,
            library_cmds::people_list,
            library_cmds::companies_list,
            library_cmds::meetings_with,
            library_cmds::recipes_list,
            library_cmds::recipe_save,
            library_cmds::recipe_delete,
            library_cmds::threads_list,
            library_cmds::thread_get,
            library_cmds::thread_delete,
            library_cmds::chat_send,
            library_cmds::calendar_events,
            library_cmds::calendar_refresh,
            library_cmds::note_for_event,
            library_cmds::brief_for_event,
            library_cmds::join_event,
            library_cmds::dock_window,
            library_cmds::dictate_start,
            library_cmds::dictate_stop,
            library_cmds::import_audio,
            library_cmds::export_docx,
            library_cmds::export_all,
            library_cmds::read_text_file,
            library_cmds::image_data_url,
            commands::feedback_send,
            commands::feedback_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Cereal")
        .run(|app, ev| match ev {
            // Cmd+Q, Dock > Quit, logout, `app.restart()`: finish a recording so
            // its files are complete. When the person quits (no exit code yet),
            // also let the page save what's still waiting, like closing the window.
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                hub::finish_recording(app);
                if code.is_none() && app.get_webview_window("main").is_some() {
                    api.prevent_exit();
                    quit_after_flush(app);
                }
            }
            tauri::RunEvent::Exit => hub::clear_state(app),
            _ => {}
        });
}
