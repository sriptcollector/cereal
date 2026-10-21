//! Notices when a call starts so it can offer to take notes, and when it ends so it
//! can stop. Nothing is recorded until the user clicks "Take notes".
//!
//! Windows: the privacy registry Windows keeps for the microphone indicator says which
//! apps are using the mic right now (LastUsedTimeStop = 0). Browser calls are named
//! from the window title (Google Meet, Teams or Zoom on the web…).
//! macOS: Zoom's in-meeting helper process (CptHost). (More apps: see docs/FEATURES.md.)

use crate::state::St;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Call {
    /// Stable key (the program using the mic).
    pub key: String,
    /// Friendly name: "Zoom", "Google Meet", "Microsoft Teams", "Slack huddle"…
    pub app: String,
}

pub static DETECTED: Lazy<Mutex<Option<Call>>> = Lazy::new(|| Mutex::new(None));
static SNOOZED: Lazy<Mutex<HashMap<String, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn friendly(exe: &str, titles: &[String]) -> Option<String> {
    let e = exe.to_lowercase();
    let e = e.rsplit(['\\', '/', '#']).next().unwrap_or(&e).to_string();
    // Never react to ourselves or to dictation tools.
    if e.starts_with("toolbay") || e.contains("wispr") || e.contains("superwhisper") {
        return None;
    }
    let browsers = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe", "arc.exe", "chromium.exe"];
    if browsers.contains(&e.as_str()) {
        let t = titles.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>();
        let has = |p: &str| t.iter().any(|x| x.contains(p));
        return Some(
            if has("meet.google.com") || has("google meet") || t.iter().any(|x| x.starts_with("meet - ") || x.contains(" - meet ")) {
                "Google Meet"
            } else if has("microsoft teams") || has("| teams") {
                "Microsoft Teams"
            } else if has("zoom") {
                "Zoom"
            } else if has("whereby") {
                "Whereby"
            } else if has("jitsi") {
                "Jitsi Meet"
            } else if has("webex") {
                "Webex"
            } else if has("discord") {
                "Discord"
            } else {
                "A browser call"
            }
            .to_string(),
        );
    }
    let name = match e.as_str() {
        "zoom.exe" | "zoom" | "cpthost.exe" => "Zoom",
        "ms-teams.exe" | "teams.exe" | "msteams.exe" => "Microsoft Teams",
        "slack.exe" => "Slack huddle",
        "discord.exe" => "Discord",
        "webex.exe" | "ciscocollabhost.exe" | "webexmta.exe" | "atmgr.exe" => "Webex",
        "skype.exe" | "lync.exe" => "Skype",
        "whatsapp.exe" => "WhatsApp call",
        "signal.exe" => "Signal call",
        "telegram.exe" => "Telegram call",
        "facetime" => "FaceTime",
        "gotomeeting.exe" | "g2mcomm.exe" => "GoTo Meeting",
        "ringcentral.exe" => "RingCentral",
        "dialpad.exe" => "Dialpad",
        "aircall.exe" => "Aircall",
        _ => {
            if e.starts_with("mstea") {
                "Microsoft Teams"
            } else if e.starts_with("zoom") {
                "Zoom"
            } else {
                return None;
            }
        }
    };
    Some(name.to_string())
}

#[cfg(windows)]
fn window_titles() -> Vec<String> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};
    unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
        let v = &mut *(l.0 as *mut Vec<String>);
        if IsWindowVisible(h).as_bool() {
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(h, &mut buf);
            if n > 0 {
                v.push(String::from_utf16_lossy(&buf[..n as usize]));
            }
        }
        BOOL(1)
    }
    let mut v: Vec<String> = vec![];
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

/// Apps using the microphone right now.
#[cfg(windows)]
pub fn mic_users() -> Vec<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let mut v = vec![];
    let base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
    let hk = RegKey::predef(HKEY_CURRENT_USER);
    let in_use = |k: &RegKey| -> bool {
        let stop: u64 = k.get_value("LastUsedTimeStop").unwrap_or(1);
        let start: u64 = k.get_value("LastUsedTimeStart").unwrap_or(0);
        stop == 0 && start != 0
    };
    if let Ok(np) = hk.open_subkey(format!("{base}\\NonPackaged")) {
        for name in np.enum_keys().flatten() {
            if let Ok(k) = np.open_subkey(&name) {
                if in_use(&k) {
                    v.push(name);
                }
            }
        }
    }
    if let Ok(pk) = hk.open_subkey(base) {
        for name in pk.enum_keys().flatten() {
            if name == "NonPackaged" {
                continue;
            }
            if let Ok(k) = pk.open_subkey(&name) {
                if in_use(&k) {
                    v.push(name);
                }
            }
        }
    }
    v
}

#[cfg(target_os = "macos")]
pub fn mic_users() -> Vec<String> {
    let out = std::process::Command::new("ps").args(["-axco", "comm"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    out.lines().map(|l| l.trim()).filter(|l| *l == "CptHost").map(|l| l.to_string()).collect()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn mic_users() -> Vec<String> {
    vec![]
}

pub fn active_calls() -> Vec<Call> {
    let users = mic_users();
    if users.is_empty() {
        return vec![];
    }
    #[cfg(windows)]
    let titles = window_titles();
    #[cfg(not(windows))]
    let titles: Vec<String> = vec![];
    let mut v: Vec<Call> = vec![];
    for u in users {
        if let Some(app) = friendly(&u, &titles) {
            if !v.iter().any(|c| c.app == app) {
                v.push(Call { key: u, app });
            }
        }
    }
    v
}

pub fn snooze(app: &str) {
    SNOOZED.lock().insert(app.to_string(), Instant::now());
}

/// Background loop: offers notes when a call starts, stops when it ends.
pub fn start(st: St) {
    std::thread::spawn(move || {
        let mut prev: Vec<Call> = vec![];
        let mut gone_since: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let calls = active_calls();
            let (detect, auto_stop) = {
                let s = st.settings.read();
                (s.detect_meetings && s.onboarded, s.auto_stop)
            };
            let recording_app = st.session.lock().as_ref().map(|s| s.app.clone());
            // New call?
            for c in &calls {
                if prev.iter().any(|p| p.app == c.app) {
                    continue;
                }
                log::info!("call detected: {} ({})", c.app, c.key);
                let snoozed = SNOOZED.lock().get(&c.app).map(|t| t.elapsed() < Duration::from_secs(600)).unwrap_or(false);
                if detect && recording_app.is_none() && !snoozed {
                    *DETECTED.lock() = Some(c.clone());
                    crate::show_popup(&st, c);
                }
            }
            if calls.is_empty() {
                if DETECTED.lock().take().is_some() {
                    crate::hide_popup(&st);
                }
            }
            // The call we're recording ended?
            match recording_app {
                Some(app) if !app.is_empty() && auto_stop => {
                    if calls.iter().any(|c| c.app == app) {
                        gone_since = None;
                    } else {
                        let since = *gone_since.get_or_insert_with(Instant::now);
                        if since.elapsed() > Duration::from_secs(20) {
                            log::info!("auto-stop: {app} stopped using the microphone");
                            gone_since = None;
                            crate::recorder::stop(&st);
                        }
                    }
                }
                _ => gone_since = None,
            }
            prev = calls;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(friendly(r"C:#Users#me#AppData#Roaming#Zoom#bin#Zoom.exe", &[]), Some("Zoom".into()));
        assert_eq!(friendly(r"C:#Program Files#Google#Chrome#Application#chrome.exe", &["Meet - abc-defg-hij - Google Chrome".into()]), Some("Google Meet".into()));
        assert_eq!(friendly("MSTeams_8wekyb3d8bbwe", &[]), Some("Microsoft Teams".into()));
        assert_eq!(friendly(r"C:#x#toolbay-flow.exe", &[]), None);
        assert_eq!(friendly(r"C:#x#obs64.exe", &[]), None);
    }
}
