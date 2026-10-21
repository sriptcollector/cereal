//! Silent updates for standalone installs. Notes checks GitHub Releases at start and
//! every 6 hours, downloads a signed update in the background, and installs it only
//! when no meeting is being recorded or processed, so it never interrupts a call.
//! When launched from the Toolbay hub this is off: the hub updates Notes itself.

use crate::state::St;
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

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

async fn check_and_install(st: &St) -> anyhow::Result<()> {
    let Some(app) = st.app.clone() else { return Ok(()) };
    let Some(update) = app.updater()?.check().await? else { return Ok(()) };
    log::info!("update {} available", update.version);
    let bytes = update.download(|_, _| {}, || {}).await?;
    loop {
        let busy = st.session.lock().is_some() || st.process_lock.try_lock().is_err();
        if !busy {
            break;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    log::info!("installing update {}", update.version);
    update.install(bytes)?;
    app.restart();
}
