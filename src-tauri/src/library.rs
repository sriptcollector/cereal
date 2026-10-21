//! Organising notes: folders (with an icon and a description), files added to a
//! folder as context for chat, trash with 30-day restore, chat threads, saved recipes,
//! and the People / Companies views built from meeting attendees.

use crate::db::{now, Db, Meeting};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS folders(
    id TEXT PRIMARY KEY, name TEXT NOT NULL, icon TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '',
    sort INTEGER NOT NULL DEFAULT 0, auto_match TEXT NOT NULL DEFAULT '', webhook TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS files(
    id TEXT PRIMARY KEY, folder_id TEXT NOT NULL, name TEXT NOT NULL, text TEXT NOT NULL, added_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS threads(
    id TEXT PRIMARY KEY, scope TEXT NOT NULL, scope_id TEXT NOT NULL DEFAULT '', title TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS recipes(
    id TEXT PRIMARY KEY, cmd TEXT NOT NULL, name TEXT NOT NULL, prompt TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'single',
    updated_at TEXT NOT NULL);
";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// An emoji.
    pub icon: String,
    pub description: String,
    pub sort: i64,
    /// Meeting titles containing this text are filed here automatically (recurring meetings).
    pub auto_match: String,
    /// New notes in this folder are posted here (Slack incoming webhook or any URL).
    pub webhook: String,
    pub created_at: String,
    #[serde(default)]
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub id: String,
    pub folder_id: String,
    pub name: String,
    pub chars: usize,
    pub added_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    /// "meeting" | "folder" | "all" | "person" | "company" | "selected"
    pub scope: String,
    pub scope_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    /// The /shortcut.
    pub cmd: String,
    pub name: String,
    pub prompt: String,
    /// "single" (one meeting) | "multi" (several meetings)
    pub kind: String,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub key: String,
    pub name: String,
    pub email: String,
    pub company: String,
    pub meetings: i64,
    pub last: String,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    pub key: String,
    pub name: String,
    pub domain: String,
    pub people: Vec<String>,
    pub meetings: i64,
    pub last: String,
}

pub fn builtin_recipes() -> Vec<Recipe> {
    let r = |cmd: &str, name: &str, kind: &str, prompt: &str| Recipe { id: format!("builtin-{cmd}"), cmd: cmd.into(), name: name.into(), prompt: prompt.into(), kind: kind.into(), builtin: true };
    vec![
        r("tldr", "TL;DR", "single", "Give me a TL;DR of this meeting in 3 bullets."),
        r("todos", "List recent todos", "multi", "List the action items from these meetings that are still open, grouped by owner, with the meeting each came from."),
        r("actions", "Action items by owner", "single", "List every action item, grouped by owner, with due dates where mentioned."),
        r("email", "Write follow-up email", "single", "Write a short, friendly follow-up email to the other attendees: thank them, recap the key points and decisions, and list next steps with owners. Ready to copy."),
        r("coach", "Coach me", "single", "Give me honest, specific feedback on how I ran this conversation: what went well, and two things to do better next time."),
        r("recap", "Write weekly recap", "multi", "Write a recap of my meetings from the last 7 days: main topics, decisions, and my open action items."),
        r("blindspots", "Blind spots", "multi", "Looking across these meetings, what risks, open questions or recurring problems am I not paying enough attention to?"),
        r("decisions", "List key decisions", "multi", "List the key decisions made in these meetings, newest first, with the meeting each came from."),
        r("missed", "What did I miss?", "single", "Summarize what was said in the last few minutes, in 3 bullets."),
        r("brief", "Prep me for this meeting", "single", "Using my past notes with these people, give me 2-3 bullets to prepare: open threads, what they care about, and what to follow up on."),
        r("catchup", "Catch me up", "multi", "Catch me up on these meetings: the main things that happened and what needs my attention."),
        r("year", "My year in meetings", "multi", "Give me a fun, honest recap of my year in meetings: how many, the main themes, the people I met with most, and the biggest decisions."),
    ]
}

const FREE_MAIL: &[&str] = &["gmail.com", "googlemail.com", "outlook.com", "hotmail.com", "live.com", "yahoo.com", "icloud.com", "me.com", "proton.me", "protonmail.com", "aol.com", "gmx.com"];

/// "Dana Lee <dana@acme.com>, Sam" -> [(name, email)].
pub fn parse_attendees(s: &str) -> Vec<(String, String)> {
    s.split([',', ';', '\n'])
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let (Some(a), Some(b)) = (p.find('<'), p.find('>')) {
                (p[..a].trim().trim_matches('"').to_string(), p[a + 1..b].trim().to_lowercase())
            } else if p.contains('@') {
                (String::new(), p.to_lowercase())
            } else {
                (p.to_string(), String::new())
            }
        })
        .collect()
}

pub fn company_of(email: &str) -> Option<(String, String)> {
    let domain = email.split('@').nth(1)?.to_lowercase();
    if FREE_MAIL.contains(&domain.as_str()) {
        return None;
    }
    let stem = domain.split('.').rev().nth(1).unwrap_or(&domain).to_string();
    let mut name: Vec<char> = stem.chars().collect();
    if let Some(c) = name.first_mut() {
        *c = c.to_ascii_uppercase();
    }
    Some((name.into_iter().collect(), domain))
}

fn folder_row(r: &rusqlite::Row) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: r.get(0)?,
        name: r.get(1)?,
        icon: r.get(2)?,
        description: r.get(3)?,
        sort: r.get(4)?,
        auto_match: r.get(5)?,
        webhook: r.get(6)?,
        created_at: r.get(7)?,
        count: r.get(8)?,
    })
}

impl Db {
    /// 0.1 stored the folder's name on the meeting; now it's a folder id.
    pub fn migrate_folder_names(&self) -> Result<()> {
        let names: Vec<String> = {
            let c = self.c.lock();
            let mut st = c.prepare("SELECT DISTINCT folder FROM meetings WHERE folder<>'' AND folder NOT IN (SELECT id FROM folders)")?;
            let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            v
        };
        for n in names {
            let id = uuid::Uuid::new_v4().to_string();
            let c = self.c.lock();
            c.execute("INSERT INTO folders(id,name,icon,created_at) VALUES(?1,?2,'📁',?3)", params![id, n, now()])?;
            c.execute("UPDATE meetings SET folder=?1 WHERE folder=?2", params![id, n])?;
        }
        Ok(())
    }

    // ---------------- folders ----------------

    pub fn folders(&self) -> Result<Vec<Folder>> {
        let c = self.c.lock();
        let mut st = c.prepare(
            "SELECT f.id,f.name,f.icon,f.description,f.sort,f.auto_match,f.webhook,f.created_at,
             (SELECT COUNT(*) FROM meetings m WHERE m.folder=f.id AND m.deleted_at IS NULL)
             FROM folders f ORDER BY f.sort, f.created_at",
        )?;
        let v = st.query_map([], folder_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn folder(&self, id: &str) -> Result<Option<Folder>> {
        Ok(self.folders()?.into_iter().find(|f| f.id == id))
    }

    pub fn save_folder(&self, f: &Folder) -> Result<Folder> {
        let mut f = f.clone();
        if f.id.is_empty() {
            f.id = uuid::Uuid::new_v4().to_string();
            f.created_at = now();
        }
        self.c.lock().execute(
            "INSERT INTO folders(id,name,icon,description,sort,auto_match,webhook,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET name=?2,icon=?3,description=?4,sort=?5,auto_match=?6,webhook=?7",
            params![f.id, f.name, f.icon, f.description, f.sort, f.auto_match, f.webhook, if f.created_at.is_empty() { now() } else { f.created_at.clone() }],
        )?;
        Ok(f)
    }

    /// Deletes a folder; its notes stay (back in My notes).
    pub fn delete_folder(&self, id: &str) -> Result<()> {
        let c = self.c.lock();
        c.execute("UPDATE meetings SET folder='' WHERE folder=?1", [id])?;
        c.execute("DELETE FROM files WHERE folder_id=?1", [id])?;
        c.execute("DELETE FROM folders WHERE id=?1", [id])?;
        Ok(())
    }

    /// A folder whose auto-match text appears in this title (recurring meetings).
    pub fn auto_folder_for(&self, title: &str) -> Option<String> {
        let t = title.to_lowercase();
        self.folders().ok()?.into_iter().find(|f| !f.auto_match.trim().is_empty() && t.contains(&f.auto_match.trim().to_lowercase())).map(|f| f.id)
    }

    // ---------------- files ----------------

    pub fn add_file(&self, folder: &str, name: &str, text: &str) -> Result<()> {
        self.c.lock().execute(
            "INSERT INTO files(id,folder_id,name,text,added_at) VALUES(?1,?2,?3,?4,?5)",
            params![uuid::Uuid::new_v4().to_string(), folder, name, text, now()],
        )?;
        Ok(())
    }

    pub fn files(&self, folder: &str) -> Result<Vec<FileItem>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,folder_id,name,length(text),added_at FROM files WHERE folder_id=?1 ORDER BY added_at DESC")?;
        let v = st
            .query_map([folder], |r| Ok(FileItem { id: r.get(0)?, folder_id: r.get(1)?, name: r.get(2)?, chars: r.get::<_, i64>(3)? as usize, added_at: r.get(4)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn file_texts(&self, folder: &str) -> Result<Vec<(String, String)>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT name,text FROM files WHERE folder_id=?1 ORDER BY added_at")?;
        let v = st.query_map([folder], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn delete_file(&self, id: &str) -> Result<()> {
        self.c.lock().execute("DELETE FROM files WHERE id=?1", [id])?;
        Ok(())
    }

    // ---------------- trash ----------------

    pub fn trash_meeting(&self, id: &str) -> Result<()> {
        self.c.lock().execute("UPDATE meetings SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
        Ok(())
    }

    pub fn restore_meeting(&self, id: &str) -> Result<()> {
        self.c.lock().execute("UPDATE meetings SET deleted_at=NULL WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn trash(&self) -> Result<Vec<Meeting>> {
        let ids: Vec<String> = {
            let c = self.c.lock();
            let mut st = c.prepare("SELECT id FROM meetings WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC")?;
            let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            v
        };
        Ok(ids.iter().filter_map(|i| self.meeting(i).ok().flatten()).collect())
    }

    /// Meetings in the trash for more than 30 days (to delete for good).
    pub fn expired_trash(&self) -> Result<Vec<String>> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id FROM meetings WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        let v = st.query_map([cutoff], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    // ---------------- chat threads ----------------

    pub fn threads(&self, scope: Option<&str>, scope_id: Option<&str>, limit: usize) -> Result<Vec<Thread>> {
        let c = self.c.lock();
        let mut st = c.prepare(
            "SELECT id,scope,scope_id,title,created_at,updated_at FROM threads
             WHERE (?1 IS NULL OR scope=?1) AND (?2 IS NULL OR scope_id=?2) ORDER BY updated_at DESC LIMIT ?3",
        )?;
        let v = st
            .query_map(params![scope, scope_id, limit as i64], |r| {
                Ok(Thread { id: r.get(0)?, scope: r.get(1)?, scope_id: r.get(2)?, title: r.get(3)?, created_at: r.get(4)?, updated_at: r.get(5)? })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn thread(&self, id: &str) -> Result<Option<Thread>> {
        let c = self.c.lock();
        Ok(c.query_row("SELECT id,scope,scope_id,title,created_at,updated_at FROM threads WHERE id=?1", [id], |r| {
            Ok(Thread { id: r.get(0)?, scope: r.get(1)?, scope_id: r.get(2)?, title: r.get(3)?, created_at: r.get(4)?, updated_at: r.get(5)? })
        })
        .optional()?)
    }

    pub fn create_thread(&self, scope: &str, scope_id: &str, title: &str) -> Result<Thread> {
        let t = Thread { id: uuid::Uuid::new_v4().to_string(), scope: scope.into(), scope_id: scope_id.into(), title: title.chars().take(80).collect(), created_at: now(), updated_at: now() };
        self.c.lock().execute(
            "INSERT INTO threads(id,scope,scope_id,title,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![t.id, t.scope, t.scope_id, t.title, t.created_at, t.updated_at],
        )?;
        Ok(t)
    }

    pub fn touch_thread(&self, id: &str) -> Result<()> {
        self.c.lock().execute("UPDATE threads SET updated_at=?2 WHERE id=?1", params![id, now()])?;
        Ok(())
    }

    pub fn delete_thread(&self, id: &str) -> Result<()> {
        let c = self.c.lock();
        c.execute("DELETE FROM chats WHERE thread_id=?1", [id])?;
        c.execute("DELETE FROM threads WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn add_thread_msg(&self, thread: &str, role: &str, content: &str) -> Result<i64> {
        let c = self.c.lock();
        c.execute("INSERT INTO chats(thread_id,role,content,created_at) VALUES(?1,?2,?3,?4)", params![thread, role, content, now()])?;
        Ok(c.last_insert_rowid())
    }

    pub fn thread_msgs(&self, thread: &str) -> Result<Vec<crate::db::ChatMsg>> {
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,meeting_id,role,content,created_at FROM chats WHERE thread_id=?1 ORDER BY id")?;
        let v = st
            .query_map([thread], |r| Ok(crate::db::ChatMsg { id: r.get(0)?, meeting_id: r.get(1)?, role: r.get(2)?, content: r.get(3)?, created_at: r.get(4)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    // ---------------- recipes ----------------

    pub fn recipes(&self) -> Result<Vec<Recipe>> {
        let mut v = builtin_recipes();
        let c = self.c.lock();
        let mut st = c.prepare("SELECT id,cmd,name,prompt,kind FROM recipes ORDER BY name")?;
        let custom = st
            .query_map([], |r| Ok(Recipe { id: r.get(0)?, cmd: r.get(1)?, name: r.get(2)?, prompt: r.get(3)?, kind: r.get(4)?, builtin: false }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.extend(custom);
        Ok(v)
    }

    pub fn save_recipe(&self, r: &Recipe) -> Result<Recipe> {
        let mut r = r.clone();
        if r.id.is_empty() || r.builtin {
            r.id = format!("custom-{}", uuid::Uuid::new_v4().simple());
            r.builtin = false;
        }
        r.cmd = r.cmd.trim().trim_start_matches('/').replace(' ', "-").to_lowercase();
        self.c.lock().execute(
            "INSERT INTO recipes(id,cmd,name,prompt,kind,updated_at) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET cmd=?2,name=?3,prompt=?4,kind=?5,updated_at=?6",
            params![r.id, r.cmd, r.name, r.prompt, r.kind, now()],
        )?;
        Ok(r)
    }

    pub fn delete_recipe(&self, id: &str) -> Result<()> {
        self.c.lock().execute("DELETE FROM recipes WHERE id=?1", [id])?;
        Ok(())
    }

    // ---------------- people & companies ----------------

    pub fn people(&self) -> Result<Vec<Person>> {
        let mut map: BTreeMap<String, Person> = BTreeMap::new();
        for m in self.meetings(None)? {
            let mut names: Vec<(String, String)> = parse_attendees(&m.attendees);
            if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(&m.speaker_names) {
                for v in o.values() {
                    if let Some(n) = v.as_str().filter(|n| !n.is_empty()) {
                        if !names.iter().any(|(a, _)| a.eq_ignore_ascii_case(n)) {
                            names.push((n.to_string(), String::new()));
                        }
                    }
                }
            }
            for (name, email) in names {
                let key = if !email.is_empty() { email.clone() } else { name.to_lowercase() };
                if key.is_empty() {
                    continue;
                }
                let p = map.entry(key.clone()).or_insert_with(|| Person {
                    key: key.clone(),
                    name: if name.is_empty() { email.split('@').next().unwrap_or("").to_string() } else { name.clone() },
                    email: email.clone(),
                    company: company_of(&email).map(|c| c.0).unwrap_or_default(),
                    ..Default::default()
                });
                p.meetings += 1;
                if m.created_at > p.last {
                    p.last = m.created_at.clone();
                }
            }
        }
        let mut v: Vec<Person> = map.into_values().collect();
        v.sort_by(|a, b| b.last.cmp(&a.last));
        Ok(v)
    }

    pub fn companies(&self) -> Result<Vec<Company>> {
        let mut map: BTreeMap<String, Company> = BTreeMap::new();
        let mut meetings_by: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
        for m in self.meetings(None)? {
            for (name, email) in parse_attendees(&m.attendees) {
                let Some((cname, domain)) = company_of(&email) else { continue };
                let c = map.entry(domain.clone()).or_insert_with(|| Company { key: domain.clone(), name: cname, domain: domain.clone(), ..Default::default() });
                let who = if name.is_empty() { email.clone() } else { name };
                if !c.people.contains(&who) {
                    c.people.push(who);
                }
                if m.created_at > c.last {
                    c.last = m.created_at.clone();
                }
                meetings_by.entry(domain).or_default().insert(m.id.clone());
            }
        }
        for (d, ms) in meetings_by {
            if let Some(c) = map.get_mut(&d) {
                c.meetings = ms.len() as i64;
            }
        }
        let mut v: Vec<Company> = map.into_values().collect();
        v.sort_by(|a, b| b.last.cmp(&a.last));
        Ok(v)
    }

    /// Meetings with a person (by email or name) or a company (by domain).
    pub fn meetings_with(&self, person: Option<&str>, domain: Option<&str>) -> Result<Vec<Meeting>> {
        Ok(self
            .meetings(None)?
            .into_iter()
            .filter(|m| {
                let at = parse_attendees(&m.attendees);
                let names = m.speaker_names.to_lowercase();
                person.map(|p| at.iter().any(|(n, e)| e == p || n.to_lowercase() == p) || names.contains(&format!("\"{p}\""))).unwrap_or(true)
                    && domain.map(|d| at.iter().any(|(_, e)| e.ends_with(&format!("@{d}")))).unwrap_or(true)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attendees() {
        let a = parse_attendees("Dana Lee <Dana@Acme.com>, sam@gmail.com; Priya");
        assert_eq!(a[0], ("Dana Lee".into(), "dana@acme.com".into()));
        assert_eq!(a[1].1, "sam@gmail.com");
        assert_eq!(a[2].0, "Priya");
        assert_eq!(company_of("dana@acme.com"), Some(("Acme".into(), "acme.com".into())));
        assert_eq!(company_of("x@gmail.com"), None);
    }
    #[test]
    fn folders_trash_people() {
        let db = Db::open(std::path::Path::new(":memory:")).unwrap();
        let f = db.save_folder(&Folder { name: "Clients".into(), icon: "🤝".into(), auto_match: "acme sync".into(), ..Default::default() }).unwrap();
        let m = Meeting { id: "m1".into(), title: "Weekly Acme sync".into(), created_at: now(), status: "ready".into(), folder: f.id.clone(), ..Default::default() };
        db.create_meeting(&m).unwrap();
        db.set("m1", "attendees", "Dana <dana@acme.com>, Sam <sam@acme.com>").unwrap();
        assert_eq!(db.folders().unwrap()[0].count, 1);
        assert_eq!(db.auto_folder_for("Acme Sync #12").as_deref(), Some(f.id.as_str()));
        assert_eq!(db.people().unwrap().len(), 2);
        assert_eq!(db.companies().unwrap()[0].people.len(), 2);
        db.trash_meeting("m1").unwrap();
        assert!(db.meetings(None).unwrap().is_empty());
        assert_eq!(db.trash().unwrap().len(), 1);
        db.restore_meeting("m1").unwrap();
        assert_eq!(db.meetings(None).unwrap().len(), 1);
        let t = db.create_thread("all", "", "What did we decide?").unwrap();
        db.add_thread_msg(&t.id, "user", "hi").unwrap();
        assert_eq!(db.thread_msgs(&t.id).unwrap().len(), 1);
        assert!(db.recipes().unwrap().len() >= 10);
    }
}
