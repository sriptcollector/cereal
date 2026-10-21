import { useEffect, useState } from "react";
import { api, ModelsInfo, Settings } from "../api";
import { Icon, LOGO, useSettings, useTheme } from "./ui";
import Credit from "./Credit";

type Step = "welcome" | "you" | "calendar" | "audio" | "meetings" | "ready";
const ROLES = ["Founder or exec", "Sales", "Customer success", "Product", "Engineering", "Design", "Recruiting", "Investing", "Consulting", "Student", "Something else"];

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [s, save] = useSettings();
  useTheme(s?.theme);
  const [step, setStep] = useState<Step>("welcome");
  const [check, setCheck] = useState<{ state: "idle" | "running" | "done"; mic: number; sys: number; error?: string }>({ state: "idle", mic: 0, sys: 0 });
  const [info, setInfo] = useState<ModelsInfo | null>(null);
  const [platform, setPlatform] = useState("windows");
  const [cal, setCal] = useState("");
  const [calMsg, setCalMsg] = useState("");

  useEffect(() => {
    api.models().then(setInfo);
    api.appInfo().then((a) => setPlatform(a.platform));
  }, []);

  if (!s) return <div className="boot" />;
  const steps: Step[] = ["welcome", "you", "calendar", "audio", "meetings", "ready"];
  const idx = steps.indexOf(step);
  const next = () => setStep(steps[Math.min(idx + 1, steps.length - 1)]);
  const back = () => setStep(steps[Math.max(idx - 1, 0)]);

  const runCheck = async () => {
    setCheck({ state: "running", mic: 0, sys: 0 });
    try {
      const [mic, sys] = await api.audioCheck(5);
      setCheck({ state: "done", mic, sys });
    } catch (e) {
      setCheck({ state: "done", mic: 0, sys: 0, error: String(e) });
    }
  };
  const connect = async () => {
    if (!cal.trim()) return next();
    await save({ calendars: [...s.calendars, { id: Math.random().toString(36).slice(2), name: "Calendar", url: cal.trim(), enabled: true }] });
    setCalMsg("Checking…");
    try {
      const n = await api.refreshCalendars();
      setCalMsg(`Connected. ${n} upcoming events.`);
      setTimeout(next, 900);
    } catch (e) {
      setCalMsg(String(e));
    }
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
            <img className="ob-logo" src={LOGO} alt="Cereal" />
            <h1 className="h-serif">The notepad for your meetings</h1>
            <p className="lead">Cereal listens to your calls in Zoom, Google Meet, Teams, Slack or anything else, with no bot joining. Jot a few words; when the call ends, the AI on your computer writes clean notes.</p>
            <ul className="ob-points">
              <li>{Icon.shield} Everything stays on this computer. No account, no cloud, no tracking.</li>
              <li>{Icon.people} Knows who said what: you, and each of the other people.</li>
              <li>{Icon.sparkle} Free and open source. No limits.</li>
            </ul>
            <div className="ob-actions">
              <button className="btn dark big" onClick={next}>
                Get started
              </button>
            </div>
            <Credit />
          </>
        )}

        {step === "you" && (
          <>
            <h1 className="h-serif">About you</h1>
            <p className="lead">This helps Cereal write notes the way you'd want them.</p>
            <label className="field">
              <span>Your name</span>
              <input value={s.name} autoFocus placeholder="e.g. Sam Lee" onChange={(e) => save({ name: e.target.value })} />
            </label>
            <label className="field">
              <span>What do you do?</span>
              <select value={s.role} onChange={(e) => save({ role: e.target.value, default_template: defaultTemplateFor(e.target.value, s) })}>
                <option value="">Choose one</option>
                {ROLES.map((r) => (
                  <option key={r}>{r}</option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Job title (optional)</span>
              <input value={s.job_title} placeholder="e.g. Head of Sales" onChange={(e) => save({ job_title: e.target.value })} />
            </label>
            <div className="ob-actions">
              <button className="btn ghost" onClick={back}>
                Back
              </button>
              <button className="btn dark big" onClick={next}>
                Continue
              </button>
            </div>
          </>
        )}

        {step === "calendar" && (
          <>
            <h1 className="h-serif">Connect your calendar</h1>
            <p className="lead">See what's coming up, get a nudge a minute before meetings, and have notes named with the people in them. Optional.</p>
            <p className="muted small">
              Paste your calendar's private iCal address. Google Calendar: Settings › your calendar › “Secret address in iCal format”. Outlook: Settings › Calendar › Shared calendars › Publish › ICS link. It stays on this computer.
            </p>
            <input style={{ width: "100%" }} value={cal} placeholder="https://calendar.google.com/calendar/ical/…/basic.ics" onChange={(e) => setCal(e.target.value)} />
            {calMsg && <p className="muted small">{calMsg}</p>}
            <div className="ob-actions">
              <button className="btn ghost" onClick={back}>
                Back
              </button>
              <button className="btn ghost" onClick={next}>
                Skip
              </button>
              <button className="btn dark big" onClick={connect} disabled={!cal.trim()}>
                Connect
              </button>
            </div>
          </>
        )}

        {step === "audio" && (
          <>
            <h1 className="h-serif">Check your audio</h1>
            <p className="lead">
              Cereal records <b>your microphone</b> (you) and <b>your computer's sound</b> (everyone else). Press Test, say a few words and play any sound.
              {platform === "macos" && " macOS asks to allow the microphone and “Screen & System Audio Recording”; Cereal only uses the sound, never your screen."}
            </p>
            <div className={`ob-meter ${check.mic > 0.04 ? "ok" : ""}`}>
              {Icon.mic} Your microphone <b>{check.state === "done" ? (check.mic > 0.04 ? "Works" : "Silent") : check.state === "running" ? "Listening…" : ""}</b>
            </div>
            <div className={`ob-meter ${check.sys > 0.02 ? "ok" : ""}`}>
              {Icon.people} Computer sound <b>{check.state === "done" ? (check.sys > 0.02 ? "Works" : "Nothing played") : check.state === "running" ? "Listening…" : ""}</b>
            </div>
            {check.error && <p className="warn-text">{check.error}</p>}
            <button className="btn" onClick={runCheck} disabled={check.state === "running"} style={{ marginTop: 6 }}>
              {check.state === "running" ? "Listening for 5 seconds…" : check.state === "done" ? "Test again" : "Test"}
            </button>
            <p className="muted small">No headphones? That's fine: Cereal removes the echo of the others from your microphone.</p>
            <div className="ob-actions">
              <button className="btn ghost" onClick={back}>
                Back
              </button>
              <button className="btn dark big" onClick={next}>
                {check.state === "done" ? "Continue" : "Skip for now"}
              </button>
            </div>
          </>
        )}

        {step === "meetings" && (
          <>
            <h1 className="h-serif">When a call starts</h1>
            <p className="lead">Cereal notices when Zoom, Meet, Teams, Slack or another call app starts using your microphone.</p>
            <div className="choice">
              {(
                [
                  ["record", "Take notes automatically", "A small indicator shows it's recording, with one-click Stop. Short calls with no talking are thrown away."],
                  ["ask", "Ask me first", "A small card in the corner asks if you want notes."],
                  ["nothing", "Do nothing", "Start notes yourself from Cereal or the tray."],
                ] as const
              ).map(([v, t, d]) => (
                <label key={v} className={s.meeting_start === v ? "on" : ""}>
                  <input type="radio" checked={s.meeting_start === v} onChange={() => save({ meeting_start: v })} />
                  <span>
                    <b>{t}</b>
                    <small>{d}</small>
                  </span>
                </label>
              ))}
            </div>
            <div className="banner" style={{ marginTop: 12 }}>
              {Icon.shield}
              <div className="t">
                <b>Recording is your responsibility.</b> In many places (several US states, Canada, the EU) everyone on a call must agree before it's recorded. Cereal reminds you each time and gives you a message to paste in the chat:
                <q>{s.consent_message}</q>
              </div>
            </div>
            <div className="ob-actions">
              <button className="btn ghost" onClick={back}>
                Back
              </button>
              <button className="btn dark big" onClick={next}>
                I understand
              </button>
            </div>
          </>
        )}

        {step === "ready" && (
          <>
            <h1 className="h-serif">You're set</h1>
            <p className="lead">Join a call and Cereal takes it from there. You can also start a Quick note any time (Ctrl+N).</p>
            {info && (
              <p className="muted small">
                AI for this computer: {info.choice.why}. {info.notesReady ? "Ready." : "The notes model downloads in the background; recording works right away."}
              </p>
            )}
            <div className="ob-actions">
              <button
                className="btn dark big"
                onClick={async () => {
                  await save({ onboarded: true });
                  onDone();
                }}
              >
                Open Cereal
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function defaultTemplateFor(role: string, s: Settings) {
  if (/Sales|success/i.test(role)) return "sales-call";
  if (/Recruiting/.test(role)) return "interview";
  if (/Investing/.test(role)) return "pitch";
  if (/Student/.test(role)) return "lecture";
  return s.default_template || "auto";
}
