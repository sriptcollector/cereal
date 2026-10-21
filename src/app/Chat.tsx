import { useEffect, useState } from "react";
import { api, Source, Thread as ThreadT } from "../api";
import type { Go } from "./App";
import { ago, Icon, useRecipes } from "./ui";
import AskBar, { AskCard, Thread, useThread } from "./ChatBar";

const SCOPE_LABEL: Record<string, string> = { all: "All notes", selected: "Selected notes", folder: "Folder", person: "Person", company: "Company", meeting: "Meeting" };

export default function ChatPage({ go, threadId, question, scope = "all", scopeId = "" }: { go: Go; threadId?: string; question?: string; scope?: string; scopeId?: string }) {
  const t = useThread(scope, scopeId, threadId);
  const [recent, setRecent] = useState<ThreadT[]>([]);
  useEffect(() => {
    api.threads(null, null).then(setRecent);
    if (question) t.ask(question);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const openSource = (s: Source) => go({ page: "meeting", id: s.meetingId });
  const started = t.msgs.length > 0 || t.streaming !== null;

  if (!started) {
    return (
      <div className="page">
        <div className="topbar" />
        <div className="page-body" style={{ paddingTop: "8vh" }}>
          <h1 className="h-serif" style={{ fontSize: 26, marginBottom: 16 }}>
            Ask anything
          </h1>
          {scope !== "all" && (
            <div className="pill" style={{ marginBottom: 10 }}>
              {Icon.sliders} {SCOPE_LABEL[scope] ?? scope}
              {scope === "selected" ? ` (${scopeId.split(",").length})` : ""}
            </div>
          )}
          <AskCard placeholder="Type / for recipes" kind="multi" chips={0} onSubmit={(q, a) => t.ask(q, a)} />
          <div className="list-day" style={{ marginTop: 26 }}>
            Recipes
          </div>
          <RecipeChips onPick={(p) => t.ask(p)} />
          {recent.length > 0 && (
            <>
              <div className="list-day" style={{ marginTop: 26 }}>
                Recent chats
              </div>
              <div className="list">
                {recent.slice(0, 12).map((r) => (
                  <button key={r.id} className="item" onClick={() => t.setThreadId(r.id)}>
                    <span style={{ width: 28, display: "grid", placeItems: "center", color: "var(--muted)" }}>{Icon.chat}</span>
                    <div className="item-main">
                      <div className="item-title" style={{ fontWeight: 400 }}>
                        {r.title || "Chat"}
                      </div>
                    </div>
                    <div className="item-meta">{ago(r.updatedAt)}</div>
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    );
  }
  return (
    <div className="page">
      <div className="topbar">
        <div className="crumb">
          <button onClick={() => t.reset()} title="New chat">
            {Icon.back}
          </button>
        </div>
        <span className="muted small">{SCOPE_LABEL[scope] ?? "Chat"}</span>
        <div className="spacer" />
        <button className="btn ghost" onClick={() => t.reset()}>
          {Icon.plus} New chat
        </button>
      </div>
      <div className="page-body" style={{ flex: 1 }}>
        <Thread t={t} onSource={openSource} />
      </div>
      <AskBar scope={scope} scopeId={scopeId} placeholder="Ask a follow-up" onSubmit={(q) => t.ask(q)} />
    </div>
  );
}

function RecipeChips({ onPick }: { onPick: (prompt: string) => void }) {
  const [recipes] = useRecipes();
  const [all, setAll] = useState(false);
  const list = recipes.filter((r) => r.kind === "multi");
  return (
    <div className="recipe-chips">
      {(all ? list : list.slice(0, 5)).map((r) => (
        <button key={r.id} className="rchip" onClick={() => onPick(r.prompt)}>
          <span className="slash">/</span>
          {r.name}
        </button>
      ))}
      {!all && list.length > 5 && (
        <button className="rchip" style={{ paddingLeft: 10 }} onClick={() => setAll(true)}>
          {Icon.layout} See all
        </button>
      )}
    </div>
  );
}
