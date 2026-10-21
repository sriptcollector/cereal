import { KeyboardEvent, ReactNode, useEffect, useRef, useState } from "react";
import { api, ChatMsg, on, Recipe, Source } from "../api";
import { copy, Icon, toast, useRecipes } from "./ui";
import { Markdown } from "./md";

// Settings › Chat: Enter sends (default) or Ctrl+Enter sends and Enter adds a line.
let enterSends = true;
const loadEnter = () => api.getSettings().then((x) => (enterSends = x.chat_enter !== "newline")).catch(() => {});
loadEnter();
on("settings-changed", loadEnter);
/** Recipes that only make sense during a meeting (shown as chips while recording). */
const LIVE = ["missed", "smart", "explain"];

/** Splits "Follow-ups: a | b" off the end of an answer. */
export function splitFollowUps(text: string): [string, string[]] {
  // Only the answer's last line, so a "Follow-ups" section inside an answer stays.
  const m = /(?:^|\n)\s*\**follow-?ups?\**\s*:\**\s*([^\n]*?)\s*$/i.exec(text);
  if (!m || !m[1].includes("?")) return [text, []];
  const qs = m[1]
    .split(/\s*\|\s*/)
    .map((q) => q.trim())
    .filter((q) => q.length > 3)
    .slice(0, 3);
  return [text.slice(0, m.index).trimEnd(), qs];
}

const sendKey = (e: { key: string; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) =>
  e.key === "Enter" && (enterSends ? !e.shiftKey : e.ctrlKey || e.metaKey);

/** A chat thread: messages, streaming answer, sources. */
export function useThread(scope: string, scopeId: string, initialThread?: string | null) {
  const [threadId, setThreadId] = useState<string | null>(initialThread ?? null);
  const [msgs, setMsgs] = useState<ChatMsg[]>([]);
  const [sources, setSources] = useState<Source[]>([]);
  const [streaming, setStreaming] = useState<string | null>(null);
  const [error, setError] = useState("");
  const rid = useRef("");
  useEffect(() => {
    if (threadId) api.thread(threadId).then((t) => (setMsgs(t.msgs), setSources(t.sources)));
    else setMsgs([]);
  }, [threadId]);
  useEffect(
    () =>
      on<{ requestId: string; text: string }>("chat-token", (t) => {
        if (t.requestId === rid.current) setStreaming((s) => (s ?? "") + t.text);
      }),
    [],
  );
  const ask = async (q: string, attachment?: string | null) => {
    if (!q.trim() || streaming !== null) return;
    setError("");
    rid.current = Math.random().toString(36).slice(2);
    setMsgs((m) => [...m, { id: -Date.now(), meetingId: null, role: "user", content: q, createdAt: new Date().toISOString() }]);
    setStreaming("");
    try {
      const a = await api.chatSend({ threadId, scope, scopeId, question: q, attachment, requestId: rid.current });
      setSources(a.sources);
      if (a.threadId !== threadId) setThreadId(a.threadId);
      const t = await api.thread(a.threadId);
      setMsgs(t.msgs);
    } catch (e) {
      setError(String(e));
    }
    setStreaming(null);
  };
  const reset = () => {
    setThreadId(null);
    setMsgs([]);
    setSources([]);
  };
  return { threadId, setThreadId, msgs, sources, streaming, error, ask, reset };
}

/** Answer text with [n] citations as small clickable chips. */
function Cited({ text, sources, onSource }: { text: string; sources: Source[]; onSource?: (s: Source) => void }) {
  if (!sources.length) return <Markdown md={text} />;
  const withLinks = text.replace(/\[(\d{1,2})\]/g, (m, n) => (sources.some((s) => s.n === Number(n)) ? `⟦${n}⟧` : m));
  return <Markdown md={withLinks} cite={(n) => sources.find((s) => s.n === n)} onCite={onSource} />;
}

export function Thread({ t, onSource }: { t: ReturnType<typeof useThread>; onSource?: (s: Source) => void }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [t.msgs.length, t.streaming]);
  const lastAi = [...t.msgs].reverse().find((m) => m.role === "assistant");
  return (
    <div className="thread">
      {t.msgs.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="msg-user">
            {m.content}
          </div>
        ) : (
          <div key={m.id} className="msg-ai">
            <Cited text={splitFollowUps(m.content)[0]} sources={m === lastAi ? t.sources : []} onSource={onSource} />
            {m === lastAi && t.sources.length > 0 && (
              <div className="sources">
                {t.sources.map((s) => (
                  <button key={s.n} className="pill" onClick={() => onSource?.(s)}>
                    <span className="cite">{s.n}</span> {s.title}
                  </button>
                ))}
              </div>
            )}
            {m === lastAi && t.streaming === null && splitFollowUps(m.content)[1].length > 0 && (
              <div className="followups">
                {splitFollowUps(m.content)[1].map((q) => (
                  <button key={q} onClick={() => t.ask(q)}>
                    {q}
                  </button>
                ))}
              </div>
            )}
            <div className="tools">
              <button className="icon-btn" title="Copy" onClick={() => copy(splitFollowUps(m.content)[0].replace(/\[\d+\]/g, ""))}>
                {Icon.copy}
              </button>
            </div>
          </div>
        ),
      )}
      {t.streaming !== null && <div className="msg-ai">{t.streaming ? <Markdown md={splitFollowUps(t.streaming)[0]} /> : <p className="thinking">Reading your notes…</p>}</div>}
      {t.error && <div className="banner bad">{t.error}</div>}
      <div ref={end} />
    </div>
  );
}

/** Mic button: dictate a question (transcribed on this computer). */
export function VoiceButton({ onText }: { onText: (t: string) => void }) {
  const [on, setOn] = useState(false);
  const [busy, setBusy] = useState(false);
  const toggle = async () => {
    if (busy) return;
    if (!on) {
      try {
        await api.dictateStart();
        setOn(true);
      } catch (e) {
        toast(String(e));
      }
      return;
    }
    setOn(false);
    setBusy(true);
    try {
      const t = await api.dictateStop();
      if (t.trim()) onText(t.trim());
    } catch (e) {
      toast(String(e));
    }
    setBusy(false);
  };
  return (
    <button className={`icon-btn round ${on ? "dark" : ""}`} title={on ? "Stop and use what I said" : "Ask by voice"} onClick={toggle}>
      {busy ? <span className="spinner" /> : Icon.mic}
    </button>
  );
}

function useSlash(value: string, kind: "single" | "multi" | "any", live?: boolean) {
  const [recipes] = useRecipes();
  const slash = value.startsWith("/") ? value.slice(1).toLowerCase() : null;
  const list =
    slash === null
      ? []
      : recipes.filter(
          (r) => (kind === "any" || r.kind === kind) && (!LIVE.includes(r.cmd) || live) && (r.cmd.startsWith(slash) || r.name.toLowerCase().includes(slash)),
        );
  return list;
}

function RecipeList({ list, sel, setSel, pick }: { list: Recipe[]; sel: number; setSel: (n: number) => void; pick: (r: Recipe) => void }) {
  if (!list.length) return null;
  return (
    <div className="recipes-pop">
      {list.map((r, i) => (
        <button key={r.id} className={i === sel ? "on" : ""} onMouseEnter={() => setSel(i)} onClick={() => pick(r)}>
          <span className="slash">/</span>
          <span>{r.name}</span>
          <span className="spacer" />
          <span className="muted small">/{r.cmd}</span>
        </button>
      ))}
    </div>
  );
}

/** The floating "Ask anything" bar at the bottom of a page. */
export default function AskBar({
  scope,
  placeholder,
  onSubmit,
  live,
  extra,
}: {
  scope: string;
  scopeId: string;
  placeholder: string;
  onSubmit: (q: string) => void;
  live?: boolean;
  extra?: ReactNode;
}) {
  const [v, setV] = useState("");
  const [sel, setSel] = useState(0);
  const [recipes] = useRecipes();
  const list = useSlash(v, scope === "meeting" ? "single" : scope === "all" || scope === "folder" || scope === "selected" ? "multi" : "any", live);
  const send = (q: string) => {
    if (!q.trim()) return;
    setV("");
    onSubmit(q);
  };
  const key = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (list.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      setSel((s) => (s + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length);
    } else if (sendKey(e)) {
      e.preventDefault();
      if (list.length) send(list[Math.min(sel, list.length - 1)].prompt);
      else send(v);
    } else if (e.key === "Escape") setV("");
  };
  return (
    <div className="askbar-wrap">
      <RecipeList list={list} sel={sel} setSel={setSel} pick={(r) => send(r.prompt)} />
      {live && !v && (
        <div className="live-chips">
          {recipes
            .filter((r) => LIVE.includes(r.cmd))
            .map((r) => (
              <button key={r.id} onClick={() => send(r.prompt)}>
                {r.name}
              </button>
            ))}
        </div>
      )}
      <div className="askbar">
        {extra}
        <textarea rows={1} value={v} placeholder={placeholder} onChange={(e) => (setV(e.target.value), setSel(0))} onKeyDown={key} />
        {!v && <span className="hint">/ for recipes</span>}
        <VoiceButton onText={(t) => setV(t)} />
        <button className="icon-btn green" disabled={!v.trim()} onClick={() => send(v)} aria-label="Ask">
          {Icon.up}
        </button>
      </div>
    </div>
  );
}

/** The big "Ask anything" card (Chat page, folder pages). */
export function AskCard({ placeholder, onSubmit, kind, chips, compact }: { placeholder: string; onSubmit: (q: string, attachment?: string | null) => void; kind: "single" | "multi"; chips?: number; compact?: boolean }) {
  const [v, setV] = useState("");
  const [sel, setSel] = useState(0);
  const [attachment, setAttachment] = useState<{ name: string; text: string } | null>(null);
  const [recipes] = useRecipes();
  const list = useSlash(v, kind);
  const send = (q: string) => {
    if (!q.trim()) return;
    setV("");
    onSubmit(q, attachment?.text ?? null);
    setAttachment(null);
  };
  const attach = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const p = await open({ multiple: false, filters: [{ name: "Text", extensions: ["txt", "md", "csv", "json", "html"] }] });
    if (typeof p !== "string") return;
    try {
      const { readTextFile } = await import("./readfile");
      setAttachment({ name: p.split(/[\\/]/).pop() ?? "file", text: await readTextFile(p) });
    } catch (e) {
      toast(String(e));
    }
  };
  if (compact) {
    return (
      <div style={{ position: "relative" }}>
        <RecipeList list={list} sel={sel} setSel={setSel} pick={(r) => send(r.prompt)} />
        <div className="askbar" style={{ maxWidth: "none" }}>
          <textarea
            rows={1}
            value={v}
            placeholder={placeholder}
            onChange={(e) => (setV(e.target.value), setSel(0))}
            onKeyDown={(e) => {
              if (sendKey(e)) {
                e.preventDefault();
                if (list.length) send(list[Math.min(sel, list.length - 1)].prompt);
                else send(v);
              }
            }}
          />
          <button className="icon-btn green" onClick={() => send(v)} aria-label="Ask">
            {Icon.up}
          </button>
        </div>
        <div className="recipe-links">
          {recipes
            .filter((r) => r.kind === kind && !LIVE.includes(r.cmd))
            .slice(0, chips ?? 3)
            .map((r) => (
              <button key={r.id} onClick={() => send(r.prompt)}>
                {Icon.slash} {r.name}
              </button>
            ))}
          <span className="spacer" />
          <span className="muted small">/ for all recipes</span>
        </div>
      </div>
    );
  }
  return (
    <div style={{ position: "relative" }}>
      <RecipeList list={list} sel={sel} setSel={setSel} pick={(r) => send(r.prompt)} />
      <div className="askcard">
        <textarea
          rows={2}
          value={v}
          placeholder={placeholder}
          onChange={(e) => (setV(e.target.value), setSel(0))}
          onKeyDown={(e) => {
            if (list.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
              e.preventDefault();
              setSel((s) => (s + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length);
            } else if (sendKey(e)) {
              e.preventDefault();
              if (list.length) send(list[Math.min(sel, list.length - 1)].prompt);
              else send(v);
            }
          }}
        />
        <div className="askcard-tools">
          <button className="icon-btn" title="Attach a text file as context" onClick={attach}>
            {Icon.clip}
          </button>
          {attachment && (
            <span className="pill">
              {Icon.file} {attachment.name}
              <button className="icon-btn" style={{ width: 18, height: 18 }} onClick={() => setAttachment(null)}>
                {Icon.close}
              </button>
            </span>
          )}
          <div className="spacer" />
          <VoiceButton onText={(t) => setV(t)} />
          <button className="icon-btn green" disabled={!v.trim()} onClick={() => send(v)} aria-label="Ask">
            {Icon.up}
          </button>
        </div>
      </div>
      {chips !== 0 && (
        <div className="recipe-chips" style={{ marginTop: 12 }}>
          {recipes
            .filter((r) => r.kind === kind && !LIVE.includes(r.cmd))
            .slice(0, chips ?? 5)
            .map((r) => (
              <button key={r.id} className="rchip" onClick={() => send(r.prompt)}>
                <span className="slash">/</span>
                {r.name}
              </button>
            ))}
        </div>
      )}
    </div>
  );
}
