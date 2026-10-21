import { useEffect, useState } from "react";
import { api, CalEvent, Call, on, Settings } from "../api";
import { copy, Icon, LOGO, useTheme } from "./ui";

type Mode = { kind: "ask"; call: Call } | { kind: "auto"; app: string; meetingId: string; consent: boolean } | { kind: "event"; event: CalEvent } | { kind: "still" } | { kind: "corner" };

const hm = (iso: string) => new Date(iso).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });

/** The small card in the top-right corner. */
export default function Popup() {
  const [mode, setMode] = useState<Mode | null>(null);
  const [busy, setBusy] = useState(false);
  const [s, setS] = useState<Settings | null>(null);
  const [ev, setEv] = useState<CalEvent | null>(null);
  useTheme(s?.theme);
  useEffect(() => {
    api.getSettings().then(setS);
    api.detected().then((c) => c && setMode((m) => m ?? { kind: "ask", call: c }));
    const a = on<Call>("detected", (c) => (setMode({ kind: "ask", call: c }), setBusy(false)));
    const b = on<{ app: string; meetingId: string; consent: boolean }>("auto-started", (x) => (setMode({ kind: "auto", ...x }), setBusy(false), api.getSettings().then(setS)));
    const c = on<{ event: CalEvent }>("event-starting", (x) => (setMode({ kind: "event", event: x.event }), setBusy(false)));
    const d = on("still-there", () => (setMode({ kind: "still" }), setBusy(false)));
    const e2 = on("hot-corner", () => (setMode({ kind: "corner" }), setBusy(false)));
    return () => {
      a();
      b();
      c();
      d();
      e2();
    };
  }, []);
  // An "auto-started" card without the consent note fades after 8 seconds.
  useEffect(() => {
    if (mode?.kind === "auto" && !mode.consent) {
      const t = setTimeout(() => api.popupAction(false).catch(() => {}), 8000);
      return () => clearTimeout(t);
    }
  }, [mode]);

  // A detected call during a calendar meeting is titled with that meeting.
  useEffect(() => {
    if (mode?.kind === "ask") api.currentEvent().then(setEv).catch(() => setEv(null));
  }, [mode]);
  // A calendar card goes away 4 minutes after the meeting starts.
  useEffect(() => {
    if (mode?.kind !== "event") return;
    const ms = new Date(mode.event.start).getTime() + 4 * 60_000 - Date.now();
    const t = setTimeout(() => api.popupAction(false).catch(() => {}), Math.max(0, ms));
    return () => clearTimeout(t);
  }, [mode]);

  if (!mode) return <div className="popup adhoc" />;

  if (mode.kind === "corner") {
    return (
      <div className="popup adhoc">
        <div className="popup-head">
          <img src={LOGO} alt="" />
          <div style={{ minWidth: 0 }}>
            <div className="popup-title">New note</div>
            <div className="popup-sub">Start taking notes now?</div>
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
              api
                .recordStart(null)
                .then((m) => api.openHub(`meeting:${m.id}`))
                .finally(() => api.popupAction(false));
            }}
          >
            Start taking notes
          </button>
        </div>
      </div>
    );
  }

  if (mode.kind === "still") {
    return (
      <div className="popup adhoc">
        <div className="popup-head">
          <img src={LOGO} alt="" />
          <div style={{ minWidth: 0 }}>
            <div className="popup-title">Still there?</div>
            <div className="popup-sub">Nobody has spoken for a while. Cereal stops in a minute.</div>
          </div>
        </div>
        <div className="popup-actions">
          <button className="btn ghost" onClick={() => api.recordStop().then(() => api.popupAction(false))}>
            Stop
          </button>
          <button className="btn dark" onClick={() => api.stillHere()}>
            Keep going
          </button>
        </div>
      </div>
    );
  }

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
              {hm(e.start)}–{hm(e.end)}
              {e.attendees.length ? ` · ${e.attendees.length} ${e.attendees.length === 1 ? "person" : "people"}` : ""}
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
  const label = /huddle/i.test(app) ? "Huddle detected" : /^(A browser|FaceTime|WhatsApp|Signal|Telegram)/.test(app) ? "Call detected" : "Meeting detected";
  return (
    <div className="popup adhoc">
      <div className="popup-head">
        <img src={LOGO} alt="" />
        <div style={{ minWidth: 0 }}>
          <div className="popup-title">{ev ? ev.title : label}</div>
          <div className="popup-sub">{ev ? `${hm(ev.start)}–${hm(ev.end)} · ${app}` : `${app} is using your microphone`}</div>
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
