//! Quick Chat: Alt+Space anywhere opens a small ask box at the bottom of the screen
//! (as in Granola). Enter opens the answer in Cereal's chat.

use crate::state::St;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const W: f64 = 620.0;
const H: f64 = 64.0;

pub fn register(app: &AppHandle, st: &St) {
    if !st.settings.read().quick_chat {
        return;
    }
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
    let sc = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let r = app.global_shortcut().on_shortcut(sc, |app, _, ev| {
        if ev.state() == ShortcutState::Pressed {
            toggle(app);
        }
    });
    if let Err(e) = r {
        // Another app (e.g. PowerToys Run) may own Alt+Space.
        log::warn!("quick chat shortcut: {e}");
    }
}

pub fn toggle(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("quick") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            return;
        }
        place(app, &w);
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    match WebviewWindowBuilder::new(app, "quick", WebviewUrl::App("index.html#quick".into()))
        .title("Ask Cereal")
        .inner_size(W, H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
    {
        Ok(w) => {
            place(app, &w);
            let _ = w.show();
            let _ = w.set_focus();
        }
        Err(e) => log::warn!("quick chat: {e}"),
    }
}

fn place(app: &AppHandle, w: &tauri::WebviewWindow) {
    if let Ok(Some(mon)) = app.primary_monitor() {
        let wa = mon.work_area();
        let s = mon.scale_factor();
        let size = PhysicalSize::new((W * s) as u32, (H * s) as u32);
        let _ = w.set_size(size);
        let _ = w.set_position(PhysicalPosition::new(wa.position.x + (wa.size.width as i32 - size.width as i32) / 2, wa.position.y + wa.size.height as i32 - size.height as i32 - (96.0 * s) as i32));
    }
}
