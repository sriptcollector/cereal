//! macOS: the computer's sound (the other people in the call) through
//! ScreenCaptureKit audio capture (macOS 13+). The screen itself is never recorded:
//! the video side is set to a 2x2 frame once a second and ignored.
//! Needs the "Screen & System Audio Recording" permission; macOS asks the first time.

use crate::capture::Track;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use screencapturekit::cm::{CMSampleBuffer, CMTime};
use screencapturekit::prelude::*;
use objc2_foundation::NSString;
use std::sync::Arc;

type ClockFn = Box<dyn Fn() -> Option<u64> + Send + Sync>;

/// Shown while recording, and on a meeting that couldn't be recorded.
pub const MIC_BLOCKED: &str = "macOS isn't letting Cereal use your microphone. Open System Settings > Privacy & Security > Microphone, turn on Cereal, then quit and reopen Cereal.";
pub const MIC_SILENT: &str = "Your microphone is sending only silence. If macOS asked about the microphone, allow Cereal in System Settings > Privacy & Security > Microphone; otherwise check that the mic isn't muted.";
pub const SOUND_BLOCKED: &str = "macOS hasn't allowed Cereal to hear your computer's sound yet. Open System Settings > Privacy & Security > Screen & System Audio Recording, turn on Cereal, then start again.";

mod ffi {
    use objc2_foundation::NSString;
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {
        pub static AVMediaTypeAudio: &'static NSString;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGPreflightScreenCaptureAccess() -> bool;
    }
}

/// The microphone permission is off (denied, or restricted by the Mac's owner).
/// Not asked yet counts as allowed: macOS asks when the microphone opens.
pub fn mic_blocked() -> bool {
    let Some(cls) = objc2::runtime::AnyClass::get(c"AVCaptureDevice") else { return false };
    // AVAuthorizationStatus: 0 not determined, 1 restricted, 2 denied, 3 authorized.
    let status: isize = unsafe {
        let media: &NSString = ffi::AVMediaTypeAudio;
        objc2::msg_send![cls, authorizationStatusForMediaType: media]
    };
    status == 1 || status == 2
}

/// "Screen & System Audio Recording" is allowed (checks only, never asks).
pub fn sound_allowed() -> bool {
    unsafe { ffi::CGPreflightScreenCaptureAccess() }
}

struct Handler {
    track: Arc<Mutex<Track>>,
    clock: Arc<ClockFn>,
}

impl SCStreamOutputTrait for Handler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if !matches!(of_type, SCStreamOutputType::Audio) {
            return;
        }
        let Some(expected) = (self.clock)() else { return }; // paused
        let Ok(list) = sample.audio_buffer_list() else { return };
        // Mono float32, non-interleaved: one buffer.
        let mut mono: Vec<f32> = vec![];
        let mut n_buf = 0usize;
        for b in &list {
            let d = b.data();
            let s: Vec<f32> = d.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            if mono.is_empty() {
                mono = s;
            } else {
                for (a, v) in mono.iter_mut().zip(s.iter()) {
                    *a += *v;
                }
            }
            n_buf += 1;
        }
        if n_buf > 1 {
            for a in mono.iter_mut() {
                *a /= n_buf as f32;
            }
        }
        if !mono.is_empty() {
            self.track.lock().push_external(&mono, 48_000, expected);
        }
    }
}

pub struct SystemAudio {
    stream: SCStream,
}

impl SystemAudio {
    pub fn start(track: Arc<Mutex<Track>>, clock: ClockFn) -> Result<SystemAudio> {
        let perm = || anyhow!("{SOUND_BLOCKED}");
        let content = SCShareableContent::get().map_err(|_| perm())?;
        let display = content.displays().into_iter().next().ok_or_else(perm)?;
        let filter = SCContentFilter::create().with_display(&display).with_excluding_windows(&[]).build().map_err(|e| anyhow!("{e:?}"))?;
        let config = SCStreamConfiguration::new()
            .with_width(2)
            .with_height(2)
            .with_minimum_frame_interval(&CMTime::new(1, 1))
            .with_captures_audio(true)
            .with_excludes_current_process_audio(true)
            .with_sample_rate(48_000)
            .with_channel_count(1);
        let mut stream = SCStream::new(&filter, &config).map_err(|e| anyhow!("{e:?}"))?;
        stream.add_output_handler(Handler { track, clock: Arc::new(clock) }, SCStreamOutputType::Audio).map_err(|e| anyhow!("{e:?}"))?;
        stream.start_capture().map_err(|_| perm())?;
        Ok(SystemAudio { stream })
    }
}

impl Drop for SystemAudio {
    fn drop(&mut self) {
        let _ = self.stream.stop_capture();
    }
}
