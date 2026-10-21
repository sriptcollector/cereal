import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Account, api, ModelsInfo, on, Provider } from "../api";
import { fmtBytes, Page, Row, Section, Toggle, useSettings, useTemplates } from "./ui";
import { DEFAULT_CONSENT } from "./consent";
import Credit from "./Credit";

const LANGS: [string, string][] = [
  ["auto", "Detect automatically"], ["en", "English"], ["es", "Spanish"], ["fr", "French"], ["de", "German"], ["it", "Italian"], ["pt", "Portuguese"],
  ["nl", "Dutch"], ["pl", "Polish"], ["ru", "Russian"], ["uk", "Ukrainian"], ["tr", "Turkish"], ["ar", "Arabic"], ["hi", "Hindi"], ["ja", "Japanese"],
  ["ko", "Korean"], ["zh", "Chinese"], ["vi", "Vietnamese"], ["id", "Indonesian"], ["sv", "Swedish"],
];

export default function SettingsPage() {
  const [s, save] = useSettings();
  const [templates] = useTemplates();
  const [mics, setMics] = useState<string[]>([]);
  const [info, setInfo] = useState<ModelsInfo | null>(null);
  const [check, setCheck] = useState<string>("");
  const [status, setStatus] = useState("");
  const [account, setAccount] = useState<Account | null>(null);
  const [sync, setSync] = useState<[boolean, boolean]>([false, false]);
  const [app, setApp] = useState<{ version: string; fromHub: boolean; dataDir: string } | null>(null);
  const [advanced, setAdvanced] = useState(false);

  useEffect(() => {
    api.mics().then(setMics);
    api.models().then(setInfo);
    api.account().then(setAccount);
    api.syncStatus().then(setSync);
    api.appInfo().then(setApp);
    const u1 = on<{ busy: boolean; message: string }>("model-status", (x) => {
      setStatus(x.message);
      if (!x.busy) api.models().then(setInfo);
    });
    return u1;
  }, []);

  if (!s) return <Page title="Settings">{null}</Page>;

  const runCheck = async () => {
    setCheck("Listening for 4 seconds: say something, and play a sound on your computer…");
    try {
      const [mic, sys] = await api.audioCheck(4);
      setCheck(`${mic > 0.04 ? "✓ Microphone hears you" : "✗ Microphone was silent"} · ${sys > 0.02 ? "✓ Computer sound is recorded" : "· No computer sound heard (play something to test)"}`);
    } catch (e) {
      setCheck(String(e));
    }
  };

  const speechModels = info?.models.filter((m) => m.kind === "speech") ?? [];
  const llmModels = info?.models.filter((m) => m.kind === "llm") ?? [];
  const cloud = (p: Provider) => p !== "local";

  return (
    <Page title="Settings">
      <Section title="General">
        <Row title="Your name" desc="Used instead of “You” in transcripts and notes.">
          <input value={s.name} placeholder="You" onChange={(e) => save({ name: e.target.value })} />
        </Row>
        <Row title="Meeting language" desc="The language people speak in your meetings.">
          <select value={s.language} onChange={(e) => save({ language: e.target.value })}>
            {LANGS.map(([c, n]) => (
              <option key={c} value={c}>
                {n}
              </option>
            ))}
          </select>
        </Row>
        <Row title="Default template" desc="How notes are laid out unless you pick another one.">
          <select value={s.default_template} onChange={(e) => save({ default_template: e.target.value })}>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </select>
        </Row>
        {!app?.fromHub && (
          <Row title="Start with your computer" desc="So it can offer to take notes when a call starts.">
            <Toggle on={s.launch_at_login} onChange={(v) => save({ launch_at_login: v })} />
          </Row>
        )}
      </Section>

      <Section title="Recording">
        <Row title="Offer to take notes when a call starts" desc="When Zoom, Google Meet, Teams, Slack or another call app starts using your microphone, a small popup asks if you want notes. Nothing records until you click.">
          <Toggle on={s.detect_meetings} onChange={(v) => save({ detect_meetings: v })} />
        </Row>
        <Row title="Stop when the call ends" desc="Stops 20 seconds after the call app lets go of the microphone, or after 15 minutes of silence.">
          <Toggle on={s.auto_stop} onChange={(v) => save({ auto_stop: v })} />
        </Row>
        <Row title="Microphone" desc="Your side of the call.">
          <select value={s.microphone ?? ""} onChange={(e) => save({ microphone: e.target.value || null })}>
            <option value="">System default</option>
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
        <Row title="Test your audio" desc={check || "Checks that both your microphone and your computer's sound come through."}>
          <button className="btn" onClick={runCheck}>
            Test
          </button>
        </Row>
        <Row title="Keep recordings" desc="Audio files stay on this computer only. Transcripts and notes are always kept.">
          <select value={s.keep_audio} onChange={(e) => save({ keep_audio: e.target.value as "keep" })}>
            <option value="keep">Keep</option>
            <option value="30d">Delete after 30 days</option>
            <option value="delete">Delete once notes are written</option>
          </select>
        </Row>
      </Section>

      <Section title="Recording consent">
        <Row title="Remind me to tell people" desc="Some places (several US states, much of Europe, and others) require everyone's consent before a call is recorded. You're responsible for following the law where you and the other people are.">
          <Toggle on={s.consent_reminder} onChange={(v) => save({ consent_reminder: v })} />
        </Row>
        <div className="row col">
          <div className="row-title">“Let others know” message</div>
          <textarea rows={3} value={s.consent_message} onChange={(e) => save({ consent_message: e.target.value })} />
          {s.consent_message !== DEFAULT_CONSENT && (
            <button className="btn ghost small" onClick={() => save({ consent_message: DEFAULT_CONSENT })}>
              Reset to default
            </button>
          )}
        </div>
      </Section>

      <Section title="AI models" aside={status ? <span className="muted small">{status}</span> : undefined}>
        <Row
          title="Pick models for this computer automatically"
          desc={
            info
              ? `${info.hardware.gpu_name || "No dedicated GPU"}${info.hardware.vram_mb ? `, ${Math.round(info.hardware.vram_mb / 1024)} GB` : ""} · ${Math.round(info.hardware.ram_mb / 1024)} GB memory · ${info.hardware.threads} threads. Using ${info.choice.speech_model} for speech and ${info.choice.llm_model} for notes.`
              : "Checking this computer…"
          }
        >
          <Toggle on={s.model_mode === "auto"} onChange={(v) => save({ model_mode: v ? "auto" : "manual" })} />
        </Row>
        {info && !info.notesReady && <Row title="Notes model" desc="Downloading in the background. Recording and transcripts already work; notes are written as soon as it's ready." />}
        <Row title="Advanced" desc="Choose models yourself, or use your own cloud API key.">
          <button className="btn ghost" onClick={() => setAdvanced(!advanced)}>
            {advanced ? "Hide" : "Show"}
          </button>
        </Row>
        {advanced && (
          <>
            <Row title="Speech model" desc={`In use now: ${info?.speechInUse || "…"}`}>
              <select value={s.speech_model} disabled={s.model_mode === "auto"} onChange={(e) => save({ speech_model: e.target.value })}>
                {speechModels.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.label} · {fmtBytes(m.sizeMb)}
                    {m.installed ? "" : " (downloads)"}
                  </option>
                ))}
              </select>
            </Row>
            <Row title="Notes model">
              <select value={s.llm_model} disabled={s.model_mode === "auto"} onChange={(e) => save({ llm_model: e.target.value })}>
                {llmModels.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.label} · {fmtBytes(m.sizeMb)}
                    {m.installed ? "" : " (downloads)"}
                  </option>
                ))}
              </select>
            </Row>
            <Row title="Use the graphics card" desc="Much faster when there is one.">
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
            <Row title="Notes AI" desc="Local by default. A cloud model gets your transcript and notes when it writes notes or answers.">
              <select value={s.llm_provider} onChange={(e) => save({ llm_provider: e.target.value as Provider })}>
                <option value="local">On this computer</option>
                <option value="open_ai">OpenAI-compatible API</option>
                <option value="anthropic">Anthropic API</option>
              </select>
            </Row>
            {cloud(s.llm_provider) && (
              <div className="row col">
                <input
                  value={s.llm_cloud.base_url}
                  onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, base_url: e.target.value } })}
                  placeholder={s.llm_provider === "anthropic" ? "https://api.anthropic.com" : "https://api.openai.com/v1"}
                />
                <input value={s.llm_cloud.model} onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, model: e.target.value } })} placeholder={s.llm_provider === "anthropic" ? "claude-sonnet-5-5" : "gpt-4.1-mini"} />
                <input type="password" value={s.llm_cloud.api_key} onChange={(e) => save({ llm_cloud: { ...s.llm_cloud, api_key: e.target.value } })} placeholder="API key (stays on this computer)" />
              </div>
            )}
          </>
        )}
      </Section>

      <Section title="Toolbay account">
        {sync[0] ? (
          <>
            <Row title={account ? `Signed in as ${account.name || account.handle}` : "Signed in through the Toolbay app"} desc="Your settings and templates sync to your other computers. Recordings, transcripts and notes stay on this computer." />
            <Row title="Sync settings and templates">
              <Toggle on={s.sync_enabled} onChange={(v) => save({ sync_enabled: v })} />
            </Row>
          </>
        ) : (
          <Row title="Not signed in" desc="Open Cereal from the Toolbay app while signed in to sync settings and templates between computers. Everything works without an account." />
        )}
      </Section>

      <Section title="Privacy">
        <Row title="Everything stays on this computer" desc="Audio, transcripts and notes are stored and processed locally. No bot joins your calls. No analytics, no tracking, no account needed. The only internet use is downloading AI models, Toolbay sync if you're signed in, and a cloud engine if you add your own key." />
        <Row title="Your data folder" desc={app?.dataDir}>
          <button className="btn ghost" onClick={() => api.openDataDir()}>
            Open
          </button>
        </Row>
      </Section>

      <Section title="About">
        <Row title={`Cereal ${app?.version ?? ""}`} desc="Free and open source (MIT).">
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
    </Page>
  );
}
