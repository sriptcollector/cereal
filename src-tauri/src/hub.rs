//! Living under the Toolbay hub: show/hide/quit requests, running state, the
//! signed-in Toolbay account, and the desktop shortcut.
//!
//! Control: the hub (or anyone) runs `cereal.exe --show | --hide | --quit`, and
//! `--start` / `--stop` (take notes now / stop), `--open <page>` ("meeting:<id>", "settings"…).
//! Notes is single-instance, so that second process forwards its arguments to the
//! running one and exits. While hidden, Notes keeps working from the tray (and keeps
//! recording if a meeting is on).
//!
//! State: `<app data>/running.json` = {pid, version, visible, window, updatedAt, hub, recording},
//! rewritten on every show/hide/record change and removed on quit.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn handle_args(app: &AppHandle, args: &[String]) {
    if let Some(u) = args.iter().find(|a| a.starts_with("cereal://")) {
        open_url(app, u);
    } else if let Some(t) = args.iter().find_map(|a| a.strip_prefix("--task=")) {
        // Jump-list tasks (Windows taskbar right-click).
        match t {
            "new-note" => handle_args(app, &["--start".to_string()]),
            "settings" => crate::show_hub(app, Some("settings")),
            "restart" => app.restart(),
            _ => quit(app),
        }
    } else if args.iter().any(|a| a == "--quit") {
        quit(app);
    } else if args.iter().any(|a| a == "--hide") {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
        crate::WANT_VISIBLE.store(false, std::sync::atomic::Ordering::SeqCst);
        write_state(app);
    } else if args.iter().any(|a| a == "--start") {
        // Start taking notes now (a new note) and show it.
        if let Some(st) = app.try_state::<crate::state::St>() {
            match crate::recorder::start(&st, None, "", "") {
                Ok(m) => crate::show_hub(app, Some(&format!("meeting:{}", m.id))),
                Err(e) => log::warn!("--start: {e:#}"),
            }
        }
    } else if args.iter().any(|a| a == "--stop") {
        if let Some(st) = app.try_state::<crate::state::St>() {
            crate::recorder::stop(&st);
        }
    } else if let Some(i) = args.iter().position(|a| a == "--open") {
        // --open <page>: "settings", "templates", "ask", "home" or "meeting:<id>".
        crate::show_hub(app, args.get(i + 1).map(|s| s.as_str()));
    } else {
        crate::show_hub(app, None);
    }
}

/// cereal:// links: new-note, open, settings, ask?q=…, meeting/<id>, folder/<id>.
pub fn open_url(app: &AppHandle, url: &str) {
    // The same link can arrive twice (forwarded arguments and the link event).
    static LAST: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap();
        if last.as_ref().map(|(u, t)| u == url && t.elapsed() < std::time::Duration::from_secs(2)).unwrap_or(false) {
            return;
        }
        *last = Some((url.to_string(), std::time::Instant::now()));
    }
    let Ok(u) = url::Url::parse(url) else { return };
    let host = u.host_str().unwrap_or("");
    let rest = u.path().trim_matches('/');
    log::info!("link: {host}/{rest}");
    match host {
        "new-note" | "start" => handle_args(app, &["--start".to_string()]),
        "settings" => crate::show_hub(app, Some("settings")),
        "ask" => {
            let q = u.query_pairs().find(|(k, _)| k == "q").map(|(_, v)| v.into_owned()).unwrap_or_default();
            crate::show_hub(app, Some(&format!("ask:{q}")));
        }
        "meeting" | "note" if !rest.is_empty() => crate::show_hub(app, Some(&format!("meeting:{rest}"))),
        "folder" if !rest.is_empty() => crate::show_hub(app, Some(&format!("folder:{rest}"))),
        _ => crate::show_hub(app, None),
    }
}

pub fn quit(app: &AppHandle) {
    // Finish a recording properly so the audio files are complete.
    if let Some(st) = app.try_state::<crate::state::St>() {
        if let Some(sess) = st.session.lock().take() {
            if let Some(c) = sess.cap {
                c.stop();
            }
        }
    }
    let _ = std::fs::remove_file(state_path(app));
    app.exit(0);
}

fn state_path(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("running.json")
}

pub fn write_state(app: &AppHandle) {
    let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    let v = serde_json::json!({
        "pid": std::process::id(),
        "version": app.package_info().version.to_string(),
        "visible": visible,
        "window": if visible { "shown" } else { "hidden" },
        "updatedAt": chrono::Utc::now().to_rfc3339(),
        "hub": from_hub(),
        "recording": app.try_state::<crate::state::St>().map(|s| s.session.lock().is_some()).unwrap_or(false),
    });
    let _ = std::fs::write(state_path(app), v.to_string());
}

/// Started by the Toolbay hub (it sets TOOLBAY_HUB=1 and passes --from-toolbay).
pub fn from_hub() -> bool {
    std::env::var("TOOLBAY_HUB").map(|v| v == "1").unwrap_or(false) || std::env::args().any(|a| a == "--from-toolbay")
}

#[derive(Serialize, Clone, Default)]
pub struct Account {
    pub name: String,
    pub handle: String,
    pub avatar_url: Option<String>,
}

/// The hub passes the user's toolbay.ai token in TOOLBAY_TOKEN. It's only held in
/// memory and only sent to toolbay.ai.
pub async fn account() -> Option<Account> {
    let token = crate::account::token()?;
    let r = reqwest::Client::new()
        .get("https://toolbay.ai/api/v1/me")
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    let v: serde_json::Value = r.error_for_status().ok()?.json().await.ok()?;
    let u = v.get("user").unwrap_or(&v);
    Some(Account {
        name: u["name"].as_str().unwrap_or_default().to_string(),
        handle: u["handle"].as_str().unwrap_or_default().to_string(),
        avatar_url: u["avatarUrl"].as_str().map(|s| s.to_string()),
    })
}
