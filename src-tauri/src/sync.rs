//! Settings and custom templates follow the user to their other computers through
//! the Toolbay account (toolbay.ai /api/v1/me/tools/cereal/sync). Only when
//! the person is signed in to the Toolbay app, and only these small items:
//! recordings, transcripts and notes never leave this computer.
//!
//! Keys: "settings" (the Synced subset), "templates/<id>" (one per custom template).

use crate::db::Template;
use crate::state::St;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

const API: &str = "https://toolbay.ai/api/v1/me/tools/cereal/sync";

fn token() -> Option<String> {
    crate::account::token()
}

pub fn available(st: &St) -> bool {
    token().is_some() && st.settings.read().sync_enabled
}

/// Versions we last saw per key, and the server revision.
fn versions(st: &St) -> HashMap<String, i64> {
    st.db.kv("sync.versions").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_versions(st: &St, v: &HashMap<String, i64>) {
    let _ = st.db.set_kv("sync.versions", &serde_json::to_string(v).unwrap_or_default());
}

/// Pulls changes from the account and applies them.
pub async fn pull(st: &St) -> Result<()> {
    let Some(tok) = token() else { return Ok(()) };
    if !st.settings.read().sync_enabled {
        return Ok(());
    }
    let since: i64 = st.db.kv("sync.rev").and_then(|s| s.parse().ok()).unwrap_or(0);
    let url = if since > 0 { format!("{API}?since={since}") } else { API.to_string() };
    let r = reqwest::Client::new().get(url).bearer_auth(&tok).timeout(Duration::from_secs(15)).send().await?;
    if !r.status().is_success() {
        return Err(anyhow!("sync GET {}", r.status()));
    }
    let v: Value = r.json().await?;
    let mut vers = versions(st);
    if let Some(items) = v["items"].as_object() {
        for (key, item) in items {
            let ver = item["version"].as_i64().unwrap_or(0);
            vers.insert(key.clone(), ver);
            let deleted = item["deleted"].as_bool().unwrap_or(false);
            if key == "settings" && !deleted {
                if let Ok(s) = serde_json::from_value::<crate::settings::Synced>(item["value"].clone()) {
                    let mut cur = st.settings.read().clone();
                    cur.apply_synced(&s);
                    settings_no_push(st, cur);
                }
            } else if let Some(id) = key.strip_prefix("templates/") {
                if deleted {
                    let _ = st.db.delete_template(id);
                } else if let Ok(mut t) = serde_json::from_value::<Template>(item["value"].clone()) {
                    t.id = id.to_string();
                    let _ = st.db.save_template(&t);
                }
            }
        }
        if !items.is_empty() {
            st.emit("templates-changed", ());
        }
    }
    save_versions(st, &vers);
    if let Some(rev) = v["rev"].as_i64() {
        let _ = st.db.set_kv("sync.rev", &rev.to_string());
    }
    Ok(())
}

fn settings_no_push(st: &St, s: crate::settings::Settings) {
    let _ = crate::settings::save(&st.config_dir, &s);
    *st.settings.write() = s;
    st.emit("settings-changed", ());
}

/// Pushes one change (value None = delete). Retries once on a version conflict.
pub async fn push(st: &St, key: &str, value: Option<Value>) -> Result<()> {
    let Some(tok) = token() else { return Ok(()) };
    if !st.settings.read().sync_enabled {
        return Ok(());
    }
    for _ in 0..2 {
        let mut vers = versions(st);
        let base = *vers.get(key).unwrap_or(&0);
        let change = match &value {
            Some(v) => json!({"key": key, "baseVersion": base, "value": v}),
            None => json!({"key": key, "baseVersion": base, "deleted": true}),
        };
        let r = reqwest::Client::new().put(API).bearer_auth(&tok).json(&json!({"changes": [change]})).timeout(Duration::from_secs(15)).send().await?;
        if !r.status().is_success() {
            return Err(anyhow!("sync PUT {}", r.status()));
        }
        let v: Value = r.json().await?;
        if let Some(a) = v["applied"][key]["version"].as_i64() {
            vers.insert(key.to_string(), a);
            save_versions(st, &vers);
            return Ok(());
        }
        // Conflict: this computer's change is the newest, so retry on top of the server's.
        let server_ver = v["conflicts"][key]["version"].as_i64().unwrap_or(0);
        vers.insert(key.to_string(), server_ver);
        save_versions(st, &vers);
    }
    Err(anyhow!("sync conflict on {key}"))
}

pub fn push_settings(st: &St) {
    if !available(st) {
        return;
    }
    let st = st.clone();
    let v = serde_json::to_value(st.settings.read().synced()).unwrap_or_default();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = push(&st, "settings", Some(v)).await {
            log::warn!("sync settings: {e:#}");
        }
    });
}

pub fn push_template(st: &St, t: Option<&Template>, id: &str) {
    if !available(st) {
        return;
    }
    let st = st.clone();
    let key = format!("templates/{id}");
    let v = t.map(|t| serde_json::to_value(t).unwrap_or_default());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = push(&st, &key, v).await {
            log::warn!("sync template: {e:#}");
        }
    });
}

/// On start and every 5 minutes.
pub fn start(st: St) {
    if token().is_none() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = pull(&st).await {
                log::warn!("sync pull: {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });
}
