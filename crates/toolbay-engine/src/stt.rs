//! Speech to text: the local whisper.cpp server, or an optional hosted engine
//! (OpenAI-compatible /audio/transcriptions: Groq, OpenAI and others) when the user
//! adds their own key.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CloudCfg {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl CloudCfg {
    pub fn ready(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.api_key.trim().is_empty() && !self.model.trim().is_empty()
    }
}

/// A piece of recognised speech, times in seconds from the start of the audio sent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Seg {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn lang(language: &str) -> String {
    if language.is_empty() { "auto".into() } else { language.into() }
}

/// Plain text of a short clip (local server).
pub async fn transcribe(base: &str, wav: Vec<u8>, language: &str, prompt: &str) -> Result<String> {
    Ok(join(&transcribe_segments(base, wav, language, prompt, Duration::from_secs(120)).await?))
}

/// Timed segments (local server). Long files are fine: whisper.cpp works through them
/// in 30-second windows.
pub async fn transcribe_segments(base: &str, wav: Vec<u8>, language: &str, prompt: &str, timeout: Duration) -> Result<Vec<Seg>> {
    let part = reqwest::multipart::Part::bytes(wav).file_name("audio.wav").mime_str("audio/wav")?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("response_format", "verbose_json")
        .text("temperature", "0.0")
        .text("temperature_inc", "0.2")
        .text("language", lang(language));
    if !prompt.is_empty() {
        form = form.text("prompt", prompt.to_string());
    }
    let r = reqwest::Client::new().post(format!("{base}/inference")).multipart(form).timeout(timeout).send().await?;
    let v: Value = r.error_for_status()?.json().await?;
    if let Some(e) = v.get("error") {
        return Err(anyhow!("speech engine: {e}"));
    }
    Ok(parse_segments(&v))
}

/// Hosted engine, timed segments.
pub async fn transcribe_cloud(cfg: &CloudCfg, wav: Vec<u8>, language: &str, prompt: &str) -> Result<Vec<Seg>> {
    let part = reqwest::multipart::Part::bytes(wav).file_name("audio.wav").mime_str("audio/wav")?;
    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", cfg.model.clone())
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "segment")
        .text("temperature", "0");
    if !language.is_empty() && language != "auto" {
        form = form.text("language", language.to_string());
    }
    if !prompt.is_empty() {
        form = form.text("prompt", prompt.chars().take(800).collect::<String>());
    }
    let r = reqwest::Client::new()
        .post(format!("{}/audio/transcriptions", cfg.base_url.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .multipart(form)
        .timeout(Duration::from_secs(300))
        .send()
        .await?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("cloud speech {}: {}", status.as_u16(), v["error"]["message"].as_str().unwrap_or("request failed")));
    }
    Ok(parse_segments(&v))
}

fn parse_segments(v: &Value) -> Vec<Seg> {
    let mut out = vec![];
    if let Some(a) = v["segments"].as_array() {
        for s in a {
            let text = s["text"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() || is_noise(&text) {
                continue;
            }
            out.push(Seg { start: s["start"].as_f64().unwrap_or(0.0), end: s["end"].as_f64().unwrap_or(0.0), text });
        }
    } else if let Some(t) = v["text"].as_str() {
        let t = t.trim();
        if !t.is_empty() && !is_noise(t) {
            out.push(Seg { start: 0.0, end: 0.0, text: t.to_string() });
        }
    }
    out
}

pub fn join(segs: &[Seg]) -> String {
    segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
}

/// Whisper's stock outputs for silence or music ("[BLANK_AUDIO]", "(music)", "Thank you.").
pub fn is_noise(t: &str) -> bool {
    let l = t.trim().to_lowercase();
    let l = l.trim_matches(|c: char| !c.is_alphanumeric() && c != '[' && c != ']' && c != '(' && c != ')' && c != ' ');
    if l.is_empty() {
        return true;
    }
    if (l.starts_with('[') && l.ends_with(']')) || (l.starts_with('(') && l.ends_with(')')) || (l.starts_with('*') && l.ends_with('*')) {
        return true;
    }
    matches!(l, "thank you" | "thanks for watching" | "thank you for watching" | "you" | "bye" | "so" | "uh" | "um" | "please subscribe" | "subtitles by the amara.org community")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn noise() {
        assert!(is_noise("[BLANK_AUDIO]"));
        assert!(is_noise(" Thank you. "));
        assert!(is_noise("(music)"));
        assert!(!is_noise("Thank you for the update on the roadmap."));
    }
    #[test]
    fn parse() {
        let v: Value = serde_json::json!({"text":"a b","segments":[{"start":0.0,"end":1.5,"text":" Hello there."},{"start":1.5,"end":2.0,"text":" [BLANK_AUDIO]"}]});
        let s = parse_segments(&v);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "Hello there.");
    }
}
