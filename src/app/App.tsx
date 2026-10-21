import { useEffect, useState } from "react";
import { api, on, RecState } from "../api";
import { Icon, Logo, useSettings } from "./ui";
import { fmtDur } from "./md";
import Onboarding from "./Onboarding";
import Home from "./Home";
import MeetingPage from "./Meeting";
import AskPage from "./Ask";
import Templates from "./Templates";
import SettingsPage from "./Settings";

export type Route = { page: "home" | "ask" | "templates" | "settings" } | { page: "meeting"; id: string; panel?: string } | { page: "folder"; folder: string };

export default function App() {
  const [settings, , reload] = useSettings();
  const [route, setRoute] = useState<Route>({ page: "home" });
  const [version, setVersion] = useState("");
  const [rec, setRec] = useState<RecState | null>(null);
  const [folders, setFolders] = useState<string[]>([]);

  useEffect(() => {
    api.appInfo().then((i) => setVersion(i.version));
    api.recState().then(setRec);
    const loadFolders = () => api.folders().then(setFolders);
    loadFolders();
    const u1 = on<string>("navigate", (p) => {
      // "meeting:<id>" or "meeting:<id>:transcript"
      if (p.startsWith("meeting:")) {
        const [, mid, panel] = p.split(":");
        setRoute({ page: "meeting", id: mid, panel });
      }
      else setRoute({ page: p as "home" });
    });
    const u2 = on<RecState>("recording", setRec);
    const u3 = on("meeting-updated", loadFolders);
    return () => {
      u1();
      u2();
      u3();
    };
  }, []);

  // Ctrl/Cmd+J opens chat (as in Granola); Ctrl/Cmd+N starts a new note.
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "j") {
        e.preventDefault();
        if (route.page !== "meeting") setRoute({ page: "ask" });
        else document.querySelector<HTMLTextAreaElement>(".chatbar textarea")?.focus();
      }
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setRoute({ page: "home" });
        setTimeout(() => document.querySelector<HTMLInputElement>(".search input")?.focus(), 50);
      }
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [route]);

  if (!settings) return <div className="boot" />;
  if (!settings.onboarded) return <Onboarding onDone={reload} />;

  const go = (r: Route) => setRoute(r);
  const active = (p: string) => (route.page === p ? "active" : "");

  return (
    <div className="app">
      <aside className="side">
        <div className="brand">
          <Logo />
          <div>
            <div className="brand-name">Notes</div>
            <div className="brand-by">by Toolbay</div>
          </div>
        </div>
        {rec?.meetingId && (
          <button className="rec-chip" onClick={() => go({ page: "meeting", id: rec.meetingId! })}>
            <span className={`rec-dot ${rec.levels?.paused ? "paused" : ""}`} />
            <span>{rec.levels?.paused ? "Paused" : "Taking notes"}</span>
            <span className="mono">{fmtDur(rec.levels?.secs ?? 0)}</span>
          </button>
        )}
        <nav>
          <button className={active("home")} onClick={() => go({ page: "home" })}>
            {Icon.home}
            <span>Home</span>
          </button>
          <button className={active("ask")} onClick={() => go({ page: "ask" })}>
            {Icon.chat}
            <span>Ask your notes</span>
          </button>
          <button className={active("templates")} onClick={() => go({ page: "templates" })}>
            {Icon.layout}
            <span>Templates</span>
          </button>
        </nav>
        {folders.length > 0 && (
          <>
            <div className="side-label">Folders</div>
            <nav>
              {folders.map((f) => (
                <button key={f} className={route.page === "folder" && route.folder === f ? "active" : ""} onClick={() => go({ page: "folder", folder: f })}>
                  {Icon.folder}
                  <span>{f}</span>
                </button>
              ))}
            </nav>
          </>
        )}
        <div className="side-spacer" />
        <nav>
          <button className={active("settings")} onClick={() => go({ page: "settings" })}>
            {Icon.gear}
            <span>Settings</span>
          </button>
        </nav>
        <div className="side-foot">
          <span className="free-pill">Free · Local · Open source</span>
          <span className="ver">v{version}</span>
        </div>
      </aside>
      <main className="content">
        {route.page === "home" && <Home go={go} />}
        {route.page === "folder" && <Home go={go} folder={route.folder} />}
        {route.page === "meeting" && <MeetingPage key={route.id + (route.panel ?? "")} id={route.id} panel={route.panel} go={go} />}
        {route.page === "ask" && <AskPage go={go} />}
        {route.page === "templates" && <Templates />}
        {route.page === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}
