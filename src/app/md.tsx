import { ReactNode } from "react";
import type { Source } from "../api";

/** A small Markdown renderer for chat answers and printing: headings, nested bullets,
 *  numbered lists, checkboxes, bold/italic/code, and ⟦n⟧ citation chips. */

type Block = { k: "h"; level: number; text: string } | { k: "li"; depth: number; text: string; check: null | boolean; num: string | null } | { k: "p"; text: string };

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

type CiteFn = (n: number) => Source | undefined;

function inline(s: string, cite?: CiteFn, onCite?: (s: Source) => void): ReactNode[] {
  const parts: ReactNode[] = [];
  const re = /(\*\*[^*]+\*\*|\*[^*\s][^*]*\*|`[^`]+`|⟦\d+⟧)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(s))) {
    if (m.index > last) parts.push(s.slice(last, m.index));
    const t = m[0];
    if (t.startsWith("⟦")) {
      const n = Number(t.slice(1, -1));
      const src = cite?.(n);
      parts.push(
        <button key={i++} className="cite" title={src?.title} onClick={() => src && onCite?.(src)}>
          {n}
        </button>,
      );
    } else if (t.startsWith("**")) parts.push(<strong key={i++}>{t.slice(2, -2)}</strong>);
    else if (t.startsWith("`")) parts.push(<code key={i++}>{t.slice(1, -1)}</code>);
    else parts.push(<em key={i++}>{t.slice(1, -1)}</em>);
    last = m.index + t.length;
  }
  if (last < s.length) parts.push(s.slice(last));
  return parts;
}

export function Markdown({ md, cite, onCite }: { md: string; cite?: CiteFn; onCite?: (s: Source) => void }) {
  const blocks = parse(md);
  return (
    <div className="md">
      {blocks.map((b, i) => {
        if (b.k === "h") return <h3 key={i}>{inline(b.text, cite, onCite)}</h3>;
        if (b.k === "p") return <p key={i}>{inline(b.text, cite, onCite)}</p>;
        return (
          <div key={i} className={`li d${b.depth} ${b.check === true ? "done" : ""}`}>
            {b.check !== null ? <input type="checkbox" checked={b.check} readOnly /> : b.num ? <span className="num">{b.num}</span> : <span className="dot" />}
            <span>{inline(b.text, cite, onCite)}</span>
          </div>
        );
      })}
    </div>
  );
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
    .map((l) => words(l.replace(/^\s*([-*•]|\d+[.)]|#+|\[.?\])\s*/, "")))
    .filter((w) => w.length >= 1);
  return (text: string) => {
    if (!lines.length) return false;
    const t = new Set(words(text));
    if (!t.size) return false;
    return lines.some((l) => {
      const hit = l.filter((w) => t.has(w)).length;
      return l.length <= 2 ? hit === l.length && hit > 0 : hit / l.length >= 0.6;
    });
  };
}

/** Transcript lines most similar to a notes line (the "where did this come from" view). */
export function sourcesFor<T extends { text: string }>(line: string, segs: T[], n = 4): T[] {
  const w = new Set(words(line));
  if (!w.size) return [];
  return segs
    .map((s) => {
      const sw = words(s.text);
      const hit = sw.filter((x) => w.has(x)).length;
      return { s, score: hit / Math.sqrt(sw.length + 4) };
    })
    .filter((x) => x.score > 0.35)
    .sort((a, b) => b.score - a.score)
    .slice(0, n)
    .map((x) => x.s);
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
