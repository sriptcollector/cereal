import { useEffect, useState } from "react";
import { api, Call, on } from "../api";
import { Logo } from "./ui";

/** The small "Call detected" card in the top-right corner. */
export default function Popup() {
  const [call, setCall] = useState<Call | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.detected().then(setCall);
    return on<Call>("detected", (c) => {
      setCall(c);
      setBusy(false);
    });
  }, []);
  const label = !call ? "Call detected" : call.app.includes("huddle") ? "Huddle detected" : call.app.startsWith("A browser") ? "Call detected" : "Meeting detected";
  return (
    <div className="popup">
      <Logo size={34} />
      <div className="popup-text">
        <div className="popup-title">{label}</div>
        <div className="popup-sub">{call ? `${call.app} is using your microphone` : "A call app is using your microphone"}</div>
      </div>
      <div className="popup-actions">
        <button
          className="btn primary"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            api.popupAction(true).catch(() => setBusy(false));
          }}
        >
          Take notes
        </button>
        <button className="btn ghost" onClick={() => api.popupAction(false)}>
          Not now
        </button>
      </div>
    </div>
  );
}
