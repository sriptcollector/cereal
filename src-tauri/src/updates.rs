//! Silent updates for standalone installs. Notes checks GitHub Releases at start and
//! every 6 hours, downloads a signed update in the background, and installs it only
//! when no meeting is being recorded or processed, so it never interrupts a call.
//! While it installs nothing can start recording, and any session is finished
//! properly before Cereal restarts.
//! When launched from the Toolbay hub this is off: the hub updates Notes itself.

use crate::state::St;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

/// An update is about to install: `recorder::start` refuses (the recording
/// would be cut off by the restart).
pub static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn start(st: &St) {
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            if let Err(e) = check_and_install(&st).await {
                log::info!("update check: {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

fn busy(st: &St) -> bool {
    st.session.lock().is_some() || st.process_lock.try_lock().is_err()
}

async fn check_and_install(st: &St) -> anyhow::Result<()> {
    let Some(app) = st.app.clone() else { return Ok(()) };
    let Some(update) = app.updater()?.check().await? else { return Ok(()) };
    log::info!("update {} available", update.version);
    let bytes = update.download(|_, _| {}, || {}).await?;
    loop {
        if !busy(st) {
            // Block new recordings, give one that was already starting time to
            // show up (opening the devices takes up to 10 s), then check again.
            INSTALLING.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(12)).await;
            if !busy(st) {
                break;
            }
            INSTALLING.store(false, Ordering::SeqCst);
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    log::info!("installing update {}", update.version);
    // Never leave a half-written recording (on Windows the installer exits Cereal).
    crate::hub::finish_recording(&app);
    if let Err(e) = update.install(bytes) {
        INSTALLING.store(false, Ordering::SeqCst);
        return Err(e.into());
    }
    crate::hub::restart(&app);
}
