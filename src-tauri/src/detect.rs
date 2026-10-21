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
    if e.starts_with("ai.toolbay.") {
        return None;
    }
    let browsers = [
        "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe", "arc.exe", "chromium.exe",
        "com.google.chrome", "com.apple.safari", "company.thebrowser.browser", "com.microsoft.edgemac", "org.mozilla.firefox", "com.brave.browser", "com.operasoftware.opera", "com.vivaldi.vivaldi",
    ];
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
        "zoom.exe" | "zoom" | "cpthost.exe" | "cpthost" | "us.zoom.xos" => "Zoom",
        "com.microsoft.teams2" | "com.microsoft.teams" => "Microsoft Teams",
        "com.tinyspeck.slackmacgap" => "Slack huddle",
        "com.hnc.discord" => "Discord",
        "com.cisco.webexmeetingsapp" | "com.webex.meetingmanager" | "com.cisco.webex.meetings" => "Webex",
        "net.whatsapp.whatsapp" => "WhatsApp call",
        "ms-teams.exe" | "teams.exe" | "msteams.exe" => "Microsoft Teams",
        "slack.exe" => "Slack huddle",
        "discord.exe" => "Discord",
        "webex.exe" | "ciscocollabhost.exe" | "webexmta.exe" | "atmgr.exe" => "Webex",
        "skype.exe" | "lync.exe" => "Skype",
        "whatsapp.exe" => "WhatsApp call",
        "signal.exe" => "Signal call",
        "telegram.exe" => "Telegram call",
        "facetime" | "com.apple.facetime" => "FaceTime",
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
mod mac_audio {
    //! macOS 14.2+: CoreAudio says which apps are capturing audio input right now.
    use std::ffi::c_void;
    use std::ptr::null;

    #[repr(C)]
    struct Addr {
        selector: u32,
        scope: u32,
        element: u32,
    }
    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(obj: u32, addr: *const Addr, qsize: u32, qdata: *const c_void, size: *mut u32) -> i32;
        fn AudioObjectGetPropertyData(obj: u32, addr: *const Addr, qsize: u32, qdata: *const c_void, size: *mut u32, data: *mut c_void) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringGetCString(s: *const c_void, buf: *mut i8, size: isize, encoding: u32) -> u8;
        fn CFRelease(p: *const c_void);
    }
    const fn cc(s: &[u8; 4]) -> u32 {
        ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
    }
    const SYSTEM: u32 = 1;
    const GLOBAL: u32 = cc(b"glob");

    unsafe fn get<T: Default + Copy>(obj: u32, sel: u32) -> Option<T> {
        let a = Addr { selector: sel, scope: GLOBAL, element: 0 };
        let mut v = T::default();
        let mut size = std::mem::size_of::<T>() as u32;
        (AudioObjectGetPropertyData(obj, &a, 0, null(), &mut size, &mut v as *mut T as *mut c_void) == 0).then_some(v)
    }

    /// Bundle ids of apps whose audio input is running; None if this macOS can't tell.
    pub fn input_bundles() -> Option<Vec<String>> {
        unsafe {
            let a = Addr { selector: cc(b"prs#"), scope: GLOBAL, element: 0 };
            let mut size = 0u32;
            if AudioObjectGetPropertyDataSize(SYSTEM, &a, 0, null(), &mut size) != 0 || size == 0 {
                return None;
            }
            let mut ids = vec![0u32; size as usize / 4];
            if AudioObjectGetPropertyData(SYSTEM, &a, 0, null(), &mut size, ids.as_mut_ptr() as *mut c_void) != 0 {
                return None;
            }
            let mut out = vec![];
            for id in ids {
                let running: u32 = get(id, cc(b"piri")).unwrap_or(0);
                if running == 0 {
                    continue;
                }
                let cf: usize = get(id, cc(b"pbid")).unwrap_or(0);
                if cf == 0 {
                    continue;
                }
                let mut buf = [0i8; 256];
                if CFStringGetCString(cf as *const c_void, buf.as_mut_ptr(), 256, 0x0800_0100) != 0 {
                    let s = std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().to_string();
                    out.push(s);
                }
                CFRelease(cf as *const c_void);
            }
            Some(out)
        }
    }
}

#[cfg(target_os = "macos")]
pub fn mic_users() -> Vec<String> {
    if let Some(v) = mac_audio::input_bundles() {
        return v;
    }
    // Older macOS: Zoom's in-meeting helper process.
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
            let (onboarded, auto_stop, s) = {
                let s = st.settings.read();
                (s.onboarded, s.auto_stop, s.clone())
            };
            let recording_app = st.session.lock().as_ref().map(|s| s.app.clone());
            // New call?
            for c in &calls {
                if prev.iter().any(|p| p.app == c.app) {
                    continue;
                }
                let rule = s.rule_for(&c.app);
                log::info!("call detected: {} ({}) -> {rule}", c.app, c.key);
                let snoozed = SNOOZED.lock().get(&c.app).map(|t| t.elapsed() < Duration::from_secs(600)).unwrap_or(false);
                if !onboarded || recording_app.is_some() || snoozed {
                    continue;
                }
                match rule.as_str() {
                    "record" => match crate::recorder::start_with(&st, None, &c.app, "", true) {
                        Ok(m) => {
                            *DETECTED.lock() = Some(c.clone());
                            crate::show_auto_started(&st, c, &m.id, !s.auto_consent_shown);
                            if !s.auto_consent_shown {
                                let mut s2 = st.settings.read().clone();
                                s2.auto_consent_shown = true;
                                let _ = st.save_settings(s2);
                            }
                        }
                        Err(e) => log::warn!("auto-record {}: {e:#}", c.app),
                    },
                    "ask" => {
                        *DETECTED.lock() = Some(c.clone());
                        crate::show_popup(&st, c);
                    }
                    _ => {}
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
