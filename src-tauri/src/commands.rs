//! Commands the UI calls.

use crate::db::{Meeting, Segment, Template};
use crate::settings::Settings;
use crate::state::St;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

type R<T> = Result<T, String>;

fn e<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn get_settings(st: State<St>) -> Settings {
    st.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, st: State<St>, settings: Settings) -> R<()> {
    let old = st.settings.read().clone();
    st.save_settings(settings.clone()).map_err(e)?;
    if old.synced() != settings.synced() {
        crate::sync::push_settings(&st);
    }
    if old.launch_at_login != settings.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let _ = if settings.launch_at_login { al.enable() } else { al.disable() };
    }
    if old.model_mode != settings.model_mode || old.speech_model != settings.speech_model || old.llm_model != settings.llm_model || old.use_gpu != settings.use_gpu || old.language != settings.language {
        *st.whisper.lock() = None;
        *st.llama.lock() = None;
        crate::engine::auto_setup(&st);
    }
    crate::refresh_tray(&st);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    platform: &'static str,
    from_hub: bool,
    data_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, st: State<St>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        platform: if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
        from_hub: crate::hub::from_hub(),
        data_dir: st.data_dir.to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub fn ui_ready(app: AppHandle) {
    crate::ui_ready(&app);
}

// ---------------- meetings ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingFull {
    meeting: Meeting,
    segments: Vec<Segment>,
}

#[tauri::command]
pub fn meetings_list(st: State<St>, folder: Option<String>) -> R<Vec<Meeting>> {
    st.db.meetings(folder.as_deref()).map_err(e)
}

#[tauri::command]
pub fn meeting_get(st: State<St>, id: String) -> R<MeetingFull> {
    let meeting = st.db.meeting(&id).map_err(e)?.ok_or("This note was deleted.")?;
    Ok(MeetingFull { segments: st.db.segments(&id).map_err(e)?, meeting })
}

#[tauri::command]
pub fn meeting_create(st: State<St>, title: String) -> R<Meeting> {
    let m = Meeting {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        created_at: crate::db::now(),
        status: "draft".into(),
        template_id: st.settings.read().default_template.clone(),
        ..Default::default()
    };
    st.db.create_meeting(&m).map_err(e)?;
    Ok(m)
}

#[tauri::command]
pub fn meeting_update(st: State<St>, id: String, field: String, value: String) -> R<()> {
    st.db.set(&id, &field, &value).map_err(e)
}



#[tauri::command]
pub fn rename_speaker(st: State<St>, id: String, speaker: String, name: String) -> R<()> {
    let m = st.db.meeting(&id).map_err(e)?.ok_or("missing")?;
    let mut map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&m.speaker_names).unwrap_or_default();
    if name.trim().is_empty() {
        map.remove(&speaker);
    } else {
        map.insert(speaker, serde_json::Value::String(name.trim().to_string()));
    }
    st.db.set(&id, "speaker_names", &serde_json::Value::Object(map).to_string()).map_err(e)?;
    st.db.reindex(&id).map_err(e)
}

#[tauri::command]
pub fn segment_update(st: State<St>, meeting_id: String, segment_id: i64, text: String) -> R<()> {
    st.db.update_segment(segment_id, &text).map_err(e)?;
    st.db.reindex(&meeting_id).map_err(e)
}

#[tauri::command]
pub fn segment_delete(st: State<St>, meeting_id: String, segment_id: i64) -> R<()> {
    st.db.delete_segment(segment_id).map_err(e)?;
    st.db.reindex(&meeting_id).map_err(e)
}

// ---------------- recording ----------------

#[tauri::command]
pub fn record_start(st: State<St>, meeting_id: Option<String>, app: Option<String>) -> R<Meeting> {
    crate::hide_popup(&st);
    let app = app.unwrap_or_default();
    crate::recorder::start(&st, meeting_id, &app, "").map_err(e)
}

#[tauri::command]
pub fn record_stop(st: State<St>) -> Option<String> {
    crate::recorder::stop(&st)
}

#[tauri::command]
pub fn record_pause(st: State<St>, paused: bool) {
    crate::recorder::pause(&st, paused)
}

#[tauri::command]
pub fn rec_state(st: State<St>) -> crate::recorder::RecState {
    crate::recorder::state(&st)
}

/// Rewrites the AI notes (optionally with another template).
#[tauri::command]
pub async fn regenerate(st: State<'_, St>, id: String, template_id: Option<String>) -> R<()> {
    let st = st.inner().clone();
    st.db.set(&id, "status", "processing").map_err(e)?;
    st.db.set(&id, "step", "Writing your notes…").map_err(e)?;
    st.db.set(&id, "error", "").map_err(e)?;
    st.meeting_changed(&id);
    tauri::async_runtime::spawn(async move {
        let _g = st.process_lock.lock().await;
        match crate::notes::enhance(&st, &id, template_id).await {
            Ok(_) => {
                let _ = st.db.set(&id, "status", "ready");
            }
            Err(err) => {
                let _ = st.db.set(&id, "status", "failed");
                let _ = st.db.set(&id, "error", &format!("{err:#}"));
            }
        }
        let _ = st.db.set(&id, "step", "");
        st.meeting_changed(&id);
    });
    Ok(())
}

/// Runs the whole after-call pipeline again (transcript, speakers, notes).
#[tauri::command]
pub fn reprocess(st: State<St>, id: String) -> R<()> {
    let st = st.inner().clone();
    st.db.set(&id, "status", "processing").map_err(e)?;
    st.db.set(&id, "error", "").map_err(e)?;
    st.meeting_changed(&id);
    tauri::async_runtime::spawn(async move { crate::process::run(&st, &id, None).await });
    Ok(())
}

// ---------------- templates ----------------

#[tauri::command]
pub fn templates_list(st: State<St>) -> Vec<Template> {
    crate::notes::all_templates(&st)
}

#[tauri::command]
pub fn template_save(st: State<St>, mut template: Template) -> R<Template> {
    if template.id.is_empty() || crate::notes::builtin().iter().any(|b| b.id == template.id) {
        template.id = format!("custom-{}", uuid::Uuid::new_v4().simple());
    }
    template.builtin = false;
    template.updated_at = crate::db::now();
    st.db.save_template(&template).map_err(e)?;
    crate::sync::push_template(&st, Some(&template), &template.id);
    Ok(template)
}

#[tauri::command]
pub fn template_delete(st: State<St>, id: String) -> R<()> {
    st.db.delete_template(&id).map_err(e)?;
    crate::sync::push_template(&st, None, &id);
    Ok(())
}

// ---------------- search & chat ----------------

#[tauri::command]
pub fn search(st: State<St>, q: String) -> R<Vec<crate::db::Hit>> {
    st.db.search(&q, 50).map_err(e)
}




// ---------------- audio & models ----------------

#[tauri::command]
pub fn list_mics() -> Vec<String> {
    crate::capture::list_mics()
}

#[tauri::command]
pub async fn audio_check(st: State<'_, St>, secs: f64) -> R<(f32, f32)> {
    if st.session.lock().is_some() {
        return Err("Stop the current recording first.".into());
    }
    let mic = st.settings.read().microphone.clone();
    tauri::async_runtime::spawn_blocking(move || crate::capture::quick_check(secs.clamp(1.0, 10.0), mic)).await.map_err(e)?.map_err(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    id: String,
    label: String,
    kind: String,
    size_mb: u32,
    note: String,
    installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
    hardware: toolbay_engine::Hardware,
    choice: crate::engine::Choice,
    models: Vec<ModelRow>,
    notes_ready: bool,
    speech_in_use: String,
}

#[tauri::command]
pub fn models_info(st: State<St>) -> ModelsInfo {
    let hw = toolbay_engine::detect();
    let lang = st.settings.read().language.clone();
    ModelsInfo {
        choice: crate::engine::recommend(&hw, &lang),
        hardware: hw,
        models: toolbay_engine::MODELS
            .iter()
            .filter(|m| m.kind != "speaker")
            .map(|m| ModelRow { id: m.id.into(), label: m.label.into(), kind: m.kind.into(), size_mb: m.size_mb, note: m.note.into(), installed: st.store.find_model(m).is_some() })
            .collect(),
        notes_ready: crate::engine::notes_ready(&st),
        speech_in_use: crate::engine::speech_model(&st).map(|x| x.0).unwrap_or_default(),
    }
}

#[tauri::command]
pub fn rerun_auto(st: State<St>) {
    crate::engine::auto_setup(&st);
}

#[tauri::command]
pub async fn download_model(st: State<'_, St>, id: String) -> R<()> {
    let m = toolbay_engine::model(&id).ok_or("unknown model")?;
    let p = crate::engine::progress(&st);
    st.store.ensure_model(m, &p).await.map_err(|err| format!("{err:#}"))?;
    Ok(())
}

// ---------------- detection popup ----------------

#[tauri::command]
pub fn detected() -> Option<crate::detect::Call> {
    crate::detect::DETECTED.lock().clone()
}

#[tauri::command]
pub fn popup_action(app: AppHandle, st: State<St>, take: bool) -> R<()> {
    let call = crate::detect::DETECTED.lock().take();
    crate::hide_popup(&st);
    if let Some(c) = call {
        if take {
            let m = crate::recorder::start(&st, None, &c.app, "").map_err(e)?;
            crate::show_hub(&app, Some(&format!("meeting:{}", m.id)));
        } else {
            crate::detect::snooze(&c.app);
        }
    }
    Ok(())
}

// ---------------- misc ----------------

#[tauri::command]
pub fn open_hub(app: AppHandle, page: Option<String>) {
    crate::show_hub(&app, page.as_deref());
}

/// Saves notes as a Markdown file in Downloads and shows it in the file manager.
#[tauri::command]
pub fn export_markdown(app: AppHandle, name: String, content: String) -> R<String> {
    let dir = app.path().download_dir().map_err(e)?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '-' }).collect();
    let safe = if safe.trim().is_empty() { "Meeting notes".to_string() } else { safe.trim().to_string() };
    let mut p = dir.join(format!("{safe}.md"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{safe} ({i}).md"));
        i += 1;
    }
    std::fs::write(&p, content).map_err(e)?;
    reveal(&p);
    Ok(p.to_string_lossy().to_string())
}

pub fn reveal(p: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).creation_flags(0x0800_0000).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    }
}

#[tauri::command]
pub fn open_data_dir(st: State<St>) {
    reveal(&st.data_dir.join("notes.sqlite"));
}

#[tauri::command]
pub fn reveal_audio(st: State<St>, id: String) {
    if let Some(p) = crate::process::parts(&st, &id).last() {
        reveal(&p.join("mic.wav"));
    }
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    crate::hub::quit(&app);
}

#[tauri::command]
pub async fn toolbay_account() -> Option<crate::hub::Account> {
    crate::hub::account().await
}

#[tauri::command]
pub fn sync_status(st: State<St>) -> (bool, bool) {
    (crate::account::token().is_some(), crate::sync::available(&st))
}

/// Live captions open/closed on the floating indicator.
#[tauri::command]
pub fn pill_wide(app: AppHandle, wide: bool) {
    crate::pill_wide(&app, wide);
}

/// Camera button: the screen goes into the note being recorded.
#[tauri::command]
pub fn screenshot(st: State<St>) -> R<()> {
    crate::screens::screenshot_into_note(&st).map(|_| ()).map_err(e)
}

/// Hides the Alt+Space ask box.
#[tauri::command]
pub fn quick_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.hide();
    }
}

/// Makes an earlier version (other template) the current notes; the current ones become a tab.
#[tauri::command]
pub fn use_version(st: State<St>, id: String, index: usize) -> R<()> {
    let m = st.db.meeting(&id).map_err(e)?.ok_or("missing note")?;
    let mut v: Vec<serde_json::Value> = serde_json::from_str(&m.versions).unwrap_or_default();
    if index >= v.len() {
        return Err("That version is gone.".into());
    }
    let pick = v.remove(index);
    let cur_name = crate::notes::template(&st, &m.template_id).name;
    v.insert(0, serde_json::json!({"templateId": m.template_id, "name": cur_name, "notes": m.enhanced, "at": crate::db::now()}));
    st.db.set(&id, "enhanced", pick["notes"].as_str().unwrap_or_default()).map_err(e)?;
    st.db.set(&id, "template_id", pick["templateId"].as_str().unwrap_or("auto")).map_err(e)?;
    st.db.set(&id, "versions", &serde_json::to_string(&v).map_err(e)?).map_err(e)?;
    st.meeting_changed(&id);
    Ok(())
}

/// Where Cereal is installed (for the MCP config other apps need).
#[tauri::command]
pub fn exe_path() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
}

/// Toggles the small always-on-top window; returns true when mini.
#[tauri::command]
pub fn mini_window(app: AppHandle) -> bool {
    crate::mini_window(&app)
}

#[tauri::command]
pub async fn send_to_notion(st: State<'_, St>, id: String) -> R<String> {
    crate::connectors::to_notion(&st, &id).await.map_err(e)
}

#[tauri::command]
pub async fn send_to_hubspot(st: State<'_, St>, id: String) -> R<usize> {
    crate::connectors::to_hubspot(&st, &id).await.map_err(e)
}

#[tauri::command]
pub async fn post_text(st: State<'_, St>, folder: String, text: String) -> R<()> {
    crate::connectors::post_text(&st, &folder, &text).await.map_err(e)
}

/// Opens a calendar event draft in the default calendar app.
#[tauri::command]
pub fn calendar_draft(app: AppHandle, summary: String, details: String) -> R<()> {
    let p = crate::connectors::calendar_draft(&summary, &details).map_err(e)?;
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(p.display().to_string(), None::<&str>).map_err(e)
}

/// "Keep going" on the Still there? card.
#[tauri::command]
pub fn still_here(st: State<St>) {
    crate::recorder::still_here(&st);
    crate::hide_popup(&st);
}

/// The calendar event happening now (for the "Meeting detected" card).
#[tauri::command]
pub fn current_event() -> Option<crate::calendar::Event> {
    crate::calendar::current_event()
}

#[tauri::command]
pub fn pill_click(app: AppHandle, st: State<St>) {
    let page = crate::recorder::recording_id(&st).map(|id| format!("meeting:{id}"));
    crate::show_hub(&app, page.as_deref());
}

/// Per-app rule from the popup or Settings: "record" | "ask" | "nothing" | "ignore" | "" (default).
#[tauri::command]
pub fn set_app_rule(st: State<St>, app: String, rule: String) -> R<()> {
    let mut s = st.settings.read().clone();
    if rule.is_empty() {
        s.app_rules.remove(&app);
    } else {
        s.app_rules.insert(app, rule);
    }
    st.save_settings(s).map_err(e)?;
    crate::sync::push_settings(&st);
    Ok(())
}

#[tauri::command]
pub fn feedback_info() -> bool {
    crate::account::token().is_some()
}

/// In-app feedback to Cereal's space on toolbay.ai. Debug info only when the person ticks it.
#[tauri::command]
pub async fn feedback_send(st: State<'_, St>, kind: String, text: String, screenshot: Option<String>, include_debug: bool) -> R<String> {
    let debug = if include_debug {
        let s = st.settings.read().clone();
        let hw = toolbay_engine::detect();
        let log = std::fs::read_to_string(st.data_dir.join("cereal.log")).unwrap_or_default();
        let tail: Vec<&str> = log.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
        Some(serde_json::json!({
            "gpu": hw.gpu_name, "vramMb": hw.vram_mb, "ramMb": hw.ram_mb, "threads": hw.threads,
            "speechModel": s.speech_model, "llmModel": s.llm_model, "useGpu": s.use_gpu, "language": s.language,
            "meetingStart": s.meeting_start, "captureSystem": s.capture_system,
            "log": tail.join("
").chars().take(9000).collect::<String>(),
        }))
    } else {
        None
    };
    let vis = if include_debug && kind == "bug" { Some("maker".to_string()) } else { None };
    crate::account::send_feedback(&kind, &text, screenshot, debug, vis).await.map_err(|err| format!("{err}"))
}
