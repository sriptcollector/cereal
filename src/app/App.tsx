import { useEffect, useRef, useState } from "react";
import { api, Hit, on, RecState } from "../api";
import { Icon, LogoImg, Modal, toast, useFolders, useSettings, useTheme, dayLabel } from "./ui";
import { fmtDur } from "./md";
import Onboarding from "./Onboarding";
import Home from "./Home";
import MeetingPage from "./Meeting";
import ChatPage from "./Chat";
import FolderPage from "./Folder";
import { PeoplePage, CompaniesPage, PersonPage, CompanyPage } from "./People";
import Templates from "./Templates";
import RecipesPage from "./Recipes";
import TrashPage from "./Trash";
import SettingsPage from "./Settings";
import Feedback from "./Feedback";

export type Route =
  | { page: "home" | "people" | "companies" | "templates" | "recipes" | "trash" | "mynotes" }
  | { page: "settings"; section?: string }
  | { page: "meeting"; id: string; panel?: string }
  | { page: "chat"; threadId?: string; question?: string; scope?: string; scopeId?: string }
  | { page: "folder"; id: string; tab?: string }
  | { page: "person"; key: string; name: string }
  | { page: "company"; domain: string; name: string };

export type Go = (r: Route) => void;

export default function App() {
  const [settings, , reload] = useSettings();
  useTheme(settings?.theme);
  const [route, setRouteRaw] = useState<Route>({ page: "home" });
  // Back (Ctrl+[) walks this history.
  const back = useRef<Route[]>([]);
  const setRoute = (r: Route) =>
    setRouteRaw((cur) => {
      if (JSON.stringify(cur) !== JSON.stringify(r)) back.current = [...back.current, cur].slice(-50);
      return r;
    });
  const [version, setVersion] = useState("");
  const [rec, setRec] = useState<RecState | null>(null);
  const [folders] = useFolders();
  const [collapsed, setCollapsed] = useState(false);
  const [palette, setPalette] = useState(false);
  const [feedback, setFeedback] = useState(false);
  const [trashCount, setTrashCount] = useState(0);

  useEffect(() => {
    api.appInfo().then((i) => setVersion(i.version));
    api.recState().then(setRec);
    const loadTrash = () => api.trashList().then((t) => setTrashCount(t.length));
    loadTrash();
    const u1 = on<string>("navigate", (p) => {
      if (p.startsWith("meeting:")) {
        const [, mid, panel] = p.split(":");
        setRoute({ page: "meeting", id: mid, panel });
      } else if (p.startsWith("folder:")) {
        const [, fid, tab] = p.split(":");
        setRoute({ page: "folder", id: fid, tab });
      } else if (p === "settings") setRoute({ page: "settings" });
      else if (p.startsWith("ask:")) setRoute({ page: "chat", question: p.slice(4) });
      else if (p === "chat" || p === "ask") setRoute({ page: "chat" });
      else setRoute({ page: p as "home" });
    });
    const u2 = on<RecState>("recording", setRec);
    const u3 = on("meeting-deleted", loadTrash);
    return () => {
      u1();
      u2();
      u3();
    };
  }, []);

  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;
      const key = e.key.toLowerCase();
      if (key === "k" || key === "/") {
        e.preventDefault();
        setPalette(true);
      } else if (key === ",") {
        e.preventDefault();
        setRoute({ page: "settings" });
      } else if (key === "[") {
        e.preventDefault();
        const prev = back.current.pop();
        if (prev) setRouteRaw(prev);
      } else if (key === "j") {
        e.preventDefault();
        const box = document.querySelector<HTMLTextAreaElement>(".askbar textarea, .askcard textarea");
        if (box) box.focus();
        else setRoute({ page: "chat" });
      } else if (key === "n") {
        e.preventDefault();
        quickNote();
      } else if (key === "\\") {
        e.preventDefault();
        setCollapsed((c) => !c);
      }
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  });

  if (!settings) return <div className="boot" />;
  if (!settings.onboarded) return <Onboarding onDone={reload} />;

  const go: Go = (r) => setRoute(r);
  const is = (p: string, extra?: (r: Route) => boolean) => route.page === p && (!extra || extra(route));
  const quickNote = async () => {
    try {
      const m = await api.recordStart(null);
      go({ page: "meeting", id: m.id });
    } catch (e) {
      toast(String(e));
    }
  };
  const who = settings.name.trim() || "My notes";

  return (
    <div className={`app ${collapsed ? "collapsed" : ""}`}>
      <aside className="side">
        <div className="side-top">
          <div className="side-brand">
            <LogoImg alt="" />
            Cereal
          </div>
          <div className="spacer" />
          <button className="icon-btn" title="Hide sidebar (Ctrl+\)" onClick={() => setCollapsed(true)}>
            {Icon.sidebar}
          </button>
        </div>
        {rec?.meetingId && (
          <button className="rec-chip" onClick={() => go({ page: "meeting", id: rec.meetingId! })}>
            <span className={`rec-dot ${rec.levels?.paused ? "paused" : ""}`} />
            <span>{rec.levels?.paused ? "Paused" : rec.app ? `Taking notes · ${rec.app}` : "Taking notes"}</span>
            <span className="time">{fmtDur(rec.levels?.secs ?? 0)}</span>
          </button>
        )}
        <button className="side-search" onClick={() => setPalette(true)}>
          {Icon.search}
          <span>Search</span>
          <kbd>Ctrl K</kbd>
        </button>
        <div className="side-scroll">
          <nav className="nav">
            <button className={is("home") ? "on" : ""} onClick={() => go({ page: "home" })}>
              {Icon.home}
              <span className="t">Home</span>
            </button>
            <button className={is("chat") ? "on" : ""} onClick={() => go({ page: "chat" })}>
              {Icon.chat}
              <span className="t">Chat</span>
            </button>
          </nav>
          <div className="side-label">
            <span>Spaces</span>
            <button
              className="icon-btn"
              title="New folder"
              onClick={async () => {
                const f = await api.saveFolder({ name: "New folder", icon: "📁" });
                go({ page: "folder", id: f.id });
              }}
            >
              {Icon.plus}
            </button>
          </div>
          <nav className="nav">
            <button className={is("mynotes") ? "on" : ""} onClick={() => go({ page: "mynotes" })}>
              {Icon.lock}
              <span className="t">My notes</span>
            </button>
            {/* Starred folders first, subfolders under their folder. */}
            {[...folders.filter((f) => !f.parentId)]
              .sort((a, b) => Number(b.favorite) - Number(a.favorite))
              .flatMap((f) => [f, ...folders.filter((c) => c.parentId === f.id)])
              .map((f) => (
                <button key={f.id} className={`${f.parentId ? "sub" : ""} ${is("folder", (r) => (r as { id: string }).id === f.id) ? "on" : ""}`} onClick={() => go({ page: "folder", id: f.id })}>
                  <span className="emoji">{f.icon || "📁"}</span>
                  <span className="t">{f.name}</span>
                  {f.favorite && <span className="star">★</span>}
                  {f.count > 0 && <span className="count">{f.count}</span>}
                </button>
              ))}
          </nav>
          <div className="side-label">
            <span>More</span>
          </div>
          <nav className="nav">
            <button className={is("people") || is("person") ? "on" : ""} onClick={() => go({ page: "people" })}>
              {Icon.people}
              <span className="t">People</span>
            </button>
            <button className={is("companies") || is("company") ? "on" : ""} onClick={() => go({ page: "companies" })}>
              {Icon.building}
              <span className="t">Companies</span>
            </button>
            <button className={is("templates") ? "on" : ""} onClick={() => go({ page: "templates" })}>
              {Icon.layout}
              <span className="t">Templates</span>
            </button>
            <button className={is("recipes") ? "on" : ""} onClick={() => go({ page: "recipes" })}>
              {Icon.slash}
              <span className="t">Recipes</span>
            </button>
            {trashCount > 0 && (
              <button className={is("trash") ? "on" : ""} onClick={() => go({ page: "trash" })}>
                {Icon.trash}
                <span className="t">Trash</span>
                <span className="count">{trashCount}</span>
              </button>
            )}
          </nav>
        </div>
        <div className="side-bottom">
          <nav className="nav">
            <button onClick={() => setFeedback(true)}>
              {Icon.feedback}
              <span className="t">Send feedback</span>
            </button>
          </nav>
          <button className="side-who" onClick={() => go({ page: "settings" })} title="Settings">
            <span className="av">{who.charAt(0).toUpperCase()}</span>
            <span className="name">{settings.name.trim() ? `${settings.name.trim()}'s notes` : "Settings"}</span>
            <span className="ver">v{version}</span>
            {Icon.gear}
          </button>
        </div>
      </aside>
      <main className="content">
        {collapsed && (
          <button className="icon-btn" style={{ position: "absolute", left: 8, top: 8, zIndex: 30 }} title="Show sidebar" onClick={() => setCollapsed(false)}>
            {Icon.sidebar}
          </button>
        )}
        {route.page === "home" && <Home go={go} quickNote={quickNote} />}
        {route.page === "mynotes" && <Home go={go} quickNote={quickNote} myNotes />}
        {route.page === "meeting" && <MeetingPage key={route.id + (route.panel ?? "")} id={route.id} panel={route.panel} go={go} />}
        {route.page === "chat" && <ChatPage key={(route.threadId ?? "new") + (route.scopeId ?? "")} go={go} threadId={route.threadId} question={route.question} scope={route.scope} scopeId={route.scopeId} />}
        {route.page === "folder" && <FolderPage key={route.id + (route.tab ?? "")} id={route.id} tab={route.tab} go={go} />}
        {route.page === "people" && <PeoplePage go={go} />}
        {route.page === "person" && <PersonPage key={route.key} go={go} personKey={route.key} name={route.name} />}
        {route.page === "companies" && <CompaniesPage go={go} />}
        {route.page === "company" && <CompanyPage key={route.domain} go={go} domain={route.domain} name={route.name} />}
        {route.page === "templates" && <Templates />}
        {route.page === "recipes" && <RecipesPage />}
        {route.page === "trash" && <TrashPage go={go} />}
        {route.page === "settings" && <SettingsPage go={go} section={route.section} />}
      </main>
      {palette && <Palette go={go} onClose={() => setPalette(false)} />}
      {feedback && <Feedback onClose={() => setFeedback(false)} />}
    </div>
  );
}

/** Ctrl+K: search every note, transcript and title. */
function Palette({ go, onClose }: { go: Go; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [sel, setSel] = useState(0);
  useEffect(() => {
    if (!q.trim()) {
      setHits([]);
      return;
    }
    const t = setTimeout(() => api.search(q).then((h) => (setHits(h), setSel(0))).catch(() => setHits([])), 100);
    return () => clearTimeout(t);
  }, [q]);
  const open = (h: Hit) => {
    onClose();
    go({ page: "meeting", id: h.meetingId });
  };
  return (
    <Modal onClose={onClose} className="palette">
      <input
        autoFocus
        placeholder="Search notes, transcripts and people"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") (e.preventDefault(), setSel((s) => Math.min(s + 1, hits.length - 1)));
          if (e.key === "ArrowUp") (e.preventDefault(), setSel((s) => Math.max(s - 1, 0)));
          if (e.key === "Enter" && hits[sel]) open(hits[sel]);
          if (e.key === "Enter" && !hits.length && q.trim()) {
            onClose();
            go({ page: "chat", question: q });
          }
        }}
      />
      <div className="results">
        {q && hits.length === 0 && (
          <button className="res on" onClick={() => (onClose(), go({ page: "chat", question: q }))}>
            <div>Ask “{q}”</div>
            <div className="snip">No matching notes. Press Enter to ask your notes instead.</div>
          </button>
        )}
        {hits.map((h, i) => (
          <button key={h.meetingId} className={`res ${i === sel ? "on" : ""}`} onMouseEnter={() => setSel(i)} onClick={() => open(h)}>
            <div>
              {h.title || "Untitled meeting"} <span className="muted small">· {dayLabel(h.createdAt)}</span>
            </div>
            <div className="snip">
              {h.snippet.split(/(\[[^\]]+\])/).map((p, j) => (p.startsWith("[") && p.endsWith("]") ? <mark key={j}>{p.slice(1, -1)}</mark> : <span key={j}>{p}</span>))}
            </div>
          </button>
        ))}
        {!q && <div className="empty-line" style={{ padding: "10px 10px" }}>Type to search. Ctrl+J asks your notes; Ctrl+N starts a quick note.</div>}
      </div>
    </Modal>
  );
}
