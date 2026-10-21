import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { api, on, Settings, Template } from "../api";

/** Shared settings state with optimistic save. */
export function useSettings(): [Settings | null, (patch: Partial<Settings>) => Promise<void>, () => void] {
  const [s, setS] = useState<Settings | null>(null);
  const reload = useCallback(() => {
    api.getSettings().then(setS);
  }, []);
  useEffect(() => {
    reload();
    return on("settings-changed", reload);
  }, [reload]);
  const save = useCallback(async (patch: Partial<Settings>) => {
    const cur = await api.getSettings();
    const next = { ...cur, ...patch };
    setS(next);
    await api.saveSettings(next);
  }, []);
  return [s, save, reload];
}

export function useTemplates(): [Template[], () => void] {
  const [t, setT] = useState<Template[]>([]);
  const reload = useCallback(() => {
    api.templates().then(setT);
  }, []);
  useEffect(() => {
    reload();
    return on("templates-changed", reload);
  }, [reload]);
  return [t, reload];
}

export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" className="logo" aria-hidden>
      <rect x="32" y="32" width="960" height="960" rx="224" fill="#141414" stroke="#2f2f2f" strokeWidth="16" />
      <rect x="262" y="214" width="500" height="596" rx="72" fill="#fafafa" stroke="none" />
      <rect x="352" y="334" width="320" height="40" rx="20" fill="#0a0a0a" stroke="none" />
      <rect x="352" y="424" width="250" height="40" rx="20" fill="#0a0a0a" stroke="none" />
      <g fill="#8b6dff" stroke="none">
        <rect x="352" y="560" width="40" height="80" rx="20" />
        <rect x="422" y="520" width="40" height="160" rx="20" />
        <rect x="492" y="548" width="40" height="104" rx="20" />
        <rect x="562" y="500" width="40" height="200" rx="20" />
        <rect x="632" y="572" width="40" height="56" rx="20" />
      </g>
    </svg>
  );
}

export function Toggle({ on: value, onChange, disabled }: { on: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={value} disabled={disabled} className={`toggle ${value ? "on" : ""}`} onClick={() => onChange(!value)}>
      <span />
    </button>
  );
}

export function Row({ title, desc, children }: { title: ReactNode; desc?: ReactNode; children?: ReactNode }) {
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-title">{title}</div>
        {desc && <div className="row-desc">{desc}</div>}
      </div>
      {children && <div className="row-ctl">{children}</div>}
    </div>
  );
}

export function Section({ title, children, aside, id }: { title?: ReactNode; children: ReactNode; aside?: ReactNode; id?: string }) {
  return (
    <section className="section" id={id}>
      {(title || aside) && (
        <div className="section-head">
          {title && <h3>{title}</h3>}
          {aside}
        </div>
      )}
      <div className="card">{children}</div>
    </section>
  );
}

export function Page({ title, sub, children, actions }: { title: ReactNode; sub?: ReactNode; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>{title}</h1>
          {sub && <p className="sub">{sub}</p>}
        </div>
        {actions && <div className="page-actions">{actions}</div>}
      </header>
      {children}
    </div>
  );
}

/** A button that opens a small menu below it. */
export function Menu({ label, children, align = "right", className = "" }: { label: ReactNode; children: (close: () => void) => ReactNode; align?: "left" | "right"; className?: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const f = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const k = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", f);
    document.addEventListener("keydown", k);
    return () => {
      document.removeEventListener("mousedown", f);
      document.removeEventListener("keydown", k);
    };
  }, [open]);
  return (
    <div className={`menu-wrap ${className}`} ref={ref}>
      <button className={`btn ghost ${open ? "pressed" : ""}`} onClick={() => setOpen(!open)}>
        {label}
      </button>
      {open && <div className={`menu ${align}`}>{children(() => setOpen(false))}</div>}
    </div>
  );
}

let toastTimer: number | undefined;
export function toast(text: string) {
  let el = document.getElementById("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    document.body.appendChild(el);
  }
  el.textContent = text;
  el.className = "show";
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => el && (el.className = ""), 2400);
}

export async function copy(text: string, what = "Copied") {
  try {
    await navigator.clipboard.writeText(text);
    toast(what);
  } catch {
    toast("Couldn't copy");
  }
}

export function dayLabel(iso: string) {
  const d = new Date(iso);
  const today = new Date();
  const y = new Date();
  y.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === y.toDateString()) return "Yesterday";
  return d.toLocaleDateString([], { weekday: "long", month: "short", day: "numeric", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

export function timeOf(iso: string) {
  return new Date(iso).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function fmtBytes(mb: number) {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

export const Icon = {
  home: <svg viewBox="0 0 20 20"><path d="M3 9.5L10 4l7 5.5V16a1 1 0 0 1-1 1h-3.5v-4.5h-5V17H4a1 1 0 0 1-1-1z" /></svg>,
  chat: <svg viewBox="0 0 20 20"><path d="M4 4.5h12a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H9l-4 3v-3H4a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1z" /></svg>,
  layout: <svg viewBox="0 0 20 20"><path d="M3.5 4h13v12h-13zM3.5 8h13M8 8v8" /></svg>,
  folder: <svg viewBox="0 0 20 20"><path d="M3 5.5a1 1 0 0 1 1-1h3.5l1.5 2h7a1 1 0 0 1 1 1V15a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z" /></svg>,
  gear: <svg viewBox="0 0 20 20"><path d="M10 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" /></svg>,
  search: <svg viewBox="0 0 20 20"><path d="M9 4a5 5 0 1 0 0 10A5 5 0 0 0 9 4zM12.7 12.7L16.5 16.5" /></svg>,
  plus: <svg viewBox="0 0 20 20"><path d="M10 4v12M4 10h12" /></svg>,
  mic: <svg viewBox="0 0 20 20"><path d="M10 3a2.5 2.5 0 0 0-2.5 2.5v4a2.5 2.5 0 0 0 5 0v-4A2.5 2.5 0 0 0 10 3zM5 9.5a5 5 0 0 0 10 0M10 14.5V17" /></svg>,
  stop: <svg viewBox="0 0 20 20"><rect x="5.5" y="5.5" width="9" height="9" rx="1.5" /></svg>,
  pause: <svg viewBox="0 0 20 20"><path d="M7.5 5v10M12.5 5v10" /></svg>,
  play: <svg viewBox="0 0 20 20"><path d="M7 5l8 5-8 5z" /></svg>,
  sparkle: <svg viewBox="0 0 20 20"><path d="M10 3l1.6 4.4L16 9l-4.4 1.6L10 15l-1.6-4.4L4 9l4.4-1.6zM15.5 13.5l.6 1.4 1.4.6-1.4.6-.6 1.4-.6-1.4-1.4-.6 1.4-.6z" /></svg>,
  retry: <svg viewBox="0 0 20 20"><path d="M15.5 8A6 6 0 1 0 16 12M16 3.5V8h-4.5" /></svg>,
  share: <svg viewBox="0 0 20 20"><path d="M10 3v9M6.5 6.5L10 3l3.5 3.5M4 11v5h12v-5" /></svg>,
  more: <svg viewBox="0 0 20 20"><path d="M5 10h.01M10 10h.01M15 10h.01" strokeWidth="2.6" /></svg>,
  copy: <svg viewBox="0 0 20 20"><path d="M7 7h9v10H7zM4 13V3h9" /></svg>,
  trash: <svg viewBox="0 0 20 20"><path d="M4 6h12M8 6V4h4v2M6 6l1 11h6l1-11" /></svg>,
  edit: <svg viewBox="0 0 20 20"><path d="M13 4l3 3-8.5 8.5H4.5v-3z" /></svg>,
  check: <svg viewBox="0 0 20 20"><path d="M4.5 10.5l3.5 3.5 7.5-8" /></svg>,
  close: <svg viewBox="0 0 20 20"><path d="M5 5l10 10M15 5L5 15" /></svg>,
  back: <svg viewBox="0 0 20 20"><path d="M12 4l-6 6 6 6" /></svg>,
  transcript: <svg viewBox="0 0 20 20"><path d="M3 7v6M6 5v10M9 8v4M12 4v12M15 7v6" /></svg>,
  send: <svg viewBox="0 0 20 20"><path d="M10 16V4M5 9l5-5 5 5" /></svg>,
  doc: <svg viewBox="0 0 20 20"><path d="M5 3h7l3 3v11H5zM12 3v3h3M8 10h4M8 13h4" /></svg>,
  people: <svg viewBox="0 0 20 20"><path d="M7.5 9a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM3 16c0-2.5 2-4.5 4.5-4.5S12 13.5 12 16M13 4.2a2.5 2.5 0 0 1 0 4.6M14.5 11.8c1.5.6 2.5 2.2 2.5 4.2" /></svg>,
  shield: <svg viewBox="0 0 20 20"><path d="M10 3l6 2.5V10c0 3.5-2.6 6-6 7-3.4-1-6-3.5-6-7V5.5z" /></svg>,
};
