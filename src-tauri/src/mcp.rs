//! Your notes for other AI apps and scripts, all on this computer (Granola has a hosted
//! MCP server and a companion CLI; Cereal's run locally and read the same database).
//!
//!   cereal --mcp                       MCP server over stdio (Claude Desktop, Cursor, …)
//!   cereal notes list [--limit N]      recent notes
//!   cereal notes get <id>              one note (your notes + AI notes)
//!   cereal notes transcript <id>       its transcript
//!   cereal notes search <words>        full-text search
//!   cereal folders list                folders
//! Add --json to any of the commands for JSON output.

use crate::db::{Db, Meeting};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn open_db() -> Result<Db> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default()
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support")).unwrap_or_default()
    };
    let dir = std::env::var_os("CEREAL_DATA").map(PathBuf::from).unwrap_or_else(|| base.join("ai.toolbay.cereal"));
    let f = dir.join("notes.sqlite");
    if !f.exists() {
        return Err(anyhow!("No Cereal notes found at {}", f.display()));
    }
    Db::open(&f)
}

fn names(m: &Meeting) -> Value {
    serde_json::from_str(&m.speaker_names).unwrap_or(json!({}))
}

fn summary(m: &Meeting) -> Value {
    json!({"id": m.id, "title": if m.title.is_empty() { "Untitled" } else { &m.title }, "date": m.created_at, "minutes": (m.duration_s / 60.0).round(), "attendees": m.attendees, "folder": m.folder})
}

fn note(db: &Db, id: &str) -> Result<Value> {
    let m = db.meeting(id)?.filter(|m| m.deleted_at.is_none()).ok_or_else(|| anyhow!("No note with id {id}"))?;
    let mut v = summary(&m);
    v["my_notes"] = json!(m.my_notes);
    v["notes"] = json!(m.enhanced);
    Ok(v)
}

fn transcript(db: &Db, id: &str) -> Result<String> {
    let m = db.meeting(id)?.ok_or_else(|| anyhow!("No note with id {id}"))?;
    let n = names(&m);
    Ok(db
        .segments(id)?
        .iter()
        .map(|s| {
            let who = n.get(&s.speaker).and_then(|x| x.as_str()).unwrap_or(&s.speaker);
            format!("[{:02}:{:02}] {who}: {}", (s.start / 60.0) as u64, (s.start % 60.0) as u64, s.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn list(db: &Db, limit: usize, folder: Option<&str>) -> Result<Value> {
    Ok(Value::Array(db.meetings(folder)?.iter().take(limit).map(summary).collect()))
}

fn search(db: &Db, q: &str) -> Result<Value> {
    Ok(Value::Array(db.search(q, 20)?.into_iter().map(|h| json!({"id": h.meeting_id, "title": h.title, "date": h.created_at, "snippet": h.snippet})).collect()))
}

fn folders(db: &Db) -> Result<Value> {
    Ok(Value::Array(db.folders()?.into_iter().map(|f| json!({"id": f.id, "name": f.name, "notes": f.count, "parent": f.parent_id})).collect()))
}

// ---------------- CLI ----------------

pub fn cli(args: &[String]) -> Option<i32> {
    // Only as the first argument, so app arguments can never be mistaken for it.
    let i = 1;
    if !matches!(args.get(i).map(|s| s.as_str()), Some("notes" | "folders")) {
        return None;
    }
    attach_console();
    let json_out = args.iter().any(|a| a == "--json");
    let rest: Vec<&str> = args[i + 1..].iter().map(|s| s.as_str()).filter(|a| *a != "--json").collect();
    let r = (|| -> Result<Value> {
        let db = open_db()?;
        match (args[i].as_str(), rest.first().copied().unwrap_or("list")) {
            ("folders", _) => folders(&db),
            ("notes", "list") => {
                let limit = rest.iter().position(|a| *a == "--limit").and_then(|k| rest.get(k + 1)).and_then(|n| n.parse().ok()).unwrap_or(20);
                list(&db, limit, None)
            }
            ("notes", "get") => note(&db, rest.get(1).ok_or_else(|| anyhow!("Usage: cereal notes get <id>"))?),
            ("notes", "transcript") => Ok(json!(transcript(&db, rest.get(1).ok_or_else(|| anyhow!("Usage: cereal notes transcript <id>"))?)?)),
            ("notes", "search") => search(&db, &rest[1..].join(" ")),
            _ => Err(anyhow!("Commands: notes list | notes get <id> | notes transcript <id> | notes search <words> | folders list")),
        }
    })();
    match r {
        Ok(v) if json_out => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
        Ok(Value::String(s)) => println!("{s}"),
        Ok(Value::Array(a)) => {
            for x in a {
                let date = x["date"].as_str().unwrap_or("").get(..10).unwrap_or("");
                let name = x["title"].as_str().or(x["name"].as_str()).unwrap_or("");
                let extra = x["snippet"].as_str().map(|s| format!("  {s}")).unwrap_or_default();
                println!("{}  {date}  {name}{extra}", x["id"].as_str().unwrap_or(""));
            }
        }
        Ok(v) => {
            println!("# {}\n{}\n", v["title"].as_str().unwrap_or(""), v["date"].as_str().unwrap_or(""));
            let notes = v["notes"].as_str().filter(|s| !s.is_empty()).or(v["my_notes"].as_str()).unwrap_or("");
            println!("{notes}");
        }
        Err(e) => {
            eprintln!("{e:#}");
            return Some(1);
        }
    }
    let _ = std::io::stdout().flush();
    Some(0)
}

/// The release exe has no console of its own; print into the one it was started from.
fn attach_console() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

// ---------------- MCP (JSON-RPC 2.0 over stdio) ----------------

fn tools() -> Value {
    let s = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    json!([
        {"name": "list_meetings", "description": "List recent meeting notes from Cereal (newest first).", "inputSchema": s(json!({"limit": {"type": "integer", "description": "How many (default 20)"}, "folder": {"type": "string", "description": "Folder id to limit to"}}), &[])},
        {"name": "get_meeting", "description": "Get one meeting's notes: the user's own notes and the AI-written notes.", "inputSchema": s(json!({"id": {"type": "string"}}), &["id"])},
        {"name": "get_transcript", "description": "Get one meeting's transcript with speaker names and times.", "inputSchema": s(json!({"id": {"type": "string"}}), &["id"])},
        {"name": "search_meetings", "description": "Full-text search across titles, notes and transcripts.", "inputSchema": s(json!({"query": {"type": "string"}}), &["query"])},
        {"name": "list_folders", "description": "List the user's folders.", "inputSchema": s(json!({}), &[])},
    ])
}

fn call(db: &Db, name: &str, a: &Value) -> Result<String> {
    let id = || a["id"].as_str().ok_or_else(|| anyhow!("id is required"));
    let v = match name {
        "list_meetings" => list(db, a["limit"].as_u64().unwrap_or(20) as usize, a["folder"].as_str())?,
        "get_meeting" => note(db, id()?)?,
        "get_transcript" => return transcript(db, id()?),
        "search_meetings" => search(db, a["query"].as_str().unwrap_or(""))?,
        "list_folders" => folders(db)?,
        _ => return Err(anyhow!("unknown tool {name}")),
    };
    Ok(serde_json::to_string_pretty(&v)?)
}

pub fn serve() -> i32 {
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e:#}");
            return 1;
        }
    };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        // Notifications (no id) need no reply.
        let Some(id) = req.get("id").cloned() else { continue };
        let p = &req["params"];
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({"protocolVersion": p["protocolVersion"].as_str().unwrap_or("2024-11-05"), "capabilities": {"tools": {}}, "serverInfo": {"name": "cereal", "version": env!("CARGO_PKG_VERSION")}})),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => Ok(match call(&db, p["name"].as_str().unwrap_or(""), &p["arguments"]) {
                Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
                Err(e) => json!({"content": [{"type": "text", "text": format!("{e:#}")}], "isError": true}),
            }),
            m => Err(json!({"code": -32601, "message": format!("Method not found: {m}")})),
        };
        let msg = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        let _ = writeln!(out, "{msg}");
        let _ = out.flush();
    }
    0
}
