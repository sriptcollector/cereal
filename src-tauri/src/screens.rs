//! Screenshots during a meeting: the floating indicator's camera button puts the screen
//! into the note (like Granola's capture button).

use crate::state::St;
use anyhow::{anyhow, Result};
use base64::Engine;

/// The screen the call is most likely on: the one under the mouse, else the primary.
fn capture() -> Result<image::RgbaImage> {
    let mons = xcap::Monitor::all()?;
    let m = mons.iter().find(|m| m.is_primary().unwrap_or(false)).or(mons.first()).ok_or_else(|| anyhow!("No screen found"))?;
    Ok(m.capture_image()?)
}

/// JPEG data URL, at most 1600 px wide (Granola scales to 1600 too).
pub fn data_url(img: image::RgbaImage) -> Result<String> {
    let img = image::DynamicImage::ImageRgba8(img);
    let img = if img.width() > 1600 { img.resize(1600, 1600 * img.height() / img.width(), image::imageops::FilterType::Triangle) } else { img };
    let mut buf = std::io::Cursor::new(vec![]);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80).encode_image(&img.to_rgb8())?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner())))
}

/// Adds a screenshot to the end of the note being recorded.
pub fn screenshot_into_note(st: &St) -> Result<String> {
    let id = crate::recorder::recording_id(st).ok_or_else(|| anyhow!("Not taking notes right now."))?;
    let url = data_url(capture()?)?;
    let m = st.db.meeting(&id)?.ok_or_else(|| anyhow!("missing meeting"))?;
    let at = chrono::Local::now().format("%-I:%M %p");
    let mut notes = m.my_notes.trim_end().to_string();
    if !notes.is_empty() {
        notes.push_str("\n\n");
    }
    notes.push_str(&format!("![Screenshot {at}]({url})\n"));
    st.db.set(&id, "my_notes", &notes)?;
    st.meeting_changed(&id);
    Ok(id)
}

// ---------------- snapshots of shared screens (Settings, off by default) ----------------

/// 64x36 grey thumbnail for change detection.
fn thumb(img: &image::RgbaImage) -> Vec<u8> {
    image::imageops::resize(&image::DynamicImage::ImageRgba8(img.clone()).to_luma8(), 64, 36, image::imageops::FilterType::Triangle).into_raw()
}

fn diff(a: &[u8], b: &[u8]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f64).sum::<f64>() / a.len().max(1) as f64
}

/// While recording: every 15 s, keep the screen when it changed noticeably (a new slide,
/// another app shared). At most 40 per meeting, saved with the meeting.
pub fn snapshot_loop(st: St, meeting_id: String) {
    std::thread::spawn(move || {
        let dir = st.meetings_dir().join(&meeting_id).join("screens");
        let mut last: Option<Vec<u8>> = None;
        let mut kept = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(15));
            let still = st.session.lock().as_ref().map(|s| s.meeting_id == meeting_id).unwrap_or(false);
            if !still || kept >= 40 {
                return;
            }
            if !st.settings.read().screen_snapshots {
                continue;
            }
            let Ok(img) = capture() else { continue };
            let t = thumb(&img);
            if last.as_ref().map(|l| diff(l, &t) < 10.0).unwrap_or(false) {
                continue;
            }
            last = Some(t);
            let _ = std::fs::create_dir_all(&dir);
            let img = image::DynamicImage::ImageRgba8(img);
            let img = if img.width() > 1600 { img.resize(1600, 1600 * img.height() / img.width(), image::imageops::FilterType::Triangle) } else { img };
            let f = dir.join(format!("{}.jpg", chrono::Local::now().format("%H%M%S")));
            if img.to_rgb8().save_with_format(&f, image::ImageFormat::Jpeg).is_ok() {
                kept += 1;
                st.emit("screens-changed", meeting_id.clone());
            }
        }
    });
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub name: String,
    /// Small preview (data URL).
    pub thumb: String,
}

pub fn list(st: &St, id: &str) -> Vec<Shot> {
    let dir = st.meetings_dir().join(id).join("screens");
    let mut v: Vec<_> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "jpg").unwrap_or(false)).collect()).unwrap_or_default();
    v.sort();
    v.into_iter()
        .filter_map(|p| {
            let img = image::open(&p).ok()?;
            let small = img.resize(320, 180, image::imageops::FilterType::Triangle);
            let mut buf = std::io::Cursor::new(vec![]);
            small.to_rgb8().write_to(&mut buf, image::ImageFormat::Jpeg).ok()?;
            Some(Shot { name: p.file_name()?.to_string_lossy().to_string(), thumb: format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner())) })
        })
        .collect()
}

fn shot_path(st: &St, id: &str, name: &str) -> Result<std::path::PathBuf> {
    if name.contains(['/', '\\']) || name.contains("..") {
        return Err(anyhow!("bad name"));
    }
    Ok(st.meetings_dir().join(id).join("screens").join(name))
}

/// Puts a snapshot into the note (your notes).
pub fn add_to_note(st: &St, id: &str, name: &str) -> Result<()> {
    let img = image::open(shot_path(st, id, name)?)?.to_rgba8();
    let url = data_url(img)?;
    let m = st.db.meeting(id)?.ok_or_else(|| anyhow!("missing meeting"))?;
    let mut notes = m.my_notes.trim_end().to_string();
    if !notes.is_empty() {
        notes.push_str("\n\n");
    }
    notes.push_str(&format!("![Screen {}]({url})\n", name.trim_end_matches(".jpg")));
    st.db.set(id, "my_notes", &notes)?;
    st.meeting_changed(id);
    Ok(())
}

pub fn remove(st: &St, id: &str, name: &str) -> Result<()> {
    std::fs::remove_file(shot_path(st, id, name)?)?;
    Ok(())
}
