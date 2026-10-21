import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, Meeting, on, RecState, Segment, Settings, Source } from "../api";
import type { Go } from "./App";
import { copy, dayLabel, Icon, Menu, Modal, timeOf, toast, useFolders, useSettings, useTemplates } from "./ui";
import { fmtDur, fmtMinutes, Markdown } from "./md";
import AskBar, { Thread, useThread } from "./ChatBar";
import NoteEditor from "./Editor";
import { TemplatePicker } from "./Templates";

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

function namesMap(m: Meeting | null): Record<string, string> {
  try {
    return JSON.parse(m?.speakerNames || "{}");
  } catch {
    return {};
  }
}

export function notesMarkdown(m: Meeting, segs: Segment[], withTranscript: boolean) {
  const names = namesMap(m);
  const title = m.title || "Meeting notes";
  let md = `# ${title}\n\n_${dayLabel(m.createdAt)}, ${timeOf(m.createdAt)}${m.durationS ? ` · ${fmtMinutes(m.durationS)}` : ""}${m.attendees ? ` · ${m.attendees}` : ""}_\n\n`;
  md += (m.enhanced || m.myNotes || "").trim() + "\n";
  if (withTranscript && segs.length) {
    md += `\n## Transcript\n\n`;
    for (const s of segs) md += `**${names[s.speaker] || s.speaker}** (${fmtDur(s.start)}): ${s.text}\n\n`;
  }
  return md;
}

/** Finds text in the page, wrapping around (Chromium's window.find). */
function findNext(q: string, backwards: boolean) {
  if (!q) return;
  const w = window as unknown as { find?: (s: string, cs: boolean, back: boolean, wrap: boolean) => boolean };
  if (w.find && !w.find(q, false, backwards, true)) toast("Not found");
}

export default function MeetingPage({ id, go, panel }: { id: string; go: Go; panel?: string }) {
  const [m, setM] = useState<Meeting | null>(null);
  const [find, setFind] = useState<string | null>(null);
  const [verIdx, setVerIdx] = useState(-1);
  const keys = useRef<((e: KeyboardEvent) => void) | null>(null);
  const [segs, setSegs] = useState<Segment[]>([]);
  const [rec, setRec] = useState<RecState | null>(null);
  const [settings] = useSettings();
  const [templates] = useTemplates();
  const [folders] = useFolders();
  const [view, setView] = useState<"ai" | "mine">("mine");
  const [showTx, setShowTx] = useState(panel === "transcript");
  const [chatOpen, setChatOpen] = useState(false);
  const [streamNotes, setStreamNotes] = useState("");
  const [consentOpen, setConsentOpen] = useState(false);
  const [myNotes, setMyNotes] = useState("");
  const [title, setTitle] = useState("");
  const [people, setPeople] = useState<string[]>([]);
  const [attEdit, setAttEdit] = useState(false);
  const [tplPick, setTplPick] = useState(false);
  const [brief, setBrief] = useState<string | null>(null);
  const thread = useThread("meeting", id);
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
        setView(r.meeting.enhanced ? "ai" : "mine");
      }
    } catch {
      go({ page: "home" });
    }
  }, [id, go]);

  useEffect(() => {
    load();
    api.recState().then(setRec);
    api.people().then((p) => setPeople(p.map((x) => x.name).filter(Boolean)));
    api.threads("meeting", id).then((t) => t[0] && thread.setThreadId(t[0].id));
    const u1 = on<Meeting>("meeting-updated", (x) => {
      if (x.id !== id) return;
      setM((prev) => {
        if (prev && !prev.enhanced && x.enhanced) setView("ai");
        if (prev && x.enhanced !== prev.enhanced) setStreamNotes("");
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
    const u5 = on<string>("meeting-deleted", (d) => d === id && go({ page: "home" }));
    return () => {
      u1();
      u2();
      u3();
      u4();
      u5();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, load]);

  const recordingThis = rec?.meetingId === id;
  const recordingOther = !!rec?.meetingId && !recordingThis;
  const paused = !!rec?.levels?.paused;

  const wasRecording = useRef(false);
  useEffect(() => {
    if (recordingThis && !wasRecording.current && settings?.consent_reminder) setConsentOpen(true);
    wasRecording.current = recordingThis;
  }, [recordingThis, settings?.consent_reminder]);

  const saveNotes = useDebounced<string>((v) => api.update(id, "myNotes", v), 400);
  const saveTitle = useDebounced<string>((v) => api.update(id, "title", v), 400);
  const saveAi = useDebounced<string>((v) => api.update(id, "enhanced", v), 600);

  const names = useMemo(() => namesMap(m), [m]);
  const speakerName = (s: string) => names[s] || s;
  const tplName = templates.find((t) => t.id === (m?.templateId || "auto"))?.name ?? "Auto";
  const attendees = (m?.attendees ?? "").split(/[,;\n]/).map((a) => a.trim()).filter(Boolean);
  const speakers = useMemo(() => Array.from(new Set(segs.map((s) => s.speaker))), [segs]);
  const folder = folders.find((f) => f.id === m?.folder);

  useEffect(() => {
    const k = (e: KeyboardEvent) => keys.current?.(e);
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, []);
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
  const regenerate = async (tid?: string) => {
    // Like Granola: rewriting replaces edits made to the AI notes.
    if (m.enhanced && !tid && !window.confirm("Rewrite the notes? Changes you made to the AI notes will be replaced.")) return;
    setStreamNotes("");
    setView("ai");
    await api.regenerate(id, tid ?? m.templateId);
  };
  const pickTemplate = (tid: string) => {
    setTplPick(false);
    if (canWrite) regenerate(tid);
    else api.update(id, "templateId", tid).then(load);
  };
  const print = () => {
    const prev = document.title;
    document.title = m.title || "Meeting notes";
    window.print();
    document.title = prev;
  };
  const exportMd = async () => toast(`Saved to ${await api.exportMarkdown(m.title || "Meeting notes", notesMarkdown(m, segs, true))}`);
  const exportDocx = async () => toast(`Saved to ${await api.exportDocx(m.title || "Meeting notes", notesMarkdown(m, segs, false))}`);
  const emailDraft = () => {
    const to = attendees.map((a) => /<([^>]+)>/.exec(a)?.[1] ?? (a.includes("@") ? a : "")).filter(Boolean).join(",");
    const body = notesMarkdown(m, segs, false).replace(/^# .*\n+/, "");
    window.open(`mailto:${encodeURIComponent(to)}?subject=${encodeURIComponent("Notes: " + (m.title || "our meeting"))}&body=${encodeURIComponent(body.slice(0, 1800))}`);
  };
  const openSource = (s: Source) => s.meetingId !== id && go({ page: "meeting", id: s.meetingId });
  const loadBrief = async () => {
    if (!m.eventId) return;
    setBrief("");
    try {
      setBrief((await api.briefForEvent(m.eventId)) || "No past notes with these people yet.");
    } catch (e) {
      setBrief(String(e));
    }
  };

  // Ctrl+F finds in this note (Ctrl+G / Enter next, Shift for previous); Ctrl+Shift+G rewrites.
  keys.current = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      const key = e.key.toLowerCase();
      if (key === "f") {
        e.preventDefault();
        setFind((f) => (f === null ? "" : f));
        setTimeout(() => document.querySelector<HTMLInputElement>(".findbar input")?.select(), 0);
      } else if (key === "g" && e.shiftKey) {
        e.preventDefault();
        if (canWrite) regenerate();
      } else if (key === "g" && find) {
        e.preventDefault();
        findNext(find, false);
      }
  };

  const aiText = streamNotes ? streamNotes.replace(/^\s*Title:.*\n?/i, "") : m.enhanced;
  const showAi = view === "ai" && (m.enhanced || streamNotes);
  const versions: { templateId: string; name: string; notes: string }[] = (() => {
    try {
      return JSON.parse(m.versions || "[]");
    } catch {
      return [];
    }
  })();

  return (
    <div className={`meeting ${showTx ? "with-tx" : ""}`}>
      {find !== null && (
        <div className="findbar">
          <input
            autoFocus
            placeholder="Find in note"
            value={find}
            onChange={(e) => setFind(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") findNext(find, e.shiftKey);
              if (e.key === "Escape") setFind(null);
            }}
          />
          <button className="icon-btn" title="Previous (Shift+Enter)" onClick={() => findNext(find, true)}>
            ↑
          </button>
          <button className="icon-btn" title="Next (Enter)" onClick={() => findNext(find, false)}>
            ↓
          </button>
          <button className="icon-btn" title="Close (Esc)" onClick={() => setFind(null)}>
            ✕
          </button>
        </div>
      )}
      <div className="meeting-main">
        <div className="topbar">
          <div className="crumb">
            <button onClick={() => go({ page: "home" })} title="Back">
              {Icon.back}
            </button>
            <button onClick={() => (folder ? go({ page: "folder", id: folder.id }) : go({ page: "home" }))} title={folder ? folder.name : "Home"}>
              {folder ? <span style={{ fontSize: 12 }}>{folder.icon || "📁"}</span> : Icon.home}
            </button>
          </div>
          <div className="spacer" />
          {(m.enhanced || processing) && (
            <Menu
              label={
                <>
                  {view === "ai" ? Icon.sparkle : Icon.doc} {view === "ai" ? "Enhanced notes" : "My notes"} {Icon.chevron}
                </>
              }
            >
              {(close) => (
                <>
                  <button className={view === "ai" ? "on" : ""} onClick={() => (setView("ai"), close())}>
                    {Icon.sparkle} Enhanced notes
                  </button>
                  <button className={view === "mine" ? "on" : ""} onClick={() => (setView("mine"), close())}>
                    {Icon.doc} My notes
                  </button>
                </>
              )}
            </Menu>
          )}
          <button className="btn ghost" onClick={() => setTplPick(true)} title="Template">
            {Icon.layout} {tplName}
          </button>
          {m.enhanced && !processing && (
            <button className="icon-btn" onClick={() => regenerate()} title="Rewrite notes with this template" disabled={!canWrite}>
              {Icon.retry}
            </button>
          )}
          <Menu btnClass="btn dark" label="Share">
            {(close) => (
              <>
                <button onClick={() => (close(), copy(notesMarkdown(m, segs, false), "Notes copied as Markdown"))}>{Icon.copy} Copy notes</button>
                <button onClick={() => (close(), copy(segs.map((s) => `${speakerName(s.speaker)}: ${s.text}`).join("\n"), "Transcript copied"))} disabled={!segs.length}>
                  {Icon.transcript} Copy transcript
                </button>
                <button onClick={() => (close(), emailDraft())}>{Icon.share} Email the notes</button>
                <div className="menu-sep" />
                <button onClick={() => (close(), exportMd())}>{Icon.doc} Save as Markdown</button>
                <button onClick={() => (close(), exportDocx())}>{Icon.doc} Save as Word</button>
                <button onClick={() => (close(), print())}>{Icon.doc} Save as PDF…</button>
                <div className="menu-sep" />
                <button onClick={() => (close(), copy(settings.consent_message, "Paste it in the meeting chat"))}>{Icon.shield} Copy “let others know” message</button>
              </>
            )}
          </Menu>
          <button className="icon-btn dark" title="Copy notes" onClick={() => copy(notesMarkdown(m, segs, false), "Notes copied")}>
            {Icon.link}
          </button>
          <Menu label={Icon.more} btnClass="icon-btn">
            {(close) => (
              <>
                {m.hasAudio && (
                  <button onClick={() => (close(), api.reprocess(id))} disabled={recordingThis || processing}>
                    {Icon.retry} Redo transcript and notes
                  </button>
                )}
                {m.hasAudio && <button onClick={() => (close(), api.revealAudio(id))}>{Icon.folder} Show recording files</button>}
                <button onClick={() => (close(), api.dock())}>{Icon.sidebar} Move beside the call</button>
                <div className="menu-sep" />
                <button className="danger" onClick={() => (close(), api.trash(id).then(() => (toast("Moved to trash"), go({ page: "home" }))))}>
                  {Icon.trash} Move to trash
                </button>
              </>
            )}
          </Menu>
          <button className={`icon-btn ${showTx ? "dark" : ""}`} onClick={() => setShowTx(!showTx)} title="Transcript">
            {Icon.transcript}
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
              <span className="pill">
                {Icon.calendar} {dayLabel(m.createdAt)} {timeOf(m.createdAt)}
                {m.durationS > 0 ? ` · ${fmtMinutes(m.durationS)}` : ""}
              </span>
              <button className="pill" onClick={() => setAttEdit(true)}>
                {Icon.people} {attendees.length ? `${attendees.length} attendee${attendees.length > 1 ? "s" : ""}` : speakers.length > 1 ? `${speakers.length} speakers` : "Add attendees"}
              </button>
              <Menu
                btnClass="pill"
                align="left"
                label={
                  <>
                    {Icon.folder} {folder ? `${folder.icon} ${folder.name}` : "Select folder"} {Icon.chevron}
                  </>
                }
              >
                {(close) => (
                  <>
                    {folders.map((f) => (
                      <button key={f.id} className={f.id === m.folder ? "on" : ""} onClick={() => (close(), api.update(id, "folder", f.id).then(load))}>
                        <span>{f.icon || "📁"}</span> {f.name}
                      </button>
                    ))}
                    {m.folder && <button onClick={() => (close(), api.update(id, "folder", "").then(load))}>{Icon.lock} My notes (no folder)</button>}
                    <div className="menu-sep" />
                    <button
                      onClick={async () => {
                        close();
                        const f = await api.saveFolder({ name: "New folder", icon: "📁" });
                        await api.update(id, "folder", f.id);
                        load();
                      }}
                    >
                      {Icon.plus} New folder
                    </button>
                  </>
                )}
              </Menu>
              {m.app && <span className="pill">{m.app}</span>}
            </div>

            {m.eventId && !m.enhanced && segs.length === 0 && (
              <div className="brief">
                <div className="brief-title">Before you join</div>
                {brief === null ? (
                  <button className="btn" onClick={loadBrief}>
                    {Icon.sparkle} Prep me from past notes with these people
                  </button>
                ) : brief === "" ? (
                  <span className="thinking">Reading your past notes…</span>
                ) : (
                  <Markdown md={brief} />
                )}
              </div>
            )}
            {consentOpen && recordingThis && <Consent settings={settings} onClose={() => setConsentOpen(false)} />}
            {processing && (
              <div className="banner work">
                <span className="spinner" />
                <span className="t">{m.step || "Writing your notes…"}</span>
              </div>
            )}
            {m.status === "failed" && m.error && (
              <div className="banner bad">
                <span className="t">{m.error}</span>
                <button className="btn" onClick={() => regenerate()}>
                  Write notes
                </button>
              </div>
            )}

            {showAi && versions.length > 0 && !processing && (
              <div className="ver-tabs">
                <button className={verIdx < 0 ? "on" : ""} onClick={() => setVerIdx(-1)}>
                  {tplName}
                </button>
                {versions.map((v, i) => (
                  <button key={i} className={verIdx === i ? "on" : ""} onClick={() => setVerIdx(i)}>
                    {v.name}
                  </button>
                ))}
              </div>
            )}
            {showAi && verIdx >= 0 && versions[verIdx] ? (
              <div className="ver-view">
                <Markdown md={versions[verIdx].notes} />
                <button
                  className="btn"
                  onClick={() =>
                    api.useVersion(id, verIdx).then(() => {
                      setVerIdx(-1);
                      load();
                    })
                  }
                >
                  Use these notes
                </button>
              </div>
            ) : showAi ? (
              <NoteEditor
                key="ai"
                ai
                markdown={aiText}
                myNotes={myNotes}
                editable={!processing}
                onChange={(md) => saveAi(md)}
                placeholder=""
                people={people}
                segments={segs}
                speakerName={speakerName}
              />
            ) : (
              <NoteEditor
                key="mine"
                markdown={myNotes}
                onChange={(md) => (setMyNotes(md), saveNotes(md))}
                placeholder={recordingThis ? "Write notes… A few words are enough. Type / for a template." : "Write notes, or press the record button when the meeting starts."}
                people={people}
                autoFocus={!m.enhanced}
                onTemplateSlash={() => setTplPick(true)}
              />
            )}
            {showAi && myNotes.trim() && !processing && (
              <div className="ai-legend">
                <span>
                  <i className="mine" />
                  From your notes
                </span>
                <span>
                  <i className="ai" />
                  Added by Cereal
                </span>
              </div>
            )}
          </div>
        </div>

        {chatOpen && (
          <div className="chat-panel">
            <div className="chat-panel-head">
              <span>Ask about this meeting</span>
              <div className="spacer" />
              {thread.msgs.length > 0 && (
                <button className="btn ghost" onClick={() => thread.reset()}>
                  New chat
                </button>
              )}
              <button className="icon-btn" onClick={() => setChatOpen(false)} aria-label="Close">
                {Icon.close}
              </button>
            </div>
            <Thread t={thread} onSource={openSource} />
          </div>
        )}

        <div className="dock">
          <div className="dock-row">
            {recordingThis ? (
              <div className="recctl">
                <Menu
                  up
                  align="left"
                  btnClass=""
                  label={
                    <>
                      <Bars v={Math.max(rec?.levels?.mic ?? 0, rec?.levels?.system ?? 0)} off={paused} />
                      {Icon.chevron}
                    </>
                  }
                >
                  {(close) => (
                    <>
                      <div className="menu-label">
                        You {Math.round((rec?.levels?.mic ?? 0) * 100)}% · Them {Math.round((rec?.levels?.system ?? 0) * 100)}%
                      </div>
                      {rec?.levels?.mic_device && <div className="menu-label">Mic: {rec.levels.mic_device}</div>}
                      {rec?.levels?.system_error && <div className="menu-label" style={{ color: "var(--warn)" }}>{rec.levels.system_error}</div>}
                      <button onClick={() => (close(), api.recordPause(!paused))}>
                        {paused ? Icon.play : Icon.pause} {paused ? "Resume" : "Pause"}
                      </button>
                    </>
                  )}
                </Menu>
                <span className="time">{fmtDur(rec?.levels?.secs ?? 0)}</span>
                <button title="Stop and write notes" onClick={() => api.recordStop()}>
                  <span className="stop" />
                </button>
              </div>
            ) : processing ? (
              <div className="recctl">
                <span className="spinner" style={{ margin: "0 6px" }} />
                <span className="time">{m.step || "Writing notes…"}</span>
              </div>
            ) : (
              <div className="recctl">
                <button onClick={start} disabled={recordingOther} title={recordingOther ? "Another meeting is being recorded" : "Record this meeting"}>
                  <Bars v={0} off /> <span style={{ fontSize: 12, fontWeight: 500 }}>{m.hasAudio ? "Resume" : "Start"}</span>
                </button>
                {!m.enhanced && canWrite && (
                  <button onClick={() => regenerate()} title="Write notes now">
                    {Icon.sparkle} <span style={{ fontSize: 12, fontWeight: 500 }}>Enhance</span>
                  </button>
                )}
              </div>
            )}
          </div>
          <AskBar
            scope="meeting"
            scopeId={id}
            live={recordingThis}
            placeholder={recordingThis ? "Ask anything about this meeting so far" : "Ask anything"}
            onSubmit={(q) => {
              setChatOpen(true);
              thread.ask(q);
            }}
          />
        </div>
      </div>

      {showTx && <Transcript m={m} segs={segs} live={recordingThis} onClose={() => setShowTx(false)} reload={load} speakerName={speakerName} />}
      {attEdit && <AttendeesModal m={m} onClose={() => setAttEdit(false)} onSaved={load} />}
      {tplPick && (
        <Modal onClose={() => setTplPick(false)}>
          <h2>Templates</h2>
          <p className="muted" style={{ marginTop: 0 }}>
            {canWrite ? "Pick one and your notes are rewritten in that shape." : "Your notes will be written in this shape after the meeting."}
          </p>
          <TemplatePicker templates={templates} current={m.templateId} onPick={pickTemplate} />
        </Modal>
      )}

      <div className="print-area">
        <h1>{m.title || "Meeting notes"}</h1>
        <p>
          {dayLabel(m.createdAt)}, {timeOf(m.createdAt)}
          {m.durationS ? ` · ${fmtMinutes(m.durationS)}` : ""}
        </p>
        <Markdown md={m.enhanced || m.myNotes} />
      </div>
    </div>
  );
}

function Bars({ v, off }: { v: number; off?: boolean }) {
  const k = [0.45, 0.8, 1, 0.65];
  return (
    <span className={`bars ${off ? "off" : ""}`}>
      {k.map((x, i) => (
        <i key={i} style={{ transform: `scaleY(${off ? 0.35 : Math.max(0.25, Math.min(1, v * 3 * x + 0.2))})` }} />
      ))}
    </span>
  );
}

function Consent({ settings, onClose }: { settings: Settings; onClose: () => void }) {
  return (
    <div className="banner">
      {Icon.shield}
      <div className="t">
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

function AttendeesModal({ m, onClose, onSaved }: { m: Meeting; onClose: () => void; onSaved: () => void }) {
  const [v, setV] = useState(m.attendees.split(/[,;]/).map((a) => a.trim()).filter(Boolean).join("\n"));
  return (
    <Modal onClose={onClose}>
      <h2>Attendees</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        One per line, like <i>Dana Lee &lt;dana@acme.com&gt;</i>. They show up in People and Companies and help the notes.
      </p>
      <textarea style={{ width: "100%" }} rows={6} value={v} onChange={(e) => setV(e.target.value)} autoFocus />
      <div className="actions">
        <button className="btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button
          className="btn dark"
          onClick={async () => {
            await api.update(m.id, "attendees", v.split("\n").map((x) => x.trim()).filter(Boolean).join(", "));
            onSaved();
            onClose();
          }}
        >
          Save
        </button>
      </div>
    </Modal>
  );
}

function Transcript({ m, segs, live, onClose, reload, speakerName }: { m: Meeting; segs: Segment[]; live: boolean; onClose: () => void; reload: () => void; speakerName: (s: string) => string }) {
  const [q, setQ] = useState("");
  const [rename, setRename] = useState<{ speaker: string; name: string } | null>(null);
  const [edit, setEdit] = useState<{ id: number; text: string } | null>(null);
  const end = useRef<HTMLDivElement>(null);
  const shown = q ? segs.filter((s) => s.text.toLowerCase().includes(q.toLowerCase())) : segs;
  const speakers = Array.from(new Set(segs.map((s) => s.speaker)));
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
        <span>Transcript</span>
        <div className="spacer" />
        <button className="icon-btn" title="Copy transcript" onClick={() => copy(segs.map((s) => `${speakerName(s.speaker)}: ${s.text}`).join("\n"), "Transcript copied")}>
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
                  style={{ height: 26, width: 140, borderRadius: 999, fontSize: 12 }}
                  value={rename.name}
                  placeholder={s}
                  onChange={(e) => setRename({ speaker: s, name: e.target.value })}
                  onBlur={() => (api.renameSpeaker(m.id, s, rename.name).then(reload), setRename(null))}
                  onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                />
              ) : (
                <button key={s} className="pill" onClick={() => setRename({ speaker: s, name: speakerName(s) === s ? "" : speakerName(s) })} title="Name this speaker">
                  {speakerName(s)} {Icon.edit}
                </button>
              ),
            )}
          </div>
        )}
      </div>
      <div className="tx-list">
        {shown.length === 0 && <div className="empty-line">{live ? "Listening… text appears a few seconds after people speak." : "No transcript yet."}</div>}
        {groups.map((g) => {
          const first = g[0];
          const text = g.map((x) => x.text).join(" ");
          return (
            <div key={first.id} className={`tx-line ${first.source === "mic" ? "me" : "them"}`}>
              <div className="tx-who">
                {speakerName(first.speaker)} · {fmtDur(first.start)}
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
                  <button className="icon-btn" title="Copy" onClick={() => copy(text)}>
                    {Icon.copy}
                  </button>
                  <button className="icon-btn" title="Edit" onClick={() => setEdit({ id: first.id, text })}>
                    {Icon.edit}
                  </button>
                  <button
                    className="icon-btn"
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
