import { useEffect, useState } from "react";
import { api, CalEvent, Call, on, Settings } from "../api";
import { copy, Icon, LOGO, useTheme } from "./ui";

type Mode = { kind: "ask"; call: Call } | { kind: "auto"; app: string; meetingId: string; consent: boolean } | { kind: "event"; event: CalEvent };

/** The small card in the top-right corner. */
export default function Popup() {
  const [mode, setMode] = useState<Mode | null>(null);
  const [busy, setBusy] = useState(false);
  const [s, setS] = useState<Settings | null>(null);
  useTheme(s?.theme);
  useEffect(() => {
    api.getSettings().then(setS);
    api.detected().then((c) => c && setMode((m) => m ?? { kind: "ask", call: c }));
    const a = on<Call>("detected", (c) => (setMode({ kind: "ask", call: c }), setBusy(false)));
    const b = on<{ app: string; meetingId: string; consent: boolean }>("auto-started", (x) => (setMode({ kind: "auto", ...x }), setBusy(false), api.getSettings().then(setS)));
    const c = on<{ event: CalEvent }>("event-starting", (x) => (setMode({ kind: "event", event: x.event }), setBusy(false)));
    return () => {
      a();
      b();
      c();
    };
  }, []);
  // An "auto-started" card without the consent note fades after 8 seconds.
  useEffect(() => {
    if (mode?.kind === "auto" && !mode.consent) {
      const t = setTimeout(() => api.popupAction(false).catch(() => {}), 8000);
      return () => clearTimeout(t);
    }
  }, [mode]);

  if (!mode) return <div className="popup adhoc" />;

  if (mode.kind === "auto") {
    return (
      <div className="popup adhoc">
        <div className="popup-head">
          <img src={LOGO} alt="" />
          <div style={{ minWidth: 0 }}>
            <div className="popup-title">
              <span className="rec-dot" style={{ display: "inline-block", marginRight: 6 }} />
              Taking notes
            </div>
            <div className="popup-sub">{mode.app}: recording started</div>
          </div>
        </div>
        {mode.consent && <div className="popup-note">Some places require everyone's permission to record. Let the others know:</div>}
        <div className="popup-actions">
          <button className="btn ghost" onClick={() => s && copy(s.consent_message, "Copied")}>
            {Icon.copy} Let others know
          </button>
          <button className="btn" onClick={() => api.recordStop().then(() => api.popupAction(false))}>
            Stop
          </button>
          <button className="btn dark" onClick={() => api.openHub(`meeting:${mode.meetingId}`).then(() => api.popupAction(false))}>
            Open
          </button>
        </div>
      </div>
    );
  }

  if (mode.kind === "event") {
    const e = mode.event;
    return (
      <div className="popup">
        <div className="popup-head">
          <img src={LOGO} alt="" />
          <div style={{ minWidth: 0 }}>
            <div className="popup-title">{e.title}</div>
            <div className="popup-sub">
              Starting now{e.attendees.length ? ` · ${e.attendees.length} people` : ""}
            </div>
          </div>
        </div>
        <div className="popup-actions">
          <button className="btn ghost" onClick={() => api.popupAction(false)}>
            Not now
          </button>
          <button
            className="btn dark"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              api.joinEvent(e.id, true).catch(() => setBusy(false));
            }}
          >
            {e.link ? "Join & take notes" : "Take notes"}
          </button>
        </div>
      </div>
    );
  }

  const app = mode.call.app;
  const label = app.includes("huddle") ? "Huddle detected" : app.startsWith("A browser") ? "Call detected" : "Meeting detected";
  return (
    <div className="popup adhoc">
      <div className="popup-head">
        <img src={LOGO} alt="" />
        <div style={{ minWidth: 0 }}>
          <div className="popup-title">{label}</div>
          <div className="popup-sub">{app} is using your microphone</div>
        </div>
      </div>
      <div className="popup-actions">
        <button className="btn ghost" title="Never offer for this app" onClick={() => api.setAppRule(app, "ignore").then(() => api.popupAction(false))}>
          Ignore app
        </button>
        <button className="btn ghost" onClick={() => api.popupAction(false)}>
          Not now
        </button>
        <button
          className="btn dark"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            api.popupAction(true).catch(() => setBusy(false));
          }}
        >
          Take notes
        </button>
      </div>
    </div>
  );
}
