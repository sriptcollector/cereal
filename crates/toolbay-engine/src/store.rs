//! Where models and engines live, and the downloader.
//!
//! Each tool has its own folder (`<app data>/models`, `<app data>/engines`) and can
//! also read, never write, the folders of other Toolbay tools and the installer's
//! bundled files. A model Toolbay Flow already downloaded is used as is.

use crate::catalog::{EngineInfo, ModelInfo};
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// (id, bytes done, bytes total, stage: "downloading" | "extracting" | "done")
pub type ProgressFn = dyn Fn(&str, u64, u64, &str) + Send + Sync;

#[derive(Clone, Debug)]
pub struct Store {
    pub models: PathBuf,
    pub engines: PathBuf,
    /// Read-only places to look first: the installer's `bundled/` folder, other tools.
    pub bundled: Option<PathBuf>,
    pub shared: Vec<PathBuf>,
}

impl Store {
    pub fn new(data: &Path, bundled: Option<PathBuf>) -> Self {
        let s = Store { models: data.join("models"), engines: data.join("engines"), bundled, shared: other_tools() };
        let _ = std::fs::create_dir_all(&s.models);
        let _ = std::fs::create_dir_all(&s.engines);
        s
    }

    /// The model file if it is anywhere we can read it.
    pub fn find_model(&self, m: &ModelInfo) -> Option<PathBuf> {
        let own = self.models.join(m.file);
        if own.exists() {
            return Some(own);
        }
        if let Some(b) = &self.bundled {
            let p = b.join("models").join(m.file);
            if p.exists() {
                return Some(p);
            }
        }
        self.shared.iter().map(|d| d.join("models").join(m.file)).find(|p| p.exists() && !p.with_extension("part").exists())
    }

    /// Versioned by the package URL, so a new engine build never mixes with an old one.
    pub fn engine_dir(&self, e: &EngineInfo) -> PathBuf {
        self.engines.join(e.id).join(pkg_name(e))
    }

    pub fn find_engine(&self, e: &EngineInfo) -> Option<PathBuf> {
        if let Some(b) = &self.bundled {
            if let Some(p) = find_file(&b.join("engines").join(e.id), e.exe) {
                return Some(p);
            }
        }
        if let Some(p) = find_file(&self.engine_dir(e), e.exe) {
            return Some(p);
        }
        self.shared.iter().find_map(|d| find_file(&d.join("engines").join(e.id).join(pkg_name(e)), e.exe))
    }

    pub async fn ensure_model(&self, m: &ModelInfo, progress: &ProgressFn) -> Result<PathBuf> {
        if let Some(p) = self.find_model(m) {
            return Ok(p);
        }
        let dest = self.models.join(m.file);
        if m.url.ends_with(".tar.bz2") {
            // An archive with a single model.onnx inside (sherpa-onnx packaging).
            let tmp = self.models.join(format!("{}.tmp", m.id));
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp)?;
            let pkg = tmp.join("pkg.tar.bz2");
            download(m.id, m.url, &pkg, progress).await?;
            progress(m.id, 0, 0, "extracting");
            untar(&pkg, &tmp)?;
            let onnx = find_file(&tmp, "model.onnx").ok_or_else(|| anyhow!("model.onnx not found in {}", m.url))?;
            std::fs::rename(onnx, &dest)?;
            let _ = std::fs::remove_dir_all(&tmp);
            progress(m.id, 1, 1, "done");
        } else {
            download(m.id, m.url, &dest, progress).await?;
        }
        Ok(dest)
    }

    pub async fn ensure_engine(&self, e: &EngineInfo, progress: &ProgressFn) -> Result<PathBuf> {
        if let Some(p) = self.find_engine(e) {
            return Ok(p);
        }
        let dir = self.engine_dir(e);
        std::fs::create_dir_all(&dir)?;
        let archive = dir.join(if e.url.ends_with(".zip") { "pkg.zip" } else if e.url.ends_with(".tar.bz2") { "pkg.tar.bz2" } else { "pkg.tar.gz" });
        download(e.id, e.url, &archive, progress).await?;
        progress(e.id, 0, 0, "extracting");
        let (a, d) = (archive.clone(), dir.clone());
        tokio::task::spawn_blocking(move || extract(&a, &d)).await??;
        let _ = std::fs::remove_file(&archive);
        progress(e.id, 1, 1, "done");
        let exe = find_file(&dir, e.exe).ok_or_else(|| anyhow!("{} not found in engine package", e.exe))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
        }
        Ok(exe)
    }
}

fn pkg_name(e: &EngineInfo) -> String {
    let pkg = e.url.rsplit('/').take(2).collect::<Vec<_>>().join("-");
    pkg.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect()
}

/// App data folders of the other Toolbay tools on this computer (read-only use).
fn other_tools() -> Vec<PathBuf> {
    let ids = ["ai.toolbay.flow", "ai.toolbay.notes"];
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    };
    base.map(|b| ids.iter().map(|i| b.join(i)).filter(|p| p.exists()).collect()).unwrap_or_default()
}

pub fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut dirs = vec![];
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            dirs.push(p);
        } else if p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(name)).unwrap_or(false) {
            return Some(p);
        }
    }
    dirs.iter().find_map(|d| find_file(d, name))
}

/// Streams a URL to `dest` with progress, via a .part file.
pub async fn download(id: &str, url: &str, dest: &Path, progress: &ProgressFn) -> Result<()> {
    let client = reqwest::Client::builder().user_agent("Toolbay/1.0").connect_timeout(Duration::from_secs(20)).build()?;
    let resp = client.get(url).send().await?.error_for_status().with_context(|| format!("download {url}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut f = std::fs::File::create(&part)?;
    let mut done = 0u64;
    let mut last = Instant::now();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        f.write_all(&chunk)?;
        done += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(200) {
            last = Instant::now();
            progress(id, done, total, "downloading");
        }
    }
    f.flush()?;
    drop(f);
    if total > 0 && done != total {
        return Err(anyhow!("download incomplete ({done}/{total} bytes)"));
    }
    std::fs::rename(&part, dest)?;
    progress(id, done, total, "done");
    Ok(())
}

fn tar_exe() -> PathBuf {
    // Windows' own bsdtar reads zip and bz2; a Git Bash GNU tar on PATH may not.
    if cfg!(windows) {
        PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())).join("System32").join("tar.exe")
    } else {
        PathBuf::from("tar")
    }
}

fn untar(archive: &Path, dir: &Path) -> Result<()> {
    let mut cmd = Command::new(tar_exe());
    cmd.arg("-xf").arg(archive).arg("-C").arg(dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    if !cmd.status()?.success() {
        return Err(anyhow!("could not unpack {}", archive.display()));
    }
    Ok(())
}

fn extract(archive: &Path, dir: &Path) -> Result<()> {
    if archive.extension().map(|e| e == "zip").unwrap_or(false) {
        let f = std::fs::File::open(archive)?;
        let mut z = zip::ZipArchive::new(f)?;
        for i in 0..z.len() {
            let mut entry = z.by_index(i)?;
            let Some(rel) = entry.enclosed_name() else { continue };
            let out = dir.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)?;
            } else {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut o = std::fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut o)?;
            }
        }
        Ok(())
    } else {
        untar(archive, dir)
    }
}
