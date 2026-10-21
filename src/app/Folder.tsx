import { useEffect, useState } from "react";
import { api, Company, FileItem, Folder, Meeting, on, Person } from "../api";
import type { Go } from "./App";
import { ago, Icon, Menu, Modal, pastel, toast } from "./ui";
import { NotesByDay } from "./Home";
import { AskCard, Thread, useThread } from "./ChatBar";
import { parseAttendee } from "./People";

const ICONS = ["📁", "🤝", "🧑‍💻", "📈", "🎯", "📣", "💡", "🧪", "🏗️", "🎨", "🗓️", "🧭", "💬", "🚀", "📚", "🏆", "✋", "👥", "📢", "🛠️"];

export default function FolderPage({ id, go, tab: initialTab }: { id: string; go: Go; tab?: string }) {
  const [f, setF] = useState<Folder | null>(null);
  const [tab, setTab] = useState<"notes" | "files" | "companies" | "people">((initialTab as "notes") || "notes");
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [files, setFiles] = useState<FileItem[]>([]);
  const [integrations, setIntegrations] = useState(false);
  const thread = useThread("folder", id);

  const load = () => {
    api.folders().then((fs) => setF(fs.find((x) => x.id === id) ?? null));
    api.meetings(id).then(setMeetings);
    api.files(id).then(setFiles);
  };
  useEffect(() => {
    load();
    const a = on("meeting-updated", load);
    const b = on("meeting-deleted", load);
    return () => {
      a();
      b();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  if (!f) return <div className="page" />;
  const save = (patch: Partial<Folder>) => {
    const next = { ...f, ...patch };
    setF(next);
    api.saveFolder(next);
  };
  const addFile = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const p = await open({ multiple: true, filters: [{ name: "Documents", extensions: ["txt", "md", "csv", "json", "html", "docx"] }] });
    const paths = Array.isArray(p) ? p : typeof p === "string" ? [p] : [];
    for (const path of paths) {
      try {
        await api.addFile(id, path);
      } catch (e) {
        toast(String(e));
      }
    }
    api.files(id).then(setFiles);
  };

  // People and companies in this folder's meetings.
  const people = new Map<string, Person>();
  const companies = new Map<string, Company>();
  const counted = new Set<string>();
  for (const m of meetings) {
    for (const a of m.attendees.split(/[,;\n]/).map((x) => x.trim()).filter(Boolean)) {
      const { name, email } = parseAttendee(a);
      const key = email || name.toLowerCase();
      const p = people.get(key) ?? { key, name: name || email.split("@")[0], email, company: "", meetings: 0, last: "" };
      p.meetings += 1;
      if (m.createdAt > p.last) p.last = m.createdAt;
      people.set(key, p);
      const dom = email.split("@")[1];
      if (dom && !/gmail|outlook|hotmail|yahoo|icloud|proton/.test(dom)) {
        const c = companies.get(dom) ?? { key: dom, name: dom.split(".").slice(-2, -1)[0].replace(/^\w/, (x) => x.toUpperCase()), domain: dom, people: [], meetings: 0, last: "" };
        if (!c.people.includes(p.name)) c.people.push(p.name);
        if (!counted.has(dom + m.id)) {
          counted.add(dom + m.id);
          c.meetings += 1;
        }
        if (m.createdAt > c.last) c.last = m.createdAt;
        companies.set(dom, c);
      }
    }
  }
  const started = thread.msgs.length > 0 || thread.streaming !== null;

  return (
    <div className="page">
      <div className="topbar">
        <div className="crumb">
          <button onClick={() => go({ page: "home" })}>{Icon.back}</button>
        </div>
        <div className="spacer" />
        <Menu label={Icon.more} btnClass="icon-btn">
          {(close) => (
            <button
              className="danger"
              onClick={async () => {
                close();
                await api.deleteFolder(id);
                go({ page: "home" });
              }}
            >
              {Icon.trash} Delete folder (notes stay)
            </button>
          )}
        </Menu>
      </div>
      <div className="page-body" style={{ paddingTop: 0 }}>
        <div className="title-row">
          <Menu btnClass="folder-icon" align="left" label={<span>{f.icon || "📁"}</span>}>
            {(close) => (
              <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 2, padding: 4 }}>
                {ICONS.map((ic) => (
                  <button key={ic} style={{ justifyContent: "center", fontSize: 16 }} onClick={() => (save({ icon: ic }), close())}>
                    {ic}
                  </button>
                ))}
              </div>
            )}
          </Menu>
          <input className="title-input" style={{ fontSize: 24 }} value={f.name} onChange={(e) => save({ name: e.target.value })} />
          <button className="btn" onClick={() => setIntegrations(true)}>
            {Icon.plug} Integrations
          </button>
        </div>
        <input className="desc-input" placeholder="Add a description" value={f.description} onChange={(e) => save({ description: e.target.value })} />

        <div style={{ marginTop: 20 }}>
          {started ? (
            <div className="card pad" style={{ marginBottom: 12 }}>
              <div style={{ display: "flex", marginBottom: 6 }}>
                <span className="muted small">Ask about {f.name}</span>
                <div className="spacer" />
                <button className="btn ghost" onClick={() => thread.reset()}>
                  Clear
                </button>
              </div>
              <Thread t={thread} onSource={(s) => go({ page: "meeting", id: s.meetingId })} />
            </div>
          ) : null}
          <AskCard placeholder="Ask about folder" kind="multi" chips={3} compact onSubmit={(q, a) => thread.ask(q, a)} />
        </div>

        <div className="tabs">
          {(["notes", "files", "companies", "people"] as const).map((t) => (
            <button key={t} className={`pill ${tab === t ? "on" : ""}`} onClick={() => setTab(t)}>
              {t === "notes" ? Icon.doc : t === "files" ? Icon.file : t === "companies" ? Icon.building : Icon.people}
              {t[0].toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>

        {tab === "notes" &&
          (meetings.length ? (
            <NotesByDay meetings={meetings} go={go} />
          ) : (
            <div className="empty-line">No notes here yet. Pick this folder from a note's “Select folder”, or set auto-filing in Integrations.</div>
          ))}
        {tab === "files" && (
          <>
            <div className="dropzone">
              Files give Cereal more context when you ask about this folder.
              <div style={{ marginTop: 10 }}>
                <button className="btn" onClick={addFile}>
                  {Icon.upload} Add files
                </button>
              </div>
            </div>
            <div className="list" style={{ marginTop: 10 }}>
              {files.map((x) => (
                <div key={x.id} className="item">
                  <span style={{ width: 28, display: "grid", placeItems: "center", color: "var(--muted)" }}>{Icon.file}</span>
                  <div className="item-main">
                    <div className="item-title">{x.name}</div>
                    <div className="item-sub">{Math.round(x.chars / 1000)}k characters</div>
                  </div>
                  <span className="item-meta">{ago(x.addedAt)}</span>
                  <button className="icon-btn" onClick={() => api.deleteFile(x.id).then(() => api.files(id).then(setFiles))}>
                    {Icon.trash}
                  </button>
                </div>
              ))}
            </div>
          </>
        )}
        {tab === "companies" && (
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
              {[...companies.values()].map((c) => (
                <tr key={c.domain} className="click" onClick={() => go({ page: "company", domain: c.domain, name: c.name })}>
                  <td>
                    <span className="logo-dot" style={{ background: pastel(c.name), color: "#333" }}>
                      {c.name[0]}
                    </span>
                    {c.name}
                  </td>
                  <td className="muted">{c.people.slice(0, 3).join(", ")}</td>
                  <td>{c.meetings}</td>
                  <td className="muted">{ago(c.last)}</td>
                </tr>
              ))}
              {companies.size === 0 && (
                <tr>
                  <td colSpan={4} className="muted">
                    Companies appear when notes have attendees with work email addresses.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
        {tab === "people" && (
          <table className="table">
            <thead>
              <tr>
                <th>Person</th>
                <th>Email</th>
                <th>Notes</th>
                <th>Last note</th>
              </tr>
            </thead>
            <tbody>
              {[...people.values()].map((p) => (
                <tr key={p.key} className="click" onClick={() => go({ page: "person", key: p.key, name: p.name })}>
                  <td>
                    <span className="logo-dot" style={{ background: pastel(p.name), color: "#333" }}>
                      {p.name[0]?.toUpperCase()}
                    </span>
                    {p.name}
                  </td>
                  <td className="muted">{p.email}</td>
                  <td>{p.meetings}</td>
                  <td className="muted">{ago(p.last)}</td>
                </tr>
              ))}
              {people.size === 0 && (
                <tr>
                  <td colSpan={4} className="muted">
                    People appear when notes have attendees (from your calendar, or added on the note).
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>
      {integrations && (
        <Modal onClose={() => setIntegrations(false)}>
          <h2>Integrations</h2>
          <p className="muted" style={{ marginTop: 0 }}>
            Automate this folder. Everything runs on this computer.
          </p>
          <label className="field">
            <span>File meetings here automatically when the title contains</span>
            <input value={f.autoMatch} placeholder="e.g. Weekly sync" onChange={(e) => save({ autoMatch: e.target.value })} />
          </label>
          <label className="field">
            <span>Post new notes to a Slack channel or any webhook (paste the URL)</span>
            <input value={f.webhook} placeholder="https://hooks.slack.com/services/…" onChange={(e) => save({ webhook: e.target.value })} />
          </label>
          <p className="muted small">
            Slack: create an Incoming Webhook for the channel and paste it here. Zapier, Make, n8n and others: use a “Catch hook” URL; Cereal sends the title, date, attendees and notes as JSON.
          </p>
          <div className="actions">
            <button className="btn dark" onClick={() => setIntegrations(false)}>
              Done
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
