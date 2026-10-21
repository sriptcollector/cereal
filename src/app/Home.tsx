import { useEffect, useMemo, useState } from "react";
import { api, CalEvent, Meeting, on } from "../api";
import type { Go } from "./App";
import { AvatarSq, dayLabel, Icon, timeOf, toast } from "./ui";
import AskBar from "./ChatBar";

function attendeesLine(m: Meeting) {
  const names = m.attendees
    .split(/[,;\n]/)
    .map((a) => a.replace(/<.*>/, "").trim() || a.trim())
    .filter(Boolean)
    .map((a) => (a.includes("@") ? a.split("@")[0] : a.split(" ")[0]));
  if (!names.length) return m.app ? m.app : "";
  if (names.length <= 2) return names.join(" & ");
  return `${names.slice(0, 2).join(", ")} & ${names.length - 2} other${names.length > 3 ? "s" : ""}`;
}

function statusChip(m: Meeting) {
  if (m.status === "recording") return <span className="chip live">Live</span>;
  if (m.status === "paused") return <span className="chip">Paused</span>;
  if (m.status === "processing") return <span className="chip work">Writing notes</span>;
  if (m.status === "failed") return <span className="chip bad">Needs attention</span>;
  return null;
}

export function NoteRow({ m, go, selectable, selected, onSelect }: { m: Meeting; go: Go; selectable?: boolean; selected?: boolean; onSelect?: (v: boolean) => void }) {
  return (
    <button className={`item ${selected ? "sel" : ""}`} onClick={() => go({ page: "meeting", id: m.id })}>
      {selectable && <input type="checkbox" className="check" checked={!!selected} onClick={(e) => e.stopPropagation()} onChange={(e) => onSelect?.(e.target.checked)} />}
      <AvatarSq text={m.title || "U"} />
      <div className="item-main">
        <div className="item-title">
          {m.title || (m.status === "recording" ? "New meeting" : "Untitled meeting")}
          {statusChip(m)}
        </div>
        <div className="item-sub">{attendeesLine(m) || "Me"}</div>
      </div>
      <div className="item-meta">{timeOf(m.createdAt)}</div>
    </button>
  );
}

export function NotesByDay({ meetings, go, selectable, selected, setSelected }: { meetings: Meeting[]; go: Go; selectable?: boolean; selected?: Set<string>; setSelected?: (s: Set<string>) => void }) {
  const groups = useMemo(() => {
    const g: [string, Meeting[]][] = [];
    for (const m of meetings) {
      const d = dayLabel(m.createdAt);
      const last = g[g.length - 1];
      if (last && last[0] === d) last[1].push(m);
      else g.push([d, [m]]);
    }
    return g;
  }, [meetings]);
  return (
    <>
      {groups.map(([day, ms]) => (
        <div key={day}>
          <div className="list-day">{day}</div>
          <div className="list">
            {ms.map((m) => (
              <NoteRow
                key={m.id}
                m={m}
                go={go}
                selectable={selectable}
                selected={selected?.has(m.id)}
                onSelect={(v) => {
                  const n = new Set(selected);
                  if (v) n.add(m.id);
                  else n.delete(m.id);
                  setSelected?.(n);
                }}
              />
            ))}
          </div>
        </div>
      ))}
    </>
  );
}

function ComingUp({ go }: { go: Go }) {
  const [events, setEvents] = useState<CalEvent[] | null>(null);
  const [hasCal, setHasCal] = useState(false);
  const [showSolo, setShowSolo] = useState(true);
  useEffect(() => {
    const load = () => {
      api.events().then(setEvents);
      api.getSettings().then((s) => (setHasCal(s.calendars.some((c) => c.enabled)), setShowSolo(s.show_solo_events)));
    };
    load();
    return on("calendar-changed", load);
  }, []);
  if (!hasCal) {
    return (
      <div className="card connect-cal">
        {Icon.calendar}
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 500 }}>Connect your calendar</div>
          <div className="muted small">See what's coming up, get a nudge a minute before meetings, and have notes titled with the people in them.</div>
        </div>
        <button className="btn" onClick={() => go({ page: "settings", section: "calendar" })}>
          Connect
        </button>
      </div>
    );
  }
  const now = Date.now();
  // Granola hides events with no other people unless "Show events with no participants" is on.
  const upcoming = (events ?? []).filter((e) => new Date(e.end).getTime() > now && !e.allDay && !e.declined && (showSolo || e.attendees.length > 0 || !!e.link)).slice(0, 12);
  const days: [Date, CalEvent[]][] = [];
  for (const e of upcoming) {
    const d = new Date(e.start);
    const last = days[days.length - 1];
    if (last && last[0].toDateString() === d.toDateString()) last[1].push(e);
    else if (days.length < 3) days.push([d, [e]]);
  }
  const open = async (e: CalEvent) => {
    const m = await api.noteForEvent(e.id);
    go({ page: "meeting", id: m.id });
  };
  return (
    <div className="card coming">
      {days.length === 0 && <div className="empty-line" style={{ padding: "16px 18px" }}>Nothing else on your calendar for the next two weeks.</div>}
      {days.map(([d, evs]) => (
        <div key={d.toDateString()} className="coming-day">
          <div className="coming-date">
            <span className="num">{d.getDate()}</span>
            <span className="md">
              <b>
                {d.toLocaleDateString([], { month: "long" })}
                {d.toDateString() === new Date().toDateString() && <span className="today-dot" />}
              </b>
              {d.toLocaleDateString([], { weekday: "short" })}
            </span>
          </div>
          <div>
            {evs.map((e) => (
              <button key={e.id} className="ev" onClick={() => open(e)} title={e.link ? "Open the note for this meeting" : ""}>
                <span className="bar" />
                <span>
                  <div className="ev-title">{e.title}</div>
                  <div className="ev-time">
                    {timeOf(e.start)}–{timeOf(e.end)}
                    {e.attendees.length > 1 ? ` · ${e.attendees.length} people` : ""}
                  </div>
                </span>
              </button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

export default function Home({ go, quickNote, myNotes }: { go: Go; quickNote: () => void; myNotes?: boolean }) {
  const [meetings, setMeetings] = useState<Meeting[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  useEffect(() => {
    const load = () => api.meetings(null).then((ms) => setMeetings(myNotes ? ms.filter((m) => !m.folder) : ms));
    load();
    const a = on("meeting-updated", load);
    const b = on("meeting-deleted", load);
    return () => {
      a();
      b();
    };
  }, [myNotes]);

  const blank = async () => {
    const m = await api.createMeeting("");
    go({ page: "meeting", id: m.id });
  };
  const importFile = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const p = await open({ multiple: false, filters: [{ name: "Audio", extensions: ["mp3", "m4a", "wav", "flac", "ogg", "aac", "mp4", "webm"] }] });
    if (typeof p === "string") {
      try {
        const m = await api.importAudio(p);
        go({ page: "meeting", id: m.id });
      } catch (e) {
        toast(String(e));
      }
    }
  };

  return (
    <div className="page">
      <div className="topbar">
        <div className="spacer" />
        <button className="btn ghost" onClick={importFile} title="Turn a recording into notes">
          {Icon.upload} Import audio
        </button>
        <button className="btn ghost" onClick={blank}>
          {Icon.doc} Blank note
        </button>
        <button className="btn" onClick={quickNote} title="Start taking notes now (Ctrl+N)">
          {Icon.plus} Quick note
        </button>
      </div>
      <div className="page-body" style={{ paddingTop: 0 }}>
        {!myNotes && (
          <>
            <h1 className="h-serif" style={{ marginBottom: 14 }}>
              Coming up
            </h1>
            <ComingUp go={go} />
          </>
        )}
        {myNotes && <h1 className="h-serif">My notes</h1>}
        {selected.size > 0 && (
          <div className="banner" style={{ marginTop: 16 }}>
            <span className="t">{selected.size} selected</span>
            <button className="btn" onClick={() => go({ page: "chat", scope: "selected", scopeId: [...selected].join(",") })}>
              {Icon.chat} Ask about these
            </button>
            <button className="btn ghost" onClick={() => setSelected(new Set())}>
              Clear
            </button>
          </div>
        )}
        {meetings && meetings.length === 0 ? (
          <div className="card pad" style={{ marginTop: 22 }}>
            <div style={{ fontWeight: 500, marginBottom: 6 }}>Your notes will appear here</div>
            <ol className="muted" style={{ margin: 0, paddingLeft: 18, display: "flex", flexDirection: "column", gap: 4 }}>
              <li>Join a call in Zoom, Google Meet, Teams or Slack. Cereal starts taking notes (you'll see a small indicator), or click Quick note.</li>
              <li>Type a few rough notes as you go.</li>
              <li>When the call ends, the AI on this computer writes clean notes from your notes and the transcript.</li>
            </ol>
          </div>
        ) : (
          <NotesByDay meetings={meetings ?? []} go={go} selectable selected={selected} setSelected={setSelected} />
        )}
      </div>
      <AskBar scope="all" scopeId="" placeholder="Ask anything" onSubmit={(q) => go({ page: "chat", question: q })} />
    </div>
  );
}
