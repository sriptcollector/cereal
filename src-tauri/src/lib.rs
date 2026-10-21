//! Toolbay Notes: free, local, open-source AI meeting notes. No bot joins your call.

mod capture;
#[cfg(target_os = "macos")]
mod capture_mac;
mod cli;
mod commands;
mod db;
mod detect;
mod engine;
mod hub;
mod logging;
mod notes;
mod process;
mod recorder;
mod settings;
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
        .title("Toolbay Notes")
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
            // Closing the window keeps Notes in the tray (and keeps recording).
            api.prevent_close();
            let _ = w2.hide();
            hub::write_state(w2.app_handle());
            update_pill(w2.app_handle());
        }
        tauri::WindowEvent::Focused(_) => update_pill(w2.app_handle()),
        _ => {}
    });
    Ok(())
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
                .title("Toolbay Notes")
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

pub fn hide_popup(st: &St) {
    if let Some(app) = &st.app {
        if let Some(w) = app.get_webview_window("popup") {
            let _ = w.hide();
        }
    }
}

// ---------------- floating recording pill ----------------

const PILL_W: f64 = 64.0;
const PILL_H: f64 = 132.0;

/// While recording and the main window isn't in front, a small pill on the right edge
/// shows that notes are being taken; click it to come back.
pub fn update_pill(app: &AppHandle) {
    let recording = app.try_state::<St>().map(|s| s.session.lock().is_some()).unwrap_or(false);
    let main_front = app.get_webview_window("main").map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false)).unwrap_or(false);
    let want = recording && !main_front;
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
            .title("Toolbay Notes recording")
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
    let open = MenuItem::with_id(app, "open", "Open Toolbay Notes", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", if rec { "Stop taking notes" } else { "Start taking notes" }, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Toolbay Notes", true, None::<&str>)?;
    Menu::with_items(app, &[&open, &toggle, &PredefinedMenuItem::separator(app)?, &settings, &PredefinedMenuItem::separator(app)?, &quit])
}

pub fn refresh_tray(st: &St) {
    let Some(app) = st.app.clone() else { return };
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let (Some(tray), Ok(menu)) = (app2.tray_by_id("notes"), tray_menu(&app2)) {
            let rec = app2.state::<St>().session.lock().is_some();
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(if rec { "Toolbay Notes: taking notes" } else { "Toolbay Notes" }));
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
        "quit" => hub::quit(app),
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
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| hub::handle_args(app, &args)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            logging::init(&data_dir);
            let st = St::new(Some(handle.clone()), config_dir, data_dir, St::bundled_dir(&handle))?;
            app.manage(st.clone());

            let s = st.settings.read().clone();
            let hidden = std::env::args().any(|a| a == "--hidden") && s.onboarded;
            create_main(&handle, !hidden)?;

            TrayIconBuilder::with_id("notes")
                .icon(app.default_window_icon().cloned().unwrap())
                .tooltip("Toolbay Notes")
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
            if s.launch_at_login && !hub::from_hub() {
                use tauri_plugin_autostart::ManagerExt;
                let _ = handle.autolaunch().enable();
            }
            engine::auto_setup(&st);
            detect::start(st.clone());
            sync::start(st.clone());
            recover(&st);
            // `--start` / `--open <page>` on first launch too (not only when forwarded).
            let first: Vec<String> = std::env::args().collect();
            if first.iter().any(|a| a == "--start" || a == "--open") {
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
                std::thread::sleep(Duration::from_secs(6 * 3600));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::app_info,
            commands::ui_ready,
            commands::meetings_list,
            commands::meeting_get,
            commands::meeting_create,
            commands::meeting_update,
            commands::meeting_delete,
            commands::folders,
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
            commands::chat_list,
            commands::chat_ask,
            commands::chat_clear,
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
            commands::pill_click,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Toolbay Notes")
        .run(|_app, ev| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = ev {
                // Keep running in the tray when windows close; real quits pass a code.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
