//! App-wide state shared by commands, the recorder and background jobs.

use crate::db::Db;
use crate::settings::{self, Settings};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use toolbay_engine::server::Server;
use toolbay_engine::Store;

#[derive(Clone)]
pub struct St {
    /// None when running headless (--process / --bench).
    pub app: Option<AppHandle>,
    pub settings: Arc<RwLock<Settings>>,
    pub db: Arc<Db>,
    pub store: Store,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub whisper: Arc<Mutex<Option<Server>>>,
    pub llama: Arc<Mutex<Option<Server>>>,
    /// Starting an engine is slow; never start two at once.
    pub engine_lock: Arc<tokio::sync::Mutex<()>>,
    /// One meeting is processed at a time (they share the engines).
    pub process_lock: Arc<tokio::sync::Mutex<()>>,
    pub session: Arc<Mutex<Option<crate::recorder::Session>>>,
}

impl St {
    pub fn new(app: Option<AppHandle>, config_dir: PathBuf, data_dir: PathBuf, bundled: Option<PathBuf>) -> anyhow::Result<St> {
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        migrate_from_toolbay_notes(&data_dir);
        let db = Db::open(&data_dir.join("notes.sqlite"))?;
        Ok(St {
            app,
            settings: Arc::new(RwLock::new(settings::load(&config_dir))),
            db: Arc::new(db),
            store: Store::new(&data_dir, bundled),
            config_dir,
            data_dir,
            whisper: Default::default(),
            llama: Default::default(),
            engine_lock: Default::default(),
            process_lock: Default::default(),
            session: Default::default(),
        })
    }

    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Some(a) = &self.app {
            let _ = a.emit(event, payload);
        }
    }

    pub fn meetings_dir(&self) -> PathBuf {
        self.data_dir.join("meetings")
    }

    pub fn save_settings(&self, s: Settings) -> anyhow::Result<()> {
        settings::save(&self.config_dir, &s)?;
        *self.settings.write() = s;
        self.emit("settings-changed", ());
        Ok(())
    }

    pub fn meeting_changed(&self, id: &str) {
        if let Ok(Some(m)) = self.db.meeting(id) {
            self.emit("meeting-updated", m);
        }
    }

    /// The installer's bundled/ folder (starter models and engines).
    pub fn bundled_dir(app: &AppHandle) -> Option<PathBuf> {
        app.path().resource_dir().ok().map(|d| d.join("bundled")).filter(|d| d.exists())
    }
}

/// Cereal was called "Toolbay Notes" (app id ai.toolbay.notes) before 0.2. Move its
/// notes, recordings, settings and models over once, the first time Cereal starts.
fn migrate_from_toolbay_notes(data_dir: &std::path::Path) {
    if data_dir.join("notes.sqlite").exists() {
        return;
    }
    let Some(old) = data_dir.parent().map(|p| p.join("ai.toolbay.notes")) else { return };
    if !old.join("notes.sqlite").exists() {
        return;
    }
    for name in ["notes.sqlite", "notes.sqlite-wal", "notes.sqlite-shm", "settings.json", "meetings", "models", "engines"] {
        let (a, b) = (old.join(name), data_dir.join(name));
        if a.exists() && !b.exists() && std::fs::rename(&a, &b).is_err() && a.is_file() {
            let _ = std::fs::copy(&a, &b);
        }
    }
    log::info!("moved data from Toolbay Notes ({})", old.display());
}
