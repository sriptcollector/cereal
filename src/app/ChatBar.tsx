import { KeyboardEvent, useEffect, useRef, useState } from "react";
import { api, ChatMsg, on } from "../api";
import { copy, Icon } from "./ui";
import { Markdown } from "./md";

/** Saved prompts, picked with "/" (Granola calls these recipes). */
export const RECIPES: { cmd: string; label: string; prompt: string; live?: boolean; multi?: boolean }[] = [
  { cmd: "tldr", label: "TL;DR", prompt: "Give me a TL;DR of this meeting in 3 bullets." },
  { cmd: "actions", label: "Action items by owner", prompt: "List every action item, grouped by owner, with due dates where mentioned." },
  { cmd: "email", label: "Follow-up email", prompt: "Write a short, friendly follow-up email to the other attendees: thank them, recap the key points and decisions, and list next steps with owners. Ready to copy." },
  { cmd: "decisions", label: "Decisions made", prompt: "What was decided in this meeting? Bullets only." },
  { cmd: "missed", label: "What did I miss?", prompt: "Summarize what was said in the last few minutes, in 3 bullets.", live: true },
  { cmd: "questions", label: "Open questions", prompt: "What questions were left open or need a follow-up?" },
  { cmd: "coach", label: "Coach me", prompt: "Give me honest, specific feedback on how I ran this conversation: what went well, and two things to do better next time." },
  { cmd: "week", label: "My week", prompt: "Summarize my meetings from the last 7 days: main topics, decisions and my open action items.", multi: true },
  { cmd: "todos", label: "My open to-dos", prompt: "List the action items assigned to me across my recent meetings, with the meeting each came from.", multi: true },
];

let pending: string | null = null;
/** Ask a question on the Ask page (used by the Home chat bar). */
export function queueQuestion(q: string) {
  pending = q;
}
export function takeQueued() {
  const q = pending;
  pending = null;
  return q;
}

export function useChat(meetingId: string | null) {
  const [msgs, setMsgs] = useState<ChatMsg[]>([]);
  const [streaming, setStreaming] = useState<string | null>(null);
  const [error, setError] = useState("");
  const rid = useRef("");
  useEffect(() => {
    api.chats(meetingId).then(setMsgs);
    return on<{ requestId: string; text: string }>("chat-token", (t) => {
      if (t.requestId === rid.current) setStreaming((s) => (s ?? "") + t.text);
    });
  }, [meetingId]);
  const ask = async (q: string) => {
    if (!q.trim() || streaming !== null) return;
    setError("");
    rid.current = Math.random().toString(36).slice(2);
    setMsgs((m) => [...m, { id: -Date.now(), meetingId, role: "user", content: q, createdAt: new Date().toISOString() }]);
    setStreaming("");
    try {
      await api.ask(meetingId, q, rid.current);
    } catch (e) {
      setError(String(e));
    }
    setStreaming(null);
    setMsgs(await api.chats(meetingId));
  };
  const clear = async () => {
    await api.clearChat(meetingId);
    setMsgs([]);
  };
  return { msgs, streaming, error, ask, clear };
}

export function Thread({ chat }: { chat: ReturnType<typeof useChat> }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [chat.msgs.length, chat.streaming]);
  return (
    <div className="thread">
      {chat.msgs.map((m) => (
        <div key={m.id} className={`bubble ${m.role}`}>
          {m.role === "assistant" ? <Markdown md={m.content} /> : <p>{m.content}</p>}
          {m.role === "assistant" && (
            <button className="icon-btn tiny" onClick={() => copy(m.content)} title="Copy">
              {Icon.copy}
            </button>
          )}
        </div>
      ))}
      {chat.streaming !== null && (
        <div className="bubble assistant">{chat.streaming ? <Markdown md={chat.streaming} /> : <p className="thinking">Reading your notes…</p>}</div>
      )}
      {chat.error && <div className="bubble error">{chat.error}</div>}
      <div ref={end} />
    </div>
  );
}

export default function ChatBar({
  meetingId,
  placeholder,
  onSubmit,
  onOpen,
  live,
}: {
  meetingId: string | null;
  placeholder: string;
  onSubmit?: (q: string) => void;
  onOpen?: () => void;
  live?: boolean;
}) {
  const [v, setV] = useState("");
  const [sel, setSel] = useState(0);
  const slash = v.startsWith("/") ? v.slice(1).toLowerCase() : null;
  const list = slash !== null ? RECIPES.filter((r) => (meetingId ? !r.multi : !r.live && (r.multi || true)) && (r.live ? live : true) && (r.cmd.startsWith(slash) || r.label.toLowerCase().includes(slash))) : [];
  const send = (q: string) => {
    if (!q.trim()) return;
    setV("");
    if (onSubmit) onSubmit(q);
    else {
      queueQuestion(q);
      onOpen?.();
    }
  };
  const key = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (list.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      setSel((s) => (s + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length);
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (list.length) send(list[Math.min(sel, list.length - 1)].prompt);
      else send(v);
    } else if (e.key === "Escape") setV("");
  };
  return (
    <div className="chatbar">
      {list.length > 0 && (
        <div className="recipes">
          {list.map((r, i) => (
            <button key={r.cmd} className={i === sel ? "on" : ""} onMouseEnter={() => setSel(i)} onClick={() => send(r.prompt)}>
              <span className="mono">/{r.cmd}</span> {r.label}
            </button>
          ))}
        </div>
      )}
      <div className="chatbar-box">
        {Icon.sparkle}
        <textarea rows={1} value={v} placeholder={placeholder} onChange={(e) => (setV(e.target.value), setSel(0))} onKeyDown={key} />
        <span className="hint">/ for recipes</span>
        <button className="icon-btn send" disabled={!v.trim()} onClick={() => send(v)} aria-label="Ask">
          {Icon.send}
        </button>
      </div>
    </div>
  );
}
