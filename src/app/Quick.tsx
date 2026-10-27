import { useEffect, useRef, useState } from "react";
import { api, Settings } from "../api";
import { LogoImg, useTheme } from "./ui";

/** Alt+Space: ask about your meetings from anywhere. */
export default function Quick() {
  const [q, setQ] = useState("");
  const [s, setS] = useState<Settings | null>(null);
  const box = useRef<HTMLInputElement>(null);
  useTheme(s?.theme);
  useEffect(() => {
    api.getSettings().then(setS);
    const f = () => box.current?.focus();
    window.addEventListener("focus", f);
    f();
    return () => window.removeEventListener("focus", f);
  }, []);
  const send = () => {
    const t = q.trim();
    if (!t) return;
    setQ("");
    api.quickHide();
    api.openHub(`ask:${t}`);
  };
  return (
    <div className="quick">
      <LogoImg alt="" />
      <input
        ref={box}
        placeholder="Ask anything about your meetings"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") send();
          if (e.key === "Escape") api.quickHide();
        }}
        onBlur={() => setTimeout(() => api.quickHide(), 150)}
      />
      <span className="kbd">Enter</span>
    </div>
  );
}
