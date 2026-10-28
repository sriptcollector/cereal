//! The person's Toolbay sign-in: the Toolbay app's token (Windows Credential
//! Manager, or a private file on macOS, never the macOS keychain) (it
//! changes when they sign in or out there), or the one the hub passed at launch
//! (TOOLBAY_TOKEN). Held in memory only, only ever sent to toolbay.ai.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;

pub const BASE: &str = "https://toolbay.ai";
/// Cereal's slug in the toolbay.ai catalog.
pub const SLUG: &str = "cereal";

/// The Toolbay app's saved sign-in on this computer. Windows: Credential Manager
/// (service "ai.toolbay.desktop", user "session"). macOS: the 0600 file
/// `~/Library/Application Support/ai.toolbay.desktop/session` that the Toolbay
/// app writes; the keychain is never touched on macOS (ad-hoc signed updates
/// would make macOS ask for the login password every time).
fn hub_token() -> Option<String> {
    #[cfg(windows)]
    {
        keyring::Entry::new("ai.toolbay.desktop", "session").and_then(|e| e.get_password()).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME")?;
        let p = std::path::PathBuf::from(home).join("Library/Application Support/ai.toolbay.desktop/session");
        std::fs::read_to_string(p).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }
}

pub fn tokens() -> Vec<String> {
    let mut v = vec![];
    if let Some(t) = hub_token() {
        v.push(t);
    }
    if let Ok(t) = std::env::var("TOOLBAY_TOKEN") {
        if !t.is_empty() && !v.contains(&t) {
            v.push(t);
        }
    }
    v
}

pub fn token() -> Option<String> {
    tokens().into_iter().next()
}

/// Posts in-app feedback to Cereal's community space on toolbay.ai.
/// Returns the post URL. Errors are the site's own plain words.
pub async fn send_feedback(kind: &str, text: &str, screenshot: Option<String>, debug: Option<Value>, visibility: Option<String>) -> Result<String> {
    let tok = token().ok_or_else(|| anyhow!("Sign in to the Toolbay app first, then send your feedback."))?;
    let mut body = json!({
        "type": kind,
        "text": text,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "os": if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
    });
    if let Some(s) = screenshot {
        body["screenshot"] = json!(s);
    }
    if let Some(d) = debug {
        body["debug"] = d;
    }
    if let Some(v) = visibility {
        body["visibility"] = json!(v);
    }
    let r = reqwest::Client::new()
        .post(format!("{BASE}/api/v1/tools/{SLUG}/feedback"))
        .bearer_auth(tok)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| anyhow!("Couldn't reach toolbay.ai. Check your internet connection and try again."))?;
    let status = r.status().as_u16();
    let v: Value = r.json().await.unwrap_or_default();
    match status {
        200 | 201 => Ok(format!("{BASE}{}", v["url"].as_str().unwrap_or("/tools/cereal/community"))),
        403 => Err(anyhow!("Sign in to the Toolbay app first, then send your feedback.")),
        409 => Err(anyhow!("You already sent this today. Thanks!")),
        429 => Err(anyhow!("That's a lot of feedback for one hour. Please try again later.")),
        _ => Err(anyhow!("{}", v["error"].as_str().unwrap_or("Couldn't send your feedback. Please try again."))),
    }
}
