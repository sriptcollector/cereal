import { useEffect, useMemo, useState } from "react";
import { api, Hit, Meeting, on } from "../api";
import type { Route } from "./App";
import { dayLabel, Icon, timeOf, toast } from "./ui";
import { fmtMinutes } from "./md";
import ChatBar from "./ChatBar";

function statusChip(m: Meeting) {
  switch (m.status) {
    case "recording":
      return <span className="chip live">Taking notes</span>;
    case "paused":
      return <span className="chip">Paused</span>;
    case "processing":
      return <span className="chip work">Writing notes…</span>;
    case "failed":
      return <span className="chip bad">Needs attention</span>;
    default:
      return null;
  }
}

function preview(m: Meeting) {
  const src = m.enhanced || m.myNotes;
  const line = src
    .split("\n")
    .filter((l) => !/^\s*#/.test(l))
    .map((l) => l.replace(/^\s*(#+|[-*•]|\d+[.)])\s*(\[.\]\s*)?/, "").replace(/\*\*/g, "").trim())
    .find((l) => l.length > 3);
  return line ?? "";
}

export default function Home({ go, folder }: { go: (r: Route) => void; folder?: string }) {
  const [meetings, setMeetings] = useState<Meeting[] | null>(null);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hit[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const load = () => api.meetings(folder ?? null).then(setMeetings);
    load();
    return on("meeting-updated", load);
  }, [folder]);

  useEffect(() => {
    if (!q.trim()) {
      setHits(null);
      return;
    }
    const t = setTimeout(() => api.search(q).then(setHits).catch(() => setHits([])), 120);
    return () => clearTimeout(t);
  }, [q]);

  const groups = useMemo(() => {
    const g: [string, Meeting[]][] = [];
    for (const m of meetings ?? []) {
      const d = dayLabel(m.createdAt);
      const last = g[g.length - 1];
      if (last && last[0] === d) last[1].push(m);
      else g.push([d, [m]]);
    }
    return g;
  }, [meetings]);

  const start = async () => {
    setBusy(true);
    try {
      const m = await api.recordStart(null);
      if (folder) await api.update(m.id, "folder", folder);
      go({ page: "meeting", id: m.id });
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
  };
  const blank = async () => {
    const m = await api.createMeeting("");
    if (folder) await api.update(m.id, "folder", folder);
    go({ page: "meeting", id: m.id });
  };

  return (
    <div className="home">
      <div className="home-scroll">
        <header className="home-head">
          <div>
            <h1>{folder ?? "Your notes"}</h1>
            <p className="sub">{folder ? "Notes in this folder" : "Cereal offers to take notes when a call starts in Zoom, Google Meet, Teams or Slack. No bot joins your call."}</p>
          </div>
          <div className="page-actions">
            <button className="btn ghost" onClick={blank}>
              {Icon.doc} Blank note
            </button>
            <button className="btn primary" onClick={start} disabled={busy}>
              {Icon.mic} New note
            </button>
          </div>
        </header>

        <div className="search">
          {Icon.search}
          <input placeholder="Search all your meetings" value={q} onChange={(e) => setQ(e.target.value)} />
          {q && (
            <button className="icon-btn" onClick={() => setQ("")} aria-label="Clear">
              {Icon.close}
            </button>
          )}
          <kbd>Ctrl K</kbd>
        </div>

        {hits ? (
          <div className="list">
            {hits.length === 0 && <div className="empty-line">Nothing found for “{q}”.</div>}
            {hits.map((h) => (
              <button key={h.meetingId} className="item" onClick={() => go({ page: "meeting", id: h.meetingId })}>
                <div className="item-main">
                  <div className="item-title">{h.title || "Untitled meeting"}</div>
                  <div className="item-snip">
                    {h.snippet.split(/(\[[^\]]+\])/).map((p, i) => (p.startsWith("[") && p.endsWith("]") ? <mark key={i}>{p.slice(1, -1)}</mark> : <span key={i}>{p}</span>))}
                  </div>
                </div>
                <div className="item-meta">{dayLabel(h.createdAt)}</div>
              </button>
            ))}
          </div>
        ) : meetings && meetings.length === 0 ? (
          <div className="welcome">
            <div className="welcome-title">Your first meeting notes start here</div>
            <ol>
              <li>Join a call in Zoom, Google Meet, Teams or Slack. Cereal offers to take notes, or click <b>New note</b>.</li>
              <li>Type rough notes as you go. A few words are enough.</li>
              <li>When the call ends, the AI on your computer turns your notes and the transcript into clean notes.</li>
            </ol>
          </div>
        ) : (
          groups.map(([day, ms]) => (
            <div key={day} className="day">
              <div className="day-label">{day}</div>
              <div className="list">
                {ms.map((m) => (
                  <button key={m.id} className="item" onClick={() => go({ page: "meeting", id: m.id })}>
                    <div className="item-main">
                      <div className="item-title">
                        {m.title || (m.status === "recording" ? "New meeting" : "Untitled meeting")} {statusChip(m)}
                      </div>
                      <div className="item-snip">{preview(m) || (m.app ? `Recorded from ${m.app}` : "")}</div>
                    </div>
                    <div className="item-meta">
                      <span>{timeOf(m.createdAt)}</span>
                      {m.durationS > 0 && <span>{fmtMinutes(m.durationS)}</span>}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
      <ChatBar meetingId={null} placeholder="Ask anything across your meetings" onOpen={() => go({ page: "ask" })} />
    </div>
  );
}
