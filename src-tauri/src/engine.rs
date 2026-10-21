//! Running the local AI: picks models for this computer, downloads what is missing
//! in the background, and starts the whisper.cpp / llama.cpp servers on demand.

use crate::state::St;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use toolbay_engine::chat::{Llm, Provider};
use toolbay_engine::server::{spawn_server, threads, wait_ready};
use toolbay_engine::{catalog, Hardware};

/// Shipped inside the installer, so recording and transcripts work right away.
pub const STARTER_SPEECH: &str = "base.en-q5";

#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub tier: &'static str,
    pub speech_model: String,
    pub llm_model: String,
    pub final_pass: bool,
    pub why: String,
}

/// Meeting notes don't need to be instant like dictation, so CPU-only computers still
/// get a capable notes model; speech stays small enough to keep up live.
pub fn recommend(h: &Hardware, language: &str) -> Choice {
    let english = language == "en";
    let ml = |en: &str, multi: &str| if english { en.to_string() } else { multi.to_string() };
    let nvidia = h.gpu_vendor == "nvidia";
    let apple = h.gpu_vendor == "apple";
    let other_gpu = matches!(h.gpu_vendor.as_str(), "amd" | "intel") && h.vram_mb >= 3500;
    let gpu_name = if h.gpu_name.is_empty() { "GPU".to_string() } else { h.gpu_name.clone() };
    if (nvidia && h.vram_mb >= 6000) || (apple && h.ram_mb >= 15000) {
        Choice { tier: "fast-gpu", speech_model: "large-v3-turbo-q5".into(), llm_model: "qwen3-4b-instruct".into(), final_pass: true, why: format!("{} with {} GB", gpu_name, h.vram_mb / 1024) }
    } else if (nvidia && h.vram_mb >= 3000) || apple {
        Choice { tier: "gpu", speech_model: "large-v3-turbo-q5".into(), llm_model: "qwen3-1.7b".into(), final_pass: true, why: format!("{} with {} GB", gpu_name, h.vram_mb / 1024) }
    } else if other_gpu {
        Choice { tier: "gpu-notes", speech_model: ml("small.en-q5", "small-q5"), llm_model: "qwen3-4b-instruct".into(), final_pass: false, why: format!("{} (notes on GPU, speech on CPU)", gpu_name) }
    } else if h.threads >= 8 && h.ram_mb >= 12000 {
        // Notes are written after the call, so a CPU can take the better notes model.
        Choice { tier: "cpu", speech_model: ml("small.en-q5", "small-q5"), llm_model: "qwen3-4b-instruct".into(), final_pass: false, why: format!("{}-thread CPU, no fast GPU", h.threads) }
    } else if h.threads >= 8 && h.ram_mb >= 7000 {
        Choice { tier: "cpu", speech_model: ml("small.en-q5", "small-q5"), llm_model: "qwen3-1.7b".into(), final_pass: false, why: format!("{}-thread CPU, no fast GPU", h.threads) }
    } else {
        Choice { tier: "light", speech_model: ml("base.en-q5", "base-q5"), llm_model: "qwen3-1.7b".into(), final_pass: false, why: format!("{}-thread CPU, {} GB memory", h.threads, h.ram_mb / 1024) }
    }
}

fn model_path(st: &St, id: &str) -> Option<PathBuf> {
    catalog::model(id).and_then(|m| st.store.find_model(m))
}

/// The speech model to use now: the chosen one if present, else a starter.
pub fn speech_model(st: &St) -> Option<(String, PathBuf)> {
    let (want, lang) = {
        let s = st.settings.read();
        (s.speech_model.clone(), s.language.clone())
    };
    if let Some(p) = model_path(st, &want) {
        return Some((want, p));
    }
    let order: &[&str] = if lang == "en" { &[STARTER_SPEECH, "base-q5", "small.en-q5", "small-q5"] } else { &["base-q5", STARTER_SPEECH, "small-q5", "small.en-q5"] };
    order.iter().find_map(|id| model_path(st, id).map(|p| (id.to_string(), p)))
}

pub async fn ensure_whisper(st: &St) -> Result<String> {
    let _g = st.engine_lock.lock().await;
    let use_gpu = st.settings.read().use_gpu;
    let (model_id, model) = speech_model(st).ok_or_else(|| anyhow!("Cereal is still getting its speech model. Try again in a moment."))?;
    // Prefer the GPU engine once it's on disk; until then use the bundled CPU one.
    let mut eng = toolbay_engine::pick_engine("whisper", use_gpu).ok_or_else(|| anyhow!("No speech engine for this computer"))?;
    let mut exe = st.store.find_engine(eng);
    if exe.is_none() {
        if let Some(cpu) = toolbay_engine::pick_engine("whisper", false) {
            if let Some(p) = st.store.find_engine(cpu) {
                eng = cpu;
                exe = Some(p);
            }
        }
    }
    let key = format!("{}|{}", eng.id, model_id);
    {
        let mut w = st.whisper.lock();
        if let Some(s) = w.as_mut() {
            if s.key == key && s.alive() {
                return Ok(s.base());
            }
        }
        *w = None;
    }
    let exe = match exe {
        Some(e) => e,
        None => st.store.ensure_engine(eng, &progress(st)).await?,
    };
    let mut args = vec!["-m".to_string(), model.to_string_lossy().to_string(), "-t".into(), threads().to_string(), "-sns".into()];
    if !eng.gpu {
        args.push("-ng".into());
    }
    let srv = spawn_server(&exe, &args, key, &st.data_dir.join("whisper-server.log"))?;
    let base = srv.base();
    *st.whisper.lock() = Some(srv);
    wait_ready(&base, "/", Duration::from_secs(90)).await?;
    log::info!("speech engine ready: {} ({})", eng.id, model_id);
    Ok(base)
}

pub fn llm_ctx(st: &St) -> usize {
    let s = st.settings.read();
    if s.llm_model == "qwen3-4b-instruct" { 16_384 } else { 12_288 }
}

pub async fn ensure_llama(st: &St) -> Result<String> {
    // The speech engine isn't needed while notes are written; on a GPU, free its
    // memory so the notes model fits entirely (otherwise it spills and gets ~10x slower).
    // During a recording the live transcript still needs it, so keep it then.
    if st.settings.read().use_gpu && st.session.lock().is_none() {
        *st.whisper.lock() = None;
    }
    let _g = st.engine_lock.lock().await;
    let (model_id, use_gpu) = {
        let s = st.settings.read();
        (s.llm_model.clone(), s.use_gpu)
    };
    // Chosen model, or any notes model already on disk while the chosen one downloads.
    let (model_id, model) = model_path(st, &model_id)
        .map(|p| (model_id.clone(), p))
        .or_else(|| ["qwen3-4b-instruct", "qwen3-1.7b"].iter().find_map(|id| model_path(st, id).map(|p| (id.to_string(), p))))
        .ok_or_else(|| anyhow!("The notes AI model is still downloading. Your transcript is saved; notes will be written as soon as it's ready."))?;
    let eng = toolbay_engine::pick_engine("llama", use_gpu).ok_or_else(|| anyhow!("No notes engine for this computer"))?;
    let key = format!("{}|{}", eng.id, model_id);
    {
        let mut l = st.llama.lock();
        if let Some(s) = l.as_mut() {
            if s.key == key && s.alive() {
                return Ok(s.base());
            }
        }
        *l = None;
    }
    let exe = match st.store.find_engine(eng) {
        Some(e) => e,
        None => st.store.ensure_engine(eng, &progress(st)).await?,
    };
    let ctx = if model_id == "qwen3-4b-instruct" { 16_384 } else { 12_288 };
    let mut args = vec![
        "-m".to_string(),
        model.to_string_lossy().to_string(),
        "-c".into(),
        ctx.to_string(),
        "-np".into(),
        "1".into(),
        "-t".into(),
        // On the CPU, use the physical cores (hyper-threads don't help).
        if eng.gpu { threads().to_string() } else { (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8) / 2).clamp(4, 16).to_string() },
        "--no-webui".into(),
        "--jinja".into(),
        "-ngl".into(),
        if eng.gpu { "99".into() } else { "0".into() },
    ];
    if eng.gpu {
        // 8-bit KV cache: half the video memory for long meetings, same quality.
        args.extend(["-fa".into(), "on".into(), "-ctk".into(), "q8_0".into(), "-ctv".into(), "q8_0".into()]);
    }
    let srv = spawn_server(&exe, &args, key, &st.data_dir.join("llama-server.log"))?;
    let base = srv.base();
    *st.llama.lock() = Some(srv);
    wait_ready(&base, "/health", Duration::from_secs(180)).await?;
    log::info!("notes engine ready: {} ({})", eng.id, model_id);
    Ok(base)
}

/// The language model to use (local unless the user added their own key).
pub async fn llm(st: &St) -> Result<Llm> {
    let (provider, cloud) = {
        let s = st.settings.read();
        (s.llm_provider, s.llm_cloud.clone())
    };
    if provider != Provider::Local && cloud.api_key.trim().is_empty() {
        return Err(anyhow!("Add your API key in Settings > AI models, or switch back to the local model."));
    }
    let local_base = if provider == Provider::Local { Some(ensure_llama(st).await?) } else { None };
    Ok(Llm { provider, local_base, cloud, temperature: 0.2 })
}

pub struct DiarFiles {
    pub exe: PathBuf,
    pub segmentation: PathBuf,
    pub embedding: PathBuf,
}

pub async fn diar_files(st: &St) -> Result<DiarFiles> {
    let eng = toolbay_engine::pick_engine("sherpa", false).ok_or_else(|| anyhow!("No speaker engine for this computer"))?;
    let p = progress(st);
    let exe = st.store.ensure_engine(eng, &p).await?;
    let seg = st.store.ensure_model(catalog::model("pyannote-seg-3").unwrap(), &p).await?;
    let emb = st.store.ensure_model(catalog::model("campplus-en").unwrap(), &p).await?;
    Ok(DiarFiles { exe, segmentation: seg, embedding: emb })
}

#[derive(Clone, Serialize)]
struct Prog {
    id: String,
    done: u64,
    total: u64,
    stage: String,
}

pub fn progress(st: &St) -> impl Fn(&str, u64, u64, &str) + Send + Sync + 'static {
    let st = st.clone();
    move |id: &str, done: u64, total: u64, stage: &str| st.emit("download-progress", Prog { id: id.into(), done, total, stage: stage.into() })
}

#[derive(Clone, Serialize)]
pub struct ModelStatus {
    pub busy: bool,
    pub message: String,
}

static SETTING_UP: AtomicBool = AtomicBool::new(false);

/// In Auto mode: check the hardware, pick models, and download what is missing in
/// the background. Safe to call often.
pub fn auto_setup(st: &St) {
    if SETTING_UP.swap(true, Ordering::SeqCst) {
        return;
    }
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        let hw = toolbay_engine::detect();
        let mut s = st.settings.read().clone();
        if s.model_mode == "auto" {
            let c = recommend(&hw, &s.language);
            if s.speech_model != c.speech_model || s.llm_model != c.llm_model || s.hw_fingerprint != hw.fingerprint() {
                s.speech_model = c.speech_model.clone();
                s.llm_model = c.llm_model.clone();
                s.final_pass = c.final_pass;
                s.hw_fingerprint = hw.fingerprint();
                s.auto_reason = c.why.clone();
                let _ = st.save_settings(s.clone());
            }
        }
        let emit = |busy: bool, message: &str| st.emit("model-status", ModelStatus { busy, message: message.into() });
        let p = progress(&st);
        let res: Result<()> = async {
            // Speaker models are small and ship in the installer; fetch them if not.
            let _ = diar_files(&st).await;
            let speech = catalog::model(&s.speech_model);
            let eng = toolbay_engine::pick_engine("whisper", s.use_gpu);
            let need_speech = speech.map(|m| st.store.find_model(m).is_none()).unwrap_or(false) || eng.map(|e| st.store.find_engine(e).is_none()).unwrap_or(false);
            if need_speech {
                emit(true, "Getting a better speech model for this computer…");
                if let Some(e) = eng {
                    st.store.ensure_engine(e, &p).await?;
                }
                if let Some(m) = speech {
                    st.store.ensure_model(m, &p).await?;
                }
                // Next transcription picks up the new model.
                *st.whisper.lock() = None;
            }
            if s.llm_provider == Provider::Local {
                if let Some(m) = catalog::model(&s.llm_model) {
                    if st.store.find_model(m).is_none() {
                        emit(true, "Getting the AI model that writes your notes…");
                        if let Some(e) = toolbay_engine::pick_engine("llama", s.use_gpu) {
                            st.store.ensure_engine(e, &p).await?;
                        }
                        st.store.ensure_model(m, &p).await?;
                        *st.llama.lock() = None;
                    }
                }
            }
            Ok(())
        }
        .await;
        SETTING_UP.store(false, Ordering::SeqCst);
        match res {
            Ok(()) => emit(false, ""),
            Err(e) => {
                log::warn!("model setup: {e:#}");
                emit(false, "Couldn't download the AI models yet. Recording still works; Cereal will retry when you're online.");
            }
        }
    });
}

/// Are the notes model files on disk (or is a cloud model set up)?
pub fn notes_ready(st: &St) -> bool {
    let s = st.settings.read();
    if s.llm_provider != Provider::Local {
        return !s.llm_cloud.api_key.trim().is_empty();
    }
    ["qwen3-4b-instruct", "qwen3-1.7b"].iter().any(|id| catalog::model(id).and_then(|m| st.store.find_model(m)).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiers() {
        let h = Hardware { gpu_vendor: "nvidia".into(), gpu_name: "RTX".into(), vram_mb: 12_000, ram_mb: 32_000, threads: 16 };
        assert_eq!(recommend(&h, "auto").tier, "fast-gpu");
        let h = Hardware { threads: 4, ram_mb: 8_000, ..Default::default() };
        assert_eq!(recommend(&h, "en").speech_model, "base.en-q5");
    }
}
