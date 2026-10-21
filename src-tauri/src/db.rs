//! Local database (notes.sqlite): meetings, transcript segments, templates, chats,
//! and a full-text index for search. Nothing here leaves the computer.

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_s: f64,
    /// Meeting app it was recorded from ("Zoom", "Google Meet", …), if known.
    pub app: String,
    /// draft | recording | paused | processing | ready | failed
    pub status: String,
    /// What the app is doing right now, in plain words (while processing).
    pub step: String,
    pub template_id: String,
    /// The user's own rough notes (markdown-ish text).
    pub my_notes: String,
    /// AI notes (markdown), empty until generated.
    pub enhanced: String,
    pub folder: String,
    pub attendees: String,
    /// JSON object: speaker label -> display name ("Speaker 1" -> "Dana").
    pub speaker_names: String,
    pub error: String,
    pub has_audio: bool,
    pub updated_at: String,
    /// Set when moved to the trash (kept 30 days).
    pub deleted_at: Option<String>,
    /// Calendar event this note belongs to (ICS UID), if any.
    pub event_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: i64,
    /// "mic" (you) | "system" (them)
    pub source: String,
    /// "You", "Them", "Speaker 1", …
    pub speaker: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub is_final: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    /// One section per line: "Heading: what to put there".
    pub sections: String,
    /// Extra instructions for the AI.
    pub instructions: String,
    pub builtin: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChatMsg {
    pub id: i64,
    pub meeting_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub meeting_id: String,
    pub title: String,
    pub created_at: String,
    pub snippet: String,
}

pub struct Db {
    pub(crate) c: Mutex<Connection>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

const MEETING_COLS: &str = "id,title,created_at,started_at,ended_at,duration_s,app,status,step,template_id,my_notes,enhanced,folder,attendees,speaker_names,error,has_audio,updated_at,deleted_at,event_id";

fn meeting_row(r: &rusqlite::Row) -> rusqlite::Result<Meeting> {
    Ok(Meeting {
        id: r.get(0)?,
        title: r.get(1)?,
        created_at: r.get(2)?,
        started_at: r.get(3)?,
        ended_at: r.get(4)?,
        duration_s: r.get(5)?,
        app: r.get(6)?,
        status: r.get(7)?,
        step: r.get(8)?,
        template_id: r.get(9)?,
        my_notes: r.get(10)?,
        enhanced: r.get(11)?,
        folder: r.get(12)?,
        attendees: r.get(13)?,
        speaker_names: r.get(14)?,
        error: r.get(15)?,
        has_audio: r.get::<_, i64>(16)? != 0,
        updated_at: r.get(17)?,
        deleted_at: r.get(18)?,
        event_id: r.get(19)?,
    })
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let c = Connection::open(path)?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS meetings(
                id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
                started_at TEXT, ended_at TEXT, duration_s REAL NOT NULL DEFAULT 0, app TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'draft', step TEXT NOT NULL DEFAULT '', template_id TEXT NOT NULL DEFAULT 'auto',
                my_notes TEXT NOT NULL DEFAULT '', enhanced TEXT NOT NULL DEFAULT '', folder TEXT NOT NULL DEFAULT '',
                attendees TEXT NOT NULL DEFAULT '', speaker_names TEXT NOT NULL DEFAULT '{}', error TEXT NOT NULL DEFAULT '',
                has_audio INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS segments(
                id INTEGER PRIMARY KEY AUTOINCREMENT, meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
                source TEXT NOT NULL, speaker TEXT NOT NULL, start REAL NOT NULL, end REAL NOT NULL, text TEXT NOT NULL,
                is_final INTEGER NOT NULL DEFAULT 0);
            CREATE INDEX IF NOT EXISTS seg_meeting ON segments(meeting_id, start);
            CREATE TABLE IF NOT EXISTS templates(
                id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', sections TEXT NOT NULL DEFAULT '',
                instructions TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS chats(
                id INTEGER PRIMARY KEY AUTOINCREMENT, meeting_id TEXT, role TEXT NOT NULL, content TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);
            CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(meeting_id UNINDEXED, title, notes, transcript, tokenize='porter unicode61');",
        )?;
        // Columns added after 0.1 (errors mean they already exist).
        for sql in [
            "ALTER TABLE meetings ADD COLUMN deleted_at TEXT",
            "ALTER TABLE meetings ADD COLUMN event_id TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE chats ADD COLUMN thread_id TEXT",
        ] {
            let _ = c.execute(sql, []);
        }
        c.execute_batch(crate::library::SCHEMA)?;
        let db = Db { c: Mutex::new(c) };
        db.migrate_folder_names()?;
        Ok(db)
    }

    // ---------------- meetings ----------------

    pub fn create_meeting(&self, m: &Meeting) -> Result<()> {
        self.c.lock().execute(
            "INSERT INTO meetings(id,title,created_at,started_at,app,status,template_id,my_notes,folder,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![m.id, m.title, m.created_at, m.started_at, m.app, m.status, m.template_id, m.my_notes, m.folder, now()],
        )?;
        self.reindex(&m.id)?;
        Ok(())
    }

    pub fn meeting(&self, id: &str) -> Result<Option<Meeting>> {
        let c = self.c.lock();
        Ok(c.query_row(&format!("SELECT {MEETING_COLS} FROM meetings WHERE id=?1"), [id], meeting_row).optional()?)
    }

    pub fn meetings(&self, folder: Option<&str>) -> Result<Vec<Meeting>> {
        let c = self.c.lock();
        let mut st = c.prepare(&format!("SELECT {MEETING_COLS} FROM meetings WHERE deleted_at IS NULL AND (?1 IS NULL OR folder=?1) ORDER BY created_at DESC LIMIT 5000"))?;
        let v = st.query_map(params![folder], meeting_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Updates one text column (whitelisted) of a meeting.
    pub fn set(&self, id: &str, field: &str, value: &str) -> Result<()> {
        const OK: &[&str] = &["title", "app", "status", "step", "template_id", "my_notes", "enhanced", "folder", "attendees", "speaker_names", "error", "started_at", "ended_at", "event_id"];
        if !OK.contains(&field) {
            anyhow::bail!("bad field {field}");
        }
        self.c.lock().execute(&format!("UPDATE meetings SET {field}=?2, updated_at=?3 WHERE id=?1"), params![id, value, now()])?;
        if matches!(field, "title" | "my_notes" | "enhanced") {
            self.reindex(id)?;
        }
        Ok(())
    }

    pub fn set_duration(&self, id: &str, secs: f64) -> Result<()> {
        self.c.lock().execute("UPDATE meetings SET duration_s=?2 WHERE id=?1", params![id, secs])?;
        Ok(())
    }

    pub fn set_has_audio(&self, id: &str, v: bool) -> Result<()> {
        self.c.lock().execute("UPDATE meetings SET has_audio=?2 WHERE id=?1", params![id, v as i64])?;
        Ok(())
    }

    pub fn delete_meeting(&self, id: &str) -> Result<()> {
        let c = self.c.lock();
        c.execute("DELETE FROM segments WHERE meeting_id=?1", [id])?;
        c.execute("DELETE FROM chats WHERE meeting_id=?1", [id])?;
        c.execute("DELETE FROM search WHERE meeting_id=?1", [id])?;
        c.execute("DELETE FROM meetings WHERE id=?1", [id])?;
        Ok(())
    }

    // ---------------- segments ----------------

    pub fn add_segment(&self, meeting: &str, s: &Segment) -> Result<i64> {
        let c = self.c.lock();
        c.execute(
            "INSERT INTO segments(meeting_id,source,speaker,start,end,text,is_final) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![meeting, s.source, s.speaker, s.start, s.end, s.text, s.is_final as i64],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn segments(&self, meeting: &str) -> Result<Vec<Segment>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,source,speaker,start,end,text,is_final FROM segments WHERE meeting_id=?1 ORDER BY start, id")?;
        let v = st
            .query_map([meeting], |r| {
                Ok(Segment { id: r.get(0)?, source: r.get(1)?, speaker: r.get(2)?, start: r.get(3)?, end: r.get(4)?, text: r.get(5)?, is_final: r.get::<_, i64>(6)? != 0 })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Swaps the live transcript for the final one in one transaction.
    pub fn replace_segments(&self, meeting: &str, segs: &[Segment]) -> Result<()> {
        {
            let mut c = self.c.lock();
            let tx = c.transaction()?;
            tx.execute("DELETE FROM segments WHERE meeting_id=?1", [meeting])?;
            for s in segs {
                tx.execute(
                    "INSERT INTO segments(meeting_id,source,speaker,start,end,text,is_final) VALUES(?1,?2,?3,?4,?5,?6,1)",
                    params![meeting, s.source, s.speaker, s.start, s.end, s.text],
                )?;
            }
            tx.commit()?;
        }
        self.reindex(meeting)
    }

    pub fn update_segment(&self, id: i64, text: &str) -> Result<()> {
        self.c.lock().execute("UPDATE segments SET text=?2 WHERE id=?1", params![id, text])?;
        Ok(())
    }

    pub fn delete_segment(&self, id: i64) -> Result<()> {
        self.c.lock().execute("DELETE FROM segments WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn rename_speaker(&self, meeting: &str, from: &str, to: &str) -> Result<()> {
        self.c.lock().execute("UPDATE segments SET speaker=?3 WHERE meeting_id=?1 AND speaker=?2", params![meeting, from, to])?;
        self.reindex(meeting)
    }

    pub fn transcript_text(&self, meeting: &str) -> Result<String> {
        Ok(self.segments(meeting)?.iter().map(|s| format!("{}: {}", s.speaker, s.text)).collect::<Vec<_>>().join("\n"))
    }

    // ---------------- search ----------------

    pub fn reindex(&self, id: &str) -> Result<()> {
        let transcript = self.transcript_text(id)?;
        let c = self.c.lock();
        let (title, notes, enhanced): (String, String, String) =
            match c.query_row("SELECT title,my_notes,enhanced FROM meetings WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()? {
                Some(v) => v,
                None => return Ok(()),
            };
        c.execute("DELETE FROM search WHERE meeting_id=?1", [id])?;
        c.execute("INSERT INTO search(meeting_id,title,notes,transcript) VALUES(?1,?2,?3,?4)", params![id, title, format!("{notes}\n{enhanced}"), transcript])?;
        Ok(())
    }

    /// Full-text search; every word must match (prefix match on the last one).
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<Hit>> {
        let words: Vec<String> = q
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty())
            .map(|w| format!("\"{}\"", w.replace('"', "")))
            .collect();
        if words.is_empty() {
            return Ok(vec![]);
        }
        let mut fts = words.join(" ");
        fts.push('*');
        let c = self.c.lock();
        let mut st = c.prepare(
            "SELECT s.meeting_id, m.title, m.created_at, snippet(search, -1, '[', ']', ' … ', 14)
             FROM search s JOIN meetings m ON m.id=s.meeting_id WHERE search MATCH ?1 AND m.deleted_at IS NULL ORDER BY rank LIMIT ?2",
        )?;
        let v = st
            .query_map(params![fts, limit as i64], |r| Ok(Hit { meeting_id: r.get(0)?, title: r.get(1)?, created_at: r.get(2)?, snippet: r.get(3)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Meeting ids that best match any of the words (for chat across meetings).
    pub fn search_any(&self, q: &str, limit: usize) -> Result<Vec<String>> {
        let words: Vec<String> = q
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2 && !STOP.contains(&w.to_lowercase().as_str()))
            .map(|w| format!("\"{}\"", w))
            .collect();
        if words.is_empty() {
            return Ok(vec![]);
        }
        let c = self.c.lock();
        let mut st = c.prepare("SELECT meeting_id FROM search WHERE search MATCH ?1 ORDER BY rank LIMIT ?2")?;
        let v = st.query_map(params![words.join(" OR "), limit as i64], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    // ---------------- templates ----------------

    pub fn custom_templates(&self) -> Result<Vec<Template>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,name,description,sections,instructions,updated_at FROM templates ORDER BY name")?;
        let v = st
            .query_map([], |r| {
                Ok(Template { id: r.get(0)?, name: r.get(1)?, description: r.get(2)?, sections: r.get(3)?, instructions: r.get(4)?, builtin: false, updated_at: r.get(5)? })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn save_template(&self, t: &Template) -> Result<()> {
        self.c.lock().execute(
            "INSERT INTO templates(id,name,description,sections,instructions,updated_at) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET name=?2,description=?3,sections=?4,instructions=?5,updated_at=?6",
            params![t.id, t.name, t.description, t.sections, t.instructions, if t.updated_at.is_empty() { now() } else { t.updated_at.clone() }],
        )?;
        Ok(())
    }

    pub fn delete_template(&self, id: &str) -> Result<()> {
        self.c.lock().execute("DELETE FROM templates WHERE id=?1", [id])?;
        Ok(())
    }

    // ---------------- chat ----------------

    pub fn add_chat(&self, meeting: Option<&str>, role: &str, content: &str) -> Result<i64> {
        let c = self.c.lock();
        c.execute("INSERT INTO chats(meeting_id,role,content,created_at) VALUES(?1,?2,?3,?4)", params![meeting, role, content, now()])?;
        Ok(c.last_insert_rowid())
    }

    pub fn chats(&self, meeting: Option<&str>) -> Result<Vec<ChatMsg>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,meeting_id,role,content,created_at FROM chats WHERE meeting_id IS ?1 ORDER BY id")?;
        let v = st
            .query_map(params![meeting], |r| Ok(ChatMsg { id: r.get(0)?, meeting_id: r.get(1)?, role: r.get(2)?, content: r.get(3)?, created_at: r.get(4)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn clear_chat(&self, meeting: Option<&str>) -> Result<()> {
        self.c.lock().execute("DELETE FROM chats WHERE meeting_id IS ?1", params![meeting])?;
        Ok(())
    }

    // ---------------- key/value ----------------

    pub fn kv(&self, k: &str) -> Option<String> {
        self.c.lock().query_row("SELECT v FROM kv WHERE k=?1", [k], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_kv(&self, k: &str, v: &str) -> Result<()> {
        self.c.lock().execute("INSERT INTO kv(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=?2", params![k, v])?;
        Ok(())
    }
}

const STOP: &[&str] = &["the", "and", "what", "who", "did", "was", "were", "about", "with", "for", "that", "this", "from", "have", "has", "our", "your", "you", "are", "how", "when", "why", "which", "any", "all", "say", "said", "tell", "give", "last", "meeting", "meetings", "call", "calls"];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crud_and_search() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        let m = Meeting { id: "m1".into(), title: "Pricing sync".into(), created_at: now(), status: "draft".into(), template_id: "auto".into(), ..Default::default() };
        db.create_meeting(&m).unwrap();
        db.add_segment("m1", &Segment { source: "system".into(), speaker: "Them".into(), start: 1.0, end: 2.0, text: "We agreed on the enterprise discount".into(), ..Default::default() }).unwrap();
        db.reindex("m1").unwrap();
        let h = db.search("enterpr", 10).unwrap();
        assert_eq!(h.len(), 1);
        assert!(h[0].snippet.contains("[enterprise]"));
        assert_eq!(db.search_any("what discount did we agree", 5).unwrap(), vec!["m1".to_string()]);
        db.set("m1", "title", "Renamed").unwrap();
        assert_eq!(db.meeting("m1").unwrap().unwrap().title, "Renamed");
        db.delete_meeting("m1").unwrap();
        assert!(db.meetings(None).unwrap().is_empty());
    }
}
