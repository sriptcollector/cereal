//! User settings, saved as settings.json in the app config folder. The parts that
//! make sense on every computer (name, language, templates, recording choices) sync
//! through the Toolbay account; device-specific ones (microphone, models) don't.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toolbay_engine::chat::Provider;
use toolbay_engine::stt::CloudCfg;

pub const DEFAULT_CONSENT: &str = "Heads up: I'm taking notes with Toolbay Notes, which transcribes this call on my computer to help me write up notes. Nothing is sent to a bot or a server. Let me know if you'd rather I didn't.";

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
    /// Offer to start notes when a meeting app starts using the microphone.
    pub detect_meetings: bool,
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
            detect_meetings: true,
            auto_stop: true,
            consent_reminder: true,
            consent_message: DEFAULT_CONSENT.into(),
            default_template: "auto".into(),
            keep_audio: "keep".into(),
            launch_at_login: true,
            sync_enabled: true,
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
    pub detect_meetings: bool,
    pub auto_stop: bool,
    pub consent_reminder: bool,
    pub consent_message: String,
    pub default_template: String,
    pub keep_audio: String,
}

impl Settings {
    pub fn synced(&self) -> Synced {
        Synced {
            name: self.name.clone(),
            language: self.language.clone(),
            capture_system: self.capture_system,
            detect_meetings: self.detect_meetings,
            auto_stop: self.auto_stop,
            consent_reminder: self.consent_reminder,
            consent_message: self.consent_message.clone(),
            default_template: self.default_template.clone(),
            keep_audio: self.keep_audio.clone(),
        }
    }
    pub fn apply_synced(&mut self, s: &Synced) {
        self.name = s.name.clone();
        if !s.language.is_empty() {
            self.language = s.language.clone();
        }
        self.capture_system = s.capture_system;
        self.detect_meetings = s.detect_meetings;
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
