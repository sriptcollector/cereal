//! After the call: a full-quality pass over the whole recording, who-said-what for
//! the other side (speaker diarization), echo removal, then the AI notes.

use crate::db::Segment;
use crate::state::St;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use toolbay_engine::diar::{self, DiarModels, Turn};
use toolbay_engine::{stt, wav};

const R: usize = wav::RATE as usize;
const FRAME: usize = R / 10; // 100 ms

#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct Timing {
    pub audio_s: f64,
    pub transcribe_s: f64,
    /// Runs at the same time as transcription.
    pub diarize_s: f64,
    /// Wall time from "stop" to a final transcript.
    pub transcript_wall_s: f64,
    pub notes_s: f64,
}

pub async fn run(st: &St, id: &str, template: Option<String>) {
    let _g = st.process_lock.lock().await;
    let t0 = Instant::now();
    match pipeline(st, id, template).await {
        Ok(t) => {
            let _ = st.db.set(id, "status", "ready");
            let _ = st.db.set(id, "step", "");
            crate::library_cmds::post_to_folder_webhook(st, id).await;
            log::info!("processed {id} in {:.1}s: {t:?}", t0.elapsed().as_secs_f64());
        }
        Err(e) => {
            log::warn!("processing {id} failed: {e:#}");
            let _ = st.db.set(id, "status", "failed");
            let _ = st.db.set(id, "step", "");
            let _ = st.db.set(id, "error", &plain_error(&e));
        }
    }
    st.meeting_changed(id);
}

fn plain_error(e: &anyhow::Error) -> String {
    let s = format!("{e:#}");
    if s.contains("still downloading") || s.contains("Add your API key") {
        return s;
    }
    if s.contains("error sending request") || s.contains("connection") {
        return "The local AI engine stopped unexpectedly. Your transcript is saved. Click \"Write notes\" to try again.".into();
    }
    format!("Couldn't finish the notes: {s}. Your recording and transcript are saved; click \"Write notes\" to try again.")
}

fn step(st: &St, id: &str, s: &str) {
    let _ = st.db.set(id, "step", s);
    st.meeting_changed(id);
}

pub fn parts(st: &St, id: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(st.meetings_dir().join(id))
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().map(|n| n.to_string_lossy().starts_with("part-")).unwrap_or(false)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub async fn pipeline(st: &St, id: &str, template: Option<String>) -> Result<Timing> {
    let mut timing = Timing::default();
    let parts = parts(st, id);
    let (final_pass, me) = {
        let s = st.settings.read();
        (s.final_pass || (s.speech_cloud_on && s.speech_cloud.ready()), s.me())
    };
    if !parts.is_empty() {
        let live = st.db.segments(id)?;
        let t = Instant::now();
        let res = transcript_from_audio(st, id, &parts, final_pass, &live, &me, &mut timing).await;
        match res {
            Ok(segs) if !segs.is_empty() || live.is_empty() => {
                st.db.replace_segments(id, &segs)?;
            }
            Ok(_) => {}
            Err(e) => log::warn!("final transcript failed, keeping the live one: {e:#}"),
        }
        timing.transcript_wall_s = t.elapsed().as_secs_f64();
        st.meeting_changed(id);
    }
    let segs = st.db.segments(id)?;
    let m = st.db.meeting(id)?.ok_or_else(|| anyhow!("meeting not found"))?;
    if segs.is_empty() && m.my_notes.trim().is_empty() {
        st.db.set(id, "enhanced", "")?;
        return Ok(timing);
    }
    step(st, id, "Writing your notes…");
    let t = Instant::now();
    crate::notes::enhance(st, id, template).await?;
    timing.notes_s = t.elapsed().as_secs_f64();
    apply_audio_policy(st, id);
    Ok(timing)
}

fn apply_audio_policy(st: &St, id: &str) {
    if st.settings.read().keep_audio == "delete" {
        for p in parts(st, id) {
            let _ = std::fs::remove_file(p.join("mic.wav"));
            let _ = std::fs::remove_file(p.join("system.wav"));
        }
        let _ = st.db.set_has_audio(id, false);
    }
}

/// Empties the trash of notes deleted more than 30 days ago, and applies the
/// transcript retention setting.
pub fn trash_pass(st: &St) {
    for id in st.db.expired_trash().unwrap_or_default() {
        let _ = st.db.delete_meeting(&id);
        let _ = std::fs::remove_dir_all(st.meetings_dir().join(&id));
    }
    let days = match st.settings.read().transcript_retention.as_str() {
        "1d" => 1,
        "1w" => 7,
        "1m" => 30,
        "3m" => 91,
        "6m" => 182,
        "1y" => 365,
        _ => return,
    };
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    for m in st.db.meetings(None).unwrap_or_default() {
        if m.created_at < cutoff && m.status != "recording" {
            let _ = st.db.replace_segments(&m.id, &[]);
        }
    }
}

/// Deletes recordings older than 30 days when the user chose that.
pub fn retention_pass(st: &St) {
    if st.settings.read().keep_audio != "30d" {
        return;
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
    for m in st.db.meetings(None).unwrap_or_default() {
        let old = chrono::DateTime::parse_from_rfc3339(&m.created_at).map(|d| d < cutoff).unwrap_or(false);
        if old && m.has_audio && m.status != "recording" {
            for p in parts(st, &m.id) {
                let _ = std::fs::remove_file(p.join("mic.wav"));
                let _ = std::fs::remove_file(p.join("system.wav"));
            }
            let _ = st.db.set_has_audio(&m.id, false);
        }
    }
}

fn load(p: &Path) -> Vec<f32> {
    std::fs::read(p).ok().and_then(|b| wav::decode(&b)).unwrap_or_default()
}

async fn transcript_from_audio(st: &St, id: &str, parts: &[PathBuf], final_pass: bool, live: &[Segment], me: &str, timing: &mut Timing) -> Result<Vec<Segment>> {
    let mut out: Vec<Segment> = vec![];
    let mut offset = 0.0;
    let n = parts.len();
    for (pi, part) in parts.iter().enumerate() {
        let mic = load(&part.join("mic.wav"));
        let sys = load(&part.join("system.wav"));
        let dur = mic.len().max(sys.len()) as f64 / R as f64;
        timing.audio_s += dur;
        // Speech on the GPU and speakers on the CPU at the same time.
        let transcribe = async {
            let t0 = Instant::now();
            let r = async {
            if final_pass {
                let label = |what: &str, p: f64| {
                    let overall = ((pi as f64 + p) / n as f64 * 100.0).round();
                    format!("Polishing the transcript ({what})… {overall}%")
                };
                let a = transcribe_track(st, &mic, |p| step(st, id, &label("you", p * 0.5))).await?;
                let b = transcribe_track(st, &sys, |p| step(st, id, &label("them", 0.5 + p * 0.5))).await?;
                Ok::<_, anyhow::Error>((a, b))
            } else {
                step(st, id, "Telling the other speakers apart…");
                // Use the live transcript for this part.
                let in_part = |s: &&Segment| s.start >= offset - 0.5 && s.start < offset + dur + 0.5;
                let a = live.iter().filter(|s| s.source == "mic").filter(in_part).map(|s| stt::Seg { start: s.start - offset, end: s.end - offset, text: s.text.clone() }).collect();
                let b = live.iter().filter(|s| s.source == "system").filter(in_part).map(|s| stt::Seg { start: s.start - offset, end: s.end - offset, text: s.text.clone() }).collect();
                Ok((a, b))
            }
            }
            .await;
            (r, t0.elapsed().as_secs_f64())
        };
        let diarize = async {
            let t = Instant::now();
            let r = diarize_track(st, &sys).await;
            (r, t.elapsed().as_secs_f64())
        };
        let ((tr, tz), (turns, dz)) = tokio::join!(transcribe, diarize);
        let (mut mic_segs, mut sys_segs) = tr?;
        timing.transcribe_s += tz;
        timing.diarize_s += dz;
        let turns = match turns {
            Ok(t) => t,
            Err(e) => {
                log::warn!("diarization: {e:#}");
                vec![]
            }
        };
        dedupe_echo(&mut mic_segs, &sys_segs);
        let labels = label_speakers(&sys_segs, &turns);
        for s in mic_segs.drain(..) {
            out.push(Segment { id: 0, source: "mic".into(), speaker: me.to_string(), start: s.start + offset, end: s.end + offset, text: s.text, is_final: true });
        }
        for (s, l) in sys_segs.drain(..).zip(labels) {
            out.push(Segment { id: 0, source: "system".into(), speaker: l, start: s.start + offset, end: s.end + offset, text: s.text, is_final: true });
        }
        offset += dur;
    }
    out.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    Ok(out)
}

// ---------------- silence compaction ----------------

/// A kept stretch of speech: position in the compacted audio and in the original.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub c0: usize,
    pub o0: usize,
    pub len: usize,
}

const GAP: usize = R * 4 / 10;

/// Removes long silences (speech models invent words in silence, and it saves time),
/// keeping a map back to the original timeline.
pub fn compact(a: &[f32]) -> (Vec<f32>, Vec<Span>) {
    let f: Vec<f32> = a.chunks(FRAME).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect();
    if f.is_empty() {
        return (vec![], vec![]);
    }
    let mut sorted = f.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let floor = sorted[sorted.len() / 5];
    let thr = (floor * 3.5).max(0.006);
    let n = f.len();
    let mut keep = vec![false; n];
    for (i, e) in f.iter().enumerate() {
        if *e > thr {
            for k in keep.iter_mut().take((i + 4).min(n)).skip(i.saturating_sub(3)) {
                *k = true;
            }
        }
    }
    // Spans of kept frames; join spans separated by less than a second.
    let mut spans: Vec<(usize, usize)> = vec![];
    let mut i = 0;
    while i < n {
        if keep[i] {
            let s = i;
            while i < n && keep[i] {
                i += 1;
            }
            match spans.last_mut() {
                Some(last) if s - last.1 < 10 => last.1 = i,
                _ => spans.push((s, i)),
            }
        } else {
            i += 1;
        }
    }
    let mut out = vec![];
    let mut map = vec![];
    for (s, e) in spans {
        let (o0, o1) = (s * FRAME, (e * FRAME).min(a.len()));
        if o1 <= o0 {
            continue;
        }
        map.push(Span { c0: out.len(), o0, len: o1 - o0 });
        out.extend_from_slice(&a[o0..o1]);
        out.extend(std::iter::repeat(0.0).take(GAP));
    }
    (out, map)
}

/// Compacted time (s) -> original time (s).
pub fn to_orig(map: &[Span], t: f64) -> f64 {
    let x = (t.max(0.0) * R as f64) as usize;
    let i = map.partition_point(|s| s.c0 <= x).saturating_sub(1);
    match map.get(i) {
        Some(s) => (s.o0 + (x - s.c0.min(x)).min(s.len)) as f64 / R as f64,
        None => t,
    }
}

async fn transcribe_track(st: &St, audio: &[f32], on_progress: impl Fn(f64)) -> Result<Vec<stt::Seg>> {
    let (c, map) = compact(audio);
    if c.len() < R {
        return Ok(vec![]);
    }
    let (cloud_on, cloud, lang) = {
        let s = st.settings.read();
        (s.speech_cloud_on, s.speech_cloud.clone(), s.language.clone())
    };
    let base = if cloud_on && cloud.ready() { None } else { Some(crate::engine::ensure_whisper(st).await?) };
    // Pieces of up to ~4 minutes, cut between spans.
    let max = R * 240;
    let mut pieces: Vec<(usize, usize)> = vec![];
    let mut start = 0;
    for s in &map {
        let end = s.c0 + s.len + GAP;
        if end - start > max && s.c0 > start {
            pieces.push((start, s.c0));
            start = s.c0;
        }
    }
    pieces.push((start, c.len()));
    let mut out: Vec<stt::Seg> = vec![];
    let vocab = st.settings.read().vocabulary.join(", ");
    let mut prompt = vocab.clone();
    for (k, (a, b)) in pieces.iter().enumerate() {
        on_progress(k as f64 / pieces.len() as f64);
        let bytes = wav::encode(&c[*a..*b]);
        let segs = match &base {
            Some(base) => stt::transcribe_segments(base, bytes, &lang, &prompt, Duration::from_secs(900)).await?,
            None => stt::transcribe_cloud(&cloud, bytes, &lang, &prompt).await?,
        };
        let off = *a as f64 / R as f64;
        for s in segs {
            let (s0, s1) = (to_orig(&map, off + s.start), to_orig(&map, off + s.end));
            prompt = s.text.clone();
            out.push(stt::Seg { start: s0, end: s1.max(s0), text: s.text });
        }
        if prompt.len() > 300 {
            prompt = prompt.chars().rev().take(250).collect::<Vec<_>>().into_iter().rev().collect();
        }
    }
    on_progress(1.0);
    Ok(out)
}

async fn diarize_track(st: &St, audio: &[f32]) -> Result<Vec<Turn>> {
    let (c, map) = compact(audio);
    if c.len() < R * 8 {
        return Ok(vec![]);
    }
    let files = crate::engine::diar_files(st).await?;
    let tmp = st.data_dir.join(format!("diar-{}.wav", std::process::id()));
    std::fs::write(&tmp, wav::encode(&c))?;
    let tmp2 = tmp.clone();
    let turns = tokio::task::spawn_blocking(move || diar::diarize(&DiarModels { exe: &files.exe, segmentation: &files.segmentation, embedding: &files.embedding }, &tmp2, None, 1.0)).await??;
    let _ = std::fs::remove_file(&tmp);
    Ok(turns.into_iter().map(|t| Turn { start: to_orig(&map, t.start), end: to_orig(&map, t.end), speaker: t.speaker }).collect())
}

/// "Them" when one voice; "Speaker 1", "Speaker 2"… (in order of first words) when
/// several. Voices with very little talk time are folded into their neighbours
/// (they are usually the same person on a bad line).
pub fn label_speakers(segs: &[stt::Seg], turns: &[Turn]) -> Vec<String> {
    let raw: Vec<Option<u32>> = segs.iter().map(|s| diar::speaker_at(turns, s.start, s.end)).collect();
    let mut talk: std::collections::HashMap<u32, f64> = Default::default();
    for (s, r) in segs.iter().zip(&raw) {
        if let Some(r) = r {
            *talk.entry(*r).or_default() += s.end - s.start;
        }
    }
    let total: f64 = talk.values().sum();
    let minor = |sp: u32| talk.get(&sp).map(|t| *t < total * 0.08).unwrap_or(true);
    let mut ids: Vec<Option<u32>> = raw.clone();
    for i in 0..ids.len() {
        if let Some(sp) = ids[i] {
            if minor(sp) {
                // Previous major speaker, else next.
                let prev = (0..i).rev().find_map(|j| ids[j].filter(|x| !minor(*x)));
                let next = ((i + 1)..ids.len()).find_map(|j| raw[j].filter(|x| !minor(*x)));
                ids[i] = prev.or(next);
            }
        }
    }
    let mut order: Vec<u32> = vec![];
    for x in ids.iter().flatten() {
        if !order.contains(x) {
            order.push(*x);
        }
    }
    if order.len() <= 1 {
        return vec!["Them".into(); segs.len()];
    }
    // Lines no voice was found for: the speaker just before (or after) them.
    let mut last = None;
    let filled: Vec<Option<u32>> = ids.iter().map(|x| { if x.is_some() { last = *x; } x.or(last) }).collect();
    let first = filled.iter().flatten().next().copied();
    filled
        .iter()
        .map(|x| match x.or(first) {
            Some(x) => format!("Speaker {}", order.iter().position(|o| *o == x).unwrap() + 1),
            None => "Them".into(),
        })
        .collect()
}

fn words(t: &str) -> Vec<String> {
    t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 1).map(|w| w.to_string()).collect()
}

/// Without headphones your microphone also hears the other people. Drop mic lines
/// that repeat what the computer's sound said at the same moment.
pub fn dedupe_echo(mic: &mut Vec<stt::Seg>, sys: &[stt::Seg]) {
    mic.retain(|m| {
        let mw = words(&m.text);
        if mw.is_empty() {
            return false;
        }
        let near: Vec<String> = sys.iter().filter(|s| s.end >= m.start - 2.0 && s.start <= m.end + 2.0).flat_map(|s| words(&s.text)).collect();
        if near.is_empty() {
            return true;
        }
        let hit = mw.iter().filter(|w| near.contains(w)).count() as f64 / mw.len() as f64;
        !(hit >= 0.6 && mw.len() >= 2 || (mw.len() == 1 && hit >= 1.0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seg(a: f64, b: f64, t: &str) -> stt::Seg {
        stt::Seg { start: a, end: b, text: t.into() }
    }
    #[test]
    fn compaction_maps_back() {
        let mut a = vec![0.0f32; R * 5];
        a.extend((0..R * 2).map(|i| (i as f32 * 0.07).sin() * 0.3));
        a.extend(vec![0.0f32; R * 10]);
        a.extend((0..R * 3).map(|i| (i as f32 * 0.05).sin() * 0.3));
        a.extend(vec![0.0f32; R * 4]);
        let (c, map) = compact(&a);
        assert_eq!(map.len(), 2);
        assert!(c.len() < R * 8);
        // 0.5 s into the compacted audio = ~0.5 s into the first span (starts ~4.7 s).
        let t = to_orig(&map, 0.5);
        assert!(t > 4.9 && t < 5.4, "{t}");
        let second = map[1].c0 as f64 / R as f64 + 1.0;
        let t2 = to_orig(&map, second);
        assert!(t2 > 17.4 && t2 < 18.0, "{t2}");
    }
    #[test]
    fn echo() {
        let sys = vec![seg(10.0, 13.0, "We should ship the beta on Friday")];
        let mut mic = vec![seg(10.2, 13.1, "we should ship the beta on friday"), seg(20.0, 22.0, "Sounds good to me")];
        dedupe_echo(&mut mic, &sys);
        assert_eq!(mic.len(), 1);
        assert_eq!(mic[0].text, "Sounds good to me");
    }
    #[test]
    fn labels() {
        let segs = vec![seg(0.0, 30.0, "a"), seg(31.0, 60.0, "b"), seg(61.0, 62.0, "c")];
        let turns = vec![Turn { start: 0.0, end: 30.5, speaker: 3 }, Turn { start: 30.5, end: 60.5, speaker: 1 }, Turn { start: 60.5, end: 62.0, speaker: 7 }];
        assert_eq!(label_speakers(&segs, &turns), vec!["Speaker 1", "Speaker 2", "Speaker 2"]);
        let one = vec![Turn { start: 0.0, end: 70.0, speaker: 0 }];
        assert_eq!(label_speakers(&segs, &one), vec!["Them", "Them", "Them"]);
    }
}
