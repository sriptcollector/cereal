import { useEffect, useState } from "react";
import { api, on, RecState, Segment, Settings } from "../api";
import { fmtDur } from "./md";
import { Icon, toast, useTheme } from "./ui";

/** Always-visible indicator while notes are being taken: open, pause, stop, screenshot
 *  into the note, and live captions. */
export default function Pill() {
  const [rec, setRec] = useState<RecState | null>(null);
  const [s, setS] = useState<Settings | null>(null);
  const [cc, setCc] = useState(false);
  const [lines, setLines] = useState<Segment[]>([]);
  useTheme(s?.theme);
  useEffect(() => {
    api.getSettings().then(setS);
    api.recState().then(setRec);
    const a = on<RecState>("recording", setRec);
    const b = on<{ meetingId: string; segment: Segment }>("live-segment", (x) => setLines((l) => [...l, x.segment].slice(-4)));
    return () => {
      a();
      b();
    };
  }, []);
  const toggleCc = () => {
    const v = !cc;
    setCc(v);
    api.pillWide(v);
  };
  const l = rec?.levels;
  const v = Math.max(l?.mic ?? 0, l?.system ?? 0);
  const paused = !!l?.paused;
  return (
    <div className={`fpill-wrap ${cc ? "wide" : ""}`}>
      {cc && (
        <div className="fcaptions">
          {lines.length === 0 && <div className="muted">Captions appear as people talk…</div>}
          {lines.map((x, i) => (
            <div key={i} className="cap">
              <b>{x.speaker}</b> {x.text}
            </div>
          ))}
        </div>
      )}
      <div className="fpill">
        <button className="open" onClick={() => api.pillClick()} title="Open the note">
          <span className={`rec-dot ${paused ? "paused" : ""}`} />
          <span className={`bars ${paused ? "off" : ""}`}>
            {[0.45, 0.8, 1, 0.65].map((k, i) => (
              <i key={i} style={{ transform: `scaleY(${paused ? 0.3 : Math.max(0.25, Math.min(1, v * 3 * k + 0.2))})` }} />
            ))}
          </span>
          <span className="time">{fmtDur(l?.secs ?? 0)}</span>
        </button>
        <button className="ctl" title={paused ? "Resume" : "Pause"} onClick={() => api.recordPause(!paused)}>
          {paused ? Icon.play : Icon.pause}
        </button>
        <button className="ctl" title="Stop and write notes" onClick={() => api.recordStop()}>
          {Icon.stop}
        </button>
        <button className="ctl" title="Add a screenshot to the note" onClick={() => api.screenshot().then(() => toast("Screenshot added"), (e) => toast(String(e)))}>
          {Icon.camera}
        </button>
        <button className={`ctl ${cc ? "on" : ""}`} title={cc ? "Hide live captions" : "Live captions"} onClick={toggleCc}>
          {Icon.captions}
        </button>
      </div>
    </div>
  );
}
