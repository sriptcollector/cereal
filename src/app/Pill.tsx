import { useEffect, useState } from "react";
import { api, on, RecState } from "../api";
import { fmtDur } from "./md";

/** Floating pill on the screen edge while notes are being taken. Click to go back. */
export default function Pill() {
  const [rec, setRec] = useState<RecState | null>(null);
  useEffect(() => {
    api.recState().then(setRec);
    return on<RecState>("recording", setRec);
  }, []);
  const l = rec?.levels;
  const v = Math.max(l?.mic ?? 0, l?.system ?? 0);
  return (
    <button className="pill" onClick={() => api.pillClick()} title="Cereal is taking notes. Click to open.">
      <span className={`rec-dot ${l?.paused ? "paused" : ""}`} />
      <span className="pill-bars">
        {[0.3, 0.7, 1, 0.6, 0.35].map((k, i) => (
          <i key={i} style={{ transform: `scaleY(${(4 + (l?.paused ? 0 : Math.min(1, v * 3) * 18 * k)) / 22})` }} />
        ))}
      </span>
      <span className="mono pill-time">{fmtDur(l?.secs ?? 0)}</span>
    </button>
  );
}
