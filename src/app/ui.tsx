import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { api, Folder, on, Recipe, Settings, Template } from "../api";
import logo from "../assets/logo.svg";

export const LOGO = logo;

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

export function useFolders(): [Folder[], () => void] {
  const [f, setF] = useState<Folder[]>([]);
  const reload = useCallback(() => {
    api.folders().then(setF);
  }, []);
  useEffect(() => {
    reload();
    const a = on("folders-changed", reload);
    const b = on("meeting-updated", reload);
    return () => {
      a();
      b();
    };
  }, [reload]);
  return [f, reload];
}

export function useRecipes(): [Recipe[], () => void] {
  const [r, setR] = useState<Recipe[]>([]);
  const reload = useCallback(() => {
    api.recipes().then(setR);
  }, []);
  useEffect(() => {
    reload();
  }, [reload]);
  return [r, reload];
}

/** Applies the light/dark theme to <html>. */
export function useTheme(theme: string | undefined) {
  useEffect(() => {
    const apply = () => {
      const dark = theme === "dark" || ((!theme || theme === "system") && window.matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);
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

/** A button that opens a small menu. */
export function Menu({
  label,
  children,
  align = "right",
  up,
  className = "",
  btnClass = "btn ghost",
  title,
}: {
  label: ReactNode;
  children: (close: () => void) => ReactNode;
  align?: "left" | "right";
  up?: boolean;
  className?: string;
  btnClass?: string;
  title?: string;
}) {
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
      <button className={`${btnClass} ${open ? "on" : ""}`} onClick={() => setOpen(!open)} title={title}>
        {label}
      </button>
      {open && <div className={`menu ${align} ${up ? "up" : ""}`}>{children(() => setOpen(false))}</div>}
    </div>
  );
}

export function Modal({ children, onClose, className = "" }: { children: ReactNode; onClose: () => void; className?: string }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="modal-back" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${className}`}>{children}</div>
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
  toastTimer = window.setTimeout(() => el && (el.className = ""), 2600);
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
  return d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "long", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

export function timeOf(iso: string) {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function ago(iso: string) {
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 86400 * 7) return `${Math.floor(s / 86400)}d`;
  return new Date(iso).toLocaleDateString([], { month: "short", day: "numeric" });
}

export function fmtBytes(mb: number) {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

const PASTELS = ["#f6d9e8", "#d9e7f6", "#e3f0cf", "#f8e7c9", "#e5dcf7", "#d5efe9", "#f6dccf"];
export function pastel(s: string) {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return PASTELS[h % PASTELS.length];
}

/** Granola-style square avatar with the first letter. */
export function AvatarSq({ text }: { text: string }) {
  const t = (text || "?").trim();
  return (
    <span className="av-sq" style={{ background: pastel(t) }}>
      {t.charAt(0).toUpperCase()}
    </span>
  );
}

const s = (d: ReactNode) => (
  <svg className="i" viewBox="0 0 20 20">
    {d}
  </svg>
);
export const Icon = {
  home: s(<path d="M3.5 9L10 3.8 16.5 9v7.2a.8.8 0 0 1-.8.8H12v-4.5H8V17H4.3a.8.8 0 0 1-.8-.8z" />),
  chat: s(<path d="M4 4.5h12a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1H8.5L5 16.5V14H4a1 1 0 0 1-1-1V5.5a1 1 0 0 1 1-1z" />),
  lock: s(<path d="M5.5 9h9v7.5h-9zM7.5 9V6.5a2.5 2.5 0 0 1 5 0V9" />),
  people: s(<path d="M7.5 9a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM3 16c0-2.5 2-4.5 4.5-4.5S12 13.5 12 16M13 4.2a2.5 2.5 0 0 1 0 4.6M14.5 11.8c1.5.6 2.5 2.2 2.5 4.2" />),
  building: s(<path d="M4 17V4h8v13M12 8h4v9M2.5 17h15M6.5 7h1M9 7h1M6.5 10h1M9 10h1M6.5 13h1M9 13h1" />),
  layout: s(<path d="M3.5 4h13v12h-13zM3.5 8h13M8 8v8" />),
  slash: s(<path d="M12.5 3.5L7.5 16.5" />),
  trash: s(<path d="M4 6h12M8 6V4h4v2M6 6l.8 11h6.4L14 6" />),
  gear: s(<path d="M10 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" />),
  search: s(<path d="M9 4a5 5 0 1 0 0 10A5 5 0 0 0 9 4zM12.7 12.7L16.5 16.5" />),
  plus: s(<path d="M10 4.5v11M4.5 10h11" />),
  sidebar: s(<path d="M3.5 4.5h13v11h-13zM8 4.5v11" />),
  back: s(<path d="M12 4.5L6.5 10l5.5 5.5" />),
  folder: s(<path d="M3 5.5a1 1 0 0 1 1-1h3.5l1.5 2h7a1 1 0 0 1 1 1V15a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z" />),
  calendar: s(<path d="M4 5h12v11H4zM4 8.5h12M7 3.5v3M13 3.5v3" />),
  mic: s(<path d="M10 3a2.5 2.5 0 0 0-2.5 2.5v4a2.5 2.5 0 0 0 5 0v-4A2.5 2.5 0 0 0 10 3zM5 9.5a5 5 0 0 0 10 0M10 14.5V17" />),
  pause: s(<path d="M7.5 5v10M12.5 5v10" />),
  play: s(<path d="M7 5l8 5-8 5z" />),
  sparkle: s(<path d="M10 3l1.5 4.5L16 9l-4.5 1.5L10 15l-1.5-4.5L4 9l4.5-1.5zM15.5 13.5l.6 1.4 1.4.6-1.4.6-.6 1.4-.6-1.4-1.4-.6 1.4-.6z" />),
  retry: s(<path d="M15.5 8A6 6 0 1 0 16 12M16 3.5V8h-4.5" />),
  link: s(<path d="M8.5 11.5l3-3M7 9.5l-1.5 1.5a2.5 2.5 0 0 0 3.5 3.5L10.5 13M13 10.5l1.5-1.5A2.5 2.5 0 0 0 11 5.5L9.5 7" />),
  more: s(<path d="M5 10h.01M10 10h.01M15 10h.01" strokeWidth={2.6} />),
  copy: s(<path d="M7 7h9v10H7zM4 13V3h9" />),
  edit: s(<path d="M13 4l3 3-8.5 8.5H4.5v-3z" />),
  check: s(<path d="M4.5 10.5l3.5 3.5 7.5-8" />),
  close: s(<path d="M5.5 5.5l9 9M14.5 5.5l-9 9" />),
  chevron: s(<path d="M6 8l4 4 4-4" />),
  up: s(<path d="M10 16V4.5M5 9.5l5-5 5 5" />),
  clip: s(<path d="M14 9.5l-4.6 4.6a3 3 0 0 1-4.3-4.2l5.4-5.4a2 2 0 0 1 2.8 2.8l-5.2 5.2a1 1 0 0 1-1.4-1.4L11.5 6.3" />),
  sliders: s(<path d="M4 6h7M14 6h2M4 14h2M9 14h7M11 4v4M7 12v4" />),
  transcript: s(<path d="M4 8v4M7 6v8M10 4v12M13 7v6M16 9v2" />),
  share: s(<path d="M10 3v9M6.5 6.5L10 3l3.5 3.5M4 11v5h12v-5" />),
  doc: s(<path d="M5 3h7l3 3v11H5zM12 3v3h3M8 10h4M8 13h4" />),
  file: s(<path d="M5 3h7l3 3v11H5zM12 3v3h3" />),
  bell: s(<path d="M10 3.5a4 4 0 0 0-4 4V11l-1.5 2.5h11L14 11V7.5a4 4 0 0 0-4-4zM8.5 16a1.5 1.5 0 0 0 3 0" />),
  shield: s(<path d="M10 3l6 2.5V10c0 3.5-2.6 6-6 7-3.4-1-6-3.5-6-7V5.5z" />),
  plug: s(<path d="M7 3v4M13 3v4M5 7h10v3a5 5 0 0 1-10 0zM10 15v2.5" />),
  upload: s(<path d="M10 13V3.5M6.5 7L10 3.5 13.5 7M4 13v3.5h12V13" />),
  feedback: s(<path d="M4 4.5h12v9H9l-3.5 3v-3H4zM7.5 8.5h5M7.5 11h3" />),
  stop: s(<rect x="6" y="6" width="8" height="8" rx="1.5" />),
};
