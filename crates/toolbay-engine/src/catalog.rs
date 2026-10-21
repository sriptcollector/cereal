//! What can be downloaded: speech models (whisper.cpp), language models (GGUF for
//! llama.cpp), speaker models (sherpa-onnx) and the engine programs that run them.
//! Shared by every Toolbay tool, so the same file names and folders are used
//! everywhere and a model downloaded by one tool can be reused by another.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// "speech" | "llm" | "speaker"
    pub kind: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub english_only: bool,
    pub note: &'static str,
}

macro_rules! speech {
    ($id:expr, $label:expr, $file:expr, $mb:expr, $en:expr, $note:expr) => {
        ModelInfo { id: $id, label: $label, kind: "speech", file: $file, url: concat!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/", $file), size_mb: $mb, english_only: $en, note: $note }
    };
}

const SHERPA: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/";

pub const MODELS: &[ModelInfo] = &[
    speech!("base.en-q5", "Base (English, compact)", "ggml-base.en-q5_1.bin", 60, true, "Starter model, fast on any computer"),
    speech!("base-q5", "Base (multilingual, compact)", "ggml-base-q5_1.bin", 60, false, "Starter model, 99 languages"),
    speech!("small.en-q5", "Small (English, compact)", "ggml-small.en-q5_1.bin", 190, true, "Good accuracy on a CPU"),
    speech!("small-q5", "Small (multilingual, compact)", "ggml-small-q5_1.bin", 190, false, "Good accuracy on a CPU, 99 languages"),
    speech!("tiny.en", "Tiny (English)", "ggml-tiny.en.bin", 78, true, "Fastest, lowest accuracy"),
    speech!("tiny", "Tiny (multilingual)", "ggml-tiny.bin", 78, false, "Fastest, lowest accuracy"),
    speech!("base.en", "Base (English)", "ggml-base.en.bin", 148, true, "Fast on any CPU"),
    speech!("base", "Base (multilingual)", "ggml-base.bin", 148, false, "Fast on any CPU"),
    speech!("small.en", "Small (English)", "ggml-small.en.bin", 488, true, "Good accuracy, fine on modern CPUs"),
    speech!("small", "Small (multilingual)", "ggml-small.bin", 488, false, "Good accuracy, 99 languages"),
    speech!("medium.en", "Medium (English)", "ggml-medium.en-q5_0.bin", 539, true, "High accuracy, GPU recommended"),
    speech!("large-v3-turbo-q5", "Large v3 Turbo (quantized)", "ggml-large-v3-turbo-q5_0.bin", 574, false, "Best speed/accuracy with a GPU, 99 languages"),
    speech!("large-v3-turbo", "Large v3 Turbo", "ggml-large-v3-turbo.bin", 1625, false, "Best accuracy with a GPU, 99 languages"),
    ModelInfo {
        id: "qwen3-1.7b",
        label: "Qwen3 1.7B (fast)",
        kind: "llm",
        file: "Qwen3-1.7B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf",
        size_mb: 1107,
        english_only: false,
        note: "Light model for computers with little memory",
    },
    ModelInfo {
        id: "qwen3-4b-instruct",
        label: "Qwen3 4B Instruct 2507 (recommended)",
        kind: "llm",
        file: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        size_mb: 2497,
        english_only: false,
        note: "Best local quality, fast with a GPU",
    },
    ModelInfo {
        id: "pyannote-seg-3",
        label: "Speaker turns (pyannote segmentation 3.0)",
        kind: "speaker",
        file: "sherpa-onnx-pyannote-segmentation-3-0.onnx",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2",
        size_mb: 6,
        english_only: false,
        note: "Finds where one voice stops and another starts",
    },
    ModelInfo {
        id: "campplus-en",
        label: "Voice fingerprints (3D-Speaker CAM++)",
        kind: "speaker",
        file: "3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx",
        size_mb: 29,
        english_only: false,
        note: "Tells the other people in a call apart",
    },
];

#[allow(dead_code)]
const _S: &str = SHERPA;

pub fn model(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// Runtime programs. Windows uses official ggml-org / k2-fsa builds; macOS whisper is
/// built by toolbay-flow's CI (Metal) and attached to its "engines-v1" release.
#[derive(Clone, Debug, Serialize)]
pub struct EngineInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub exe: &'static str,
    pub gpu: bool,
}

const SHERPA_WIN: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-win-x64-shared-MD-Release-no-tts.tar.bz2";
const SHERPA_MAC: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-osx-universal2-shared-no-tts.tar.bz2";

#[cfg(windows)]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-cpu", label: "Speech engine (CPU)", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip", size_mb: 9, exe: "whisper-server.exe", gpu: false },
    EngineInfo { id: "whisper-cuda", label: "Speech engine (NVIDIA GPU)", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-win-cuda-12.4.0-x64.zip", size_mb: 685, exe: "whisper-server.exe", gpu: true },
    EngineInfo { id: "llama-cpu", label: "Notes engine (CPU)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-win-cpu-x64.zip", size_mb: 24, exe: "llama-server.exe", gpu: false },
    EngineInfo { id: "llama-vulkan", label: "Notes engine (GPU, Vulkan)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-win-vulkan-x64.zip", size_mb: 37, exe: "llama-server.exe", gpu: true },
    EngineInfo { id: "sherpa-cpu", label: "Speaker engine", url: SHERPA_WIN, size_mb: 19, exe: "sherpa-onnx-offline-speaker-diarization.exe", gpu: false },
];

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-metal", label: "Speech engine (Metal)", url: "https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-arm64.tar.gz", size_mb: 4, exe: "whisper-server", gpu: true },
    EngineInfo { id: "llama-metal", label: "Notes engine (Metal)", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-macos-arm64.tar.gz", size_mb: 16, exe: "llama-server", gpu: true },
    EngineInfo { id: "sherpa-cpu", label: "Speaker engine", url: SHERPA_MAC, size_mb: 30, exe: "sherpa-onnx-offline-speaker-diarization", gpu: false },
];

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
pub const ENGINES: &[EngineInfo] = &[
    EngineInfo { id: "whisper-mac-intel", label: "Speech engine", url: "https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-x86_64.tar.gz", size_mb: 4, exe: "whisper-server", gpu: true },
    EngineInfo { id: "llama-mac-intel", label: "Notes engine", url: "https://github.com/ggml-org/llama.cpp/releases/download/b11748/llama-b11748-bin-macos-x64.tar.gz", size_mb: 12, exe: "llama-server", gpu: true },
    EngineInfo { id: "sherpa-cpu", label: "Speaker engine", url: SHERPA_MAC, size_mb: 30, exe: "sherpa-onnx-offline-speaker-diarization", gpu: false },
];

#[cfg(not(any(windows, target_os = "macos")))]
pub const ENGINES: &[EngineInfo] = &[];

#[allow(dead_code)]
const _U: (&str, &str) = (SHERPA_WIN, SHERPA_MAC);

pub fn engine(id: &str) -> Option<&'static EngineInfo> {
    ENGINES.iter().find(|e| e.id == id)
}

pub fn has_nvidia() -> bool {
    #[cfg(windows)]
    {
        std::path::Path::new(r"C:\Windows\System32\nvcuda.dll").exists()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// The engine of a kind ("whisper", "llama", "sherpa") for this computer.
pub fn pick_engine(kind: &str, use_gpu: bool) -> Option<&'static EngineInfo> {
    let cands: Vec<&EngineInfo> = ENGINES.iter().filter(|e| e.id.starts_with(kind)).collect();
    if use_gpu {
        if let Some(g) = cands.iter().find(|e| e.gpu && (e.id != "whisper-cuda" || has_nvidia())) {
            return Some(g);
        }
    }
    cands.iter().find(|e| !e.gpu).or(cands.first()).copied()
}
