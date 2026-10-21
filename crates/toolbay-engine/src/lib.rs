//! toolbay-engine: the local AI engine shared by Toolbay tools (Toolbay Notes, and
//! Toolbay Flow can use it too). Started from Toolbay Flow's engines.rs / models.rs /
//! llm.rs and made app-independent: no Tauri types, progress goes through a callback.
//!
//! Everything runs on 127.0.0.1. The only network traffic is model/engine downloads
//! (Hugging Face, GitHub Releases) and, if the user adds their own key, the cloud
//! engine they chose.

pub mod catalog;
pub mod chat;
pub mod diar;
pub mod hw;
pub mod server;
pub mod store;
pub mod stt;
pub mod wav;

pub use catalog::{model, pick_engine, EngineInfo, ModelInfo, ENGINES, MODELS};
pub use hw::{detect, Hardware};
pub use store::Store;
