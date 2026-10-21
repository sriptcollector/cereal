import { useEffect, useState } from "react";
import { api, on, RecState, Settings } from "../api";
import { fmtDur } from "./md";
import { Icon, useTheme } from "./ui";

/** Always-visible indicator while notes are being taken: open, pause, stop. */
export default function Pill() {
  const [rec, setRec] = useState<RecState | null>(null);
  const [s, setS] = useState<Settings | null>(null);
  useTheme(s?.theme);
  useEffect(() => {
    api.getSettings().then(setS);
    api.recState().then(setRec);
    return on<RecState>("recording", setRec);
  }, []);
  const l = rec?.levels;
  const v = Math.max(l?.mic ?? 0, l?.system ?? 0);
  const paused = !!l?.paused;
  return (
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
    </div>
  );
}
