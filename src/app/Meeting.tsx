import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, Meeting, on, RecState, Segment, Settings } from "../api";
import type { Route } from "./App";
import { copy, dayLabel, Icon, Menu, timeOf, toast, useSettings, useTemplates } from "./ui";
import { fmtDur, fmtMinutes, Markdown, toggleCheck } from "./md";
import ChatBar, { Thread, useChat } from "./ChatBar";

function useDebounced<T>(fn: (v: T) => void, ms: number) {
  const t = useRef<number | undefined>(undefined);
  const f = useRef(fn);
  f.current = fn;
  return useCallback(
    (v: T) => {
      window.clearTimeout(t.current);
      t.current = window.setTimeout(() => f.current(v), ms);
    },
    [ms],
  );
}

function speakerName(m: Meeting | null, s: string) {
  try {
    const map = JSON.parse(m?.speakerNames || "{}");
    return map[s] || s;
  } catch {
    return s;
  }
}

export function notesMarkdown(m: Meeting, segs: Segment[], withTranscript: boolean) {
  const title = m.title || "Meeting notes";
  let md = `# ${title}\n\n_${dayLabel(m.createdAt)}, ${timeOf(m.createdAt)}${m.durationS ? ` · ${fmtMinutes(m.durationS)}` : ""}${m.app ? ` · ${m.app}` : ""}_\n\n`;
  md += (m.enhanced || m.myNotes || "").trim() + "\n";
  if (withTranscript && segs.length) {
    md += `\n## Transcript\n\n`;
    for (const s of segs) md += `**${speakerName(m, s.speaker)}** (${fmtDur(s.start)}): ${s.text}\n\n`;
  }
  return md;
}

export default function MeetingPage({ id, go, panel }: { id: string; go: (r: Route) => void; panel?: string }) {
  const [m, setM] = useState<Meeting | null>(null);
  const [segs, setSegs] = useState<Segment[]>([]);
  const [rec, setRec] = useState<RecState | null>(null);
  const [settings] = useSettings();
  const [templates] = useTemplates();
  const [view, setView] = useState<"ai" | "mine">("mine");
  const [editAi, setEditAi] = useState(false);
  const [showTx, setShowTx] = useState(panel === "transcript");
  const [chatOpen, setChatOpen] = useState(false);
  const [streamNotes, setStreamNotes] = useState("");
  const [consentOpen, setConsentOpen] = useState(false);
  const [myNotes, setMyNotes] = useState("");
  const [title, setTitle] = useState("");
  const [aiDraft, setAiDraft] = useState("");
  const [folderEdit, setFolderEdit] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const chat = useChat(id);
  const loaded = useRef(false);

  const load = useCallback(async () => {
    try {
      const r = await api.meeting(id);
      setM(r.meeting);
      setSegs(r.segments);
      if (!loaded.current) {
        loaded.current = true;
        setMyNotes(r.meeting.myNotes);
        setTitle(r.meeting.title);
        setAiDraft(r.meeting.enhanced);
        setView(r.meeting.enhanced ? "ai" : "mine");
      }
      return r.meeting;
    } catch {
      go({ page: "home" });
    }
  }, [id, go]);

  useEffect(() => {
    load();
    api.recState().then(setRec);
    const u1 = on<Meeting>("meeting-updated", (x) => {
      if (x.id !== id) return;
      setM((prev) => {
        if (prev && !prev.enhanced && x.enhanced) {
          setView("ai");
          setAiDraft(x.enhanced);
          setStreamNotes("");
        } else if (prev && x.enhanced !== prev.enhanced) {
          setAiDraft(x.enhanced);
          setStreamNotes("");
        }
        if (prev && !prev.title && x.title) setTitle(x.title);
        return x;
      });
      if (x.status !== "recording" && x.status !== "paused") api.meeting(id).then((r) => setSegs(r.segments));
    });
    const u2 = on<{ meetingId: string; segment: Segment }>("live-segment", (e) => {
      if (e.meetingId === id) setSegs((s) => [...s, e.segment].sort((a, b) => a.start - b.start));
    });
    const u3 = on<RecState>("recording", setRec);
    const u4 = on<{ meetingId: string; text: string }>("notes-token", (e) => {
      if (e.meetingId === id) setStreamNotes((s) => s + e.text);
    });
    return () => {
      u1();
      u2();
      u3();
      u4();
    };
  }, [id, load]);

  const recordingThis = rec?.meetingId === id;
  const recordingOther = !!rec?.meetingId && !recordingThis;
  const paused = !!rec?.levels?.paused;

  // Consent reminder each time recording starts on this note.
  const wasRecording = useRef(false);
  useEffect(() => {
    if (recordingThis && !wasRecording.current && settings?.consent_reminder) setConsentOpen(true);
    wasRecording.current = recordingThis;
  }, [recordingThis, settings?.consent_reminder]);

  const saveNotes = useDebounced<string>((v) => api.update(id, "myNotes", v), 400);
  const saveTitle = useDebounced<string>((v) => api.update(id, "title", v), 400);
  const saveAi = useDebounced<string>((v) => api.update(id, "enhanced", v), 500);

  const tplName = useMemo(() => templates.find((t) => t.id === (m?.templateId || "auto"))?.name ?? "Auto", [templates, m?.templateId]);
  const speakers = useMemo(() => Array.from(new Set(segs.map((s) => s.speaker))), [segs]);

  if (!m || !settings) return <div className="page" />;

  const processing = m.status === "processing";
  const canWrite = !recordingThis && !processing && (segs.length > 0 || myNotes.trim().length > 0);

  const start = async () => {
    try {
      await api.recordStart(id);
    } catch (e) {
      toast(String(e));
    }
  };
  const stop = async () => {
    await api.recordStop();
    setStreamNotes("");
  };
  const regenerate = async (tid?: string) => {
    setStreamNotes("");
    setEditAi(false);
    setView("ai");
    await api.regenerate(id, tid ?? m.templateId);
  };
  const print = () => {
    const prev = document.title;
    document.title = m.title || "Meeting notes";
    window.print();
    document.title = prev;
  };
  const exportMd = async () => {
    const p = await api.exportMarkdown(m.title || "Meeting notes", notesMarkdown(m, segs, true));
    toast(`Saved to ${p}`);
  };

  const aiText = streamNotes ? streamNotes.replace(/^\s*Title:.*\n?/i, "") : m.enhanced;

  return (
    <div className={`meeting ${showTx ? "with-tx" : ""}`}>
      <div className="meeting-main">
        <div className="meeting-top">
          <button className="icon-btn" onClick={() => go({ page: "home" })} aria-label="Back">
            {Icon.back}
          </button>
          <div className="top-spacer" />
          {m.enhanced || processing ? (
            <div className="seg-toggle">
              <button className={view === "mine" ? "on" : ""} onClick={() => setView("mine")} title="Your own notes">
                My notes
              </button>
              <button className={view === "ai" ? "on" : ""} onClick={() => setView("ai")} title="Notes written by the AI">
                {Icon.sparkle} AI notes
              </button>
            </div>
          ) : null}
          <Menu
            label={
              <>
                {Icon.layout} {tplName}
              </>
            }
          >
            {(close) => (
              <>
                <div className="menu-label">Write notes with a template</div>
                {templates.map((t) => (
                  <button
                    key={t.id}
                    className={t.id === m.templateId ? "on" : ""}
                    onClick={() => {
                      close();
                      if (canWrite) regenerate(t.id);
                      else api.update(id, "templateId", t.id).then(load);
                    }}
                  >
                    <span>{t.name}</span>
                    <small>{t.description}</small>
                  </button>
                ))}
              </>
            )}
          </Menu>
          <Menu
            label={
              <>
                {Icon.share} Share
              </>
            }
          >
            {(close) => (
              <>
                <button onClick={() => (close(), copy(notesMarkdown(m, segs, false), "Notes copied as Markdown"))}>Copy notes</button>
                <button onClick={() => (close(), copy(segs.map((s) => `${speakerName(m, s.speaker)}: ${s.text}`).join("\n"), "Transcript copied"))} disabled={!segs.length}>
                  Copy transcript
                </button>
                <button onClick={() => (close(), exportMd())}>Save as Markdown file</button>
                <button onClick={() => (close(), print())}>Save as PDF…</button>
                <div className="menu-sep" />
                <button onClick={() => (close(), copy(settings.consent_message, "Message copied: paste it in the meeting chat"))}>Copy “let others know” message</button>
              </>
            )}
          </Menu>
          <Menu label={Icon.more}>
            {(close) => (
              <>
                <button onClick={() => (close(), setFolderEdit(m.folder))}>{m.folder ? `Folder: ${m.folder}` : "Add to folder…"}</button>
                {m.hasAudio && (
                  <button onClick={() => (close(), api.reprocess(id))} disabled={recordingThis || processing}>
                    Redo transcript and notes
                  </button>
                )}
                {m.hasAudio && <button onClick={() => (close(), api.revealAudio(id))}>Show recording files</button>}
                <div className="menu-sep" />
                <button className="danger" onClick={() => (close(), setConfirmDelete(true))}>
                  Delete note
                </button>
              </>
            )}
          </Menu>
          <button className={`btn ghost ${showTx ? "pressed" : ""}`} onClick={() => setShowTx(!showTx)} title="Transcript">
            {Icon.transcript} Transcript
          </button>
        </div>

        <div className="meeting-scroll">
          <div className="paper">
            <textarea
              className="title-input"
              rows={1}
              ref={(el) => {
                if (el) {
                  el.style.height = "auto";
                  el.style.height = el.scrollHeight + "px";
                }
              }}
              placeholder={recordingThis ? "New meeting" : "Untitled meeting"}
              value={title}
              onChange={(e) => {
                const v = e.target.value.replace(/[\r\n]+/g, " ");
                setTitle(v);
                saveTitle(v);
              }}
            />
            <div className="meta">
              <span>{dayLabel(m.createdAt)}</span>
              <span>{timeOf(m.createdAt)}</span>
              {m.durationS > 0 && <span>{fmtMinutes(m.durationS)}</span>}
              {m.app && <span>{m.app}</span>}
              {speakers.length > 0 && <span>{speakers.map((s) => speakerName(m, s)).join(", ")}</span>}
              {m.folder && <span className="chip">{m.folder}</span>}
            </div>

            {folderEdit !== null && (
              <div className="inline-form">
                <input autoFocus placeholder="Folder name, e.g. Clients" value={folderEdit} onChange={(e) => setFolderEdit(e.target.value)} onKeyDown={(e) => e.key === "Enter" && (api.update(id, "folder", folderEdit.trim()).then(load), setFolderEdit(null))} />
                <button className="btn primary" onClick={() => (api.update(id, "folder", folderEdit.trim()).then(load), setFolderEdit(null))}>
                  Save
                </button>
                <button className="btn ghost" onClick={() => setFolderEdit(null)}>
                  Cancel
                </button>
              </div>
            )}
            {confirmDelete && (
              <div className="banner bad">
                <span>Delete this note, its transcript and recording? This can't be undone.</span>
                <button className="btn danger" onClick={() => api.deleteMeeting(id).then(() => go({ page: "home" }))}>
                  Delete
                </button>
                <button className="btn ghost" onClick={() => setConfirmDelete(false)}>
                  Cancel
                </button>
              </div>
            )}
            {consentOpen && recordingThis && <Consent settings={settings} onClose={() => setConsentOpen(false)} />}
            {processing && (
              <div className="banner work">
                <span className="spinner" />
                <span>{m.step || "Working on your notes…"}</span>
              </div>
            )}
            {m.status === "failed" && m.error && (
              <div className="banner bad">
                <span>{m.error}</span>
                <button className="btn" onClick={() => regenerate()}>
                  Write notes
                </button>
              </div>
            )}

            {view === "mine" || (!m.enhanced && !streamNotes) ? (
              <textarea
                className="notepad"
                placeholder={
                  recordingThis
                    ? "Write rough notes here as the meeting goes. A few words are enough: the AI fills in the rest from the transcript when the call ends."
                    : "Write notes here. Click Start recording when the meeting begins."
                }
                value={myNotes}
                onChange={(e) => (setMyNotes(e.target.value), saveNotes(e.target.value))}
                autoFocus={!m.enhanced}
              />
            ) : editAi ? (
              <textarea className="notepad ai-edit" value={aiDraft} onChange={(e) => (setAiDraft(e.target.value), saveAi(e.target.value))} autoFocus />
            ) : (
              <div className="ai-notes" onDoubleClick={() => !processing && (setAiDraft(m.enhanced), setEditAi(true))}>
                <Markdown
                  md={aiText}
                  myNotes={myNotes}
                  onToggle={(line, done) => {
                    const next = toggleCheck(m.enhanced, line, done);
                    api.update(id, "enhanced", next);
                    setM({ ...m, enhanced: next });
                  }}
                />
                {processing && streamNotes && <span className="caret" />}
              </div>
            )}
            {view === "ai" && m.enhanced && !processing && (
              <div className="ai-tools">
                <span className="legend">
                  <i className="mine" /> From your notes <i className="ai" /> Added by AI
                </span>
                <button className="btn ghost small" onClick={() => (editAi ? setEditAi(false) : (setAiDraft(m.enhanced), setEditAi(true)))}>
                  {editAi ? (
                    <>
                      {Icon.check} Done
                    </>
                  ) : (
                    <>
                      {Icon.edit} Edit
                    </>
                  )}
                </button>
                <button className="btn ghost small" onClick={() => regenerate()} disabled={!canWrite}>
                  {Icon.retry} Rewrite
                </button>
              </div>
            )}
          </div>
        </div>

        {chatOpen && (
          <div className="chat-panel">
            <div className="chat-panel-head">
              <span>Ask about this meeting</span>
              <div>
                {chat.msgs.length > 0 && (
                  <button className="btn ghost small" onClick={chat.clear}>
                    Clear
                  </button>
                )}
                <button className="icon-btn" onClick={() => setChatOpen(false)} aria-label="Close">
                  {Icon.close}
                </button>
              </div>
            </div>
            <Thread chat={chat} />
          </div>
        )}

        <div className="dock">
          {recordingThis ? (
            <div className="recbar">
              <span className={`rec-dot ${paused ? "paused" : ""}`} />
              <span className="mono time">{fmtDur(rec?.levels?.secs ?? 0)}</span>
              <Meter label="You" v={rec?.levels?.mic ?? 0} off={paused} />
              <Meter label="Them" v={rec?.levels?.system ?? 0} off={paused || !settings.capture_system} />
              {rec?.levels?.system_error && <span className="warn-text" title={rec.levels.system_error}>Computer sound unavailable</span>}
              <div className="top-spacer" />
              <button className="btn ghost" onClick={() => api.recordPause(!paused)}>
                {paused ? Icon.play : Icon.pause} {paused ? "Resume" : "Pause"}
              </button>
              <button className="btn primary" onClick={stop}>
                {Icon.stop} Stop & write notes
              </button>
            </div>
          ) : (
            <div className="recbar idle">
              {processing ? (
                <span className="muted">{m.step || "Working…"}</span>
              ) : (
                <>
                  <button className="btn ghost" onClick={start} disabled={recordingOther} title={recordingOther ? "Another meeting is being recorded" : ""}>
                    {Icon.mic} {m.hasAudio ? "Resume recording" : "Start recording"}
                  </button>
                  {!m.enhanced && canWrite && (
                    <button className="btn primary" onClick={() => regenerate()}>
                      {Icon.sparkle} Write notes
                    </button>
                  )}
                </>
              )}
            </div>
          )}
          <ChatBar
            meetingId={id}
            live={recordingThis}
            placeholder={recordingThis ? "Ask anything about this meeting so far" : "Ask anything about this meeting"}
            onSubmit={(q) => {
              setChatOpen(true);
              chat.ask(q);
            }}
          />
        </div>
      </div>

      {showTx && <Transcript m={m} segs={segs} live={recordingThis} onClose={() => setShowTx(false)} reload={load} />}

      <div className="print-area">
        <h1>{m.title || "Meeting notes"}</h1>
        <p className="print-meta">
          {dayLabel(m.createdAt)}, {timeOf(m.createdAt)}
          {m.durationS ? ` · ${fmtMinutes(m.durationS)}` : ""}
          {m.app ? ` · ${m.app}` : ""}
        </p>
        <Markdown md={m.enhanced || m.myNotes} />
      </div>
    </div>
  );
}

function Meter({ label, v, off }: { label: string; v: number; off?: boolean }) {
  const bars = 5;
  return (
    <span className={`meter ${off ? "off" : ""}`} title={label}>
      <span className="meter-label">{label}</span>
      <span className="meter-bars">
        {Array.from({ length: bars }, (_, i) => (
          <i key={i} className={!off && v * bars * 1.4 > i ? "lit" : ""} />
        ))}
      </span>
    </span>
  );
}

function Consent({ settings, onClose }: { settings: Settings; onClose: () => void }) {
  return (
    <div className="banner consent">
      {Icon.shield}
      <div className="consent-text">
        <b>Let others know you're taking notes.</b> In some places everyone on a call has to agree to be recorded. Paste this in the meeting chat:
        <q>{settings.consent_message}</q>
      </div>
      <button className="btn" onClick={() => (copy(settings.consent_message, "Copied: paste it in the meeting chat"), onClose())}>
        {Icon.copy} Copy
      </button>
      <button className="icon-btn" onClick={onClose} aria-label="Dismiss">
        {Icon.close}
      </button>
    </div>
  );
}

function Transcript({ m, segs, live, onClose, reload }: { m: Meeting; segs: Segment[]; live: boolean; onClose: () => void; reload: () => void }) {
  const [q, setQ] = useState("");
  const [rename, setRename] = useState<{ speaker: string; name: string } | null>(null);
  const [edit, setEdit] = useState<{ id: number; text: string } | null>(null);
  const end = useRef<HTMLDivElement>(null);
  const shown = q ? segs.filter((s) => s.text.toLowerCase().includes(q.toLowerCase())) : segs;
  const speakers = Array.from(new Set(segs.map((s) => s.speaker)));
  // One bubble per speaker turn (consecutive lines, under ~320 characters).
  const groups: Segment[][] = [];
  for (const s of shown) {
    const g = groups[groups.length - 1];
    if (g && g[0].speaker === s.speaker && s.start - g[g.length - 1].end < 4 && g.reduce((n, x) => n + x.text.length, 0) < 320) g.push(s);
    else groups.push([s]);
  }
  useEffect(() => {
    if (live && !q) end.current?.scrollIntoView({ block: "end" });
  }, [segs.length, live, q]);
  return (
    <aside className="tx">
      <div className="tx-head">
        <b>Transcript</b>
        <div className="top-spacer" />
        <button className="icon-btn" title="Copy transcript" onClick={() => copy(segs.map((s) => `${speakerName(m, s.speaker)}: ${s.text}`).join("\n"), "Transcript copied")}>
          {Icon.copy}
        </button>
        <button className="icon-btn" onClick={onClose} aria-label="Close transcript">
          {Icon.close}
        </button>
      </div>
      <div className="tx-tools">
        <input placeholder="Search transcript" value={q} onChange={(e) => setQ(e.target.value)} />
        {speakers.length > 0 && (
          <div className="speakers">
            {speakers.map((s) =>
              rename?.speaker === s ? (
                <input
                  key={s}
                  autoFocus
                  className="speaker-input"
                  value={rename.name}
                  placeholder={s}
                  onChange={(e) => setRename({ speaker: s, name: e.target.value })}
                  onBlur={() => (api.renameSpeaker(m.id, s, rename.name).then(reload), setRename(null))}
                  onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                />
              ) : (
                <button key={s} className="speaker" onClick={() => setRename({ speaker: s, name: speakerName(m, s) === s ? "" : speakerName(m, s) })} title="Click to name this speaker">
                  {speakerName(m, s)} {Icon.edit}
                </button>
              ),
            )}
          </div>
        )}
      </div>
      <div className="tx-list">
        {shown.length === 0 && <div className="empty-line">{live ? "Listening… text appears here a few seconds after people speak." : "No transcript yet."}</div>}
        {groups.map((g) => {
          const first = g[0];
          const text = g.map((x) => x.text).join(" ");
          return (
            <div key={first.id} className={`tx-line ${first.source === "mic" ? "me" : "them"}`}>
              <div className="tx-who">
                {speakerName(m, first.speaker)} <span className="mono">{fmtDur(first.start)}</span>
              </div>
              {edit?.id === first.id ? (
                <textarea
                  autoFocus
                  className="tx-edit"
                  value={edit.text}
                  onChange={(e) => setEdit({ id: first.id, text: e.target.value })}
                  onBlur={async () => {
                    await api.segmentUpdate(m.id, first.id, edit.text);
                    for (const x of g.slice(1)) await api.segmentDelete(m.id, x.id);
                    setEdit(null);
                    reload();
                  }}
                />
              ) : (
                <div className="tx-text">{text}</div>
              )}
              {!live && (
                <div className="tx-actions">
                  <button className="icon-btn tiny" title="Copy" onClick={() => copy(text)}>
                    {Icon.copy}
                  </button>
                  <button className="icon-btn tiny" title="Edit" onClick={() => setEdit({ id: first.id, text })}>
                    {Icon.edit}
                  </button>
                  <button
                    className="icon-btn tiny"
                    title="Delete these lines"
                    onClick={async () => {
                      for (const x of g) await api.segmentDelete(m.id, x.id);
                      reload();
                    }}
                  >
                    {Icon.trash}
                  </button>
                </div>
              )}
            </div>
          );
        })}
        <div ref={end} />
      </div>
    </aside>
  );
}
