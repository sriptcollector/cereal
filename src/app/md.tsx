import { ReactNode } from "react";

/** A small Markdown renderer for meeting notes: headings, nested bullets, numbered
 *  lists, checkboxes, bold/italic/code. Lines that came from the user's own notes are
 *  marked `mine` (shown brighter); the AI's additions are shown softer. */

type Block =
  | { k: "h"; level: number; text: string }
  | { k: "li"; depth: number; text: string; check: null | boolean; num: string | null }
  | { k: "p"; text: string };

export function parse(md: string): Block[] {
  const out: Block[] = [];
  for (const raw of md.replace(/\r/g, "").split("\n")) {
    if (!raw.trim()) continue;
    const h = /^(#{1,6})\s+(.*)$/.exec(raw.trim());
    if (h) {
      out.push({ k: "h", level: h[1].length, text: h[2].replace(/\*\*/g, "") });
      continue;
    }
    const li = /^(\s*)([-*•]|\d+[.)])\s+(\[( |x|X)\]\s+)?(.*)$/.exec(raw);
    if (li) {
      const indent = li[1].replace(/\t/g, "    ").length;
      out.push({ k: "li", depth: Math.min(3, Math.floor(indent / 2)), text: li[5], check: li[3] ? li[4].toLowerCase() === "x" : null, num: /\d/.test(li[2]) ? li[2] : null });
      continue;
    }
    out.push({ k: "p", text: raw.trim() });
  }
  return out;
}

function inline(s: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const re = /(\*\*[^*]+\*\*|\*[^*\s][^*]*\*|`[^`]+`)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(s))) {
    if (m.index > last) parts.push(s.slice(last, m.index));
    const t = m[0];
    if (t.startsWith("**")) parts.push(<strong key={i++}>{t.slice(2, -2)}</strong>);
    else if (t.startsWith("`")) parts.push(<code key={i++}>{t.slice(1, -1)}</code>);
    else parts.push(<em key={i++}>{t.slice(1, -1)}</em>);
    last = m.index + t.length;
  }
  if (last < s.length) parts.push(s.slice(last));
  return parts;
}

const STOP = new Set("the a an and or to of in on for with is are was were be it this that we i you they he she at by as from our my".split(" "));
export function words(s: string): string[] {
  return s
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter((w) => w.length > 1 && !STOP.has(w));
}

/** Does this AI line carry one of the user's own note lines? */
export function mineMatcher(myNotes: string) {
  const lines = myNotes
    .split("\n")
    .map((l) => words(l.replace(/^\s*([-*•]|\d+[.)]|#+)\s*/, "")))
    .filter((w) => w.length >= 1);
  return (text: string) => {
    const t = new Set(words(text));
    if (!t.size) return false;
    return lines.some((l) => {
      const hit = l.filter((w) => t.has(w)).length;
      return l.length <= 2 ? hit === l.length && hit > 0 : hit / l.length >= 0.6;
    });
  };
}

export function Markdown({ md, myNotes, onToggle }: { md: string; myNotes?: string; onToggle?: (lineText: string, done: boolean) => void }) {
  const blocks = parse(md);
  const mine = myNotes && myNotes.trim() ? mineMatcher(myNotes) : null;
  return (
    <div className="md">
      {blocks.map((b, i) => {
        if (b.k === "h") {
          const C = (b.level <= 2 ? "h2" : "h3") as "h2" | "h3";
          return <C key={i}>{inline(b.text)}</C>;
        }
        const cls = mine ? (mine(b.text) ? "mine" : "ai") : "";
        if (b.k === "p") return <p key={i} className={cls}>{inline(b.text)}</p>;
        return (
          <div key={i} className={`li d${b.depth} ${cls} ${b.check === true ? "done" : ""}`}>
            {b.check !== null ? (
              <input type="checkbox" checked={b.check} onChange={(e) => onToggle?.(b.text, e.target.checked)} />
            ) : b.num ? (
              <span className="num">{b.num}</span>
            ) : (
              <span className="dot" />
            )}
            <span>{inline(b.text)}</span>
          </div>
        );
      })}
    </div>
  );
}

/** Flip a checkbox in the markdown source. */
export function toggleCheck(md: string, lineText: string, done: boolean): string {
  return md
    .split("\n")
    .map((l) => {
      const m = /^(\s*[-*]\s+)\[( |x|X)\](\s+)(.*)$/.exec(l);
      if (m && m[4] === lineText) return `${m[1]}[${done ? "x" : " "}]${m[3]}${m[4]}`;
      return l;
    })
    .join("\n");
}

export function fmtDur(s: number) {
  s = Math.max(0, Math.round(s));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}` : `${m}:${String(sec).padStart(2, "0")}`;
}

export function fmtMinutes(s: number) {
  if (s < 60) return s > 0 ? "<1 min" : "";
  const m = Math.round(s / 60);
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`;
}
