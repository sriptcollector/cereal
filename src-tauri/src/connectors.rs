//! Sending notes to other apps with the user's own keys (Granola's connectors):
//! Notion (an internal integration token + a parent page), HubSpot (a private app
//! token; the note is attached to attendees found by email), Slack or any webhook,
//! and calendar drafts (.ics). Nothing is sent unless the user clicks or sets it up.

use crate::db::Meeting;
use crate::state::St;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;

fn http() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap_or_default()
}

fn title(m: &Meeting) -> String {
    if m.title.trim().is_empty() {
        "Meeting notes".into()
    } else {
        m.title.clone()
    }
}

fn body_md(m: &Meeting) -> String {
    if m.enhanced.trim().is_empty() {
        m.my_notes.clone()
    } else {
        m.enhanced.clone()
    }
}

/// Markdown lines → Notion blocks (headings, to-dos, bullets, paragraphs).
fn notion_blocks(md: &str) -> Vec<Value> {
    let rt = |t: &str| json!([{ "type": "text", "text": { "content": t.chars().take(1900).collect::<String>() } }]);
    md.lines()
        .filter(|l| !l.trim().is_empty())
        .take(100)
        .map(|l| {
            let t = l.trim_start();
            if let Some(h) = t.strip_prefix("### ").or(t.strip_prefix("## ")).or(t.strip_prefix("# ")) {
                json!({"object": "block", "type": "heading_3", "heading_3": {"rich_text": rt(h)}})
            } else if let Some(x) = t.strip_prefix("- [ ] ").or(t.strip_prefix("- [x] ")) {
                json!({"object": "block", "type": "to_do", "to_do": {"rich_text": rt(x), "checked": t.starts_with("- [x]")}})
            } else if let Some(x) = t.strip_prefix("- ").or(t.strip_prefix("* ")) {
                json!({"object": "block", "type": "bulleted_list_item", "bulleted_list_item": {"rich_text": rt(x)}})
            } else {
                json!({"object": "block", "type": "paragraph", "paragraph": {"rich_text": rt(t)}})
            }
        })
        .collect()
}

/// A Notion page link or id → the 32-character id.
fn notion_id(s: &str) -> String {
    let s = s.split('?').next().unwrap_or("");
    let last = s.rsplit('/').next().unwrap_or(s);
    let hex: String = last.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() >= 32 {
        hex[hex.len() - 32..].to_string()
    } else {
        last.to_string()
    }
}

pub async fn to_notion(st: &St, id: &str) -> Result<String> {
    let (token, parent) = {
        let s = st.settings.read();
        (s.notion_token.trim().to_string(), s.notion_page.trim().to_string())
    };
    if token.is_empty() || parent.is_empty() {
        return Err(anyhow!("Add your Notion token and page in Settings › Connectors first."));
    }
    let m = st.db.meeting(id)?.ok_or_else(|| anyhow!("missing note"))?;
    let body = json!({
        "parent": {"page_id": notion_id(&parent)},
        "properties": {"title": {"title": [{"text": {"content": title(&m)}}]}},
        "children": notion_blocks(&body_md(&m)),
    });
    let r = http().post("https://api.notion.com/v1/pages").bearer_auth(token).header("Notion-Version", "2022-06-28").json(&body).send().await?;
    if !r.status().is_success() {
        let t = r.text().await.unwrap_or_default();
        return Err(anyhow!("Notion said no: {}", t.chars().take(300).collect::<String>()));
    }
    let v: Value = r.json().await?;
    Ok(v["url"].as_str().unwrap_or("").to_string())
}

pub async fn to_hubspot(st: &St, id: &str) -> Result<usize> {
    let token = st.settings.read().hubspot_token.trim().to_string();
    if token.is_empty() {
        return Err(anyhow!("Add your HubSpot private app token in Settings › Connectors first."));
    }
    let m = st.db.meeting(id)?.ok_or_else(|| anyhow!("missing note"))?;
    let c = http();
    // Attendees that are HubSpot contacts.
    let mut contacts = vec![];
    for a in m.attendees.split([',', ';']) {
        let email = match a.split('<').nth(1) {
            Some(x) => x.trim_end_matches('>').trim().to_string(),
            None if a.contains('@') => a.trim().to_string(),
            None => continue,
        };
        let q = json!({"filterGroups": [{"filters": [{"propertyName": "email", "operator": "EQ", "value": email}]}], "limit": 1});
        if let Ok(r) = c.post("https://api.hubapi.com/crm/v3/objects/contacts/search").bearer_auth(&token).json(&q).send().await {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(cid) = v["results"][0]["id"].as_str() {
                    contacts.push(cid.to_string());
                }
            }
        }
    }
    let html = format!("<h3>{}</h3>{}", title(&m), body_md(&m).lines().map(|l| format!("<p>{}</p>", l.replace('<', "&lt;"))).collect::<String>());
    let assoc: Vec<Value> = contacts.iter().map(|cid| json!({"to": {"id": cid}, "types": [{"associationCategory": "HUBSPOT_DEFINED", "associationTypeId": 202}]})).collect();
    let body = json!({"properties": {"hs_timestamp": chrono::Utc::now().to_rfc3339(), "hs_note_body": html}, "associations": assoc});
    let r = c.post("https://api.hubapi.com/crm/v3/objects/notes").bearer_auth(&token).json(&body).send().await?;
    if !r.status().is_success() {
        let t = r.text().await.unwrap_or_default();
        return Err(anyhow!("HubSpot said no: {}", t.chars().take(300).collect::<String>()));
    }
    Ok(contacts.len())
}

/// Posts text to a folder's Slack or other webhook.
pub async fn post_text(st: &St, folder: &str, text: &str) -> Result<()> {
    let f = st.db.folder(folder)?.ok_or_else(|| anyhow!("missing folder"))?;
    if f.webhook.trim().is_empty() {
        return Err(anyhow!("That folder has no Slack or webhook link yet."));
    }
    let body = if f.webhook.contains("hooks.slack.com") { json!({ "text": text }) } else { json!({ "text": text, "folder": f.name }) };
    let r = http().post(f.webhook.trim()).json(&body).send().await?;
    if !r.status().is_success() {
        return Err(anyhow!("The webhook answered HTTP {}", r.status()));
    }
    Ok(())
}

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\n', "\\n")
}

/// A calendar event draft (.ics) the user's calendar app opens for review.
pub fn calendar_draft(summary: &str, details: &str) -> Result<std::path::PathBuf> {
    let start = (chrono::Utc::now() + chrono::Duration::days(1)).date_naive().and_hms_opt(16, 0, 0).unwrap_or_default();
    let fmt = |d: chrono::NaiveDateTime| d.format("%Y%m%dT%H%M%SZ").to_string();
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Cereal//EN\r\nBEGIN:VEVENT\r\nUID:{}@cereal\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{}\r\nDESCRIPTION:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        uuid::Uuid::new_v4(),
        fmt(chrono::Utc::now().naive_utc()),
        fmt(start),
        fmt(start + chrono::Duration::minutes(30)),
        ics_escape(summary),
        ics_escape(details)
    );
    let p = std::env::temp_dir().join(format!("cereal-event-{}.ics", chrono::Utc::now().timestamp()));
    std::fs::write(&p, ics)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    #[test]
    fn blocks() {
        let b = super::notion_blocks("### Decisions\n- Ship Friday\n- [ ] Dana: send deck\nPlain line");
        assert_eq!(b.len(), 4);
        assert_eq!(b[0]["type"], "heading_3");
        assert_eq!(b[2]["type"], "to_do");
        assert_eq!(super::notion_id("https://www.notion.so/team/Meeting-notes-0123456789abcdef0123456789abcdef?pvs=4"), "0123456789abcdef0123456789abcdef");
    }
}
