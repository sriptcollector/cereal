//! Speaker diarization ("who spoke when") with the sherpa-onnx command-line tool,
//! pyannote segmentation 3.0 and a CAM++ voice-fingerprint model. CPU only, offline.

use crate::server::{adopt, quiet_command};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Stdio;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Turn {
    pub start: f64,
    pub end: f64,
    pub speaker: u32,
}

pub struct DiarModels<'a> {
    pub exe: &'a Path,
    pub segmentation: &'a Path,
    pub embedding: &'a Path,
}

/// Runs diarization on a 16 kHz mono WAV. `speakers` = known count, or None to let the
/// clustering decide (threshold: higher = fewer speakers).
pub fn diarize(m: &DiarModels, wav: &Path, speakers: Option<u32>, threshold: f32) -> Result<Vec<Turn>> {
    // 4 threads is the sweet spot (more threads barely help this model).
    let t = crate::server::threads().min(4).to_string();
    let mut cmd = quiet_command(m.exe);
    cmd.arg(format!("--segmentation.pyannote-model={}", m.segmentation.display()))
        .arg(format!("--embedding.model={}", m.embedding.display()))
        .arg(format!("--segmentation.num-threads={t}"))
        .arg(format!("--embedding.num-threads={t}"))
        // Step half a window at a time: ~5x faster than the default 0.1 with the same speakers found.
        .arg("--segmentation.pyannote-window-shift-ratio=0.5")
        .arg("--min-duration-on=0.3")
        .arg("--min-duration-off=0.5");
    match speakers {
        Some(n) if n > 0 => cmd.arg(format!("--clustering.num-clusters={n}")),
        _ => cmd.arg(format!("--clustering.cluster-threshold={threshold}")),
    };
    cmd.arg(wav).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let child = cmd.spawn()?;
    adopt(&child);
    let out = child.wait_with_output()?;
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(anyhow!("speaker engine failed: {}", text.lines().rev().take(3).collect::<Vec<_>>().join(" / ")));
    }
    Ok(parse(&text))
}

/// Lines like "0.031 -- 23.858 speaker_00".
pub fn parse(text: &str) -> Vec<Turn> {
    let mut v = vec![];
    for line in text.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() == 4 && p[1] == "--" && p[3].starts_with("speaker_") {
            if let (Ok(a), Ok(b), Ok(s)) = (p[0].parse::<f64>(), p[2].parse::<f64>(), p[3]["speaker_".len()..].parse::<u32>()) {
                v.push(Turn { start: a, end: b, speaker: s });
            }
        }
    }
    v
}

/// The speaker who talks most during [start, end], if anyone does.
pub fn speaker_at(turns: &[Turn], start: f64, end: f64) -> Option<u32> {
    let mut best: Option<(u32, f64)> = None;
    let mut totals: std::collections::BTreeMap<u32, f64> = Default::default();
    for t in turns {
        let o = t.end.min(end) - t.start.max(start);
        if o > 0.0 {
            *totals.entry(t.speaker).or_default() += o;
        }
    }
    for (s, o) in totals {
        if best.map(|b| o > b.1).unwrap_or(true) {
            best = Some((s, o));
        }
    }
    if best.is_none() {
        // No overlap: nearest turn within 2 s.
        let mid = (start + end) / 2.0;
        return turns
            .iter()
            .map(|t| (t.speaker, if mid < t.start { t.start - mid } else if mid > t.end { mid - t.end } else { 0.0 }))
            .filter(|x| x.1 < 2.0)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|x| x.0);
    }
    best.map(|b| b.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        let t = parse("progress 100.00%\n0.031 -- 23.858 speaker_00\n24.449 -- 25.968 speaker_01\n");
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].speaker, 1);
        assert_eq!(speaker_at(&t, 24.5, 25.5), Some(1));
        assert_eq!(speaker_at(&t, 1.0, 5.0), Some(0));
    }
}
