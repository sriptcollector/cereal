//! Headless modes for testing and benchmarks (use the debug build; the release exe
//! has no console):
//!
//!   toolbay-notes --record <secs> <dir>         record mic + system audio to <dir>
//!   toolbay-notes --detect                      print calls using the mic right now
//!   toolbay-notes --process <mic.wav> <system.wav> [template] [--notes "my notes"]
//!                                               full after-call pipeline, prints timings,
//!                                               transcript and notes (uses the app's models)

use crate::state::St;
use std::path::PathBuf;
use std::time::Instant;

fn data_dirs() -> (PathBuf, PathBuf) {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default()
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support")).unwrap_or_default()
    };
    // Same folders as the app (identifier ai.toolbay.notes), unless overridden.
    let d = std::env::var_os("TOOLBAY_NOTES_DATA").map(PathBuf::from).unwrap_or_else(|| base.join("ai.toolbay.notes"));
    (d.clone(), d)
}

fn bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("bundled"), dir.join("../../src-tauri/bundled"), dir.join("../../../src-tauri/bundled")].into_iter().find(|p| p.exists())
}

pub fn run(args: &[String]) -> Option<i32> {
    let pos = |f: &str| args.iter().position(|a| a == f);
    if let Some(i) = pos("--detect") {
        let _ = i;
        println!("mic users: {:?}", crate::detect::mic_users());
        println!("calls: {:?}", crate::detect::active_calls());
        return Some(0);
    }
    if let Some(i) = pos("--record") {
        let secs: f64 = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
        let dir = PathBuf::from(args.get(i + 2).cloned().unwrap_or_else(|| "rec-test".into()));
        let cap = match crate::capture::Capture::start(&dir, None, true) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e:#}");
                return Some(1);
            }
        };
        let t = Instant::now();
        while t.elapsed().as_secs_f64() < secs {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let l = cap.levels();
            println!("{:5.1}s  mic {:.2} ({})  system {:.2} ({}) {:?}", l.secs, l.mic, l.mic_device, l.system, l.system_device, l.system_error);
        }
        let d = cap.stop();
        println!("wrote {}", d.display());
        return Some(0);
    }
    if let Some(i) = pos("--diarize") {
        // --diarize <wav> <threshold> [num-threads]: speaker split on the compacted audio.
        let wavp = PathBuf::from(args.get(i + 1)?);
        let thr: f32 = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(0.8);
        let audio = toolbay_engine::wav::decode(&std::fs::read(&wavp).ok()?)?;
        let (c, _) = crate::process::compact(&audio);
        let tmp = std::env::temp_dir().join("tbn-diar.wav");
        std::fs::write(&tmp, toolbay_engine::wav::encode(&c)).ok()?;
        let b = bundled()?;
        let exe = toolbay_engine::store::find_file(&b.join("engines").join("sherpa-cpu"), if cfg!(windows) { "sherpa-onnx-offline-speaker-diarization.exe" } else { "sherpa-onnx-offline-speaker-diarization" })?;
        let m = toolbay_engine::diar::DiarModels { exe: &exe, segmentation: &b.join("models").join("sherpa-onnx-pyannote-segmentation-3-0.onnx"), embedding: &b.join("models").join("3dspeaker_speech_campplus_sv_en_voxceleb_16k.onnx") };
        let t = Instant::now();
        let turns = toolbay_engine::diar::diarize(&m, &tmp, None, thr);
        match turns {
            Ok(turns) => {
                let mut talk = std::collections::BTreeMap::<u32, f64>::new();
                for t in &turns {
                    *talk.entry(t.speaker).or_default() += t.end - t.start;
                }
                println!("compacted {:.0}s of {:.0}s, threshold {thr}: {} speakers in {:.1}s: {:?}", c.len() as f64 / 16000.0, audio.len() as f64 / 16000.0, talk.len(), t.elapsed().as_secs_f64(), talk.iter().map(|(k, v)| (*k, v.round())).collect::<Vec<_>>());
            }
            Err(e) => println!("error {e:#}"),
        }
        return Some(0);
    }
    if let Some(i) = pos("--ask") {
        // --ask <meeting-id|all> "<question>": chat answer, saved to the meeting's chat.
        let scope = args.get(i + 1)?.clone();
        let q = args.get(i + 2)?.clone();
        let (cfg, data) = data_dirs();
        let st = St::new(None, cfg, data, bundled()).ok()?;
        let t = Instant::now();
        let r = tauri::async_runtime::block_on(crate::notes::ask(&st, if scope == "all" { None } else { Some(scope) }, &q, "cli"));
        match r {
            Ok(a) => println!("{a}\n=== {:.1}s", t.elapsed().as_secs_f64()),
            Err(e) => println!("error: {e:#}"),
        }
        return Some(0);
    }
    if let Some(i) = pos("--process") {
        let mic = PathBuf::from(args.get(i + 1)?);
        let sys = PathBuf::from(args.get(i + 2)?);
        let template = args.get(i + 3).filter(|a| !a.starts_with("--")).cloned();
        let my_notes = pos("--notes").and_then(|j| args.get(j + 1).cloned()).unwrap_or_default();
        let (cfg, data) = data_dirs();
        let st = match St::new(None, cfg, data, bundled()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e:#}");
                return Some(1);
            }
        };
        let code = tauri::async_runtime::block_on(async move {
            let id = format!("cli-{}", uuid::Uuid::new_v4().simple());
            let m = crate::db::Meeting { id: id.clone(), created_at: crate::db::now(), status: "processing".into(), template_id: template.clone().unwrap_or_else(|| "auto".into()), my_notes, ..Default::default() };
            st.db.create_meeting(&m).unwrap();
            let part = st.meetings_dir().join(&id).join("part-0");
            std::fs::create_dir_all(&part).unwrap();
            std::fs::copy(&mic, part.join("mic.wav")).unwrap();
            std::fs::copy(&sys, part.join("system.wav")).unwrap();
            let t = Instant::now();
            let r = crate::process::pipeline(&st, &id, template).await;
            let total = t.elapsed().as_secs_f64();
            let code = match r {
                Ok(timing) => {
                    let m = st.db.meeting(&id).unwrap().unwrap();
                    let segs = st.db.segments(&id).unwrap();
                    let speakers: std::collections::BTreeSet<String> = segs.iter().map(|s| s.speaker.clone()).collect();
                    println!("=== TRANSCRIPT ({} segments, speakers {:?})", segs.len(), speakers);
                    for l in crate::notes::transcript_lines(&segs, &serde_json::json!({})).iter().take(400) {
                        println!("{l}");
                    }
                    println!("\n=== TITLE: {}\n=== NOTES\n{}", m.title, m.enhanced);
                    println!(
                        "\n=== TIMING audio {:.0}s | transcribe {:.1}s | speakers {:.1}s (parallel) | transcript ready {:.1}s | notes {:.1}s | total {:.1}s",
                        timing.audio_s, timing.transcribe_s, timing.diarize_s, timing.transcript_wall_s, timing.notes_s, total
                    );
                    0
                }
                Err(e) => {
                    eprintln!("failed: {e:#}");
                    1
                }
            };
            if std::env::var("KEEP").is_ok() {
                println!("=== KEPT meeting {id}");
                let _ = st.db.set(&id, "status", "ready");
                let _ = st.db.set(&id, "app", "Zoom");
            } else {
                let _ = st.db.delete_meeting(&id);
                let _ = std::fs::remove_dir_all(st.meetings_dir().join(&id));
            }
            code
        });
        return Some(code);
    }
    None
}
