import { useEffect } from "react";
import type { Route } from "./App";
import ChatBar, { RECIPES, takeQueued, Thread, useChat } from "./ChatBar";

export default function AskPage({ go: _go }: { go: (r: Route) => void }) {
  const chat = useChat(null);
  useEffect(() => {
    const q = takeQueued();
    if (q) chat.ask(q);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className="ask">
      <div className="ask-scroll">
        <header className="page-head">
          <div>
            <h1>Ask your notes</h1>
            <p className="sub">Questions about any of your meetings. Answered by the AI on this computer, from your notes and transcripts.</p>
          </div>
          {chat.msgs.length > 0 && (
            <div className="page-actions">
              <button className="btn ghost" onClick={chat.clear}>
                New chat
              </button>
            </div>
          )}
        </header>
        {chat.msgs.length === 0 && chat.streaming === null ? (
          <div className="suggest">
            {[...RECIPES.filter((r) => r.multi), { cmd: "", label: "What did we decide about pricing?", prompt: "What did we decide about pricing?" }, { cmd: "", label: "Who owes me something?", prompt: "Which people said they would send me something or do something for me?" }].map((r) => (
              <button key={r.label} onClick={() => chat.ask(r.prompt)}>
                {r.label}
              </button>
            ))}
          </div>
        ) : (
          <Thread chat={chat} />
        )}
      </div>
      <ChatBar meetingId={null} placeholder="Ask anything across your meetings" onSubmit={chat.ask} />
    </div>
  );
}
