import { useEffect, useMemo, useRef, useState } from "react";
import { Extension } from "@tiptap/core";
import { EditorContent, useEditor, ReactRenderer } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import Image from "@tiptap/extension-image";
import Mention from "@tiptap/extension-mention";
import { Placeholder } from "@tiptap/extensions";
import { Plugin } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Segment } from "../api";
import { fmtDur, mineMatcher, sourcesFor } from "./md";
import { Icon } from "./ui";

/** Greys out the lines the AI added; keeps the user's own points in full ink. */
function aiShading(getMine: () => ((t: string) => boolean) | null) {
  return Extension.create({
    name: "aiShading",
    addProseMirrorPlugins() {
      return [
        new Plugin({
          props: {
            decorations(state) {
              const mine = getMine();
              if (!mine) return DecorationSet.empty;
              const decos: Decoration[] = [];
              state.doc.descendants((node, pos) => {
                if (node.type.name === "heading") return false;
                if (node.type.name === "paragraph") {
                  const text = node.textContent;
                  if (text.trim() && !mine(text)) decos.push(Decoration.node(pos, pos + node.nodeSize, { class: "ai-line" }));
                  return false;
                }
                return true;
              });
              return DecorationSet.create(state.doc, decos);
            },
          },
        }),
      ];
    },
  });
}

type MentionItem = { id: string; label: string };

function MentionList({ items, command, sel }: { items: MentionItem[]; command: (i: MentionItem) => void; sel: number }) {
  return (
    <div className="mention-list">
      {items.length === 0 && <span className="muted small" style={{ padding: "4px 8px" }}>No people yet</span>}
      {items.map((it, i) => (
        <button key={it.id} className={i === sel ? "on" : ""} onClick={() => command(it)}>
          {it.label}
        </button>
      ))}
    </div>
  );
}

function mentionSuggestion(people: () => MentionItem[]) {
  return {
    items: ({ query }: { query: string }) =>
      people()
        .filter((p) => p.label.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 6),
    render: () => {
      let r: ReactRenderer | null = null;
      let el: HTMLDivElement | null = null;
      let sel = 0;
      let props: { items: MentionItem[]; command: (i: MentionItem) => void; clientRect?: (() => DOMRect | null) | null } | null = null;
      const place = () => {
        const rect = props?.clientRect?.();
        if (el && rect) {
          el.style.left = `${rect.left}px`;
          el.style.top = `${rect.bottom + 4}px`;
        }
      };
      const draw = () => r?.updateProps({ items: props?.items ?? [], command: props?.command, sel });
      return {
        onStart: (p: typeof props) => {
          props = p;
          sel = 0;
          r = new ReactRenderer(MentionList, { props: { items: p!.items, command: p!.command, sel }, editor: (p as unknown as { editor: never }).editor });
          el = document.createElement("div");
          el.style.position = "fixed";
          el.style.zIndex = "60";
          el.appendChild(r.element);
          document.body.appendChild(el);
          place();
        },
        onUpdate: (p: typeof props) => {
          props = p;
          sel = 0;
          draw();
          place();
        },
        onKeyDown: ({ event }: { event: KeyboardEvent }) => {
          const n = props?.items.length ?? 0;
          if (event.key === "ArrowDown") {
            sel = (sel + 1) % Math.max(n, 1);
            draw();
            return true;
          }
          if (event.key === "ArrowUp") {
            sel = (sel + Math.max(n, 1) - 1) % Math.max(n, 1);
            draw();
            return true;
          }
          if (event.key === "Enter" && n) {
            props?.command(props.items[sel]);
            return true;
          }
          return false;
        },
        onExit: () => {
          el?.remove();
          r?.destroy();
        },
      };
    },
  };
}

/** Shrinks a pasted/dropped image and returns a data URL (kept with the note, offline). */
async function imageToDataUrl(file: File): Promise<string> {
  const url = URL.createObjectURL(file);
  const img = document.createElement("img");
  await new Promise((ok, bad) => ((img.onload = ok), (img.onerror = bad), (img.src = url)));
  const scale = Math.min(1, 1400 / Math.max(img.width, img.height));
  const c = document.createElement("canvas");
  c.width = Math.round(img.width * scale);
  c.height = Math.round(img.height * scale);
  c.getContext("2d")!.drawImage(img, 0, 0, c.width, c.height);
  URL.revokeObjectURL(url);
  return c.toDataURL("image/jpeg", 0.85);
}

export default function NoteEditor({
  markdown,
  onChange,
  placeholder,
  editable = true,
  ai,
  myNotes,
  people = [],
  segments,
  speakerName,
  autoFocus,
  onTemplateSlash,
}: {
  markdown: string;
  onChange: (md: string) => void;
  placeholder: string;
  editable?: boolean;
  /** AI notes view: shade AI lines, offer the source view. */
  ai?: boolean;
  myNotes?: string;
  people?: string[];
  segments?: Segment[];
  speakerName?: (s: string) => string;
  autoFocus?: boolean;
  /** "/" at the start of an empty line in My notes opens the template picker. */
  onTemplateSlash?: () => void;
}) {
  const mineRef = useRef<((t: string) => boolean) | null>(null);
  mineRef.current = ai && myNotes && myNotes.trim() ? mineMatcher(myNotes) : null;
  const peopleRef = useRef<MentionItem[]>([]);
  peopleRef.current = people.map((p) => ({ id: p, label: p }));
  const changeRef = useRef(onChange);
  changeRef.current = onChange;
  const lastMd = useRef(markdown);
  const wrap = useRef<HTMLDivElement>(null);
  const [src, setSrc] = useState<{ top: number; line: string } | null>(null);
  const [hoverTop, setHoverTop] = useState<number | null>(null);
  const hoverLine = useRef("");

  const extensions = useMemo(
    () => [
      StarterKit.configure({ link: { openOnClick: true, autolink: true } }),
      Markdown,
      TaskList,
      TaskItem.configure({ nested: true }),
      Image.configure({ allowBase64: true }),
      Mention.configure({ HTMLAttributes: { class: "mention" }, suggestion: mentionSuggestion(() => peopleRef.current) as never, renderText: ({ node }) => `@${node.attrs.label ?? node.attrs.id}` }),
      Placeholder.configure({ placeholder }),
      aiShading(() => mineRef.current),
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const editor = useEditor({
    extensions,
    content: markdown,
    contentType: "markdown",
    editable,
    autofocus: autoFocus ? "end" : false,
    onUpdate: ({ editor }) => {
      const md = editor.getMarkdown();
      lastMd.current = md;
      changeRef.current(md);
    },
    editorProps: {
      handlePaste: (view, event) => {
        const file = [...(event.clipboardData?.files ?? [])].find((f) => f.type.startsWith("image/"));
        if (!file) return false;
        imageToDataUrl(file).then((src) => view.dispatch(view.state.tr.replaceSelectionWith(view.state.schema.nodes.image.create({ src }))));
        return true;
      },
      handleDrop: (view, event) => {
        const file = [...(event.dataTransfer?.files ?? [])].find((f) => f.type.startsWith("image/"));
        if (!file) return false;
        event.preventDefault();
        imageToDataUrl(file).then((src) => view.dispatch(view.state.tr.replaceSelectionWith(view.state.schema.nodes.image.create({ src }))));
        return true;
      },
      handleKeyDown: (view, event) => {
        if (event.key === "/" && onTemplateSlash) {
          const { $from } = view.state.selection;
          if ($from.parent.textContent === "" && view.state.doc.textContent.trim() === "") {
            event.preventDefault();
            onTemplateSlash();
            return true;
          }
        }
        return false;
      },
    },
  });

  // Content changed from outside (AI notes arrived, regenerate, sync).
  useEffect(() => {
    if (!editor || markdown === lastMd.current) return;
    lastMd.current = markdown;
    editor.commands.setContent(markdown, { contentType: "markdown", emitUpdate: false });
  }, [markdown, editor]);
  useEffect(() => {
    editor?.setEditable(editable);
  }, [editable, editor]);
  // Re-shade when the user's own notes change.
  useEffect(() => {
    editor?.view.dispatch(editor.state.tr.setMeta("reshade", true));
  }, [myNotes, ai, editor]);

  const onMove = (e: React.MouseEvent) => {
    if (!ai || !segments?.length || !wrap.current) return;
    const el = (e.target as HTMLElement).closest(".ProseMirror p, .ProseMirror li > p") as HTMLElement | null;
    if (!el) return;
    const box = wrap.current.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    hoverLine.current = el.textContent ?? "";
    setHoverTop(r.top - box.top);
  };
  const found = src && segments ? sourcesFor(src.line, segments) : [];

  return (
    <div className={`editor ${ai ? "ai" : ""}`} ref={wrap} style={{ position: "relative" }} onMouseMove={onMove} onMouseLeave={() => setHoverTop(null)}>
      <EditorContent editor={editor} />
      {ai && hoverTop !== null && segments && segments.length > 0 && (
        <button className="src-btn" style={{ top: hoverTop }} title="Show where this came from" onClick={() => setSrc({ top: hoverTop, line: hoverLine.current })}>
          {Icon.search}
        </button>
      )}
      {src && (
        <div className="src-pop" style={{ top: src.top + 26, right: -10 }} onMouseLeave={() => setSrc(null)}>
          <div className="muted small" style={{ marginBottom: 4 }}>
            From the transcript
          </div>
          {found.length === 0 && <div className="muted">This line comes from your own notes or the meeting as a whole.</div>}
          {found.map((s) => (
            <div key={s.id} className="line">
              <div className="who">
                {speakerName ? speakerName(s.speaker) : s.speaker} · {fmtDur(s.start)}
              </div>
              {s.text}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
