//! Settings and custom templates follow the user to their other computers through
//! the Toolbay account (toolbay.ai /api/v1/me/tools/cereal/sync). Only when
//! the person is signed in to the Toolbay app AND turned sync on (off by
//! default; every request below checks it), and only these small items. They
//! are stored on toolbay.ai where Toolbay's server can read them (not end-to-end
//! encrypted). Recordings, transcripts, notes, API keys and calendar (iCal)
//! links are never synced.
//!
//! Keys: "settings" (the Synced subset), "templates/<id>" (one per custom template).
//! One request at a time; a change made here stays pending (in the database)
//! until the account has it, and always goes out with the value it has at send time.

use crate::db::Template;
use crate::state::St;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

const API: &str = "https://toolbay.ai/api/v1/me/tools/cereal/sync";
/// Settings are sent this long after the last change (typing in a field).
const DEBOUNCE: Duration = Duration::from_millis(1500);

/// One sync request at a time (pulls and pushes), so an older value can never
/// reach the account after a newer one.
static LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(Default::default);
/// Changes per key since Cereal started. A push only clears a key's pending mark
/// when no newer change came in while it was being sent.
static EDITS: Lazy<Mutex<HashMap<String, u64>>> = Lazy::new(Default::default);

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

/// Keys changed on this computer and not yet in the account. Kept in the
/// database, so a change made offline is still sent after a restart.
fn pending(st: &St) -> BTreeSet<String> {
    st.db.kv("sync.pending").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_pending(st: &St, p: &BTreeSet<String>) {
    let _ = st.db.set_kv("sync.pending", &serde_json::to_string(p).unwrap_or_default());
}

/// Marks `key` as changed here. Returns the change number.
fn mark(st: &St, key: &str) -> u64 {
    let mut e = EDITS.lock();
    let n = e.entry(key.to_string()).or_insert(0);
    *n += 1;
    let mut p = pending(st);
    if p.insert(key.to_string()) {
        save_pending(st, &p);
    }
    *n
}

fn edit_no(key: &str) -> u64 {
    EDITS.lock().get(key).copied().unwrap_or(0)
}

/// `key` reached the account as of change `n`: unmark it unless it changed again since.
fn sent(st: &St, key: &str, n: u64) {
    let e = EDITS.lock();
    if e.get(key).copied().unwrap_or(0) == n {
        let mut p = pending(st);
        if p.remove(key) {
            save_pending(st, &p);
        }
    }
}

/// The signed-in Toolbay account id and a token that works: the Toolbay app's
/// sign-in first, then the hub's (like Toolbay Flow's `account()`). None when
/// nobody is signed in.
async fn account() -> Result<Option<(String, String)>> {
    let mut offline = None;
    for t in crate::account::tokens() {
        match reqwest::Client::new().get(format!("{}/api/v1/me", crate::account::BASE)).bearer_auth(&t).timeout(Duration::from_secs(15)).send().await {
            Ok(r) if r.status().is_success() => {
                let v: Value = r.json().await?;
                let u = v.get("user").cloned().unwrap_or(v);
                let id = match &u["id"] {
                    Value::String(s) => s.clone(),
                    Value::Null => continue,
                    x => x.to_string(),
                };
                return Ok(Some((t, id)));
            }
            Ok(_) => {} // revoked or expired: try the next one
            Err(e) => offline = Some(e),
        }
    }
    match offline {
        Some(e) => Err(e.into()),
        None => Ok(None),
    }
}

/// The token for this round, or None when sync is off or nobody is signed in.
/// A different account than last time starts over (like Flow's `round()`): the
/// revision and versions are reset so that account's copy is read in full and wins.
async fn signed_in(st: &St) -> Result<Option<String>> {
    if !st.settings.read().sync_enabled {
        return Ok(None);
    }
    let Some((tok, id)) = account().await? else { return Ok(None) };
    let last = st.db.kv("sync.account");
    if last.as_deref() != Some(id.as_str()) {
        save_versions(st, &HashMap::new());
        let _ = st.db.set_kv("sync.rev", "0");
        // Changes made for another account don't go into this one. (No account
        // saved yet means an older version synced with this one: keep them.)
        if last.is_some() {
            log::info!("sync: signed in to a different Toolbay account, starting over");
            let _e = EDITS.lock();
            save_pending(st, &BTreeSet::new());
        }
        let _ = st.db.set_kv("sync.account", &id);
    }
    Ok(Some(tok))
}

/// Pulls changes from the account and applies them. Lock held.
async fn pull_with(st: &St, tok: &str) -> Result<()> {
    let since: i64 = st.db.kv("sync.rev").and_then(|s| s.parse().ok()).unwrap_or(0);
    let url = if since > 0 { format!("{API}?since={since}") } else { API.to_string() };
    let r = reqwest::Client::new().get(url).bearer_auth(tok).timeout(Duration::from_secs(15)).send().await?;
    if !r.status().is_success() {
        return Err(anyhow!("sync GET {}", r.status()));
    }
    let v: Value = r.json().await?;
    let mut vers = versions(st);
    if let Some(items) = v["items"].as_object() {
        for (key, item) in items {
            let ver = item["version"].as_i64().unwrap_or(0);
            vers.insert(key.clone(), ver);
            // A change made here is waiting to be sent: it goes on top of this
            // version instead of being replaced by it.
            if pending(st).contains(key) {
                continue;
            }
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

/// What `key` holds on this computer right now (None = deleted).
fn current(st: &St, key: &str) -> Result<Option<Value>> {
    if key == "settings" {
        return Ok(Some(serde_json::to_value(st.settings.read().synced())?));
    }
    let id = key.strip_prefix("templates/").ok_or_else(|| anyhow!("unknown sync key {key}"))?;
    Ok(st.db.custom_templates()?.into_iter().find(|t| t.id == id).map(|t| serde_json::to_value(t).unwrap_or_default()))
}

/// Sends `key` as it is at send time (never a value captured earlier). On a
/// version conflict, reads the account first, then sends again on top of it. Lock held.
async fn push_with(st: &St, tok: &str, key: &str) -> Result<()> {
    for attempt in 0..2 {
        let n = edit_no(key);
        let value = current(st, key)?;
        let mut vers = versions(st);
        let base = *vers.get(key).unwrap_or(&0);
        let change = match &value {
            Some(v) => json!({"key": key, "baseVersion": base, "value": v}),
            None => json!({"key": key, "baseVersion": base, "deleted": true}),
        };
        let r = reqwest::Client::new().put(API).bearer_auth(tok).json(&json!({"changes": [change]})).timeout(Duration::from_secs(15)).send().await?;
        if !r.status().is_success() {
            return Err(anyhow!("sync PUT {}", r.status()));
        }
        let v: Value = r.json().await?;
        if let Some(a) = v["applied"][key]["version"].as_i64() {
            vers.insert(key.to_string(), a);
            save_versions(st, &vers);
            sent(st, key, n);
            return Ok(());
        }
        // Conflict: another computer changed it. Read the account first (the
        // change waiting here isn't replaced), then send this computer's newest
        // value on top of the account's version.
        if attempt == 0 {
            pull_with(st, tok).await?;
        }
        if let Some(server_ver) = v["conflicts"][key]["version"].as_i64() {
            let mut vers = versions(st);
            if vers.get(key).copied().unwrap_or(0) < server_ver {
                vers.insert(key.to_string(), server_ver);
                save_versions(st, &vers);
            }
        }
    }
    Err(anyhow!("sync conflict on {key}"))
}

/// Sends `key` now (one request at a time). If that fails it stays pending and
/// goes with the next round.
async fn send(st: &St, key: &str) {
    let _g = LOCK.lock().await;
    let r = async {
        match signed_in(st).await? {
            Some(tok) => push_with(st, &tok, key).await,
            None => Ok(()),
        }
    }
    .await;
    if let Err(e) = r {
        log::warn!("sync {key}: {e:#} (sent with the next round)");
    }
}

/// Settings changed here: sent once they stop changing for a moment, with the
/// values they have then.
pub fn push_settings(st: &St) {
    if !available(st) {
        return;
    }
    let n = mark(st, "settings");
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DEBOUNCE).await;
        if edit_no("settings") != n {
            return; // a newer change sends them
        }
        send(&st, "settings").await;
    });
}

/// A custom template was saved or deleted here.
pub fn push_template(st: &St, id: &str) {
    if !available(st) {
        return;
    }
    let key = format!("templates/{id}");
    mark(st, &key);
    let st = st.clone();
    tauri::async_runtime::spawn(async move { send(&st, &key).await });
}

/// One round: changes made here and not sent yet (offline, say) go first, then
/// the account's changes are read.
async fn round(st: &St) -> Result<()> {
    let _g = LOCK.lock().await;
    let Some(tok) = signed_in(st).await? else { return Ok(()) };
    for key in pending(st) {
        if let Err(e) = push_with(st, &tok, &key).await {
            log::warn!("sync {key}: {e:#}");
        }
    }
    pull_with(st, &tok).await
}

/// On start and every 5 minutes. Does nothing (no network) while sync is off
/// or nobody is signed in.
pub fn start(st: St) {
    tauri::async_runtime::spawn(async move {
        loop {
            if available(&st) {
                if let Err(e) = round(&st).await {
                    log::warn!("sync: {e:#}");
                }
            }
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });
}

/// The person just turned sync on: read the account copy, then send this
/// computer's settings (which also replaces any calendar links an older version
/// put in the account). Templates sync as they are saved.
pub fn turned_on(st: &St) {
    if !available(st) {
        return;
    }
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        {
            let _g = LOCK.lock().await;
            let r = async {
                let Some(tok) = signed_in(&st).await? else { return Ok(()) };
                pull_with(&st, &tok).await
            }
            .await;
            if let Err(e) = r {
                log::warn!("sync pull: {e:#}");
            }
        }
        mark(&st, "settings");
        send(&st, "settings").await;
    });
}

/// Deletes everything Cereal synced from the person's Toolbay account (the
/// server keeps nothing). This computer keeps its copy.
pub async fn delete_remote(st: &St) -> Result<()> {
    let _g = LOCK.lock().await;
    let (tok, _) = account()
        .await
        .map_err(|_| anyhow!("Couldn't reach toolbay.ai. Check your internet connection and try again."))?
        .ok_or_else(|| anyhow!("Sign in to the Toolbay app first."))?;
    let r = reqwest::Client::new()
        .delete(API)
        .bearer_auth(&tok)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| anyhow!("Couldn't reach toolbay.ai. Check your internet connection and try again."))?;
    if !r.status().is_success() {
        return Err(anyhow!("Couldn't delete the synced copy ({}). Try again in a minute.", r.status()));
    }
    let _ = st.db.set_kv("sync.versions", "{}");
    let _ = st.db.set_kv("sync.rev", "0");
    let _e = EDITS.lock();
    save_pending(st, &BTreeSet::new());
    Ok(())
}
