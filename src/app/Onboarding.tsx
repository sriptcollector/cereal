import { useEffect, useState } from "react";
import { api, ModelsInfo } from "../api";
import { Icon, Logo, useSettings } from "./ui";
import Credit from "./Credit";

type Step = "welcome" | "consent" | "audio" | "ready";

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [s, save] = useSettings();
  const [step, setStep] = useState<Step>("welcome");
  const [check, setCheck] = useState<{ state: "idle" | "running" | "done"; mic: number; sys: number; error?: string }>({ state: "idle", mic: 0, sys: 0 });
  const [info, setInfo] = useState<ModelsInfo | null>(null);
  const [platform, setPlatform] = useState("windows");

  useEffect(() => {
    api.models().then(setInfo);
    api.appInfo().then((a) => setPlatform(a.platform));
  }, []);

  if (!s) return <div className="boot" />;
  const steps: Step[] = ["welcome", "consent", "audio", "ready"];
  const idx = steps.indexOf(step);

  const runCheck = async () => {
    setCheck({ state: "running", mic: 0, sys: 0 });
    try {
      const [mic, sys] = await api.audioCheck(5);
      setCheck({ state: "done", mic, sys });
    } catch (e) {
      setCheck({ state: "done", mic: 0, sys: 0, error: String(e) });
    }
  };

  const finish = async () => {
    await save({ onboarded: true });
    onDone();
  };

  return (
    <div className="ob">
      <div className="ob-card">
        <div className="ob-progress">
          {steps.map((x, i) => (
            <span key={x} className={i <= idx ? "on" : ""} />
          ))}
        </div>

        {step === "welcome" && (
          <>
            <Logo size={56} />
            <h1>Meeting notes that write themselves</h1>
            <p className="lead">Cereal listens to your calls on Zoom, Google Meet, Teams, Slack or anything else, with no bot joining. You jot a few words; when the call ends, the AI on your computer writes clean notes with decisions and action items.</p>
            <ul className="ob-points">
              <li>{Icon.shield} Everything stays on this computer. No account, no cloud, no tracking.</li>
              <li>{Icon.people} Knows who said what: you, and each of the other people.</li>
              <li>{Icon.sparkle} Free and open source. No limits.</li>
            </ul>
            <label className="field">
              <span>Your name (shown instead of “You” in notes)</span>
              <input value={s.name} placeholder="e.g. Sam" onChange={(e) => save({ name: e.target.value })} />
            </label>
            <div className="ob-actions">
              <button className="btn primary big" onClick={() => setStep("consent")}>
                Get started
              </button>
            </div>
            <Credit compact />
          </>
        )}

        {step === "consent" && (
          <>
            <div className="ob-icon">{Icon.shield}</div>
            <h1>Recording is your responsibility</h1>
            <p className="lead">Laws about recording calls differ by place. In many US states, in Canada, in the EU and elsewhere, <b>everyone on the call must agree</b> before it's recorded or transcribed.</p>
            <p className="lead">Cereal helps: each time it starts, it reminds you and gives you a short message to paste into the meeting chat.</p>
            <label className="field">
              <span>Your “let others know” message</span>
              <textarea rows={3} value={s.consent_message} onChange={(e) => save({ consent_message: e.target.value })} />
            </label>
            <label className="check">
              <input type="checkbox" checked={s.consent_reminder} onChange={(e) => save({ consent_reminder: e.target.checked })} /> Remind me every time I start taking notes
            </label>
            <div className="ob-actions">
              <button className="btn ghost" onClick={() => setStep("welcome")}>
                Back
              </button>
              <button className="btn primary big" onClick={() => setStep("audio")}>
                I understand
              </button>
            </div>
          </>
        )}

        {step === "audio" && (
          <>
            <div className="ob-icon">{Icon.mic}</div>
            <h1>Check your audio</h1>
            <p className="lead">
              Notes records two things: <b>your microphone</b> (you) and <b>your computer's sound</b> (everyone else). Press Test, say a few words, and play any sound on your computer (a video works).
              {platform === "macos" && " macOS will ask to allow the microphone and “Screen & System Audio Recording”; Notes only uses the sound, never your screen."}
            </p>
            <div className="ob-check">
              <div className={`ob-meter ${check.mic > 0.04 ? "ok" : ""}`}>
                {Icon.mic} Your microphone <b>{check.state === "done" ? (check.mic > 0.04 ? "Works" : "Silent") : check.state === "running" ? "Listening…" : ""}</b>
              </div>
              <div className={`ob-meter ${check.sys > 0.02 ? "ok" : ""}`}>
                {Icon.people} Computer sound <b>{check.state === "done" ? (check.sys > 0.02 ? "Works" : "Nothing played") : check.state === "running" ? "Listening…" : ""}</b>
              </div>
              {check.error && <p className="warn-text">{check.error}</p>}
              <button className="btn" onClick={runCheck} disabled={check.state === "running"}>
                {check.state === "running" ? "Listening for 5 seconds…" : check.state === "done" ? "Test again" : "Test"}
              </button>
            </div>
            <p className="muted small">Using speakers instead of headphones works too: Notes removes the echo of the other people from your microphone.</p>
            <div className="ob-actions">
              <button className="btn ghost" onClick={() => setStep("consent")}>
                Back
              </button>
              <button className="btn primary big" onClick={() => setStep("ready")}>
                {check.state === "done" ? "Continue" : "Skip for now"}
              </button>
            </div>
          </>
        )}

        {step === "ready" && (
          <>
            <div className="ob-icon">{Icon.check}</div>
            <h1>You're set</h1>
            <p className="lead">When a call starts, a small popup in the top-right corner asks if you want notes. You can also click <b>New note</b> any time.</p>
            <label className="check">
              <input type="checkbox" checked={s.detect_meetings} onChange={(e) => save({ detect_meetings: e.target.checked })} /> Offer to take notes when a call starts
            </label>
            {info && (
              <p className="muted small">
                AI for this computer: {info.choice.why}. {info.notesReady ? "Ready." : "The notes model downloads in the background; recording works right away."}
              </p>
            )}
            <div className="ob-actions">
              <button className="btn primary big" onClick={finish}>
                Open Cereal
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
