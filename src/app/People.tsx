import { useEffect, useState } from "react";
import { api, Company, Meeting, Person } from "../api";
import type { Go } from "./App";
import { ago, Icon, pastel } from "./ui";
import { NotesByDay } from "./Home";
import { AskCard, Thread, useThread } from "./ChatBar";

export function parseAttendee(a: string): { name: string; email: string } {
  const m = /^(.*?)<([^>]+)>/.exec(a);
  if (m) return { name: m[1].trim().replace(/^"|"$/g, ""), email: m[2].trim().toLowerCase() };
  if (a.includes("@")) return { name: "", email: a.trim().toLowerCase() };
  return { name: a.trim(), email: "" };
}

function Dot({ text }: { text: string }) {
  return (
    <span className="logo-dot" style={{ background: pastel(text), color: "#333" }}>
      {(text[0] ?? "?").toUpperCase()}
    </span>
  );
}

export function PeoplePage({ go }: { go: Go }) {
  const [list, setList] = useState<Person[] | null>(null);
  const [q, setQ] = useState("");
  useEffect(() => {
    api.people().then(setList);
  }, []);
  const shown = (list ?? []).filter((p) => !q || `${p.name} ${p.email} ${p.company}`.toLowerCase().includes(q.toLowerCase()));
  return (
    <div className="page">
      <div className="topbar" />
      <div className="page-body wide" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">People</h1>
        <p className="sub">Everyone you've met with, from your calendar and the names you've given speakers.</p>
        <input placeholder="Search people" value={q} onChange={(e) => setQ(e.target.value)} style={{ width: 280, marginBottom: 12 }} />
        <table className="table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Company</th>
              <th>Notes</th>
              <th>Last met</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((p) => (
              <tr key={p.key} className="click" onClick={() => go({ page: "person", key: p.key, name: p.name })}>
                <td>
                  <Dot text={p.name} />
                  {p.name} <span className="muted small">{p.email}</span>
                </td>
                <td className="muted">{p.company}</td>
                <td>{p.meetings}</td>
                <td className="muted">{ago(p.last)}</td>
              </tr>
            ))}
            {list && list.length === 0 && (
              <tr>
                <td colSpan={4} className="muted">
                  No people yet. Connect a calendar, add attendees to a note, or name the speakers in a transcript.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function CompaniesPage({ go }: { go: Go }) {
  const [list, setList] = useState<Company[] | null>(null);
  useEffect(() => {
    api.companies().then(setList);
  }, []);
  return (
    <div className="page">
      <div className="topbar" />
      <div className="page-body wide" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">Companies</h1>
        <p className="sub">Built from your attendees' work email addresses. Nothing is looked up online.</p>
        <table className="table">
          <thead>
            <tr>
              <th>Company</th>
              <th>People</th>
              <th>Notes</th>
              <th>Last note</th>
            </tr>
          </thead>
          <tbody>
            {(list ?? []).map((c) => (
              <tr key={c.domain} className="click" onClick={() => go({ page: "company", domain: c.domain, name: c.name })}>
                <td>
                  <Dot text={c.name} />
                  {c.name} <span className="muted small">{c.domain}</span>
                </td>
                <td className="muted">{c.people.slice(0, 3).join(", ")}{c.people.length > 3 ? ` +${c.people.length - 3}` : ""}</td>
                <td>{c.meetings}</td>
                <td className="muted">{ago(c.last)}</td>
              </tr>
            ))}
            {list && list.length === 0 && (
              <tr>
                <td colSpan={4} className="muted">
                  No companies yet. They appear when your notes have attendees with work email addresses.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Detail({ go, title, subtitle, scope, scopeId, meetings }: { go: Go; title: string; subtitle: string; scope: string; scopeId: string; meetings: Meeting[] | null }) {
  const thread = useThread(scope, scopeId);
  const started = thread.msgs.length > 0 || thread.streaming !== null;
  return (
    <div className="page">
      <div className="topbar">
        <div className="crumb">
          <button onClick={() => go({ page: scope === "person" ? "people" : "companies" })}>{Icon.back}</button>
        </div>
      </div>
      <div className="page-body" style={{ paddingTop: 0 }}>
        <div className="title-row">
          <Dot text={title} />
          <h1 className="h-serif" style={{ margin: 0 }}>
            {title}
          </h1>
        </div>
        <p className="sub" style={{ marginTop: 4 }}>
          {subtitle}
        </p>
        {started && (
          <div className="card pad" style={{ marginBottom: 12 }}>
            <Thread t={thread} onSource={(s) => go({ page: "meeting", id: s.meetingId })} />
          </div>
        )}
        <AskCard placeholder={`Ask about ${title}`} kind="multi" chips={3} compact onSubmit={(q, a) => thread.ask(q, a)} />
        <NotesByDay meetings={meetings ?? []} go={go} />
        {meetings && meetings.length === 0 && <div className="empty-line">No notes yet.</div>}
      </div>
    </div>
  );
}

export function PersonPage({ go, personKey, name }: { go: Go; personKey: string; name: string }) {
  const [ms, setMs] = useState<Meeting[] | null>(null);
  useEffect(() => {
    api.meetingsWith(personKey, null).then(setMs);
  }, [personKey]);
  return <Detail go={go} title={name} subtitle={personKey.includes("@") ? personKey : ""} scope="person" scopeId={personKey} meetings={ms} />;
}

export function CompanyPage({ go, domain, name }: { go: Go; domain: string; name: string }) {
  const [ms, setMs] = useState<Meeting[] | null>(null);
  useEffect(() => {
    api.meetingsWith(null, domain).then(setMs);
  }, [domain]);
  return <Detail go={go} title={name} subtitle={domain} scope="company" scopeId={domain} meetings={ms} />;
}
