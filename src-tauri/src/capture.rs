//! Records the meeting as two separate tracks, so we know who said what without
//! a bot in the call:
//!   - "mic": your microphone (you)
//!   - "system": what your computer plays (everyone else). Windows: WASAPI loopback
//!     of the default output device. macOS: ScreenCaptureKit audio (capture_mac.rs).
//!
//! Both tracks are resampled to 16 kHz mono, written to WAV files as they arrive and
//! kept on one clock: when a device delivers nothing (loopback is silent while
//! nobody talks), silence is filled in so the two files always line up.
//!
//! Shared-mode capture only. Nothing here mutes, ducks or pauses any other app.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use toolbay_engine::wav;

pub const RATE: u32 = wav::RATE;
const RATE_F: f64 = RATE as f64;

/// Streaming box-filter resampler (any rate -> 16 kHz).
struct Resampler {
    ratio: f64,
    pos: f64,
    buf: Vec<f32>,
}

impl Resampler {
    fn new(from: u32) -> Self {
        Resampler { ratio: from as f64 / RATE_F, pos: 0.0, buf: vec![] }
    }
    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if (self.ratio - 1.0).abs() < 1e-9 {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        while self.pos + self.ratio <= self.buf.len() as f64 {
            let a = self.pos.floor() as usize;
            let b = ((self.pos + self.ratio).floor() as usize).max(a + 1).min(self.buf.len());
            let s = &self.buf[a..b];
            out.push(s.iter().sum::<f32>() / s.len() as f32);
            self.pos += self.ratio;
        }
        let used = self.pos.floor() as usize;
        self.buf.drain(..used.min(self.buf.len()));
        self.pos -= used as f64;
    }
}

/// A 16-bit mono WAV written as audio arrives; the header is patched every few
/// seconds so even a crash leaves a playable file.
struct WavFile {
    f: std::io::BufWriter<std::fs::File>,
    n: u32,
    last_patch: Instant,
}

impl WavFile {
    fn create(p: &Path) -> Result<Self> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(p)?);
        let mut h = vec![];
        wav::header(&mut h, 0);
        f.write_all(&h)?;
        Ok(WavFile { f, n: 0, last_patch: Instant::now() })
    }
    fn write(&mut self, s: &[f32]) {
        let mut b = Vec::with_capacity(s.len() * 2);
        for v in s {
            b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        let _ = self.f.write_all(&b);
        self.n += s.len() as u32;
        if self.last_patch.elapsed() > Duration::from_secs(10) {
            self.patch();
        }
    }
    fn patch(&mut self) {
        self.last_patch = Instant::now();
        let mut h = vec![];
        wav::header(&mut h, self.n);
        let _ = self.f.flush();
        let f = self.f.get_mut();
        if f.seek(SeekFrom::Start(0)).is_ok() {
            let _ = f.write_all(&h);
            let _ = f.seek(SeekFrom::End(0));
        }
    }
}

/// One track's state, shared with the device callback.
pub struct Track {
    pub name: &'static str,
    file: Option<WavFile>,
    rs: Option<Resampler>,
    rate: u32,
    /// Samples written so far (16 kHz) = this track's position on the meeting clock.
    pub written: u64,
    /// Audio not yet handed to the live transcriber, and where it starts.
    live: Vec<f32>,
    live_start: u64,
    last_data: Instant,
    pub level: f32,
    pub device: String,
    pub error: Option<String>,
    /// Noise floor estimate (RMS) for the speech detector.
    floor: f32,
    /// Device samples in a row that were exactly zero (macOS hands out pure zeros
    /// when the microphone permission is off).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    zero_run: u64,
    /// Why this track couldn't record, if it was blocked at any point (so the
    /// recording is never thrown away as "nobody spoke").
    pub blocked: Option<String>,
}

impl Track {
    fn new(name: &'static str, path: &Path) -> Result<Self> {
        Ok(Track {
            name,
            file: Some(WavFile::create(path)?),
            rs: None,
            rate: 0,
            written: 0,
            live: vec![],
            live_start: 0,
            last_data: Instant::now(),
            level: 0.0,
            device: String::new(),
            error: None,
            floor: 0.003,
            zero_run: 0,
            blocked: None,
        })
    }

    fn append(&mut self, s: &[f32]) {
        if let Some(f) = self.file.as_mut() {
            f.write(s);
        }
        self.live.extend_from_slice(s);
        self.written += s.len() as u64;
        // Never hold more than 60 s of live audio (the transcriber may be busy).
        if self.live.len() > 60 * RATE as usize {
            let drop = self.live.len() - 45 * RATE as usize;
            self.live.drain(..drop);
            self.live_start += drop as u64;
        }
    }

    /// Fills silence up to the meeting clock (device gave nothing, or a gap).
    fn pad_to(&mut self, expected: u64) {
        if expected > self.written {
            let n = (expected - self.written) as usize;
            self.append(&vec![0.0; n]);
        }
    }

    fn push_device(&mut self, mono: &[f32], rate: u32, expected: u64) {
        if self.rate != rate || self.rs.is_none() {
            self.rate = rate;
            self.rs = Some(Resampler::new(rate));
        }
        let mut out = Vec::with_capacity(mono.len() / 2 + 8);
        self.rs.as_mut().unwrap().push(mono, &mut out);
        // A gap longer than 300 ms (device just started, or loopback was idle): fill it.
        let after = self.written + out.len() as u64;
        if expected > after + (RATE as u64 * 3 / 10) {
            self.pad_to(expected - out.len() as u64);
        }
        if !out.is_empty() {
            let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
            self.level = self.level * 0.7 + rms * 0.3;
        }
        if out.iter().all(|v| *v == 0.0) {
            self.zero_run += out.len() as u64;
        } else {
            self.zero_run = 0;
        }
        self.append(&out);
        self.last_data = Instant::now();
    }

    fn finish(&mut self) {
        if let Some(mut f) = self.file.take() {
            f.patch();
        }
    }
}

/// The meeting clock: time spent recording, not counting pauses.
struct Clock {
    base: Duration,
    since: Option<Instant>,
}

impl Clock {
    fn secs(&self) -> f64 {
        (self.base + self.since.map(|s| s.elapsed()).unwrap_or_default()).as_secs_f64()
    }
    fn samples(&self) -> u64 {
        (self.secs() * RATE_F) as u64
    }
}

pub struct Capture {
    pub dir: PathBuf,
    pub mic: Arc<Mutex<Track>>,
    pub sys: Arc<Mutex<Track>>,
    clock: Arc<Mutex<Clock>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Levels {
    pub mic: f32,
    pub system: f32,
    pub secs: f64,
    pub paused: bool,
    pub mic_device: String,
    pub system_device: String,
    pub mic_error: Option<String>,
    pub system_error: Option<String>,
}

/// A piece of audio ready for live transcription.
pub struct Chunk {
    pub start: f64,
    pub audio: Vec<f32>,
}

impl Capture {
    /// Starts recording into `dir/mic.wav` and `dir/system.wav`.
    pub fn start(dir: &Path, microphone: Option<String>, capture_system: bool) -> Result<Capture> {
        std::fs::create_dir_all(dir)?;
        let mic = Arc::new(Mutex::new(Track::new("mic", &dir.join("mic.wav"))?));
        let sys = Arc::new(Mutex::new(Track::new("system", &dir.join("system.wav"))?));
        let clock = Arc::new(Mutex::new(Clock { base: Duration::ZERO, since: Some(Instant::now()) }));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
        let (m, s, c, st) = (mic.clone(), sys.clone(), clock.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("capture".into())
            .spawn(move || run(m, s, c, st, microphone, capture_system, ready_tx))?;
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                stop.store(true, Ordering::SeqCst);
                let _ = thread.join();
                return Err(e);
            }
            Err(_) => return Err(anyhow!("The microphone didn't respond. Check that no other app is holding it exclusively, then try again.")),
        }
        Ok(Capture { dir: dir.to_path_buf(), mic, sys, clock, stop, thread: Some(thread) })
    }

    pub fn secs(&self) -> f64 {
        self.clock.lock().secs()
    }

    pub fn paused(&self) -> bool {
        self.clock.lock().since.is_none()
    }

    pub fn pause(&self) {
        let mut c = self.clock.lock();
        if let Some(s) = c.since.take() {
            c.base += s.elapsed();
        }
    }

    pub fn resume(&self) {
        let mut c = self.clock.lock();
        if c.since.is_none() {
            c.since = Some(Instant::now());
        }
    }

    pub fn levels(&self) -> Levels {
        let (m, s) = (self.mic.lock(), self.sys.lock());
        Levels {
            mic: (m.level * 6.0).min(1.0),
            system: (s.level * 6.0).min(1.0),
            secs: self.secs(),
            paused: self.paused(),
            mic_device: m.device.clone(),
            system_device: s.device.clone(),
            mic_error: m.error.clone(),
            system_error: s.error.clone(),
        }
    }

    /// The next piece of `track` worth transcribing: cut at a pause after at least
    /// `min_s` seconds, or at the quietest point once it reaches `max_s`. Pure silence
    /// is skipped (it only makes speech models invent words).
    pub fn take_chunk(&self, system: bool, min_s: f64, max_s: f64, flush: bool) -> Option<Chunk> {
        let mut t = if system { self.sys.lock() } else { self.mic.lock() };
        take_chunk(&mut t, min_s, max_s, flush)
    }

    /// Why a track couldn't record, if one was blocked at any point (macOS
    /// permission off).
    pub fn blocked(&self) -> Option<String> {
        let why: Vec<String> = [&self.mic, &self.sys].iter().filter_map(|t| t.lock().blocked.clone()).collect();
        (!why.is_empty()).then(|| why.join(" "))
    }

    pub fn stop(mut self) -> PathBuf {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.dir.clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn rms(s: &[f32]) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
}

/// 100 ms frame energies.
fn frames(s: &[f32]) -> Vec<f32> {
    s.chunks(RATE as usize / 10).map(rms).collect()
}

fn take_chunk(t: &mut Track, min_s: f64, max_s: f64, flush: bool) -> Option<Chunk> {
    let len_s = t.live.len() as f64 / RATE_F;
    if len_s < 0.5 && !flush {
        return None;
    }
    let f = frames(&t.live);
    if f.is_empty() {
        return None;
    }
    // Adaptive noise floor: the quieter frames of this buffer.
    let mut sorted = f.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = sorted[sorted.len() / 5];
    t.floor = (t.floor * 0.8 + q * 0.2).max(0.0005);
    let thr = (t.floor * 3.5).max(0.0025);
    let voiced: Vec<bool> = f.iter().map(|e| *e > thr).collect();
    let n_voiced = voiced.iter().filter(|v| **v).count();

    // Nothing but silence: drop it (keep the last half second in case speech starts).
    if n_voiced < 3 {
        if len_s > 2.0 || flush {
            let keep = if flush { 0 } else { RATE as usize / 2 };
            let drop = t.live.len().saturating_sub(keep);
            t.live.drain(..drop);
            t.live_start += drop as u64;
        }
        return None;
    }

    let tail_quiet = voiced.iter().rev().take(6).all(|v| !v); // last 600 ms quiet
    let cut_frames = if flush {
        Some(f.len())
    } else if len_s >= max_s {
        // Quietest frame in the last third.
        let from = f.len() * 2 / 3;
        let (i, _) = f[from..].iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap();
        Some(from + i + 1)
    } else if len_s >= min_s && tail_quiet {
        Some(f.len() - 3)
    } else {
        None
    };
    let cut = (cut_frames? * RATE as usize / 10).min(t.live.len());
    let audio: Vec<f32> = t.live.drain(..cut).collect();
    let start = t.live_start as f64 / RATE_F;
    t.live_start += cut as u64;
    Some(Chunk { start, audio })
}

fn mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect()
}

fn find_input(host: &cpal::Host, name: &Option<String>) -> Option<cpal::Device> {
    match name {
        Some(n) => host.input_devices().ok()?.find(|d| d.name().map(|x| &x == n).unwrap_or(false)).or_else(|| host.default_input_device()),
        None => host.default_input_device(),
    }
}

/// The output to record: the device a call app is playing to (a headset that isn't the
/// default, say), otherwise the default output.
#[cfg(not(target_os = "macos"))]
fn wanted_output(host: &cpal::Host) -> Option<cpal::Device> {
    #[cfg(windows)]
    if let Some(name) = crate::audio_route::call_output_device(crate::detect::is_call_exe) {
        if let Some(d) = host.output_devices().ok()?.find(|d| d.name().map(|n| n == name).unwrap_or(false)) {
            return Some(d);
        }
    }
    host.default_output_device()
}

fn open_stream(dev: &cpal::Device, loopback: bool, track: Arc<Mutex<Track>>, clock: Arc<Mutex<Clock>>, failed: Arc<AtomicBool>) -> Result<cpal::Stream> {
    let cfg = if loopback { dev.default_output_config()? } else { dev.default_input_config()? };
    let channels = cfg.channels() as usize;
    let rate = cfg.sample_rate().0;
    let sc: cpal::StreamConfig = cfg.config();
    let f2 = failed.clone();
    let err = move |e: cpal::StreamError| {
        log::warn!("audio stream error: {e}");
        f2.store(true, Ordering::SeqCst);
    };
    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let (tr, ck) = (track.clone(), clock.clone());
            dev.build_input_stream(
                &sc,
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let c = ck.lock();
                    if c.since.is_none() {
                        return; // paused
                    }
                    let expected = c.samples();
                    drop(c);
                    let f: Vec<f32> = data.iter().map($conv).collect();
                    tr.lock().push_device(&mono(&f, channels), rate, expected);
                },
                err,
                None,
            )?
        }};
    }
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |v: &f32| *v),
        cpal::SampleFormat::I16 => build!(i16, |v: &i16| *v as f32 / 32768.0),
        cpal::SampleFormat::I32 => build!(i32, |v: &i32| *v as f32 / 2147483648.0),
        cpal::SampleFormat::U16 => build!(u16, |v: &u16| (*v as f32 - 32768.0) / 32768.0),
        f => return Err(anyhow!("unsupported sample format {f:?}")),
    };
    stream.play()?;
    Ok(stream)
}

/// The capture thread: owns the device streams (not Send on every platform), keeps the
/// tracks on the clock, and reopens a device when it disappears or the default changes
/// (for example when headphones are plugged in mid-call).
fn run(mic: Arc<Mutex<Track>>, sys: Arc<Mutex<Track>>, clock: Arc<Mutex<Clock>>, stop: Arc<AtomicBool>, microphone: Option<String>, capture_system: bool, ready: std::sync::mpsc::Sender<Result<()>>) {
    let host = cpal::default_host();
    let mic_failed = Arc::new(AtomicBool::new(false));
    let sys_failed = Arc::new(AtomicBool::new(false));

    let open_mic = |host: &cpal::Host| -> Result<(cpal::Stream, String)> {
        let dev = find_input(host, &microphone).ok_or_else(|| anyhow!("No microphone found. Plug one in, or pick another one in Settings."))?;
        let name = dev.name().unwrap_or_default();
        mic_failed.store(false, Ordering::SeqCst);
        Ok((open_stream(&dev, false, mic.clone(), clock.clone(), mic_failed.clone())?, name))
    };

    #[cfg(not(target_os = "macos"))]
    let open_sys = |host: &cpal::Host| -> Result<(cpal::Stream, String)> {
        let dev = wanted_output(host).ok_or_else(|| anyhow!("No speakers or headphones found"))?;
        let name = dev.name().unwrap_or_default();
        sys_failed.store(false, Ordering::SeqCst);
        Ok((open_stream(&dev, true, sys.clone(), clock.clone(), sys_failed.clone())?, name))
    };

    let mut mic_stream = match open_mic(&host) {
        Ok((s, n)) => {
            mic.lock().device = n;
            Some(s)
        }
        Err(e) => {
            let _ = ready.send(Err(anyhow!("Couldn't open the microphone: {e}")));
            return;
        }
    };

    #[cfg(not(target_os = "macos"))]
    let mut sys_stream = if capture_system {
        match open_sys(&host) {
            Ok((s, n)) => {
                sys.lock().device = n;
                Some(s)
            }
            Err(e) => {
                log::warn!("system audio: {e:#}");
                sys.lock().error = Some(format!("Couldn't record your computer's sound: {e}"));
                None
            }
        }
    } else {
        None
    };
    #[cfg(target_os = "macos")]
    let start_sys = || {
        crate::capture_mac::SystemAudio::start(sys.clone(), Box::new({
            let c = clock.clone();
            move || {
                let c = c.lock();
                if c.since.is_none() { None } else { Some(c.samples()) }
            }
        }))
    };
    #[cfg(target_os = "macos")]
    let mut sys_stream = if capture_system {
        match start_sys() {
            Ok(s) => {
                sys.lock().device = "This Mac's sound".into();
                Some(s)
            }
            Err(e) => {
                log::warn!("system audio: {e:#}");
                block(&sys, &format!("{e}"));
                None
            }
        }
    } else {
        None
    };
    // Microphone permission off: the stream opens but only ever hears zeros.
    #[cfg(target_os = "macos")]
    if crate::capture_mac::mic_blocked() {
        block(&mic, crate::capture_mac::MIC_BLOCKED);
    }

    let _ = ready.send(Ok(()));
    let mut tick = 0u32;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
        tick += 1;
        let (paused, expected) = {
            let c = clock.lock();
            (c.since.is_none(), c.samples())
        };
        if !paused {
            // Keep both tracks on the clock while a device is quiet.
            for t in [&mic, &sys] {
                let mut t = t.lock();
                if t.last_data.elapsed() > Duration::from_millis(400) {
                    let target = expected.saturating_sub(RATE as u64 / 5);
                    t.pad_to(target);
                    t.level *= 0.5;
                }
            }
        }
        // Every 2 s: follow default-device changes and recover from device errors.
        if tick % 20 == 0 {
            let want_mic = find_input(&host, &microphone).and_then(|d| d.name().ok()).unwrap_or_default();
            if mic_failed.load(Ordering::SeqCst) || (want_mic != mic.lock().device && !want_mic.is_empty()) {
                drop(mic_stream.take());
                match open_mic(&host) {
                    Ok((s, n)) => {
                        log::info!("microphone now {n}");
                        let mut m = mic.lock();
                        m.device = n;
                        m.error = None;
                        mic_stream = Some(s);
                    }
                    Err(e) => mic.lock().error = Some(format!("{e}")),
                }
            }
            #[cfg(not(target_os = "macos"))]
            if capture_system {
                let want = wanted_output(&host).and_then(|d| d.name().ok()).unwrap_or_default();
                if sys_failed.load(Ordering::SeqCst) || sys_stream.is_none() || (want != sys.lock().device && !want.is_empty()) {
                    drop(sys_stream.take());
                    match open_sys(&host) {
                        Ok((s, n)) => {
                            log::info!("system audio now {n}");
                            let mut t = sys.lock();
                            t.device = n;
                            t.error = None;
                            sys_stream = Some(s);
                        }
                        Err(e) => sys.lock().error = Some(format!("Couldn't record your computer's sound: {e}")),
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                use crate::capture_mac::{MIC_BLOCKED, MIC_SILENT};
                // Permission off (checked again: it can change mid-call), or pure
                // zeros / nothing at all from the microphone for 3 seconds.
                let silent = {
                    let m = mic.lock();
                    !paused && (m.zero_run > 3 * RATE as u64 || m.last_data.elapsed() > Duration::from_secs(3))
                };
                if crate::capture_mac::mic_blocked() {
                    block(&mic, MIC_BLOCKED);
                } else if silent {
                    block(&mic, MIC_SILENT);
                } else {
                    let mut m = mic.lock();
                    if matches!(m.error.as_deref(), Some(MIC_BLOCKED | MIC_SILENT)) {
                        m.error = None;
                    }
                }
                if capture_system && sys_stream.is_none() {
                    // Try again: the person may allow it mid-call. Every 2 s once
                    // macOS says it's allowed, every 10 s otherwise.
                    let allowed = crate::capture_mac::sound_allowed();
                    if allowed || tick % 100 == 0 {
                        match start_sys() {
                            Ok(s) => {
                                log::info!("system audio started");
                                let mut t = sys.lock();
                                t.device = "This Mac's sound".into();
                                t.error = None;
                                sys_stream = Some(s);
                            }
                            Err(e) => block(&sys, &format!("{e}")),
                        }
                    } else {
                        block(&sys, crate::capture_mac::SOUND_BLOCKED);
                    }
                }
            }
        }
    }
    drop(mic_stream);
    drop(sys_stream);
    let _ = (&sys_failed, capture_system);
    // Bring both files to the same length, then close them.
    let end = clock.lock().samples();
    for t in [&mic, &sys] {
        let mut t = t.lock();
        t.pad_to(end);
        t.finish();
    }
}

/// Marks a track as blocked and says why (shown while recording).
#[cfg(target_os = "macos")]
fn block(t: &Arc<Mutex<Track>>, why: &str) {
    let mut t = t.lock();
    t.error = Some(why.to_string());
    t.blocked = Some(why.to_string());
}

#[cfg(target_os = "macos")]
impl Track {
    /// Called by the macOS system-audio capture with mono f32 at `rate`.
    pub fn push_external(&mut self, mono: &[f32], rate: u32, expected: u64) {
        self.push_device(mono, rate, expected);
    }
}

pub fn list_mics() -> Vec<String> {
    cpal::default_host().input_devices().map(|it| it.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

/// A few seconds of each source's level, for the onboarding / settings check.
pub fn quick_check(secs: f64, microphone: Option<String>) -> Result<(f32, f32)> {
    let dir = std::env::temp_dir().join(format!("cereal-check-{}", std::process::id()));
    let cap = Capture::start(&dir, microphone, true)?;
    let mut peak = (0f32, 0f32);
    let t0 = Instant::now();
    while t0.elapsed().as_secs_f64() < secs {
        std::thread::sleep(Duration::from_millis(100));
        let l = cap.levels();
        peak.0 = peak.0.max(l.mic);
        peak.1 = peak.1.max(l.system);
    }
    cap.stop();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(peak)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resampler_rate() {
        let mut r = Resampler::new(48_000);
        let mut out = vec![];
        for _ in 0..100 {
            r.push(&vec![0.25f32; 480], &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 1);
        assert!((out[100] - 0.25).abs() < 1e-6);
        let mut r = Resampler::new(44_100);
        let mut out = vec![];
        for _ in 0..100 {
            r.push(&vec![0.1f32; 441], &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2);
    }

    #[test]
    fn chunking() {
        let dir = std::env::temp_dir().join("tbn-chunk-test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = Track::new("mic", &dir.join("a.wav")).unwrap();
        // 1 s silence, 3 s "speech", 1 s silence.
        let mut s = vec![0.0f32; 16_000];
        s.extend((0..48_000).map(|i| ((i as f32) * 0.05).sin() * 0.3));
        s.extend(vec![0.0f32; 16_000]);
        t.append(&s);
        let c = take_chunk(&mut t, 2.0, 15.0, false).expect("chunk");
        assert_eq!(c.start, 0.0);
        assert!(c.audio.len() >= 4 * 16_000);
        // Only silence left -> nothing more.
        assert!(take_chunk(&mut t, 2.0, 15.0, true).is_none());
        t.finish();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
