//! macOS: the computer's sound (the other people in the call) through
//! ScreenCaptureKit audio capture (macOS 13+). The screen itself is never recorded:
//! the video side is set to a 2x2 frame once a second and ignored.
//! Needs the "Screen & System Audio Recording" permission; macOS asks the first time.

use crate::capture::Track;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use screencapturekit::cm::{CMSampleBuffer, CMTime};
use screencapturekit::prelude::*;
use std::sync::Arc;

type ClockFn = Box<dyn Fn() -> Option<u64> + Send + Sync>;

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
        let perm = || anyhow!("macOS hasn't allowed Toolbay Notes to hear your computer's sound yet. Open System Settings > Privacy & Security > Screen & System Audio Recording, turn on Toolbay Notes, then start again.");
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
