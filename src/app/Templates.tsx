import { useState } from "react";
import { api, Template } from "../api";
import { ReactNode } from "react";
import { Icon, toast, useSettings, useTemplates } from "./ui";

function Page({ title, sub, actions, children }: { title: string; sub: string; actions: ReactNode; children: ReactNode }) {
  return (
    <div className="page">
      <div className="topbar">
        <div className="spacer" />
        {actions}
      </div>
      <div className="page-body wide" style={{ paddingTop: 0 }}>
        <h1 className="h-serif">{title}</h1>
        <p className="sub">{sub}</p>
        {children}
      </div>
    </div>
  );
}

const EMPTY: Template = { id: "", name: "", description: "", sections: "Key points\nDecisions\nAction items: owner and due date", instructions: "", builtin: false, updatedAt: "" };

export default function Templates() {
  const [templates, reload] = useTemplates();
  const [settings, save] = useSettings();
  const [sel, setSel] = useState<Template | null>(null);
  const [draft, setDraft] = useState<Template | null>(null);

  const open = (t: Template) => {
    setSel(t);
    setDraft(t.builtin ? null : { ...t });
  };
  const store = async () => {
    if (!draft) return;
    if (!draft.name.trim()) return toast("Give the template a name");
    const t = await api.saveTemplate(draft);
    reload();
    setSel(t);
    setDraft({ ...t });
    toast("Template saved");
  };

  return (
    <Page
      title="Templates"
      sub="How the AI lays out your notes. Pick one per meeting from the template button, or set a default."
      actions={
        <button className="btn dark" onClick={() => (setSel(null), setDraft({ ...EMPTY }))}>
          {Icon.plus} New template
        </button>
      }
    >
      <div className="tpl-grid">
        <div className="tpl-list">
          {templates.map((t) => (
            <button key={t.id} className={`tpl ${sel?.id === t.id ? "on" : ""}`} onClick={() => open(t)}>
              <div className="tpl-name">
                {t.name}
                {settings?.default_template === t.id && <span className="chip">Default</span>}
                {!t.builtin && <span className="chip">Yours</span>}
              </div>
              <div className="tpl-desc">{t.description}</div>
            </button>
          ))}
        </div>
        <div className="tpl-detail">
          {draft ? (
            <div className="card pad">
              <label className="field">
                <span>Name</span>
                <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="e.g. Client check-in" />
              </label>
              <label className="field">
                <span>When to use it</span>
                <input value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} placeholder="Weekly calls with clients" />
              </label>
              <label className="field">
                <span>Sections, one per line ("Heading: what goes there")</span>
                <textarea rows={8} value={draft.sections} onChange={(e) => setDraft({ ...draft, sections: e.target.value })} />
              </label>
              <label className="field">
                <span>Extra instructions for the AI (optional)</span>
                <textarea rows={3} value={draft.instructions} onChange={(e) => setDraft({ ...draft, instructions: e.target.value })} placeholder="Keep it short. Quote the client's exact words for requests." />
              </label>
              <div style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 8 }}>
                {draft.id && (
                  <button
                    className="btn ghost danger"
                    onClick={async () => {
                      await api.deleteTemplate(draft.id);
                      if (settings?.default_template === draft.id) await save({ default_template: "auto" });
                      setDraft(null);
                      setSel(null);
                      reload();
                    }}
                  >
                    {Icon.trash} Delete
                  </button>
                )}
                <div className="spacer" />
                {draft.id && settings?.default_template !== draft.id && (
                  <button className="btn ghost" onClick={() => save({ default_template: draft.id })}>
                    Use by default
                  </button>
                )}
                <button className="btn dark" onClick={store}>
                  Save template
                </button>
              </div>
            </div>
          ) : sel ? (
            <div className="card pad">
              <h2 className="tpl-title">{sel.name}</h2>
              <p className="muted">{sel.description}</p>
              {sel.sections ? (
                <ul className="tpl-sections">
                  {sel.sections.split("\n").filter(Boolean).map((l) => {
                    const [h, ...rest] = l.split(":");
                    return (
                      <li key={l}>
                        <b>{h}</b>
                        {rest.length ? `: ${rest.join(":")}` : ""}
                      </li>
                    );
                  })}
                </ul>
              ) : (
                <p className="muted">The AI picks sections that fit the conversation, then adds decisions and action items.</p>
              )}
              {sel.instructions && <p className="muted small">{sel.instructions}</p>}
              <div style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 8 }}>
                <div className="spacer" />
                {settings?.default_template !== sel.id && (
                  <button className="btn ghost" onClick={() => save({ default_template: sel.id })}>
                    Use by default
                  </button>
                )}
                <button className="btn" onClick={() => setDraft({ ...sel, id: "", name: `${sel.name} (my version)`, builtin: false })}>
                  {Icon.copy} Duplicate and edit
                </button>
              </div>
            </div>
          ) : (
            <div className="empty-line">Pick a template to see its sections, or make your own.</div>
          )}
        </div>
      </div>
    </Page>
  );
}

// ---------------- picker (Granola-style: recent, mine, then by category) ----------------

const CATEGORY: Record<string, string> = {
  auto: "General",
  "team-meeting": "Team",
  "weekly-team": "Team",
  standup: "Team",
  "sprint-planning": "Team",
  retro: "Team",
  brainstorm: "Team",
  "all-hands": "Team",
  "design-crit": "Team",
  "project-kickoff": "Team",
  "one-on-one": "People",
  coaching: "People",
  interview: "Hiring",
  "interview-debrief": "Hiring",
  "sales-call": "Sales",
  discovery: "Sales",
  "pipeline-review": "Sales",
  "customer-check-in": "Customers",
  onboarding: "Customers",
  "customer-discovery": "Research",
  "research-interview": "Research",
  pitch: "Investors",
  "investor-update": "Investors",
  "board-meeting": "Investors",
  podcast: "Other",
  lecture: "Other",
};
const ORDER = ["General", "Team", "People", "Sales", "Customers", "Research", "Hiring", "Investors", "Other"];
const USE_KEY = "cereal.templateUse";

/** Remembers when a template was used (for the Recent row). */
export function recordTemplateUse(id: string) {
  try {
    const m = JSON.parse(localStorage.getItem(USE_KEY) || "{}");
    m[id] = Date.now();
    localStorage.setItem(USE_KEY, JSON.stringify(m));
  } catch {
    /* ignore */
  }
}

export function TemplatePicker({ templates, current, onPick }: { templates: Template[]; current: string; onPick: (id: string) => void }) {
  const [q, setQ] = useState("");
  const use: Record<string, number> = (() => {
    try {
      return JSON.parse(localStorage.getItem(USE_KEY) || "{}");
    } catch {
      return {};
    }
  })();
  const match = (t: Template) => !q || `${t.name} ${t.description}`.toLowerCase().includes(q.toLowerCase());
  const shown = templates.filter(match);
  const recent = q ? [] : [...templates].filter((t) => use[t.id]).sort((a, b) => use[b.id] - use[a.id]).slice(0, 5);
  const mine = shown.filter((t) => !t.builtin);
  const groups = ORDER.map((g) => [g, shown.filter((t) => t.builtin && (CATEGORY[t.id] ?? "Other") === g)] as const).filter(([, v]) => v.length);
  const item = (t: Template) => (
    <button key={t.id} className={`tpl ${t.id === current ? "on" : ""}`} onClick={() => (recordTemplateUse(t.id), onPick(t.id))}>
      <div className="tpl-name">{t.name}</div>
      <div className="tpl-desc">{t.description}</div>
    </button>
  );
  return (
    <>
      <input className="tpl-search" autoFocus placeholder="Search templates" value={q} onChange={(e) => setQ(e.target.value)} />
      <div className="tpl-list" style={{ maxHeight: "50vh" }}>
        {recent.length > 0 && (
          <>
            <div className="tpl-group">Recent</div>
            {recent.map(item)}
          </>
        )}
        {mine.length > 0 && (
          <>
            <div className="tpl-group">My templates</div>
            {mine.map(item)}
          </>
        )}
        {groups.map(([g, v]) => (
          <div key={g} style={{ display: "contents" }}>
            <div className="tpl-group">{g}</div>
            {v.map(item)}
          </div>
        ))}
        {shown.length === 0 && <div className="muted">No template matches.</div>}
      </div>
    </>
  );
}
