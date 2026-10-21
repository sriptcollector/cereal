//! Commands for organising, chat threads, calendar, import/export and voice questions.

use crate::calendar::{self, Event};
use crate::db::{now, ChatMsg, Meeting};
use crate::library::{Company, FileItem, Folder, Person, Recipe, Thread};
use crate::state::St;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

type R<T> = Result<T, String>;

fn e<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------- folders & files ----------------

#[tauri::command]
pub fn folders_list(st: State<St>) -> R<Vec<Folder>> {
    st.db.folders().map_err(e)
}

#[tauri::command]
pub fn folder_save(st: State<St>, folder: Folder) -> R<Folder> {
    let f = st.db.save_folder(&folder).map_err(e)?;
    st.emit("folders-changed", ());
    Ok(f)
}

#[tauri::command]
pub fn folder_delete(st: State<St>, id: String) -> R<()> {
    st.db.delete_folder(&id).map_err(e)?;
    st.emit("folders-changed", ());
    Ok(())
}

#[tauri::command]
pub fn files_list(st: State<St>, folder: String) -> R<Vec<FileItem>> {
    st.db.files(&folder).map_err(e)
}

/// Adds a document to a folder as context for chat. Text files are read as is;
/// other files get a plain-words error.
#[tauri::command]
pub fn file_add(st: State<St>, folder: String, path: String) -> R<()> {
    let p = PathBuf::from(&path);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    let text = match ext.as_str() {
        "txt" | "md" | "markdown" | "csv" | "json" | "html" | "htm" | "xml" | "log" | "rtf" | "tsv" | "yaml" | "yml" => std::fs::read(&p).map(|b| String::from_utf8_lossy(&b).to_string()).map_err(e)?,
        "docx" => docx_text(&p).ok_or("Couldn't read that Word file.")?,
        _ => return Err("Cereal can read text, Markdown, CSV and Word (.docx) files. Save it as one of those and try again.".into()),
    };
    st.db.add_file(&folder, &name, &text).map_err(e)
}

fn docx_text(p: &Path) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name("word/document.xml").ok()?, &mut xml).ok()?;
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for c in xml.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                if tag.starts_with("/w:p") {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    Some(out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'"))
}

#[tauri::command]
pub fn file_delete(st: State<St>, id: String) -> R<()> {
    st.db.delete_file(&id).map_err(e)
}

// ---------------- trash ----------------

#[tauri::command]
pub fn meeting_trash(st: State<St>, id: String) -> R<()> {
    if crate::recorder::recording_id(&st).as_deref() == Some(id.as_str()) {
        crate::recorder::stop(&st);
    }
    st.db.trash_meeting(&id).map_err(e)?;
    st.emit("meeting-deleted", id);
    Ok(())
}

#[tauri::command]
pub fn meeting_restore(st: State<St>, id: String) -> R<()> {
    st.db.restore_meeting(&id).map_err(e)?;
    st.meeting_changed(&id);
    Ok(())
}

#[tauri::command]
pub fn meeting_purge(st: State<St>, id: String) -> R<()> {
    st.db.delete_meeting(&id).map_err(e)?;
    let _ = std::fs::remove_dir_all(st.meetings_dir().join(&id));
    Ok(())
}

#[tauri::command]
pub fn trash_list(st: State<St>) -> R<Vec<Meeting>> {
    st.db.trash().map_err(e)
}

// ---------------- people & companies ----------------

#[tauri::command]
pub fn people_list(st: State<St>) -> R<Vec<Person>> {
    st.db.people().map_err(e)
}

#[tauri::command]
pub fn companies_list(st: State<St>) -> R<Vec<Company>> {
    st.db.companies().map_err(e)
}

#[tauri::command]
pub fn meetings_with(st: State<St>, person: Option<String>, domain: Option<String>) -> R<Vec<Meeting>> {
    st.db.meetings_with(person.as_deref(), domain.as_deref()).map_err(e)
}

// ---------------- recipes ----------------

#[tauri::command]
pub fn recipes_list(st: State<St>) -> R<Vec<Recipe>> {
    st.db.recipes().map_err(e)
}

#[tauri::command]
pub fn recipe_save(st: State<St>, recipe: Recipe) -> R<Recipe> {
    st.db.save_recipe(&recipe).map_err(e)
}

#[tauri::command]
pub fn recipe_delete(st: State<St>, id: String) -> R<()> {
    st.db.delete_recipe(&id).map_err(e)
}

// ---------------- chat threads ----------------

#[tauri::command]
pub fn threads_list(st: State<St>, scope: Option<String>, scope_id: Option<String>) -> R<Vec<Thread>> {
    st.db.threads(scope.as_deref(), scope_id.as_deref(), 50).map_err(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadFull {
    thread: Option<Thread>,
    msgs: Vec<ChatMsg>,
    sources: Vec<crate::notes::Source>,
}

#[tauri::command]
pub fn thread_get(st: State<St>, id: String) -> R<ThreadFull> {
    Ok(ThreadFull { thread: st.db.thread(&id).map_err(e)?, msgs: st.db.thread_msgs(&id).map_err(e)?, sources: crate::notes::thread_sources(&st, &id) })
}

#[tauri::command]
pub fn thread_delete(st: State<St>, id: String) -> R<()> {
    st.db.delete_thread(&id).map_err(e)
}

#[tauri::command]
pub async fn chat_send(
    st: State<'_, St>,
    thread_id: Option<String>,
    scope: String,
    scope_id: String,
    question: String,
    attachment: Option<String>,
    request_id: String,
) -> R<crate::notes::Answer> {
    crate::notes::chat(&st, thread_id, &scope, &scope_id, &question, attachment, &request_id).await.map_err(|err| format!("{err:#}"))
}

// ---------------- calendar ----------------

#[tauri::command]
pub fn calendar_events() -> Vec<Event> {
    calendar::events()
}

#[tauri::command]
pub async fn calendar_refresh(st: State<'_, St>) -> R<usize> {
    calendar::refresh(&st).await.map_err(|err| format!("Couldn't read that calendar: {err:#}"))
}

/// The note for a calendar event: the existing one, or a new draft with its title and people.
#[tauri::command]
pub fn note_for_event(st: State<St>, event_id: String) -> R<Meeting> {
    if let Some(m) = st.db.meetings(None).map_err(e)?.into_iter().find(|m| m.event_id == event_id) {
        return Ok(m);
    }
    let ev = calendar::events().into_iter().find(|x| x.id == event_id).ok_or("That event isn't in your calendar anymore.")?;
    let m = Meeting {
        id: uuid::Uuid::new_v4().to_string(),
        title: ev.title.clone(),
        created_at: now(),
        status: "draft".into(),
        template_id: st.settings.read().default_template.clone(),
        folder: st.db.auto_folder_for(&ev.title).unwrap_or_default(),
        ..Default::default()
    };
    st.db.create_meeting(&m).map_err(e)?;
    st.db.set(&m.id, "attendees", &ev.attendees.join(", ")).map_err(e)?;
    st.db.set(&m.id, "event_id", &ev.id).map_err(e)?;
    st.db.meeting(&m.id).map_err(e)?.ok_or_else(|| "missing".into())
}

#[tauri::command]
pub async fn brief_for_event(st: State<'_, St>, event_id: String) -> R<String> {
    let ev = calendar::events().into_iter().find(|x| x.id == event_id).ok_or("That event isn't in your calendar anymore.")?;
    crate::notes::brief(&st, &ev.attendees, &ev.title).await.map_err(|err| format!("{err:#}"))
}

/// From the "meeting starting" popup: open the call link, open the note, start notes.
#[tauri::command]
pub fn join_event(app: AppHandle, st: State<St>, event_id: String, record: bool) -> R<Meeting> {
    crate::hide_popup(&st);
    let m = note_for_event(st.clone(), event_id.clone())?;
    if let Some(ev) = calendar::events().into_iter().find(|x| x.id == event_id) {
        if !ev.link.is_empty() {
            use tauri_plugin_opener::OpenerExt;
            let _ = app.opener().open_url(&ev.link, None::<&str>);
        }
    }
    let m = if record { crate::recorder::start(&st, Some(m.id.clone()), "", "").map_err(e)? } else { m };
    if st.settings.read().reposition {
        crate::dock_beside_call(&app);
    }
    crate::show_hub(&app, Some(&format!("meeting:{}", m.id)));
    Ok(m)
}

#[tauri::command]
pub fn dock_window(app: AppHandle) {
    crate::dock_beside_call(&app);
}

// ---------------- voice question (dictate into the chat box) ----------------

static DICTATION: once_cell::sync::Lazy<parking_lot::Mutex<Option<crate::capture::Capture>>> = once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(None));

#[tauri::command]
pub fn dictate_start(st: State<St>) -> R<()> {
    if st.session.lock().is_some() {
        // During a meeting the mic is already being recorded; use a separate short capture anyway.
    }
    let dir = std::env::temp_dir().join(format!("cereal-ask-{}", std::process::id()));
    let mic = st.settings.read().microphone.clone();
    let cap = crate::capture::Capture::start(&dir, mic, false).map_err(e)?;
    *DICTATION.lock() = Some(cap);
    Ok(())
}

#[tauri::command]
pub async fn dictate_stop(st: State<'_, St>) -> R<String> {
    let Some(cap) = DICTATION.lock().take() else { return Ok(String::new()) };
    let dir = tauri::async_runtime::spawn_blocking(move || cap.stop()).await.map_err(e)?;
    let audio = std::fs::read(dir.join("mic.wav")).ok().and_then(|b| toolbay_engine::wav::decode(&b)).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    if audio.len() < 8000 {
        return Ok(String::new());
    }
    let lang = st.settings.read().language.clone();
    let base = crate::engine::ensure_whisper(&st).await.map_err(|err| format!("{err:#}"))?;
    toolbay_engine::stt::transcribe(&base, toolbay_engine::wav::encode(&audio), &lang, "").await.map_err(|err| format!("{err:#}"))
}

// ---------------- import & export ----------------

/// Decodes any common audio file (mp3, m4a, wav, flac, ogg…) to 16 kHz mono.
fn decode_audio(path: &Path) -> anyhow::Result<Vec<f32>> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|x| x.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow::anyhow!("no audio track"))?.clone();
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let rate = track.codec_params.sample_rate.unwrap_or(16_000);
    let mut mono = vec![];
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }
        let Ok(buf) = dec.decode(&packet) else { continue };
        let spec = *buf.spec();
        let ch = spec.channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
        sb.copy_interleaved_ref(buf);
        for f in sb.samples().chunks(ch) {
            mono.push(f.iter().sum::<f32>() / ch as f32);
        }
    }
    Ok(toolbay_engine::wav::resample(&mono, rate, 16_000))
}

/// Imports a recording (Granola can't): it becomes a note, transcribed and summarised
/// like a live one. The audio goes on the "Them" track, so speakers are told apart.
#[tauri::command]
pub async fn import_audio(st: State<'_, St>, path: String) -> R<Meeting> {
    let st = st.inner().clone();
    let p = PathBuf::from(&path);
    let p2 = p.clone();
    let audio = tauri::async_runtime::spawn_blocking(move || decode_audio(&p2)).await.map_err(e)?.map_err(|err| format!("Couldn't read that audio file ({err}). Try an MP3, M4A, WAV or FLAC file."))?;
    if audio.len() < 16_000 {
        return Err("That file has less than a second of audio.".into());
    }
    let title = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let m = Meeting { id: uuid::Uuid::new_v4().to_string(), title, created_at: now(), status: "processing".into(), template_id: st.settings.read().default_template.clone(), app: "Imported file".into(), ..Default::default() };
    st.db.create_meeting(&m).map_err(e)?;
    let part = st.meetings_dir().join(&m.id).join("part-import");
    std::fs::create_dir_all(&part).map_err(e)?;
    std::fs::write(part.join("system.wav"), toolbay_engine::wav::encode(&audio)).map_err(e)?;
    std::fs::write(part.join("mic.wav"), toolbay_engine::wav::encode(&vec![0.0; audio.len()])).map_err(e)?;
    st.db.set_duration(&m.id, audio.len() as f64 / 16_000.0).map_err(e)?;
    st.db.set_has_audio(&m.id, true).map_err(e)?;
    st.db.set(&m.id, "step", "Transcribing your file…").map_err(e)?;
    let id = m.id.clone();
    let st2 = st.clone();
    // Imported audio always gets the full-quality pass.
    tauri::async_runtime::spawn(async move {
        let fp = st2.settings.read().final_pass;
        if !fp {
            let mut s = st2.settings.read().clone();
            s.final_pass = true;
            *st2.settings.write() = s;
        }
        crate::process::run(&st2, &id, None).await;
        if !fp {
            st2.settings.write().final_pass = false;
        }
    });
    st.db.meeting(&m.id).map_err(e)?.ok_or_else(|| "missing".into())
}

fn downloads(app: &AppHandle) -> R<PathBuf> {
    app.path().download_dir().map_err(e)
}

fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '-' }).collect();
    let safe = if safe.trim().is_empty() { "Cereal export".to_string() } else { safe.trim().to_string() };
    let mut p = dir.join(format!("{safe}.{ext}"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{safe} ({i}).{ext}"));
        i += 1;
    }
    p
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Minimal Word document from Markdown (headings, bullets, checkboxes, paragraphs).
fn docx_bytes(md: &str) -> anyhow::Result<Vec<u8>> {
    let mut body = String::new();
    for line in md.lines() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            continue;
        }
        let indent = (t.len() - t.trim_start().len()) / 2;
        let tt = t.trim_start();
        let (style, text) = if let Some(h) = tt.strip_prefix("# ") {
            ("Title", h.to_string())
        } else if let Some(h) = tt.strip_prefix("## ").or_else(|| tt.strip_prefix("### ")) {
            ("Heading1", h.to_string())
        } else if let Some(b) = tt.strip_prefix("- [ ] ") {
            ("ListBullet", format!("☐ {b}"))
        } else if let Some(b) = tt.strip_prefix("- [x] ") {
            ("ListBullet", format!("☑ {b}"))
        } else if let Some(b) = tt.strip_prefix("- ").or_else(|| tt.strip_prefix("* ")) {
            (if indent > 0 { "ListBullet2" } else { "ListBullet" }, b.to_string())
        } else {
            ("Normal", tt.to_string())
        };
        let text = text.replace("**", "").replace('_', "");
        body.push_str(&format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#, xml(&text)));
    }
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#);
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:spacing w:after="80"/></w:pPr><w:rPr><w:sz w:val="22"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:pPr><w:spacing w:before="240" w:after="80"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListBullet"><w:name w:val="List Bullet"/><w:pPr><w:ind w:left="360" w:hanging="200"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="ListBullet2"><w:name w:val="List Bullet 2"/><w:pPr><w:ind w:left="720" w:hanging="200"/></w:pPr></w:style></w:styles>"#;
    let ct = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let docrels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in [("[Content_Types].xml", ct.to_string()), ("_rels/.rels", rels.to_string()), ("word/document.xml", doc), ("word/styles.xml", styles.to_string()), ("word/_rels/document.xml.rels", docrels.to_string())] {
            z.start_file(name, o)?;
            z.write_all(data.as_bytes())?;
        }
        z.finish()?;
    }
    Ok(buf.into_inner())
}

#[tauri::command]
pub fn export_docx(app: AppHandle, name: String, markdown: String) -> R<String> {
    let p = unique(&downloads(&app)?, &name, "docx");
    std::fs::write(&p, docx_bytes(&markdown).map_err(e)?).map_err(e)?;
    crate::commands::reveal(&p);
    Ok(p.to_string_lossy().to_string())
}

/// Everything in one file: CSV (title, date, notes, transcript) or JSON (full detail).
#[tauri::command]
pub fn export_all(app: AppHandle, st: State<St>, kind: String) -> R<String> {
    let meetings = st.db.meetings(None).map_err(e)?;
    let dir = downloads(&app)?;
    let p = if kind == "json" {
        let mut all = vec![];
        for m in &meetings {
            all.push(serde_json::json!({ "meeting": m, "transcript": st.db.segments(&m.id).unwrap_or_default() }));
        }
        let p = unique(&dir, &format!("Cereal notes {}", chrono::Local::now().format("%Y-%m-%d")), "json");
        std::fs::write(&p, serde_json::to_string_pretty(&all).map_err(e)?).map_err(e)?;
        p
    } else {
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut csv = String::from("title,date,duration_minutes,attendees,folder,notes,my_notes,transcript\n");
        let folders = st.db.folders().unwrap_or_default();
        for m in &meetings {
            let fname = folders.iter().find(|f| f.id == m.folder).map(|f| f.name.clone()).unwrap_or_default();
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                q(&m.title),
                q(&m.created_at),
                (m.duration_s / 60.0).round(),
                q(&m.attendees),
                q(&fname),
                q(&m.enhanced),
                q(&m.my_notes),
                q(&st.db.transcript_text(&m.id).unwrap_or_default())
            ));
        }
        let p = unique(&dir, &format!("Cereal notes {}", chrono::Local::now().format("%Y-%m-%d")), "csv");
        std::fs::write(&p, csv).map_err(e)?;
        p
    };
    crate::commands::reveal(&p);
    Ok(p.to_string_lossy().to_string())
}

/// Posts finished notes to the folder's webhook (Slack incoming webhook or any URL).
pub async fn post_to_folder_webhook(st: &St, id: &str) {
    let Ok(Some(m)) = st.db.meeting(id) else { return };
    let Ok(Some(f)) = st.db.folder(&m.folder) else { return };
    if f.webhook.trim().is_empty() || m.enhanced.trim().is_empty() {
        return;
    }
    let text = format!("*{}*\n{}", if m.title.is_empty() { "Meeting notes" } else { &m.title }, m.enhanced.replace("### ", "*").replace("- [ ] ", "☐ "));
    let body = if f.webhook.contains("hooks.slack.com") {
        serde_json::json!({ "text": text })
    } else {
        serde_json::json!({ "title": m.title, "date": m.created_at, "attendees": m.attendees, "notes": m.enhanced, "folder": f.name, "text": text })
    };
    match reqwest::Client::new().post(f.webhook.trim()).json(&body).timeout(std::time::Duration::from_secs(20)).send().await {
        Ok(r) if r.status().is_success() => log::info!("posted notes to folder webhook ({})", f.name),
        Ok(r) => log::warn!("folder webhook {}: HTTP {}", f.name, r.status()),
        Err(err) => log::warn!("folder webhook {}: {err}", f.name),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn docx() {
        let b = super::docx_bytes("# Title\n### Decisions\n- Ship it\n  - Friday\n- [ ] Dana: send deck").unwrap();
        assert!(b.len() > 500);
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("word/document.xml").unwrap(), &mut s).unwrap();
        assert!(s.contains("Ship it") && s.contains("☐ Dana: send deck"));
    }
}
