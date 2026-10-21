//! User settings, saved as settings.json in the app config folder. The parts that
//! make sense on every computer (name, language, templates, recording choices) sync
//! through the Toolbay account; device-specific ones (microphone, models) don't.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toolbay_engine::chat::Provider;
use toolbay_engine::stt::CloudCfg;

pub const DEFAULT_CONSENT: &str = "Heads up: I'm taking notes with Cereal, which transcribes this call on my computer to help me write up notes. Nothing is sent to a bot or a server. Let me know if you'd rather I didn't.";

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
    pub launch_at_login: bool,
    pub sync_enabled: bool,
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
            launch_at_login: true,
            sync_enabled: true,
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

/// The subset that follows the user to other computers through Toolbay sync.
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
    pub keep_audio: String,
    pub job_title: String,
    pub about: String,
    #[serde(default)]
    pub company_name: String,
    #[serde(default)]
    pub company_about: String,
    pub role: String,
    pub vocabulary: Vec<String>,
    pub summary_language: String,
    pub calendars: Vec<crate::calendar::CalendarSrc>,
    pub calendar_popups: bool,
    pub reposition: bool,
    pub transcript_retention: String,
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
            keep_audio: self.keep_audio.clone(),
            job_title: self.job_title.clone(),
            about: self.about.clone(),
            company_name: self.company_name.clone(),
            company_about: self.company_about.clone(),
            role: self.role.clone(),
            vocabulary: self.vocabulary.clone(),
            summary_language: self.summary_language.clone(),
            calendars: self.calendars.clone(),
            calendar_popups: self.calendar_popups,
            reposition: self.reposition,
            transcript_retention: self.transcript_retention.clone(),
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
        if !s.keep_audio.is_empty() {
            self.keep_audio = s.keep_audio.clone();
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
        self.calendars = s.calendars.clone();
        self.calendar_popups = s.calendar_popups;
        self.reposition = s.reposition;
        if !s.transcript_retention.is_empty() {
            self.transcript_retention = s.transcript_retention.clone();
        }
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

pub fn load(dir: &Path) -> Settings {
    std::fs::read_to_string(path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, s: &Settings) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = path(dir).with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s)?)?;
    std::fs::rename(tmp, path(dir))?;
    Ok(())
}
