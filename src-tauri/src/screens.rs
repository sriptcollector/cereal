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
