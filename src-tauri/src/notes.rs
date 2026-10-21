//! The AI part: turning your rough notes + the transcript into clean notes with a
//! template, and answering questions about one meeting or all of them.
//!
//! Long meetings are written in two steps so they fit a small local model: each
//! part of the transcript is boiled down to dense notes first, then the final notes
//! are written from those plus your own notes.

use crate::db::{Meeting, Segment, Template};
use crate::state::St;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::time::Duration;
use toolbay_engine::chat::{approx_tokens, msg, Llm, Msg, Provider};

fn t(id: &str, name: &str, description: &str, sections: &str, instructions: &str) -> Template {
    Template { id: id.into(), name: name.into(), description: description.into(), sections: sections.into(), instructions: instructions.into(), builtin: true, updated_at: String::new() }
}

/// Built-in templates. "auto" lets the AI pick a shape that fits the meeting.
pub fn builtin() -> Vec<Template> {
    vec![
        t("auto", "Auto", "Picks a format that fits the conversation", "", "Choose 3 to 6 section headings that fit what this meeting was actually about (for example the main topics discussed), then finish with \"Decisions\" and \"Action items\" if there were any."),
        t("one-on-one", "1:1", "Check-ins between a manager and a report, or two peers",
            "Updates: what each person shared since last time\nWins and challenges\nFeedback: given or asked for, both ways\nCareer and growth\nDecisions\nAction items: owner and due date",
            "Keep personal or sensitive topics brief and neutral."),
        t("sales-call", "Sales call", "Discovery and sales conversations with a prospect",
            "About the customer: company, role, team, current setup\nPain points: problems in their own words, impact and urgency\nRequirements: must-haves and nice-to-haves\nBudget and timeline\nDecision process: who decides, who else is involved, approval steps\nObjections and concerns\nCompetitors and alternatives mentioned\nNext steps: what was agreed, with dates",
            "Quote the customer's exact words for important pains and objections. Note numbers (seats, price, dates) exactly."),
        t("standup", "Stand-up", "Daily or weekly team stand-ups",
            "Per person: what they did, what's next, blockers (one sub-list per person)\nBlockers that need help\nAnnouncements\nAction items",
            "Use each person's name as a bold label. Keep every bullet short."),
        t("interview", "Interview", "Job interviews, from the interviewer's side",
            "Candidate: name, current role, background\nExperience highlights: relevant projects with specifics\nSkills shown: technical and soft skills, with evidence from answers\nQuestions the candidate asked\nConcerns or gaps\nOverall impression: strengths and risks, without a hire decision unless one was stated\nNext steps",
            "Stay factual and fair; attribute claims to the candidate (\"Says they…\"). Never comment on age, appearance, family, health or other protected traits."),
        t("team-meeting", "Team meeting", "Weekly syncs and team meetings",
            "Updates by topic or project\nDiscussion: key points and different views\nDecisions\nOpen questions\nAction items: owner and due date", ""),
        t("customer-discovery", "User interview", "Customer research and user interviews",
            "About them: role, context, tools they use\nCurrent workflow: how they do it today, step by step\nPain points: in their own words\nWhat they've tried\nReactions to ideas or prototypes shown\nMemorable quotes\nInsights and follow-ups",
            "Prefer the customer's own words. Separate what they said from your interpretation."),
        t("project-kickoff", "Project kick-off", "Starting a new project with a team or client",
            "Goals and success criteria\nScope: in and out\nTimeline and milestones\nRoles and owners\nRisks and dependencies\nDecisions\nAction items", ""),
        t("pitch", "Investor pitch", "Pitches and fundraising conversations",
            "Company and product\nTraction and metrics\nMarket and competition\nTeam\nThe raise: amount, terms, use of funds\nInvestor questions and concerns\nNext steps", "Note every number exactly as said."),
        t("brainstorm", "Brainstorm", "Idea sessions and workshops",
            "Problem we're solving\nIdeas: grouped by theme, with who proposed them\nFavourites and why\nConcerns\nDecisions and next steps", ""),
        t("lecture", "Lecture or class", "Talks, classes, webinars and trainings",
            "Main topics in order\nKey concepts and definitions\nExamples given\nQuestions and answers\nThings to review or do", "Write it like good study notes."),
    ]
}

pub fn all_templates(st: &St) -> Vec<Template> {
    let mut v = builtin();
    v.extend(st.db.custom_templates().unwrap_or_default());
    v
}

pub fn template(st: &St, id: &str) -> Template {
    all_templates(st).into_iter().find(|t| t.id == id).unwrap_or_else(|| builtin().remove(0))
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Transcript as speaker turns: "[12:03] Dana: …" (consecutive lines merged).
pub fn transcript_lines(segs: &[Segment], names: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut cur: Option<(String, f64, String)> = None;
    for s in segs {
        let who = names.get(&s.speaker).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).unwrap_or(&s.speaker).to_string();
        match cur.as_mut() {
            Some((w, _, text)) if *w == who && text.len() < 1200 => {
                text.push(' ');
                text.push_str(s.text.trim());
            }
            _ => {
                if let Some((w, t0, text)) = cur.take() {
                    out.push(format!("[{}] {}: {}", fmt_time(t0), w, text));
                }
                cur = Some((who, s.start, s.text.trim().to_string()));
            }
        }
    }
    if let Some((w, t0, text)) = cur {
        out.push(format!("[{}] {}: {}", fmt_time(t0), w, text));
    }
    out
}

/// Splits lines into pieces of at most `max_chars`.
fn split(lines: &[String], max_chars: usize) -> Vec<String> {
    let mut v = vec![];
    let mut cur = String::new();
    for l in lines {
        if !cur.is_empty() && cur.len() + l.len() > max_chars {
            v.push(std::mem::take(&mut cur));
        }
        cur.push_str(l);
        cur.push('\n');
    }
    if !cur.trim().is_empty() {
        v.push(cur);
    }
    v
}

const NOTES_SYSTEM: &str = r####"You are an expert note-taker. You turn a meeting into clean, skimmable notes in Markdown.

You get:
- MY NOTES: rough notes the user typed during the meeting. They show what the user cares about most.
- TRANSCRIPT (or PART NOTES for long meetings): what was said, with speaker labels. The user is "{me}".
- TEMPLATE: the sections to use.

Rules:
1. Build on MY NOTES. Every point the user wrote must appear in your notes (fix typos and expand it with details from the transcript, but keep the user's wording where you can). Headings the user wrote are sections you must keep.
2. Add the other important points from the transcript: facts, numbers, names, dates, decisions, commitments, open questions.
3. Only write what was said or written. Never invent names, numbers, dates, deadlines or owners. If an owner or a deadline wasn't said, leave it out (write "- [ ] task" with no owner).
4. Format: "### " for section headings, "- " bullets, two-space indented sub-bullets for detail. Action items as "- [ ] Owner: task (by when)". No bold text.
5. Skip sections with nothing to put in them. No intro, no conclusion, no "Summary:" preamble, no notes about these rules.
6. Write in the language the meeting was held in.
7. The very first line must be: Title: <a short, specific title for this meeting, max 8 words>
Then a blank line, then the notes."####;

const PART_SYSTEM: &str = "You condense part of a meeting transcript into dense, factual notes for a later summary. Output only bullets, at most 30. Keep: topics, key points with who said them, every number, name, date and product mentioned, decisions, commitments and action items (who, what, when), open questions. Drop small talk and filler. Never invent anything. The user is \"{me}\".";

fn template_block(t: &Template) -> String {
    let mut s = format!("TEMPLATE: {}\n", t.name);
    if !t.sections.trim().is_empty() {
        s.push_str("Sections (heading: what goes there):\n");
        for l in t.sections.lines().filter(|l| !l.trim().is_empty()) {
            s.push_str(&format!("- {}\n", l.trim()));
        }
    }
    if !t.instructions.trim().is_empty() {
        s.push_str(&format!("Instructions: {}\n", t.instructions.trim()));
    }
    s
}

fn header(m: &Meeting) -> String {
    let mut h = String::new();
    if !m.title.trim().is_empty() {
        h.push_str(&format!("Meeting: {}\n", m.title.trim()));
    }
    h.push_str(&format!("Date: {}\n", m.created_at.get(..10).unwrap_or("")));
    if !m.app.is_empty() {
        h.push_str(&format!("App: {}\n", m.app));
    }
    if !m.attendees.trim().is_empty() {
        h.push_str(&format!("Attendees: {}\n", m.attendees.trim()));
    }
    h
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NotesProgress {
    meeting_id: String,
    text: String,
}

/// Writes (or rewrites) a meeting's AI notes. Updates the title if it was empty.
pub async fn enhance(st: &St, id: &str, template_id: Option<String>) -> Result<String> {
    let m = st.db.meeting(id)?.ok_or_else(|| anyhow!("meeting not found"))?;
    if let Some(tid) = &template_id {
        st.db.set(id, "template_id", tid)?;
    }
    let tpl = template(st, template_id.as_deref().unwrap_or(&m.template_id));
    let segs = st.db.segments(id)?;
    let names: serde_json::Value = serde_json::from_str(&m.speaker_names).unwrap_or_default();
    let lines = transcript_lines(&segs, &names);
    let me = st.settings.read().me();
    let llm = crate::engine::llm(st).await?;
    // Room for the transcript in one go.
    let budget_tokens = if llm.provider == Provider::Local { crate::engine::llm_ctx(st).saturating_sub(4500) } else { 60_000 };
    let transcript = lines.join("\n");
    let body = if approx_tokens(&transcript) <= budget_tokens {
        format!("TRANSCRIPT:\n{transcript}")
    } else {
        // Long meeting: condense each part first.
        let pieces = split(&lines, (budget_tokens * 4 / 2).max(8000));
        let mut parts = String::new();
        for (i, p) in pieces.iter().enumerate() {
            let _ = st.db.set(id, "step", &format!("Reading the meeting… part {} of {}", i + 1, pieces.len()));
            st.meeting_changed(id);
            let sys = PART_SYSTEM.replace("{me}", &me);
            let u = format!("PART {} OF {}:\n{}", i + 1, pieces.len(), p);
            let out = llm.chat(&sys, &[msg("user", u)], 900, Duration::from_secs(900)).await?;
            parts.push_str(&format!("PART {} NOTES:\n{}\n\n", i + 1, out.trim()));
        }
        let _ = st.db.set(id, "step", "Writing your notes…");
        st.meeting_changed(id);
        parts
    };
    let my = if m.my_notes.trim().is_empty() { "(none)".to_string() } else { m.my_notes.trim().to_string() };
    let profile = {
        let s = st.settings.read();
        let mut p = String::new();
        if !s.job_title.trim().is_empty() || !s.about.trim().is_empty() {
            let jt = if s.job_title.trim().is_empty() { String::new() } else { format!(", {}", s.job_title.trim()) };
            let ab = if s.about.trim().is_empty() { String::new() } else { format!(". {}", s.about.trim()) };
            p.push_str(&format!("ABOUT THE USER: {me}{jt}{ab}\n"));
        }
        if !s.vocabulary.is_empty() {
            p.push_str(&format!("SPELL THESE EXACTLY: {}\n", s.vocabulary.join(", ")));
        }
        if s.summary_language != "auto" && !s.summary_language.is_empty() {
            p.push_str(&format!("WRITE THE NOTES IN LANGUAGE CODE: {}\n", s.summary_language));
        }
        p
    };
    let user = format!("{}{}\n{}\nMY NOTES:\n{}\n\n{}", profile, header(&m), template_block(&tpl), my, body);
    let sys = NOTES_SYSTEM.replace("{me}", &me);
    let id2 = id.to_string();
    let st2 = st.clone();
    let out = llm
        .chat_stream(&sys, &[msg("user", user)], 2000, Duration::from_secs(900), &move |piece: &str| {
            st2.emit("notes-token", NotesProgress { meeting_id: id2.clone(), text: piece.to_string() });
        })
        .await?;
    let (title, notes) = split_title(&out);
    let notes = unbold(&notes);
    if notes.trim().is_empty() {
        return Err(anyhow!("the AI model returned empty notes"));
    }
    st.db.set(id, "enhanced", notes.trim())?;
    if m.title.trim().is_empty() {
        if let Some(t) = title {
            st.db.set(id, "title", &t)?;
        }
    }
    st.meeting_changed(id);
    Ok(notes)
}

pub fn split_title(out: &str) -> (Option<String>, String) {
    let out = out.trim();
    let mut lines = out.lines();
    if let Some(first) = lines.next() {
        let f = first.trim().trim_start_matches('#').trim();
        if let Some(t) = f.strip_prefix("Title:").or_else(|| f.strip_prefix("**Title:**")).or_else(|| f.strip_prefix("TITLE:")) {
            let t = t.trim().trim_matches('*').trim_matches('"').trim().to_string();
            let rest = lines.collect::<Vec<_>>().join("\n");
            return (Some(t).filter(|t| !t.is_empty()), rest.trim().to_string());
        }
    }
    (None, out.to_string())
}

/// Small models over-use **bold**; keep it only for a leading "**Name:**" label.
pub fn unbold(md: &str) -> String {
    md.lines()
        .map(|l| {
            let t = l.trim_start();
            let indent = &l[..l.len() - t.len()];
            let (prefix, rest) = match t.find(' ') {
                Some(i) if t.starts_with("- ") || t.starts_with("* ") => {
                    let after = &t[i + 1..];
                    let cb = if after.starts_with("[ ] ") || after.starts_with("[x] ") { 4 } else { 0 };
                    (&t[..i + 1 + cb], &t[i + 1 + cb..])
                }
                _ => ("", t),
            };
            let (label, body) = match rest.strip_prefix("**").and_then(|r| r.find("**").map(|e| (e, r))) {
                Some((e, r)) if (r[e + 2..].starts_with(':') || r[..e].ends_with(':')) && e < 40 && !prefix.is_empty() => (format!("**{}**", &r[..e]), &r[e + 2..]),
                _ => (String::new(), rest),
            };
            format!("{indent}{prefix}{label}{}", body.replace("**", ""))
        })
        .collect::<Vec<_>>()
        .join("
")
}

// ---------------- chat ----------------

const CHAT_SYSTEM: &str = "You answer questions about the user's meetings, using only the material provided. The user is \"{me}\". Be direct and concise; use short bullets when listing. Each source is numbered like [1]; when you use something from a source, cite it with its number in square brackets right after the sentence, e.g. [2]. If the material doesn't contain the answer, say so plainly instead of guessing. If asked to write something (an email, a summary, a message), write it ready to copy.";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatToken {
    request_id: String,
    text: String,
}

#[derive(Clone, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub n: usize,
    pub meeting_id: String,
    pub title: String,
    pub date: String,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub thread_id: String,
    pub answer: String,
    pub sources: Vec<Source>,
}

fn meeting_context(st: &St, m: &Meeting, budget_chars: usize, question: &str, n: usize) -> String {
    let mut s = format!("=== [{n}] Meeting: {} ({})\n", if m.title.is_empty() { "Untitled" } else { &m.title }, m.created_at.get(..10).unwrap_or(""));
    if !m.attendees.trim().is_empty() {
        s.push_str(&format!("Attendees: {}\n", m.attendees.trim()));
    }
    if !m.enhanced.trim().is_empty() {
        s.push_str(&format!("Notes:\n{}\n", m.enhanced.trim()));
    }
    if !m.my_notes.trim().is_empty() {
        s.push_str(&format!("User's own notes:\n{}\n", m.my_notes.trim()));
    }
    let names: serde_json::Value = serde_json::from_str(&m.speaker_names).unwrap_or_default();
    let lines = transcript_lines(&st.db.segments(&m.id).unwrap_or_default(), &names);
    let room = budget_chars.saturating_sub(s.len());
    let full = lines.join("\n");
    if full.len() <= room {
        if !full.is_empty() {
            s.push_str(&format!("Transcript:\n{full}\n"));
        }
    } else if room > 500 {
        // Keep the lines that share words with the question, plus their neighbours.
        let q: Vec<String> = question.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 3).map(String::from).collect();
        let mut keep = vec![false; lines.len()];
        for (i, l) in lines.iter().enumerate() {
            let ll = l.to_lowercase();
            if q.iter().any(|w| ll.contains(w.as_str())) {
                for k in keep.iter_mut().take((i + 2).min(lines.len())).skip(i.saturating_sub(1)) {
                    *k = true;
                }
            }
        }
        let mut picked = String::new();
        for (l, k) in lines.iter().zip(keep) {
            if k && picked.len() + l.len() < room {
                picked.push_str(l);
                picked.push('\n');
            }
        }
        if picked.is_empty() {
            // The most recent lines are the best guess for "what did I miss".
            picked = full.chars().rev().take(room).collect::<Vec<_>>().into_iter().rev().collect();
        }
        s.push_str(&format!("Transcript excerpts:\n{picked}\n"));
    }
    s
}

/// The meetings a chat scope covers, most relevant first.
fn scope_meetings(st: &St, scope: &str, scope_id: &str, question: &str) -> Result<Vec<Meeting>> {
    Ok(match scope {
        "meeting" => st.db.meeting(scope_id)?.into_iter().collect(),
        "folder" => st.db.meetings(Some(scope_id))?.into_iter().take(20).collect(),
        "person" => st.db.meetings_with(Some(scope_id), None)?.into_iter().take(20).collect(),
        "company" => st.db.meetings_with(None, Some(scope_id))?.into_iter().take(20).collect(),
        "selected" => scope_id.split(',').filter_map(|id| st.db.meeting(id.trim()).ok().flatten()).collect(),
        _ => {
            let mut ids = st.db.search_any(question, 6)?;
            let ql = question.to_lowercase();
            let recent_n = if ql.contains("week") || ql.contains("recent") || ql.contains("todo") || ql.contains("year") { 15 } else { 5 };
            for m in st.db.meetings(None)?.into_iter().take(recent_n) {
                if !ids.contains(&m.id) {
                    ids.push(m.id);
                }
            }
            ids.into_iter().filter_map(|id| st.db.meeting(&id).ok().flatten()).collect()
        }
    })
}

/// Answers in a chat thread (a new thread when `thread_id` is None), streaming
/// "chat-token" events, with numbered sources the answer cites.
pub async fn chat(st: &St, thread_id: Option<String>, scope: &str, scope_id: &str, question: &str, attachment: Option<String>, request_id: &str) -> Result<Answer> {
    let llm: Llm = crate::engine::llm(st).await?;
    let budget = if llm.provider == Provider::Local { (crate::engine::llm_ctx(st).saturating_sub(3000)) * 3 } else { 200_000 };
    let me = st.settings.read().me();
    let thread = match thread_id.and_then(|t| st.db.thread(&t).ok().flatten()) {
        Some(t) => t,
        None => st.db.create_thread(scope, scope_id, question)?,
    };
    let meetings = scope_meetings(st, &thread.scope, &thread.scope_id, question)?;
    let per = budget / meetings.len().max(1);
    let mut context = String::new();
    let mut sources = vec![];
    for (i, m) in meetings.iter().enumerate() {
        context.push_str(&meeting_context(st, m, per, question, i + 1));
        context.push('\n');
        sources.push(Source { n: i + 1, meeting_id: m.id.clone(), title: if m.title.is_empty() { "Untitled meeting".into() } else { m.title.clone() }, date: m.created_at.clone() });
    }
    if thread.scope == "folder" {
        for (name, text) in st.db.file_texts(&thread.scope_id)?.into_iter().take(5) {
            context.push_str(&format!("=== File: {name}\n{}\n", text.chars().take(budget / 6).collect::<String>()));
        }
    }
    if let Some(a) = attachment.filter(|a| !a.trim().is_empty()) {
        context.push_str(&format!("=== Attached by the user:\n{}\n", a.chars().take(budget / 4).collect::<String>()));
    }
    if context.trim().is_empty() {
        context = "(no meetings yet)".into();
    }
    let history = st.db.thread_msgs(&thread.id)?;
    let mut msgs: Vec<Msg> = vec![msg("user", format!("MATERIAL:\n{context}\n\nI'll ask questions about this.")), msg("assistant", "Got it. What would you like to know?")];
    for h in history.iter().rev().take(6).rev() {
        msgs.push(Msg { role: if h.role == "user" { "user" } else { "assistant" }, content: h.content.clone() });
    }
    msgs.push(msg("user", question));
    st.db.add_thread_msg(&thread.id, "user", question)?;
    let rid = request_id.to_string();
    let st2 = st.clone();
    let answer = llm
        .chat_stream(&CHAT_SYSTEM.replace("{me}", &me), &msgs, 1400, Duration::from_secs(600), &move |p: &str| {
            st2.emit("chat-token", ChatToken { request_id: rid.clone(), text: p.to_string() });
        })
        .await?;
    st.db.add_thread_msg(&thread.id, "assistant", &answer)?;
    st.db.touch_thread(&thread.id)?;
    // Keep only the sources the answer cites (for a single meeting, always that one).
    let cited: Vec<Source> = sources.iter().filter(|s| answer.contains(&format!("[{}]", s.n))).cloned().collect();
    let sources = if cited.is_empty() && thread.scope == "meeting" { sources } else { cited };
    let _ = st.db.set_kv(&format!("sources:{}", thread.id), &serde_json::to_string(&sources).unwrap_or_default());
    Ok(Answer { thread_id: thread.id, answer, sources })
}

/// Sources of the last answer in a thread (for showing citations again later).
pub fn thread_sources(st: &St, thread_id: &str) -> Vec<Source> {
    st.db.kv(&format!("sources:{thread_id}")).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Headless --ask mode.
pub async fn ask(st: &St, meeting_id: Option<String>, question: &str, request_id: &str) -> Result<String> {
    let (scope, id) = match &meeting_id {
        Some(m) => ("meeting", m.as_str()),
        None => ("all", ""),
    };
    Ok(chat(st, None, scope, id, question, None, request_id).await?.answer)
}

/// Pre-meeting brief: 2-3 bullets from past notes with the same people.
pub async fn brief(st: &St, attendees: &[String], title: &str) -> Result<String> {
    let mut past: Vec<Meeting> = vec![];
    for a in attendees {
        let (_, email) = crate::library::parse_attendees(a).into_iter().next().unwrap_or_default();
        if email.is_empty() {
            continue;
        }
        for m in st.db.meetings_with(Some(&email), None)?.into_iter().take(4) {
            if !past.iter().any(|p| p.id == m.id) {
                past.push(m);
            }
        }
    }
    if past.is_empty() {
        return Ok(String::new());
    }
    let llm = crate::engine::llm(st).await?;
    let mut ctx = String::new();
    for (i, m) in past.iter().take(6).enumerate() {
        ctx.push_str(&meeting_context(st, m, 3000, title, i + 1));
    }
    let q = format!("I'm about to join \"{title}\". From these past meetings with the same people, give me 2-3 short bullets to prepare: open threads, commitments, what to follow up on. Bullets only, no citations.");
    llm.chat("You prepare the user for a meeting using only their past notes. Be brief and specific.", &[msg("user", format!("{ctx}\n\n{q}"))], 400, Duration::from_secs(300)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title() {
        let (t, n) = split_title("Title: Q3 pricing review\n\n### Decisions\n- Raise price");
        assert_eq!(t.as_deref(), Some("Q3 pricing review"));
        assert!(n.starts_with("### Decisions"));
        let (t, _) = split_title("### Notes\n- a");
        assert!(t.is_none());
    }
    #[test]
    fn lines_merge() {
        let s = |sp: &str, a: f64, t: &str| Segment { speaker: sp.into(), start: a, end: a + 1.0, text: t.into(), ..Default::default() };
        let segs = vec![s("You", 0.0, "Hi."), s("You", 1.0, "How are you?"), s("Speaker 1", 3.0, "Good."), s("Speaker 2", 65.0, "Hello")];
        let names = serde_json::json!({"Speaker 1": "Dana"});
        let l = transcript_lines(&segs, &names);
        assert_eq!(l, vec!["[0:00] You: Hi. How are you?", "[0:03] Dana: Good.", "[1:05] Speaker 2: Hello"]);
        assert_eq!(split(&l, 30).len(), 3);
    }
    #[test]
    fn bold() {
        assert_eq!(unbold("- **Dana:** shipped the **beta**"), "- **Dana:** shipped the beta");
        assert_eq!(unbold("  - [ ] **Sam**: send deck"), "  - [ ] **Sam**: send deck");
        assert_eq!(unbold("### **Decisions**"), "### Decisions");
    }
    #[test]
    fn templates_have_required() {
        let ids: Vec<String> = builtin().into_iter().map(|t| t.id).collect();
        for want in ["auto", "one-on-one", "sales-call", "standup", "interview"] {
            assert!(ids.contains(&want.to_string()));
        }
    }
}
