//! Short sounds when notes start, pause, resume and stop (Settings › Sounds). Our own
//! tones, synthesised here; nothing is bundled.

use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub enum Cue {
    Start,
    Pause,
    Resume,
    Stop,
}

/// Soft two-note chime: (frequency Hz, length s) per note.
fn notes(c: Cue) -> &'static [(f32, f32)] {
    match c {
        Cue::Start => &[(660.0, 0.09), (880.0, 0.16)],
        Cue::Resume => &[(740.0, 0.12)],
        Cue::Pause => &[(587.0, 0.12)],
        Cue::Stop => &[(880.0, 0.09), (587.0, 0.18)],
    }
}

fn render(c: Cue) -> Vec<f32> {
    let rate = toolbay_engine::wav::RATE as f32;
    let mut out = vec![];
    for &(f, len) in notes(c) {
        let n = (len * rate) as usize;
        for i in 0..n {
            let t = i as f32 / rate;
            // Quick attack, gentle decay; a quiet octave on top for a bell-like tone.
            let env = (t / 0.008).min(1.0) * (-(t / len) * 4.0).exp();
            out.push(0.22 * env * ((2.0 * PI * f * t).sin() + 0.25 * (4.0 * PI * f * t).sin()));
        }
    }
    out.extend(vec![0.0; 800]);
    out
}

pub fn play(st: &crate::state::St, c: Cue) {
    if !st.settings.read().sounds {
        return;
    }
    let wav = toolbay_engine::wav::encode(&render(c));
    std::thread::spawn(move || {
        #[cfg(windows)]
        unsafe {
            use windows::core::PCWSTR;
            use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
            let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
        }
        #[cfg(target_os = "macos")]
        {
            let f = std::env::temp_dir().join(format!("cereal-cue-{}.wav", std::process::id()));
            if std::fs::write(&f, &wav).is_ok() {
                let _ = std::process::Command::new("afplay").arg(&f).status();
                let _ = std::fs::remove_file(&f);
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = wav;
    });
}
