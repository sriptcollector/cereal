//! User settings, saved as settings.json in the app config folder. If the person
//! turns sync on (off by default), the parts that make sense on every computer
//! (profile, language, recording choices) sync through the Toolbay account;
//! device-specific ones (microphone, models, keys), calendar links and anything
//! that deletes data here (audio and transcript retention) never do.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toolbay_engine::chat::Provider;
use toolbay_engine::stt::CloudCfg;

pub const DEFAULT_CONSENT: &str = "Heads up: I'm taking notes with Cereal, which transcribes this call to help me write up notes. No bot joins the call. Let me know if you'd rather I didn't.";
/// The default note up to 0.4.3. Its "on my computer ... nothing is sent to a
/// server" isn't true with a cloud speech or AI option, so it is replaced.
const OLD_CONSENT: &str = "Heads up: I'm taking notes with Cereal, which transcribes this call on my computer to help me write up notes. Nothing is sent to a bot or a server. Let me know if you'd rather I didn't.";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub onboarded: bool,
    /// Shown instead of "You" in transcripts and notes.
    pub name: String,
    /// "auto" or a language code.
    pub language: String,
    pub microphone: Option<String>,
    /// Record the computer's sound (the other people in the call).
    pub capture_system: bool,
    /// When a meeting app starts using the microphone: "record" (automatically, with a
    /// visible indicator), "ask" (popup) or "nothing".
    pub meeting_start: String,
    /// Per-app overrides by friendly app name ("Zoom", "Slack huddle"…): "record" | "ask" |
    /// "nothing" | "ignore".
    pub app_rules: std::collections::BTreeMap<String, String>,
    /// The first automatic recording shows the consent note; later ones don't.
    pub auto_consent_shown: bool,
    /// Stop automatically a little after the meeting app stops using the microphone.
    pub auto_stop: bool,
    /// Show the "let others know" reminder each time recording starts.
    pub consent_reminder: bool,
    pub consent_message: String,
    /// Template used when none is picked ("auto" = guess from the meeting).
    pub default_template: String,
    /// "keep" | "30d" | "delete" (delete audio once the transcript is final).
    pub keep_audio: String,
    /// Ignored: Cereal never launches at login. Kept so saved settings still load.
    pub launch_at_login: bool,
    /// Sync the profile and recording choices through the Toolbay account
    /// (toolbay.ai can read them). Off by default: only the person turns it on.
    pub sync_enabled: bool,
    /// The person answered the sync question (or turned sync on themselves).
    /// Until then sync stays off, whatever an older version saved.
    pub sync_asked: bool,
    /// Short sounds when notes start, pause, resume and stop.
    pub sounds: bool,
    /// The floating indicator while recording when Cereal isn't in front. Automatic
    /// recordings always show it (never record silently).
    pub show_indicator: bool,
    /// Alt+Space opens a small ask box from anywhere.
    pub quick_chat: bool,
    /// Enter sends a chat message ("send") or adds a line ("newline").
    pub chat_enter: String,
    /// Show calendar events that have no other people in "Coming up".
    pub show_solo_events: bool,
    /// About your company (helps the notes AI).
    pub company_name: String,
    pub company_about: String,
    /// Notion: internal integration token and the page new notes go under (device only).
    pub notion_token: String,
    pub notion_page: String,
    /// HubSpot private app token (device only).
    pub hubspot_token: String,
    /// Keep screenshots when the screen changes during a meeting (shared slides).
    pub screen_snapshots: bool,
    /// Suggest speaker names from the meeting's attendees.
    pub speaker_suggestions: bool,
    /// Bottom-left hot corner offers to start notes.
    pub hot_corner: bool,

    // ---- profile (improves notes) ----
    pub job_title: String,
    /// Free text about you and your work.
    pub about: String,
    /// "Sales", "Engineering", "Founder", … (picks suggested templates).
    pub role: String,
    /// Names and terms the speech model and notes should spell right.
    pub vocabulary: Vec<String>,
    /// Language the notes are written in: "auto" (the meeting's) or a code.
    pub summary_language: String,

    // ---- calendar ----
    pub calendars: Vec<crate::calendar::CalendarSrc>,
    /// Popup a minute before meetings with 2+ people.
    pub calendar_popups: bool,
    /// Move the Cereal window beside the call when a meeting starts.
    pub reposition: bool,

    // ---- data ----
    /// Delete transcripts older than: "off" | "1d" | "1w" | "1m" | "3m" | "6m" | "1y".
    pub transcript_retention: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// Early (pre-release) updates.
    pub beta: bool,

    // ---- models (this computer only) ----
    /// "auto" | "manual"
    pub model_mode: String,
    pub speech_model: String,
    pub llm_model: String,
    pub use_gpu: bool,
    pub hw_fingerprint: String,
    pub auto_reason: String,
    /// Run the full-quality pass over the whole recording after the call.
    pub final_pass: bool,

    // ---- optional cloud engines (user's own keys) ----
    pub speech_cloud_on: bool,
    pub speech_cloud: CloudCfg,
    pub llm_provider: Provider,
    pub llm_cloud: CloudCfg,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            onboarded: false,
            name: String::new(),
            language: "auto".into(),
            microphone: None,
            capture_system: true,
            meeting_start: "record".into(),
            app_rules: Default::default(),
            auto_consent_shown: false,
            auto_stop: true,
            consent_reminder: true,
            consent_message: DEFAULT_CONSENT.into(),
            default_template: "auto".into(),
            keep_audio: "keep".into(),
            launch_at_login: false,
            sync_enabled: false,
            sync_asked: false,
            sounds: true,
            show_indicator: true,
            quick_chat: true,
            chat_enter: "send".into(),
            show_solo_events: true,
            company_name: String::new(),
            company_about: String::new(),
            notion_token: String::new(),
            notion_page: String::new(),
            hubspot_token: String::new(),
            screen_snapshots: false,
            speaker_suggestions: true,
            hot_corner: false,
            job_title: String::new(),
            about: String::new(),
            role: String::new(),
            vocabulary: vec![],
            summary_language: "auto".into(),
            calendars: vec![],
            calendar_popups: true,
            reposition: true,
            transcript_retention: "off".into(),
            theme: "system".into(),
            beta: false,
            model_mode: "auto".into(),
            speech_model: "base.en-q5".into(),
            llm_model: "qwen3-1.7b".into(),
            use_gpu: true,
            hw_fingerprint: String::new(),
            auto_reason: String::new(),
            final_pass: true,
            speech_cloud_on: false,
            speech_cloud: CloudCfg { base_url: "https://api.groq.com/openai/v1".into(), api_key: String::new(), model: "whisper-large-v3-turbo".into() },
            llm_provider: Provider::Local,
            llm_cloud: CloudCfg { base_url: "https://api.openai.com/v1".into(), api_key: String::new(), model: "gpt-4.1-mini".into() },
        }
    }
}

/// The subset that follows the user to other computers through Toolbay sync,
/// when they turn it on. Never calendar links: a secret iCal address gives
/// anyone who has it the whole calendar, so it stays on this computer. Never
/// `keep_audio` or `transcript_retention` either: a choice made on one computer
/// must not delete recordings or transcripts on another.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Synced {
    pub name: String,
    pub language: String,
    pub capture_system: bool,
    pub meeting_start: String,
    pub app_rules: std::collections::BTreeMap<String, String>,
    pub auto_stop: bool,
    pub consent_reminder: bool,
    pub consent_message: String,
    pub default_template: String,
    pub job_title: String,
    pub about: String,
    #[serde(default)]
    pub company_name: String,
    #[serde(default)]
    pub company_about: String,
    pub role: String,
    pub vocabulary: Vec<String>,
    pub summary_language: String,
    pub calendar_popups: bool,
    pub reposition: bool,
    pub theme: String,
}

impl Settings {
    pub fn synced(&self) -> Synced {
        Synced {
            name: self.name.clone(),
            language: self.language.clone(),
            capture_system: self.capture_system,
            meeting_start: self.meeting_start.clone(),
            app_rules: self.app_rules.clone(),
            auto_stop: self.auto_stop,
            consent_reminder: self.consent_reminder,
            consent_message: self.consent_message.clone(),
            default_template: self.default_template.clone(),
            job_title: self.job_title.clone(),
            about: self.about.clone(),
            company_name: self.company_name.clone(),
            company_about: self.company_about.clone(),
            role: self.role.clone(),
            vocabulary: self.vocabulary.clone(),
            summary_language: self.summary_language.clone(),
            calendar_popups: self.calendar_popups,
            reposition: self.reposition,
            theme: self.theme.clone(),
        }
    }
    pub fn apply_synced(&mut self, s: &Synced) {
        self.name = s.name.clone();
        if !s.language.is_empty() {
            self.language = s.language.clone();
        }
        self.capture_system = s.capture_system;
        if !s.meeting_start.is_empty() {
            self.meeting_start = s.meeting_start.clone();
        }
        self.app_rules = s.app_rules.clone();
        self.auto_stop = s.auto_stop;
        self.consent_reminder = s.consent_reminder;
        if !s.consent_message.is_empty() {
            self.consent_message = s.consent_message.clone();
        }
        if !s.default_template.is_empty() {
            self.default_template = s.default_template.clone();
        }
        self.job_title = s.job_title.clone();
        self.company_name = s.company_name.clone();
        self.company_about = s.company_about.clone();
        self.about = s.about.clone();
        self.role = s.role.clone();
        self.vocabulary = s.vocabulary.clone();
        if !s.summary_language.is_empty() {
            self.summary_language = s.summary_language.clone();
        }
        self.calendar_popups = s.calendar_popups;
        self.reposition = s.reposition;
        if !s.theme.is_empty() {
            self.theme = s.theme.clone();
        }
    }
    /// What to do when `app` starts a call.
    pub fn rule_for(&self, app: &str) -> String {
        self.app_rules.get(app).cloned().unwrap_or_else(|| self.meeting_start.clone())
    }
    pub fn me(&self) -> String {
        if self.name.trim().is_empty() { "You".into() } else { self.name.trim().to_string() }
    }
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

/// settings.json files that couldn't be read and couldn't be copied aside:
/// never overwritten while Cereal runs.
static PROTECTED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// A missing file means defaults. A file that doesn't read as settings is never
/// thrown away: it's copied to `settings.json.bad-<time>`, every field that still
/// reads is kept over the defaults (calendar links, Notion and HubSpot tokens…),
/// and if the copy couldn't be made the file is left alone.
pub fn load(dir: &Path) -> Settings {
    let p = path(dir);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(e) => {
            log::warn!("settings: can't read {}: {e}", p.display());
            PROTECTED.lock().unwrap().push(p);
            return Settings::default();
        }
    };
    let mut s = match serde_json::from_slice::<Settings>(&bytes) {
        Ok(s) => s,
        Err(e) => {
            let bad = dir.join(format!("settings.json.bad-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
            log::warn!("settings: {} doesn't read ({e}); copied to {} and kept every field that does", p.display(), bad.display());
            if let Err(e) = std::fs::copy(&p, &bad) {
                log::warn!("settings: couldn't copy it aside ({e}), so it won't be overwritten");
                PROTECTED.lock().unwrap().push(p.clone());
            }
            salvage(&String::from_utf8_lossy(&bytes))
        }
    };
    if migrate(&mut s) {
        let _ = save(dir, &s);
    }
    s
}

/// Defaults with every field of `text` that still reads laid over them.
fn salvage(text: &str) -> Settings {
    let Ok(serde_json::Value::Object(file)) = serde_json::from_str::<serde_json::Value>(text) else { return Settings::default() };
    let mut merged = serde_json::to_value(Settings::default()).unwrap_or_default();
    for (k, v) in file {
        let mut trial = merged.clone();
        trial[k.as_str()] = v;
        if serde_json::from_value::<Settings>(trial.clone()).is_ok() {
            merged = trial;
        }
    }
    serde_json::from_value(merged).unwrap_or_default()
}

/// One-time privacy migration. Up to 0.4.3 sync was on by default, so a saved
/// `sync_enabled: true` doesn't mean the person chose it: sync stays off until
/// they answer the sync question or turn it on in Settings. Also swaps the old
/// default consent note for the accurate one. Returns true when it changed something.
pub fn migrate(s: &mut Settings) -> bool {
    let mut changed = false;
    if !s.sync_asked && s.sync_enabled {
        s.sync_enabled = false;
        changed = true;
    }
    if s.consent_message == OLD_CONSENT {
        s.consent_message = DEFAULT_CONSENT.into();
        changed = true;
    }
    changed
}

pub fn save(dir: &Path, s: &Settings) -> anyhow::Result<()> {
    if PROTECTED.lock().unwrap().contains(&path(dir)) {
        anyhow::bail!("Cereal couldn't read its settings file, so it won't overwrite it. Restart Cereal to try again.");
    }
    std::fs::create_dir_all(dir)?;
    let tmp = path(dir).with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s)?)?;
    std::fs::rename(tmp, path(dir))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_is_off_by_default() {
        let s = Settings::default();
        assert!(!s.sync_enabled);
        assert!(!s.sync_asked);
    }

    #[test]
    fn old_settings_with_sync_on_are_turned_off_once() {
        let mut s: Settings = serde_json::from_str(r#"{"onboarded": true, "sync_enabled": true}"#).unwrap();
        assert!(migrate(&mut s));
        assert!(!s.sync_enabled);
        assert!(!migrate(&mut s));
    }

    #[test]
    fn a_chosen_sync_stays_on() {
        let mut s: Settings = serde_json::from_str(r#"{"sync_enabled": true, "sync_asked": true}"#).unwrap();
        assert!(!migrate(&mut s));
        assert!(s.sync_enabled);
    }

    #[test]
    fn calendar_links_never_sync() {
        let mut s = Settings::default();
        s.calendars.push(crate::calendar::CalendarSrc {
            id: "c1".into(),
            name: "Work".into(),
            url: "https://calendar.google.com/calendar/ical/secret-abc/basic.ics".into(),
            enabled: true,
        });
        let v = serde_json::to_string(&s.synced()).unwrap();
        assert!(!v.contains("secret-abc"));
        assert!(!v.contains("calendars"));
        // An item an older version synced (with calendars) never replaces local ones.
        let old: Synced = serde_json::from_str(r#"{"name": "A", "calendars": [{"id": "x", "name": "X", "url": "https://evil/ics", "enabled": true}]}"#).unwrap();
        let mut here = s.clone();
        here.apply_synced(&old);
        assert_eq!(here.calendars, s.calendars);
    }

    #[test]
    fn data_deleting_choices_never_sync() {
        let mut s = Settings::default();
        s.keep_audio = "delete".into();
        s.transcript_retention = "1d".into();
        let v = serde_json::to_string(&s.synced()).unwrap();
        assert!(!v.contains("keep_audio") && !v.contains("transcript_retention"));
        // An item an older version synced (with them) never changes them here.
        let old: Synced = serde_json::from_str(r#"{"name": "A", "keep_audio": "delete", "transcript_retention": "1d"}"#).unwrap();
        let mut here = Settings::default();
        here.apply_synced(&old);
        assert_eq!(here.keep_audio, "keep");
        assert_eq!(here.transcript_retention, "off");
        assert_eq!(here.name, "A");
    }

    #[test]
    fn a_broken_settings_file_keeps_what_reads() {
        let dir = std::env::temp_dir().join(format!("cereal-settings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Missing file: defaults.
        assert_eq!(load(&dir).name, "");
        // `theme` has the wrong type, so serde rejects the whole file.
        let text = r#"{"onboarded": true, "name": "Ana", "theme": 5, "notion_token": "secret_n", "hubspot_token": "pat-h",
            "calendars": [{"id": "c1", "name": "Work", "url": "https://cal/ics", "enabled": true}]}"#;
        std::fs::write(path(&dir), text).unwrap();
        let s = load(&dir);
        assert!(s.onboarded);
        assert_eq!(s.name, "Ana");
        assert_eq!(s.theme, "system");
        assert_eq!(s.notion_token, "secret_n");
        assert_eq!(s.hubspot_token, "pat-h");
        assert_eq!(s.calendars.len(), 1);
        // The original is kept aside, byte for byte.
        let bad: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("settings.json.bad-")).collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(std::fs::read_to_string(bad[0].path()).unwrap(), text);
        // Not valid JSON at all: defaults.
        assert_eq!(salvage("{\"name\": \"Ana\"").name, "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
