//! A recording session: capture both tracks, transcribe them live as people pause,
//! and hand the meeting to post-processing when it stops.

use crate::capture::{Capture, Chunk};
use crate::db::{now, Meeting, Segment};
use crate::state::St;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use toolbay_engine::{stt, wav};

pub struct Session {
    pub meeting_id: String,
    pub app: String,
    pub cap: Option<Capture>,
    stop: Arc<AtomicBool>,
    pub started: Instant,
    /// Last time either side was clearly talking (for the 15-minute silence stop).
    pub last_voice: Instant,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSegment {
    pub meeting_id: String,
    pub segment: Segment,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecState {
    pub meeting_id: Option<String>,
    pub app: String,
    pub levels: Option<crate::capture::Levels>,
}

pub fn state(st: &St) -> RecState {
    let s = st.session.lock();
    match s.as_ref() {
        Some(x) => RecState { meeting_id: Some(x.meeting_id.clone()), app: x.app.clone(), levels: x.cap.as_ref().map(|c| c.levels()) },
        None => RecState { meeting_id: None, app: String::new(), levels: None },
    }
}

pub fn recording_id(st: &St) -> Option<String> {
    st.session.lock().as_ref().map(|s| s.meeting_id.clone())
}

/// Starts recording. `meeting_id` = an existing note to record into (resume), or None
/// for a new note. Returns the meeting.
pub fn start(st: &St, meeting_id: Option<String>, app: &str, title: &str) -> Result<Meeting> {
    if let Some(id) = recording_id(st) {
        if Some(&id) == meeting_id.as_ref() {
            return st.db.meeting(&id)?.ok_or_else(|| anyhow!("missing meeting"));
        }
        return Err(anyhow!("Already taking notes in another meeting. Stop that one first."));
    }
    let s = st.settings.read().clone();
    let m = match meeting_id.and_then(|id| st.db.meeting(&id).ok().flatten()) {
        Some(m) => m,
        None => {
            let m = Meeting {
                id: uuid::Uuid::new_v4().to_string(),
                title: title.to_string(),
                created_at: now(),
                started_at: Some(now()),
                app: app.to_string(),
                status: "recording".into(),
                template_id: s.default_template.clone(),
                ..Default::default()
            };
            st.db.create_meeting(&m)?;
            m
        }
    };
    // Each recording run gets its own folder; a resumed note has several "parts".
    let part = format!("part-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let dir = st.meetings_dir().join(&m.id).join(&part);
    let cap = Capture::start(&dir, s.microphone.clone(), s.capture_system)?;
    let offset = st.db.segments(&m.id)?.iter().map(|x| x.end).fold(m.duration_s, f64::max);
    st.db.set(&m.id, "status", "recording")?;
    st.db.set(&m.id, "step", "")?;
    st.db.set(&m.id, "error", "")?;
    st.db.set_has_audio(&m.id, true)?;
    if !app.is_empty() && m.app.is_empty() {
        st.db.set(&m.id, "app", app)?;
    }
    let stop = Arc::new(AtomicBool::new(false));
    *st.session.lock() = Some(Session { meeting_id: m.id.clone(), app: app.to_string(), cap: Some(cap), stop: stop.clone(), started: Instant::now(), last_voice: Instant::now() });
    log::info!("recording started: {} ({})", m.id, if app.is_empty() { "manual" } else { app });
    live_loop(st.clone(), m.id.clone(), offset, stop);
    crate::refresh_tray(st);
    st.meeting_changed(&m.id);
    st.emit("recording", state(st));
    Ok(st.db.meeting(&m.id)?.unwrap_or(m))
}

pub fn pause(st: &St, paused: bool) {
    if let Some(s) = st.session.lock().as_ref() {
        if let Some(c) = &s.cap {
            if paused { c.pause() } else { c.resume() }
            let _ = st.db.set(&s.meeting_id, "status", if paused { "paused" } else { "recording" });
            st.meeting_changed(&s.meeting_id);
        }
    }
    st.emit("recording", state(st));
}

/// Stops recording and starts post-processing (final transcript, speakers, notes).
pub fn stop(st: &St) -> Option<String> {
    let sess = st.session.lock().take()?;
    sess.stop.store(true, Ordering::SeqCst);
    let id = sess.meeting_id.clone();
    let cap = sess.cap;
    let st2 = st.clone();
    std::thread::spawn(move || {
        if let Some(c) = cap {
            let secs = c.secs();
            let dir = c.stop();
            let prev = st2.db.meeting(&id).ok().flatten().map(|m| m.duration_s).unwrap_or(0.0);
            let _ = st2.db.set_duration(&id, prev + secs);
            log::info!("recording stopped: {id} ({secs:.0} s) in {}", dir.display());
        }
        let _ = st2.db.set(&id, "ended_at", &now());
        let _ = st2.db.set(&id, "status", "processing");
        let _ = st2.db.set(&id, "step", "Finishing the transcript…");
        st2.meeting_changed(&id);
        let st3 = st2.clone();
        tauri::async_runtime::spawn(async move {
            crate::process::run(&st3, &id, None).await;
        });
    });
    crate::refresh_tray(st);
    st.emit("recording", state(st));
    Some(sess.meeting_id)
}

fn speaker_for(system: bool, me: &str) -> String {
    if system { "Them".into() } else { me.into() }
}

/// Live transcription: whenever a track reaches a pause, send that piece to the
/// speech engine and show the text. Runs until the session stops.
fn live_loop(st: St, id: String, offset: f64, stop: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        let mut last_levels = Instant::now();
        let mut flushed = false;
        let mut prompt: [String; 2] = [String::new(), String::new()];
        loop {
            let stopping = stop.load(Ordering::SeqCst);
            if stopping && flushed {
                break;
            }
            let mut got = false;
            for system in [false, true] {
                let chunk: Option<Chunk> = {
                    let s = st.session.lock();
                    match s.as_ref().filter(|x| x.meeting_id == id).and_then(|x| x.cap.as_ref()) {
                        Some(c) => c.take_chunk(system, 2.5, 14.0, false),
                        None => None,
                    }
                };
                let Some(c) = chunk else { continue };
                got = true;
                if let Some(s) = st.session.lock().as_mut() {
                    s.last_voice = Instant::now();
                }
                match transcribe_live(&st, &c, &prompt[system as usize]).await {
                    Ok(segs) => {
                        let me = st.settings.read().me();
                        for sg in segs {
                            let seg = Segment {
                                id: 0,
                                source: if system { "system".into() } else { "mic".into() },
                                speaker: speaker_for(system, &me),
                                start: offset + c.start + sg.start,
                                end: offset + c.start + sg.end.max(sg.start),
                                text: sg.text.clone(),
                                is_final: false,
                            };
                            let p = &mut prompt[system as usize];
                            p.push(' ');
                            p.push_str(&sg.text);
                            if p.len() > 400 {
                                *p = p[p.len() - 300..].to_string();
                                while !p.is_char_boundary(0) {
                                    p.remove(0);
                                }
                            }
                            if let Ok(rowid) = st.db.add_segment(&id, &seg) {
                                st.emit("live-segment", LiveSegment { meeting_id: id.clone(), segment: Segment { id: rowid, ..seg } });
                            }
                        }
                    }
                    Err(e) => log::warn!("live transcription: {e:#}"),
                }
            }
            if stopping {
                flushed = true;
            }
            if last_levels.elapsed() > Duration::from_millis(100) {
                last_levels = Instant::now();
                st.emit("recording", state(&st));
                auto_stop_check(&st, &id);
            }
            if !got {
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
        }
    });
}

/// Stops after 15 minutes with no speech on either side (Granola does the same).
fn auto_stop_check(st: &St, id: &str) {
    let idle = st.session.lock().as_ref().filter(|s| s.meeting_id == id).map(|s| s.last_voice.elapsed() > Duration::from_secs(15 * 60)).unwrap_or(false);
    if idle {
        log::info!("auto-stop: 15 minutes of silence");
        stop(st);
    }
}

async fn transcribe_live(st: &St, c: &Chunk, prompt: &str) -> Result<Vec<stt::Seg>> {
    let (cloud_on, cloud, lang) = {
        let s = st.settings.read();
        (s.speech_cloud_on, s.speech_cloud.clone(), s.language.clone())
    };
    // A little silence on both ends helps whisper with the first and last word.
    let mut padded = vec![0f32; 3200];
    padded.extend_from_slice(&c.audio);
    padded.extend(vec![0f32; 3200]);
    let bytes = wav::encode(&padded);
    let mut segs = if cloud_on && cloud.ready() {
        stt::transcribe_cloud(&cloud, bytes, &lang, prompt).await?
    } else {
        let base = crate::engine::ensure_whisper(st).await?;
        stt::transcribe_segments(&base, bytes, &lang, prompt, Duration::from_secs(60)).await?
    };
    let dur = c.audio.len() as f64 / wav::RATE as f64;
    for s in segs.iter_mut() {
        s.start = (s.start - 0.2).clamp(0.0, dur);
        s.end = (s.end - 0.2).clamp(s.start, dur);
        if s.end <= s.start {
            s.end = dur;
        }
    }
    Ok(segs)
}
