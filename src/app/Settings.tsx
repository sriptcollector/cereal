import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Account, api, CalendarSrc, ModelsInfo, on, Provider } from "../api";
import type { Go } from "./App";
import { fmtBytes, Icon, Row, Section, toast, Toggle, useSettings, useTemplates } from "./ui";
import { DEFAULT_CONSENT } from "./consent";
import Credit from "./Credit";

const LANGS: [string, string][] = [
  ["auto", "Detect automatically"], ["en", "English"], ["es", "Spanish"], ["fr", "French"], ["de", "German"], ["it", "Italian"], ["pt", "Portuguese"],
  ["nl", "Dutch"], ["pl", "Polish"], ["ru", "Russian"], ["uk", "Ukrainian"], ["tr", "Turkish"], ["ar", "Arabic"], ["hi", "Hindi"], ["ja", "Japanese"],
  ["ko", "Korean"], ["zh", "Chinese"], ["vi", "Vietnamese"], ["id", "Indonesian"], ["sv", "Swedish"], ["da", "Danish"], ["no", "Norwegian"], ["fi", "Finnish"],
  ["cs", "Czech"], ["el", "Greek"], ["he", "Hebrew"], ["ro", "Romanian"], ["hu", "Hungarian"], ["th", "Thai"], ["ms", "Malay"], ["ca", "Catalan"],
];
const APPS = ["Zoom", "Google Meet", "Microsoft Teams", "Slack huddle", "Discord", "Webex", "FaceTime", "WhatsApp call", "A browser call"];
const NAV: [string, string][] = [
  ["profile", "Profile"],
  ["preferences", "Preferences"],
  ["calendar", "Calendar"],
  ["notifications", "Notifications"],
  ["consent", "Transparency"],
  ["ai", "AI models"],
  ["data", "Data & privacy"],
  ["account", "Account"],
  ["about", "About"],
];

export default function SettingsPage({ go, section }: { go: Go; section?: string }) {
  const [s, save] = useSettings();
  const [templates] = useTemplates();
  const [mics, setMics] = useState<string[]>([]);
  const [info, setInfo] = useState<ModelsInfo | null>(null);
  const [check, setCheck] = useState("");
  const [status, setStatus] = useState("");
  const [account, setAccount] = useState<Account | null>(null);
  const [sync, setSync] = useState<[boolean, boolean]>([false, false]);
  const [app, setApp] = useState<{ version: string; fromHub: boolean; dataDir: string } | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [newCal, setNewCal] = useState("");
  const [vocab, setVocab] = useState("");
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.mics().then(setMics);
    api.models().then(setInfo);
    api.account().then(setAccount);
    api.syncStatus().then(setSync);
    api.appInfo().then(setApp);
    return on<{ busy: boolean; message: string }>("model-status", (x) => {
      setStatus(x.message);
      if (!x.busy) api.models().then(setInfo);
    });
  }, []);
  useEffect(() => {
    if (section) setTimeout(() => document.getElementById(section)?.scrollIntoView({ block: "start" }), 50);
  }, [section]);
  useEffect(() => {
    if (s) setVocab(s.vocabulary.join(", "));
  }, [s?.vocabulary.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!s) return <div className="page" />;

  const runCheck = async () => {
    setCheck("Listening for 4 seconds: say something, and play a sound on your computer…");
    try {
      const [mic, sys] = await api.audioCheck(4);
      setCheck(`${mic > 0.04 ? "✓ Microphone hears you" : "✗ Microphone was silent"} · ${sys > 0.02 ? "✓ Computer sound is recorded" : "· No computer sound heard (play something to test)"}`);
    } catch (e) {
      setCheck(String(e));
    }
  };
  const addCalendar = async () => {
    const url = newCal.trim();
    if (!url) return;
    const cal: CalendarSrc = { id: Math.random().toString(36).slice(2), name: /google/.test(url) ? "Google Calendar" : /outlook|office/.test(url) ? "Outlook" : "Calendar", url, enabled: true };
    await save({ calendars: [...s.calendars, cal] });
    setNewCal("");
    try {
      const n = await api.refreshCalendars();
      toast(`Connected: ${n} upcoming events`);
    } catch (e) {
      toast(String(e));
    }
  };
  const speech = info?.models.filter((m) => m.kind === "speech") ?? [];
  const llms = info?.models.filter((m) => m.kind === "llm") ?? [];
  const cloud = (p: Provider) => p !== "local";

  return (
    <div className="page" ref={scroller}>
      <div className="topbar" />
      <div className="page-body" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">Settings</h1>
        <div className="recipe-chips" style={{ margin: "6px 0 22px" }}>
          {NAV.map(([id, label]) => (
            <button key={id} className="pill" onClick={() => document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" })}>
              {label}
            </button>
          ))}
        </div>

        <Section title="Profile" id="profile">
          <Row title="Name" desc="Used instead of “Me” in transcripts and notes.">
            <input value={s.name} placeholder="Your name" onChange={(e) => save({ name: e.target.value })} />
          </Row>
          <Row title="Job title">
            <input value={s.job_title} placeholder="e.g. Head of Sales" onChange={(e) => save({ job_title: e.target.value })} />
          </Row>
          <div className="row col">
            <div className="row-title">About you and your work</div>
            <div className="row-desc">A sentence or two that helps the notes (your company, your team, what you care about).</div>
            <textarea rows={3} value={s.about} onChange={(e) => save({ about: e.target.value })} placeholder="I run sales at a 20-person design agency. Clients are mostly SaaS startups." />
          </div>
          <Row title="Company name">
            <input value={s.company_name} placeholder="e.g. Acme" onChange={(e) => save({ company_name: e.target.value })} />
          </Row>
          <div className="row col">
            <div className="row-title">About your company</div>
            <div className="row-desc">What it does and who its customers are. Helps the notes get names and context right.</div>
            <textarea rows={2} value={s.company_about} onChange={(e) => save({ company_about: e.target.value })} placeholder="Design agency for SaaS startups, 20 people." />
          </div>
        </Section>

        <Section title="Preferences" id="preferences">
          <Row title="When a meeting starts" desc="When Zoom, Google Meet, Teams, Slack or another call app starts using your microphone.">
            <select value={s.meeting_start} onChange={(e) => save({ meeting_start: e.target.value as "record" })}>
              <option value="record">Record automatically</option>
              <option value="ask">Ask me</option>
              <option value="nothing">Do nothing</option>
            </select>
          </Row>
          <Row title="Stop when the call ends" desc="When the call app lets go of the microphone and the call has gone quiet, at the meeting's scheduled end, or after 15 minutes of silence (you're asked first). Notes are written right away.">
            <Toggle on={s.auto_stop} onChange={(v) => save({ auto_stop: v })} />
          </Row>
          <Row title="Live meeting indicator" desc="A small floating control while notes are being taken and Cereal isn't in front. Always shown for recordings that start by themselves.">
            <Toggle on={s.show_indicator} onChange={(v) => save({ show_indicator: v })} />
          </Row>
          <Row title="Sounds" desc="A short chime when notes start, pause, resume and stop.">
            <Toggle on={s.sounds} onChange={(v) => save({ sounds: v })} />
          </Row>
          <Row title="Quick chat (Alt+Space)" desc="Ask about your meetings from anywhere. Takes effect after restarting Cereal.">
            <Toggle on={s.quick_chat} onChange={(v) => save({ quick_chat: v })} />
          </Row>
          <Row title="Enter key in chat">
            <select value={s.chat_enter} onChange={(e) => save({ chat_enter: e.target.value })}>
              <option value="send">Sends the message (Shift+Enter for a new line)</option>
              <option value="newline">Adds a new line (Ctrl+Enter sends)</option>
            </select>
          </Row>
          <Row title="Move Cereal beside the call" desc="When you join a meeting from Cereal, the note opens next to the call window.">
            <Toggle on={s.reposition} onChange={(v) => save({ reposition: v })} />
          </Row>
          <Row title="Transcription language" desc="The language people speak in your meetings.">
            <select value={s.language} onChange={(e) => save({ language: e.target.value })}>
              {LANGS.map(([c, n]) => (
                <option key={c} value={c}>
                  {n}
                </option>
              ))}
            </select>
          </Row>
          <Row title="Notes language">
            <select value={s.summary_language} onChange={(e) => save({ summary_language: e.target.value })}>
              <option value="auto">Same as the meeting</option>
              {LANGS.filter(([c]) => c !== "auto").map(([c, n]) => (
                <option key={c} value={c}>
                  {n}
                </option>
              ))}
            </select>
          </Row>
          <div className="row col">
            <div className="row-title">Your words</div>
            <div className="row-desc">Names, products and jargon Cereal should always spell right, separated by commas. No limit.</div>
            <input
              value={vocab}
              onChange={(e) => setVocab(e.target.value)}
              onBlur={() => save({ vocabulary: vocab.split(",").map((w) => w.trim()).filter(Boolean) })}
              placeholder="Toolbay, Kubernetes, Priya Raman"
            />
          </div>
          <Row title="Default template" desc="How notes are laid out unless you pick another one.">
            <select value={s.default_template} onChange={(e) => save({ default_template: e.target.value })}>
              {templates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </Row>
          <Row title="Microphone">
            <select value={s.microphone ?? ""} onChange={(e) => save({ microphone: e.target.value || null })}>
              <option value="">Automatic (system default)</option>
              {mics.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </Row>
          <Row title="Record the other people (computer sound)" desc="Everyone else in the call, from what your computer plays. Your other apps' sound is never muted.">
            <Toggle on={s.capture_system} onChange={(v) => save({ capture_system: v })} />
          </Row>
          <Row title="Test your audio" desc={check || "Checks your microphone and your computer's sound."}>
            <button className="btn" onClick={runCheck}>
              Test
            </button>
          </Row>
          <Row title="Appearance">
            <select value={s.theme} onChange={(e) => save({ theme: e.target.value as "system" })}>
              <option value="system">Match the system</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </Row>
          {!app?.fromHub && (
            <Row title="Open at login" desc="So Cereal can notice meetings.">
              <Toggle on={s.launch_at_login} onChange={(v) => save({ launch_at_login: v })} />
            </Row>
          )}
        </Section>

        <Section title="Calendar" id="calendar">
          {s.calendars.map((c) => (
            <Row key={c.id} title={c.name} desc={c.url.replace(/(private-|\/ical\/).*/, "$1…")}>
              <Toggle on={c.enabled} onChange={(v) => save({ calendars: s.calendars.map((x) => (x.id === c.id ? { ...x, enabled: v } : x)) }).then(() => api.refreshCalendars())} />
              <button className="icon-btn" title="Remove" onClick={() => save({ calendars: s.calendars.filter((x) => x.id !== c.id) }).then(() => api.refreshCalendars())}>
                {Icon.trash}
              </button>
            </Row>
          ))}
          <div className="row col">
            <div className="row-title">Add a calendar</div>
            <div className="row-desc">
              Paste your calendar's private iCal address. Google Calendar: Settings › your calendar › “Secret address in iCal format”. Outlook: Settings › Calendar › Shared calendars › Publish a calendar › ICS. Cereal reads it every 10 minutes; nothing is sent anywhere else.
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <input style={{ flex: 1 }} value={newCal} placeholder="https://calendar.google.com/calendar/ical/…/private-…/basic.ics" onChange={(e) => setNewCal(e.target.value)} />
              <button className="btn dark" onClick={addCalendar} disabled={!newCal.trim()}>
                Connect
              </button>
            </div>
          </div>
          <Row title="Show events with no participants" desc="In Coming up: also your own blocks and reminders.">
            <Toggle on={s.show_solo_events} onChange={(v) => save({ show_solo_events: v })} />
          </Row>
          <Row title="Remind me a minute before meetings" desc="For calendar meetings with other people that you haven't declined.">
            <Toggle on={s.calendar_popups} onChange={(v) => save({ calendar_popups: v })} />
          </Row>
        </Section>

        <Section title="Notifications" id="notifications">
          <div className="row col">
            <div className="row-desc">What Cereal does when each app starts a call. “Default” follows “When a meeting starts” above.</div>
          </div>
          {APPS.map((a) => (
            <Row key={a} title={a}>
              <select value={s.app_rules[a] ?? ""} onChange={(e) => api.setAppRule(a, e.target.value)}>
                <option value="">Default</option>
                <option value="record">Record automatically</option>
                <option value="ask">Ask me</option>
                <option value="ignore">Ignore this app</option>
              </select>
            </Row>
          ))}
        </Section>

        <Section title="Transparency" id="consent">
          <Row title="Remind me to tell people" desc="Some places (several US states, much of Europe, and others) require everyone's consent before a call is recorded. You're responsible for following the law where you and the others are.">
            <Toggle on={s.consent_reminder} onChange={(v) => save({ consent_reminder: v })} />
          </Row>
          <div className="row col">
            <div className="row-title">“Let others know” message</div>
            <textarea rows={3} value={s.consent_message} onChange={(e) => save({ consent_message: e.target.value })} />
            {s.consent_message !== DEFAULT_CONSENT && (
              <div>
                <button className="btn ghost" onClick={() => save({ consent_message: DEFAULT_CONSENT })}>
                  Reset to default
                </button>
              </div>
            )}
          </div>
        </Section>

        <Section title="AI models" id="ai" aside={status ? <span className="muted small">{status}</span> : undefined}>
          <Row
            title="Pick models for this computer"
            desc={
              info
                ? `${info.hardware.gpu_name || "No dedicated graphics card"}${info.hardware.vram_mb ? `, ${Math.round(info.hardware.vram_mb / 1024)} GB` : ""} · ${Math.round(info.hardware.ram_mb / 1024)} GB memory. Speech: ${info.choice.speech_model}. Notes: ${info.choice.llm_model}.`
                : "Checking this computer…"
            }
          >
            <Toggle on={s.model_mode === "auto"} onChange={(v) => save({ model_mode: v ? "auto" : "manual" })} />
          </Row>
          {info && !info.notesReady && <Row title="Notes model" desc="Downloading in the background. Recording and transcripts already work." />}
          <Row title="Advanced" desc="Choose models yourself, or use your own cloud API key.">
            <button className="btn ghost" onClick={() => setAdvanced(!advanced)}>
              {advanced ? "Hide" : "Show"}
            </button>
          </Row>
          {advanced && (
            <>
              <Row title="Speech model" desc={`In use now: ${info?.speechInUse || "…"}`}>
                <select value={s.speech_model} disabled={s.model_mode === "auto"} onChange={(e) => save({ speech_model: e.target.value })}>
                  {speech.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label} · {fmtBytes(m.sizeMb)}
                      {m.installed ? "" : " (downloads)"}
                    </option>
                  ))}
                </select>
              </Row>
              <Row title="Notes model">
                <select value={s.llm_model} disabled={s.model_mode === "auto"} onChange={(e) => save({ llm_model: e.target.value })}>
                  {llms.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label} · {fmtBytes(m.sizeMb)}
                      {m.installed ? "" : " (downloads)"}
                    </option>
                  ))}
                </select>
              </Row>
              <Row title="Use the graphics card">
                <Toggle on={s.use_gpu} onChange={(v) => save({ use_gpu: v })} />
              </Row>
              <Row title="Full-quality pass after the call" desc="Transcribes the whole recording again with more context. Best with a graphics card.">
                <Toggle on={s.final_pass} onChange={(v) => save({ final_pass: v })} />
              </Row>
              <Row title="Cloud speech engine" desc="Optional. Sends audio to the service you pick (OpenAI-compatible, e.g. Groq) with your own key.">
                <Toggle on={s.speech_cloud_on} onChange={(v) => save({ speech_cloud_on: v })} />
              </Row>
              {s.speech_cloud_on && (
                <div className="row col">
                  <input value={s.speech_cloud.base_url} onChange={(e) => save({ speech_cloud: { ...s.speech_cloud, base_url: e.target.value } })} placeholder="https://api.groq.com/openai/v1" />
                  <input value={s.speech_cloud.model} onChange={(e) => save({ speech_cloud: { ...s.speech_cloud, model: e.target.value } })} placeholder="whisper-large-v3-turbo" />
                  <input type="password" value={s.speech_cloud.api_key} onChange={(e) => save({ speech_cloud: { ...s.speech_cloud, api_key: e.target.value } })} placeholder="API key (stays on this computer)" />
                </div>
              )}
              <Row title="Notes AI" desc="On this computer by default. A cloud model gets your transcript and notes when it writes notes or answers.">
                <select value={s.llm_provider} onChange={(e) => save({ llm_provider: e.target.value as Provider })}>
                  <option value="local">On this computer</option>
                  <option value="open_ai">OpenAI-compatible API</option>
                  <option value="anthropic">Anthropic API</option>
                </select>
              </Row>
              {cloud(s.llm_provider) && (
                <div className="row col">
                  <input value={s.llm_cloud.base_url} onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, base_url: e.target.value } })} placeholder={s.llm_provider === "anthropic" ? "https://api.anthropic.com" : "https://api.openai.com/v1"} />
                  <input value={s.llm_cloud.model} onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, model: e.target.value } })} placeholder={s.llm_provider === "anthropic" ? "claude-sonnet-5-5" : "gpt-4.1-mini"} />
                  <input type="password" value={s.llm_cloud.api_key} onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, api_key: e.target.value } })} placeholder="API key (stays on this computer)" />
                </div>
              )}
            </>
          )}
        </Section>

        <Section title="Data & privacy" id="data">
          <Row title="Everything stays on this computer" desc="Audio, transcripts and notes are stored and processed here. No bot joins your calls, no analytics, no account needed. Cereal never trains on your data." />
          <Row title="Keep recordings" desc="Audio files are only ever on this computer.">
            <select value={s.keep_audio} onChange={(e) => save({ keep_audio: e.target.value as "keep" })}>
              <option value="keep">Keep</option>
              <option value="30d">Delete after 30 days</option>
              <option value="delete">Delete once notes are written</option>
            </select>
          </Row>
          <Row title="Delete transcripts after" desc="Notes are kept; only the word-by-word transcript is removed.">
            <select value={s.transcript_retention} onChange={(e) => save({ transcript_retention: e.target.value })}>
              <option value="off">Never</option>
              <option value="1d">1 day</option>
              <option value="1w">1 week</option>
              <option value="1m">1 month</option>
              <option value="3m">3 months</option>
              <option value="6m">6 months</option>
              <option value="1y">1 year</option>
            </select>
          </Row>
          <Row title="Export all notes" desc="Every note with its transcript, saved to Downloads.">
            <button className="btn" onClick={() => api.exportAll("csv").then((p) => toast(`Saved ${p}`))}>
              CSV
            </button>
            <button className="btn" onClick={() => api.exportAll("json").then((p) => toast(`Saved ${p}`))}>
              JSON
            </button>
          </Row>
          <Row title="Your data folder" desc={app?.dataDir}>
            <button className="btn ghost" onClick={() => api.openDataDir()}>
              Open
            </button>
          </Row>
          <Row title="Trash">
            <button className="btn ghost" onClick={() => go({ page: "trash" })}>
              Open trash
            </button>
          </Row>
        </Section>

        <Section title="Account" id="account">
          {sync[0] ? (
            <>
              <Row title={account ? `Signed in as ${account.name || account.handle}` : "Signed in through the Toolbay app"} desc="Settings, templates and recipes sync to your other computers. Recordings, transcripts and notes stay here." />
              <Row title="Sync settings and templates">
                <Toggle on={s.sync_enabled} onChange={(v) => save({ sync_enabled: v })} />
              </Row>
            </>
          ) : (
            <Row title="Not signed in" desc="Sign in to the Toolbay app to sync settings and templates between computers and to send feedback. Everything else works without an account." />
          )}
        </Section>

        <Section title="About" id="about">
          <Row title={`Cereal ${app?.version ?? ""}`} desc="Free and open source (MIT). Part of Toolbay.">
            <button className="btn ghost" onClick={() => openUrl("https://github.com/sriptcollector/cereal")}>
              Source code
            </button>
          </Row>
          <div className="row">
            <Credit />
          </div>
          {!app?.fromHub && (
            <Row title="Quit Cereal" desc="Stops meeting detection until you open it again.">
              <button className="btn ghost danger" onClick={() => api.quit()}>
                Quit
              </button>
            </Row>
          )}
        </Section>
      </div>
    </div>
  );
}
