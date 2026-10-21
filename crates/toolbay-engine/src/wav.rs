//! 16 kHz mono 16-bit WAV in and out (what whisper.cpp and sherpa-onnx want).

pub const RATE: u32 = 16_000;

pub fn encode(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    header(&mut out, samples.len() as u32);
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

pub fn header(out: &mut Vec<u8>, n_samples: u32) {
    let data = n_samples * 2;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
}

/// Brings quiet audio up to a normal level (peak about 0.9, at most 20x louder), so
/// low call volume doesn't hurt recognition. Silence stays silence.
pub fn normalize(samples: &[f32]) -> Vec<f32> {
    let peak = samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    if peak < 1e-4 {
        return samples.to_vec();
    }
    let g = (0.9 / peak).clamp(1.0, 20.0);
    samples.iter().map(|v| v * g).collect()
}

/// Reads a 16-bit PCM WAV (any rate/channels) as 16 kHz mono f32.
pub fn decode(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12;
    let (mut ch, mut rate, mut bits) = (1u16, RATE, 16u16);
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = pos + 8;
        if id == b"fmt " {
            ch = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().ok()?).max(1);
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?);
            bits = u16::from_le_bytes(bytes[body + 14..body + 16].try_into().ok()?);
        } else if id == b"data" {
            if bits != 16 {
                return None;
            }
            // A WAV still being written has a placeholder length: read to the end.
            let end = if len == 0 || body + len > bytes.len() { bytes.len() } else { body + len };
            let frames: Vec<f32> = bytes[body..end]
                .chunks_exact(2 * ch as usize)
                .map(|f| f.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).sum::<f32>() / ch as f32)
                .collect();
            return Some(resample(&frames, rate, RATE));
        }
        pos = body + len + (len & 1);
    }
    None
}

/// Linear resampler with a small box filter when going down (speech needs up to
/// 8 kHz; whisper and the speaker models are trained on 16 kHz audio).
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() || from == 0 {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(n);
    let span = ratio.floor().max(1.0) as usize;
    for i in 0..n {
        let x = i as f64 * ratio;
        let i0 = x.floor() as usize;
        if span > 1 {
            let end = (i0 + span).min(input.len());
            let s = &input[i0.min(end.saturating_sub(1))..end];
            out.push(s.iter().sum::<f32>() / s.len().max(1) as f32);
        } else {
            let t = (x - i0 as f64) as f32;
            let a = input[i0.min(input.len() - 1)];
            let b = input[(i0 + 1).min(input.len() - 1)];
            out.push(a + (b - a) * t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn roundtrip() {
        let s: Vec<f32> = (0..1600).map(|i| ((i as f32) / 20.0).sin() * 0.5).collect();
        let w = super::encode(&s);
        let d = super::decode(&w).unwrap();
        assert_eq!(d.len(), s.len());
        assert!((d[100] - s[100]).abs() < 0.001);
    }
    #[test]
    fn down() {
        let s = vec![0.5f32; 48_000];
        assert_eq!(super::resample(&s, 48_000, 16_000).len(), 16_000);
    }
}
