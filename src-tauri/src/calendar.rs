//! Calendars, the local-first way: the user pastes their calendar's private iCal
//! address (Google Calendar: Settings > "Secret address in iCal format"; Outlook:
//! Settings > Shared calendars > Publish > ICS). Cereal reads it every 10 minutes;
//! nothing about the calendar is sent anywhere else.
//!
//! Gives: "Coming up" on Home, a popup a minute before meetings with 2+ people,
//! titles and attendees for notes, and links recordings to the event they belong to.

use crate::state::St;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSrc {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// uid + start (unique per occurrence)
    pub id: String,
    pub uid: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    /// "Name <email>" entries
    pub attendees: Vec<String>,
    pub location: String,
    /// Join link (Zoom, Meet, Teams, Webex) if one was found.
    pub link: String,
    pub calendar: String,
    pub description: String,
    /// The calendar's owner said no (no popup, like Granola).
    #[serde(default)]
    pub declined: bool,
}

static EVENTS: Lazy<Mutex<Vec<Event>>> = Lazy::new(|| Mutex::new(vec![]));
static ANNOUNCED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

pub fn events() -> Vec<Event> {
    EVENTS.lock().clone()
}

// ---------------- parsing ----------------

struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in ics.replace("\r\n", "\n").split('\n') {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn prop(line: &str) -> Option<Prop> {
    // NAME;P1=V1;P2="V2":value  (colons may appear inside quoted params)
    let mut in_q = false;
    let mut split = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_q = !in_q,
            ':' if !in_q => {
                split = Some(i);
                break;
            }
            _ => {}
        }
    }
    let i = split?;
    let (head, value) = (&line[..i], &line[i + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_uppercase();
    let params = parts.filter_map(|p| p.split_once('=').map(|(a, b)| (a.to_uppercase(), b.trim_matches('"').to_string()))).collect();
    Some(Prop { name, params, value: value.to_string() })
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

fn param<'a>(p: &'a Prop, k: &str) -> Option<&'a str> {
    p.params.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str())
}

/// Parses a date-time property to UTC. Floating times are local.
fn parse_dt(p: &Prop) -> Option<(DateTime<Utc>, bool)> {
    let v = p.value.trim();
    if param(p, "VALUE") == Some("DATE") || (v.len() == 8 && !v.contains('T')) {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        let local = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()?;
        return Some((local.with_timezone(&Utc), true));
    }
    if let Some(z) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&n), false));
    }
    let n = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    if let Some(tz) = param(p, "TZID").and_then(|t| t.parse::<chrono_tz::Tz>().ok().or_else(|| windows_tz(t))) {
        return tz.from_local_datetime(&n).earliest().map(|d| (d.with_timezone(&Utc), false));
    }
    Local.from_local_datetime(&n).earliest().map(|d| (d.with_timezone(&Utc), false))
}

/// Outlook often uses Windows zone names.
fn windows_tz(name: &str) -> Option<chrono_tz::Tz> {
    let iana = match name {
        "Pacific Standard Time" => "America/Los_Angeles",
        "Mountain Standard Time" => "America/Denver",
        "Central Standard Time" => "America/Chicago",
        "Eastern Standard Time" => "America/New_York",
        "GMT Standard Time" => "Europe/London",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "Central European Standard Time" => "Europe/Warsaw",
        "India Standard Time" => "Asia/Kolkata",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "UTC" | "Coordinated Universal Time" => "UTC",
        _ => return None,
    };
    iana.parse().ok()
}

fn find_link(text: &str) -> String {
    let pats = [
        "zoom.us/j/", "zoom.us/my/", "zoom.us/w/", "zoomgov.com/j/", "zoom.com/j/", "meet.google.com/", "teams.microsoft.com/l/meetup-join", "teams.live.com/meet",
        ".webex.com/", "whereby.com/", "meet.jit.si/", "pexip.me/", "calendly.com/events/", "app.cal.com/video/", "riverside.com/", "riverside.fm/",
    ];
    for w in text.split(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"' || c == '(' || c == ')') {
        let w = unwrap_safelink(w);
        if w.starts_with("http") && pats.iter().any(|p| w.contains(p)) {
            return w.trim_end_matches(['.', ',', ';']).to_string();
        }
    }
    String::new()
}

/// Outlook wraps links as https://*.safelinks.protection.outlook.com/?url=<encoded>&...
fn unwrap_safelink(w: &str) -> String {
    if !w.contains("safelinks.protection.outlook.com") {
        return w.to_string();
    }
    let Some(i) = w.find("url=") else { return w.to_string() };
    let enc = w[i + 4..].split('&').next().unwrap_or("");
    urlencoding::decode(enc).map(|c| c.into_owned()).unwrap_or_else(|_| w.to_string())
}

/// Zoom links open straight in the Zoom app (like Granola), falling back to the web link.
pub fn app_link(link: &str) -> String {
    let Ok(u) = url::Url::parse(link) else { return link.to_string() };
    let host = u.host_str().unwrap_or("");
    if !(host.ends_with("zoom.us") || host.ends_with("zoom.com") || host.ends_with("zoomgov.com")) {
        return link.to_string();
    }
    let segs: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    let Some(id) = segs.iter().position(|s| *s == "j").and_then(|i| segs.get(i + 1)) else { return link.to_string() };
    let pwd = u.query_pairs().find(|(k, _)| k == "pwd").map(|(_, v)| v.into_owned());
    let mut out = format!("zoommtg://{host}/join?action=join&confno={id}");
    if let Some(p) = pwd {
        out.push_str(&format!("&pwd={p}"));
    }
    out
}

struct Raw {
    uid: String,
    title: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    all_day: bool,
    attendees: Vec<String>,
    location: String,
    description: String,
    url: String,
    rrule: String,
    exdates: Vec<DateTime<Utc>>,
    recurrence_id: Option<DateTime<Utc>>,
    cancelled: bool,
    declined: bool,
}

fn weekday(s: &str) -> Option<Weekday> {
    Some(match s.trim_start_matches(|c: char| c == '-' || c == '+' || c.is_ascii_digit()) {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

/// Occurrence starts of a recurring event inside [from, to] (DAILY/WEEKLY/MONTHLY/YEARLY,
/// INTERVAL, COUNT, UNTIL, BYDAY for weekly). Enough for real meeting calendars.
fn expand(r: &Raw, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    if r.rrule.is_empty() {
        return if r.end >= from && r.start <= to { vec![r.start] } else { vec![] };
    }
    let mut freq = "";
    let mut interval = 1i64;
    let mut count: Option<usize> = None;
    let mut until: Option<DateTime<Utc>> = None;
    let mut byday: Vec<Weekday> = vec![];
    for kv in r.rrule.split(';') {
        let Some((k, v)) = kv.split_once('=') else { continue };
        match k {
            "FREQ" => freq = v,
            "INTERVAL" => interval = v.parse().unwrap_or(1).max(1),
            "COUNT" => count = v.parse().ok(),
            "UNTIL" => until = parse_dt(&Prop { name: "UNTIL".into(), params: vec![], value: v.into() }).map(|x| x.0),
            "BYDAY" => byday = v.split(',').filter_map(weekday).collect(),
            _ => {}
        }
    }
    // Work in local time so "every Tuesday at 10" stays at 10 across DST changes.
    let base = r.start.with_timezone(&Local);
    let mut out = vec![];
    let mut n = 0usize;
    let limit_until = until.unwrap_or(to).min(to + Duration::days(1));
    let push = |t: DateTime<Local>, out: &mut Vec<DateTime<Utc>>, n: &mut usize| -> bool {
        let u = t.with_timezone(&Utc);
        if u < r.start {
            return true;
        }
        if count.map(|c| *n >= c).unwrap_or(false) || u > limit_until {
            return false;
        }
        *n += 1;
        if u + (r.end - r.start) >= from && u <= to && !r.exdates.contains(&u) {
            out.push(u);
        }
        true
    };
    match freq {
        "DAILY" => {
            let mut t = base;
            for _ in 0..2000 {
                if !push(t, &mut out, &mut n) {
                    break;
                }
                t += Duration::days(interval);
            }
        }
        "WEEKLY" => {
            let days = if byday.is_empty() { vec![base.weekday()] } else { byday.clone() };
            let week0 = base - Duration::days(base.weekday().num_days_from_monday() as i64);
            'outer: for w in 0..1000 {
                let ws = week0 + Duration::weeks(w * interval);
                let mut ds: Vec<i64> = days.iter().map(|d| d.num_days_from_monday() as i64).collect();
                ds.sort();
                for d in ds {
                    if !push(ws + Duration::days(d), &mut out, &mut n) {
                        break 'outer;
                    }
                }
            }
        }
        "MONTHLY" => {
            for k in 0..240 {
                let months = base.month0() as i64 + k * interval;
                let (y, m) = (base.year() + (months / 12) as i32, (months % 12) as u32 + 1);
                let Some(d) = NaiveDate::from_ymd_opt(y, m, base.day()) else { continue };
                let Some(t) = Local.from_local_datetime(&d.and_time(base.time())).earliest() else { continue };
                if !push(t, &mut out, &mut n) {
                    break;
                }
            }
        }
        "YEARLY" => {
            for k in 0..50 {
                let Some(d) = NaiveDate::from_ymd_opt(base.year() + (k * interval) as i32, base.month(), base.day()) else { continue };
                let Some(t) = Local.from_local_datetime(&d.and_time(base.time())).earliest() else { continue };
                if !push(t, &mut out, &mut n) {
                    break;
                }
            }
        }
        _ => {
            if r.end >= from && r.start <= to {
                out.push(r.start);
            }
        }
    }
    out
}

pub fn parse(ics: &str, calendar: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<Event> {
    let mut raws: Vec<Raw> = vec![];
    let mut cur: Option<Raw> = None;
    let mut owner = String::new();
    for line in unfold(ics) {
        let l = line.trim_end();
        if cur.is_none() {
            if let Some(v) = l.strip_prefix("X-WR-CALNAME:") {
                if v.contains('@') {
                    owner = v.trim().to_lowercase();
                }
            }
        }
        if l == "BEGIN:VEVENT" {
            cur = Some(Raw {
                uid: String::new(),
                title: String::new(),
                start: Utc::now(),
                end: Utc::now(),
                all_day: false,
                attendees: vec![],
                location: String::new(),
                description: String::new(),
                url: String::new(),
                rrule: String::new(),
                exdates: vec![],
                recurrence_id: None,
                cancelled: false,
                declined: false,
            });
            continue;
        }
        if l == "END:VEVENT" {
            if let Some(r) = cur.take() {
                raws.push(r);
            }
            continue;
        }
        let Some(r) = cur.as_mut() else { continue };
        let Some(p) = prop(l) else { continue };
        match p.name.as_str() {
            "UID" => r.uid = p.value.clone(),
            "SUMMARY" => r.title = unescape(&p.value),
            "DTSTART" => {
                if let Some((t, ad)) = parse_dt(&p) {
                    r.start = t;
                    r.all_day = ad;
                    if r.end < t {
                        r.end = t;
                    }
                }
            }
            "DTEND" => {
                if let Some((t, _)) = parse_dt(&p) {
                    r.end = t;
                }
            }
            "LOCATION" => r.location = unescape(&p.value),
            "DESCRIPTION" => r.description = unescape(&p.value),
            "URL" => r.url = p.value.clone(),
            "RRULE" => r.rrule = p.value.clone(),
            "EXDATE" => {
                for v in p.value.split(',') {
                    if let Some((t, _)) = parse_dt(&Prop { name: "EXDATE".into(), params: p.params.clone(), value: v.into() }) {
                        r.exdates.push(t);
                    }
                }
            }
            "RECURRENCE-ID" => r.recurrence_id = parse_dt(&p).map(|x| x.0),
            "STATUS" => r.cancelled = p.value.eq_ignore_ascii_case("CANCELLED"),
            "ATTENDEE" | "ORGANIZER" => {
                let email = p.value.trim_start_matches("mailto:").trim_start_matches("MAILTO:").to_lowercase();
                let name = param(&p, "CN").unwrap_or("").to_string();
                if param(&p, "CUTYPE").map(|c| c == "RESOURCE" || c == "ROOM").unwrap_or(false) {
                    continue;
                }
                // A private feed is named after its owner (Google: the email address).
                if p.name == "ATTENDEE" && !owner.is_empty() && email == owner && param(&p, "PARTSTAT").map(|v| v.eq_ignore_ascii_case("DECLINED")).unwrap_or(false) {
                    r.declined = true;
                }
                let entry = if name.is_empty() || name.eq_ignore_ascii_case(&email) { email.clone() } else { format!("{name} <{email}>") };
                if !r.attendees.iter().any(|a| a.contains(&email)) {
                    r.attendees.push(entry);
                }
            }
            _ => {}
        }
    }
    // Moved single occurrences override their series occurrence.
    let overrides: Vec<(String, DateTime<Utc>)> = raws.iter().filter_map(|r| r.recurrence_id.map(|t| (r.uid.clone(), t))).collect();
    let mut out = vec![];
    for r in &raws {
        if r.cancelled {
            continue;
        }
        let starts = if r.recurrence_id.is_some() { expand(&Raw { rrule: String::new(), exdates: vec![], ..clone_raw(r) }, from, to) } else { expand(r, from, to) };
        for s in starts {
            if r.recurrence_id.is_none() && overrides.iter().any(|(u, t)| *u == r.uid && *t == s) {
                continue;
            }
            let e = s + (r.end - r.start);
            let link = [find_link(&r.url), find_link(&r.location), find_link(&r.description)].into_iter().find(|l| !l.is_empty()).unwrap_or_default();
            out.push(Event {
                id: format!("{}@{}", r.uid, s.timestamp()),
                uid: r.uid.clone(),
                title: if r.title.is_empty() { "Busy".into() } else { r.title.clone() },
                start: s.to_rfc3339(),
                end: e.to_rfc3339(),
                all_day: r.all_day,
                attendees: r.attendees.clone(),
                location: r.location.clone(),
                link,
                calendar: calendar.to_string(),
                description: r.description.chars().take(2000).collect(),
                declined: r.declined,
            });
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start));
    out
}

fn clone_raw(r: &Raw) -> Raw {
    Raw {
        uid: r.uid.clone(),
        title: r.title.clone(),
        start: r.start,
        end: r.end,
        all_day: r.all_day,
        attendees: r.attendees.clone(),
        location: r.location.clone(),
        description: r.description.clone(),
        url: r.url.clone(),
        rrule: r.rrule.clone(),
        exdates: r.exdates.clone(),
        recurrence_id: r.recurrence_id,
        cancelled: r.cancelled,
        declined: r.declined,
    }
}

// ---------------- fetching ----------------

pub async fn refresh(st: &St) -> anyhow::Result<usize> {
    let cals: Vec<CalendarSrc> = st.settings.read().calendars.iter().filter(|c| c.enabled && !c.url.trim().is_empty()).cloned().collect();
    let from = Utc::now() - Duration::days(1);
    let to = Utc::now() + Duration::days(14);
    let mut all = vec![];
    for c in cals {
        let url = c.url.trim().replacen("webcal://", "https://", 1);
        match reqwest::Client::new().get(&url).timeout(std::time::Duration::from_secs(20)).send().await {
            Ok(r) if r.status().is_success() => {
                let text = r.text().await.unwrap_or_default();
                all.extend(parse(&text, &c.name, from, to));
            }
            Ok(r) => log::warn!("calendar {}: HTTP {}", c.name, r.status()),
            Err(e) => log::warn!("calendar {}: {e}", c.name),
        }
    }
    all.sort_by(|a, b| a.start.cmp(&b.start));
    let n = all.len();
    *EVENTS.lock() = all;
    st.emit("calendar-changed", ());
    Ok(n)
}

/// The meeting a recording belongs to: started at most 15 minutes ago and still on,
/// or starting within the next 90 seconds (the latest such event wins).
pub fn current_event() -> Option<Event> {
    let now = Utc::now();
    events()
        .into_iter()
        .filter(|e| !e.all_day)
        .filter(|e| {
            let s = DateTime::parse_from_rfc3339(&e.start).map(|d| d.with_timezone(&Utc)).ok();
            let en = DateTime::parse_from_rfc3339(&e.end).map(|d| d.with_timezone(&Utc)).ok();
            match (s, en) {
                (Some(s), Some(en)) => s <= now + Duration::seconds(90) && s >= now - Duration::minutes(15) && (now <= en || s > now),
                _ => false,
            }
        })
        .last()
}

/// The linked event's end, for auto-stop.
pub fn event_end(id: &str) -> Option<DateTime<Utc>> {
    events().into_iter().find(|e| e.id == id).and_then(|e| DateTime::parse_from_rfc3339(&e.end).ok()).map(|d| d.with_timezone(&Utc))
}

pub fn next_event() -> Option<Event> {
    let now = Utc::now().to_rfc3339();
    events().into_iter().find(|e| !e.all_day && e.start > now)
}

/// Background: refresh every 10 minutes; a minute before a meeting with other people,
/// offer to take notes (or start, if the user chose automatic recording).
pub fn start(st: St) {
    tauri::async_runtime::spawn(async move {
        let mut last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(3600);
        loop {
            if last_refresh.elapsed() > std::time::Duration::from_secs(600) {
                last_refresh = std::time::Instant::now();
                if let Err(e) = refresh(&st).await {
                    log::warn!("calendar refresh: {e:#}");
                }
                crate::refresh_tray(&st);
            }
            let (popups, onboarded) = {
                let s = st.settings.read();
                (s.calendar_popups, s.onboarded)
            };
            if popups && onboarded && st.session.lock().is_none() {
                let now = Utc::now();
                // Granola: at least one other person, not declined, not all-day; shown 58 s
                // before the start (or late, up to 4 minutes in), one event per pass.
                for e in events() {
                    if e.all_day || e.declined || e.attendees.is_empty() || e.title.trim().is_empty() {
                        continue;
                    }
                    let Ok(s) = DateTime::parse_from_rfc3339(&e.start) else { continue };
                    let secs = (s.with_timezone(&Utc) - now).num_seconds();
                    if (-240..=58).contains(&secs) && ANNOUNCED.lock().insert(e.id.clone()) {
                        log::info!("meeting starting soon: {}", e.title);
                        crate::show_event_popup(&st, &e);
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    const ICS: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:abc\r\nSUMMARY:Weekly sync\\, team\r\nDTSTART:20261020T170000Z\r\nDTEND:20261020T173000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=TU,TH;COUNT=6\r\nATTENDEE;CN=Dana Lee;ROLE=REQ-PARTICIPANT:mailto:dana@acme.com\r\nATTENDEE;CN=\"Sam\":mailto:sam@acme.com\r\nLOCATION:https://us02web.zoom.us/j/123456789?pwd=x\r\nEXDATE:20261022T170000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:abc\r\nRECURRENCE-ID:20261027T170000Z\r\nSUMMARY:Weekly sync (moved)\r\nDTSTART:20261027T190000Z\r\nDTEND:20261027T193000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:x2\r\nSUMMARY:Lunch\r\nDTSTART;VALUE=DATE:20261021\r\nDTEND;VALUE=DATE:20261022\r\nDESCRIPTION:Join: https://meet.google.com/abc-defg-hij\r\n \r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_recurring() {
        let from = Utc.with_ymd_and_hms(2026, 10, 19, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 11, 10, 0, 0, 0).unwrap();
        let ev = parse(ICS, "Work", from, to);
        let sync: Vec<&Event> = ev.iter().filter(|e| e.uid == "abc").collect();
        // COUNT=6 from Tue 20th: 20,22(ex),27(moved),29, Nov 3, 5 -> 20, 27 moved, 29, 3, 5 = 5 occurrences shown
        assert_eq!(sync.len(), 5, "{:?}", sync.iter().map(|e| (&e.title, &e.start)).collect::<Vec<_>>());
        assert_eq!(sync[0].title, "Weekly sync, team");
        assert_eq!(sync[0].attendees, vec!["Dana Lee <dana@acme.com>".to_string(), "Sam <sam@acme.com>".to_string()]);
        assert!(sync[0].link.contains("zoom.us/j/123456789"));
        assert!(sync.iter().any(|e| e.title.contains("moved")));
        let lunch = ev.iter().find(|e| e.uid == "x2").unwrap();
        assert!(lunch.all_day);
        assert_eq!(lunch.link, "https://meet.google.com/abc-defg-hij");
    }

    #[test]
    fn links() {
        assert_eq!(app_link("https://us02web.zoom.us/j/123456789?pwd=abc"), "zoommtg://us02web.zoom.us/join?action=join&confno=123456789&pwd=abc");
        assert_eq!(app_link("https://meet.google.com/abc-defg-hij"), "https://meet.google.com/abc-defg-hij");
        let wrapped = "https://nam02.safelinks.protection.outlook.com/?url=https%3A%2F%2Fzoom.us%2Fj%2F42&data=x";
        assert_eq!(find_link(&format!("Join {wrapped} now")), "https://zoom.us/j/42");
    }

    #[test]
    fn declined_by_owner() {
        let ics = ["BEGIN:VCALENDAR", "X-WR-CALNAME:me@acme.com", "BEGIN:VEVENT", "UID:d1", "SUMMARY:Sync", "DTSTART:20261020T170000Z", "DTEND:20261020T173000Z", "ATTENDEE;CN=Me;PARTSTAT=DECLINED:mailto:me@acme.com", "ATTENDEE;CN=Dana:mailto:dana@acme.com", "END:VEVENT", "END:VCALENDAR"].join("\r\n");
        let from = DateTime::parse_from_rfc3339("2026-10-19T00:00:00Z").unwrap().with_timezone(&Utc);
        let ev = parse(&ics, "Work", from, from + Duration::days(3));
        assert!(ev[0].declined);
    }
}
