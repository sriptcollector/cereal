import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { Modal, toast } from "./ui";

const KINDS: [string, string][] = [
  ["feedback", "Feedback"],
  ["bug", "Something's broken"],
  ["idea", "Idea"],
  ["showoff", "Show off"],
];

/** Sends feedback to Cereal's space on toolbay.ai (needs the Toolbay app signed in). */
export default function Feedback({ onClose }: { onClose: () => void }) {
  const [kind, setKind] = useState("feedback");
  const [text, setText] = useState("");
  const [debug, setDebug] = useState(false);
  const [shot, setShot] = useState<string | null>(null);
  const [signedIn, setSignedIn] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.feedbackInfo().then(setSignedIn);
  }, []);
  const pickShot = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const p = await open({ multiple: false, filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }] });
    if (typeof p !== "string") return;
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      setShot(await invoke<string>("image_data_url", { path: p }));
    } catch (e) {
      toast(String(e));
    }
  };
  const send = async () => {
    setBusy(true);
    try {
      const url = await api.feedbackSend(kind, text, shot, debug);
      toast("Thanks! Sent to toolbay.ai");
      onClose();
      openUrl(url).catch(() => {});
    } catch (e) {
      toast(String(e));
    }
    setBusy(false);
  };
  return (
    <Modal onClose={onClose}>
      <h2>Send feedback</h2>
      <p className="muted" style={{ marginTop: 0 }}>
        It goes to Cereal's space on toolbay.ai, where the maker reads everything.
      </p>
      <div className="tabs" style={{ marginTop: 0 }}>
        {KINDS.map(([k, label]) => (
          <button key={k} className={`pill ${kind === k ? "on" : ""}`} onClick={() => setKind(k)}>
            {label}
          </button>
        ))}
      </div>
      <textarea style={{ width: "100%" }} rows={6} value={text} autoFocus placeholder={kind === "bug" ? "What happened, and what did you expect?" : "What's on your mind?"} onChange={(e) => setText(e.target.value)} />
      <label className="check">
        <input type="checkbox" checked={debug} onChange={(e) => setDebug(e.target.checked)} /> Include technical details (computer, AI models, recent log lines; no notes or recordings)
      </label>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <button className="btn ghost" onClick={pickShot}>
          {shot ? "Screenshot attached ✓" : "Attach a screenshot"}
        </button>
        {signedIn === false && <span className="warn-text">Sign in to the Toolbay app first.</span>}
      </div>
      <div className="actions">
        <button className="btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button className="btn dark" disabled={busy || text.trim().length < 3 || signedIn === false} onClick={send}>
          {busy ? "Sending…" : "Send"}
        </button>
      </div>
    </Modal>
  );
}
